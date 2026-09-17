#!/usr/bin/env python3
"""Audit the hardware-test disc's measured instruction blocks in the LINKED EXE.

Every timing probe brackets its measured interval with a pair of marker words,
`ori $zero, $zero, imm`: start = 0x34000000 | (id << 1), end = start | 1. They
write no register and no compiler emits them, so every such word in the image
is a marker. The markers exist so the final PS-X machine code can be audited
rather than trusted: a timing number only means what the docs claim if the
instructions between the markers are still the ones the source asked for.

This walks the linked EXE, pairs the markers, and digests the words between
each pair. Pin the output with --baseline and any change to a measured block
(an LLVM bump reordering a wrapper, an edit that lands inside the timed
window) shows up as a moved digest instead of a silently different cycle
count. Probes are discovered from the image, never from source text, so a
macro-generated probe or one in another module cannot fall out of the audit;
with --fail-on-change a probe that appears or disappears fails too.

Names live in the baseline, keyed by id. A new id prints as probe_NN until
someone names it there.

Layout tags (0x34008000 | n) are single non-executed words that pad the
I-cache entry targets. Their position within a 16-byte cache line is what the
entry probes depend on, so it is pinned the same way.

    python3 tools/verify-hwtest-machine-code.py <exe> [--baseline f] [--fail-on-change]
"""

from __future__ import annotations

import argparse
import pathlib
import struct
import sys

PSX_EXE_HEADER_BYTES = 0x800
MARKER_MASK = 0xFFFF_0000
MARKER_BASE = 0x3400_0000
LAYOUT_BIT = 0x8000
# A measured block is tens to hundreds of instructions; 1024 words is slack.
MAX_SPAN_WORDS = 1024


class AuditError(Exception):
    pass


def exe_words(path: pathlib.Path) -> list[int]:
    body = path.read_bytes()[PSX_EXE_HEADER_BYTES:]
    usable = len(body) - (len(body) % 4)
    return list(struct.unpack_from(f"<{usable // 4}I", body, 0))


def digest(values: list[int]) -> int:
    """FNV-1a over the measured words, matching the disc's own hash style."""
    acc = 0x811C_9DC5
    for value in values:
        acc = ((acc ^ (value & 0xFFFF_FFFF)) * 0x0100_0193) & 0xFFFF_FFFF
    return acc


def discover(words: list[int]) -> tuple[dict[int, tuple[int, int]], dict[int, int]]:
    """Return ({probe id: (start index, end index)}, {layout tag: word index})."""
    spans: dict[int, tuple[int, int]] = {}
    layout: dict[int, int] = {}
    open_id: int | None = None
    open_at = 0
    for index, word in enumerate(words):
        if word & MARKER_MASK != MARKER_BASE:
            continue
        low = word & 0xFFFF
        if low & LAYOUT_BIT:
            tag = low & ~LAYOUT_BIT
            if tag in layout:
                raise AuditError(f"layout tag {tag} appears twice")
            layout[tag] = index
            continue
        probe_id, is_end = low >> 1, low & 1
        if not is_end:
            if open_id is not None:
                raise AuditError(f"probe {open_id:02d} has no end marker before probe {probe_id:02d} starts")
            if probe_id in spans:
                raise AuditError(f"probe id {probe_id:02d} is used twice")
            open_id, open_at = probe_id, index
            continue
        if open_id != probe_id:
            raise AuditError(f"end marker for probe {probe_id:02d} without its start")
        if index - open_at > MAX_SPAN_WORDS:
            raise AuditError(f"probe {probe_id:02d} spans {index - open_at} words")
        spans[probe_id] = (open_at, index)
        open_id = None
    if open_id is not None:
        raise AuditError(f"probe {open_id:02d} has no end marker")
    return spans, layout


def parse_baseline(path: pathlib.Path) -> dict[str, tuple[str, str]]:
    """{key: (name, pinned value)}; key is 'NN' for a probe, 'Ln' for a layout tag."""
    rows: dict[str, tuple[str, str]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line or line.startswith("#") or line.startswith("id,"):
            continue
        key, name, pinned = line.split(",", 2)
        rows[key] = (name, pinned)
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exe", help="linked hardware-tests.exe")
    parser.add_argument("--baseline", help="previous output to compare against")
    parser.add_argument(
        "--fail-on-change",
        action="store_true",
        help="exit non-zero if any measured block moved, appeared or vanished (CI gate)",
    )
    args = parser.parse_args()

    words = exe_words(pathlib.Path(args.exe))
    try:
        spans, layout = discover(words)
    except AuditError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2
    if not spans:
        print(f"FAIL: no marker-bracketed probes found in {args.exe}", file=sys.stderr)
        return 2
    baseline = parse_baseline(pathlib.Path(args.baseline)) if args.baseline else {}

    current: dict[str, tuple[str, str]] = {}
    for probe_id, (first, last) in sorted(spans.items()):
        measured = words[first + 1 : last]
        key = f"{probe_id:02d}"
        name = baseline.get(key, (f"probe_{key}", ""))[0]
        current[key] = (name, f"{len(measured)},{digest(measured):#010x}")
    for tag, index in sorted(layout.items()):
        key = f"L{tag}"
        name = baseline.get(key, (f"layout_{tag}", ""))[0]
        current[key] = (name, f"line_word,{index % 4}")

    print(f"# exe={args.exe} words={len(words)} probes={len(spans)} layout_tags={len(layout)}")
    print("id,name,words,digest" + (",changed" if args.baseline else ""))
    drift: list[str] = []
    for key, (name, value) in current.items():
        row = f"{key},{name},{value}"
        if args.baseline:
            prior = baseline.get(key)
            if prior is None:
                drift.append(f"{key} ({name}): not in the baseline")
            elif prior[1] != value:
                drift.append(f"{key} ({name}): {prior[1]} -> {value}")
            row += f",{int(prior is None or prior[1] != value)}"
        print(row)
    for key, (name, _) in baseline.items():
        if key not in current:
            drift.append(f"{key} ({name}): pinned but no longer in the image")

    print(f"# drift={len(drift)}")
    for entry in drift:
        print(f"# drift: {entry}")
    if drift and args.fail_on_change:
        print(f"FAIL: {len(drift)} measured block(s) changed", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
