#!/usr/bin/env python3
"""Attribute exact PC-line or PC-word counts to linker-map symbols and cache sets.

Accepts any frontend histogram: `--pc-line-log` (instructions) or a stall-line
log (`--mmio-stall-line-log`, `--ram-load-stall-line-log`,
`--icache-stall-line-log`). Per-function totals are exact only for logs written
with `--pc-log-words` (column `pc`). A 16-byte line log (column `line_pc`) bills
a whole line to one symbol even when the line also holds the head of the next
function, so a hot callee such as memcpy leaks into whatever precedes it and a
code-size change shows per-function deltas that are pure placement.
"""

from __future__ import annotations

import argparse
import bisect
import csv
import pathlib
import re
from collections import defaultdict


MAP_ROW = re.compile(
    r"^\s*([0-9a-fA-F]+)\s+[0-9a-fA-F]+\s+([0-9a-fA-F]+)\s+\d+\s+(.+?)\s*$"
)


CLONE_SUFFIX = re.compile(r" \(\.\d+\)$")


def load_symbols(path: pathlib.Path) -> list[tuple[int, int, str]]:
    symbols: list[tuple[int, int, str]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        match = MAP_ROW.match(line)
        if not match:
            continue
        address = int(match.group(1), 16)
        size = int(match.group(2), 16)
        name = match.group(3)
        if (
            size == 0
            or not 0x8000_0000 <= address <= 0xBFFF_FFFF
            or "/" in name
            or ":(" in name
            or name.startswith((".", "BYTE(", "LONG(", "QUAD(", "*fill*"))
            or " = " in name
        ):
            continue
        symbols.append((address, address + size, name))
    symbols.sort(key=lambda item: (item[0], item[1]))
    return symbols


def load_counts(path: pathlib.Path) -> tuple[list[tuple[int, int]], bool, str]:
    """Return ([(pc, value)], is_word_log, value_name) for a frontend histogram."""
    with path.open(newline="", encoding="utf-8") as source:
        reader = csv.reader(source)
        key, value_name = next(reader)[:2]
        if key not in ("pc", "line_pc"):
            raise SystemExit(f"{path}: expected a pc or line_pc column, got {key!r}")
        counts = [(int(row[0], 16), int(row[1])) for row in reader if row]
    return counts, key == "pc", value_name


def per_symbol(
    counts: list[tuple[int, int]], symbols: list[tuple[int, int, str]], starts: list[int]
) -> dict[str, int]:
    totals: dict[str, int] = defaultdict(int)
    for pc, count in counts:
        totals[symbol_for(pc, symbols, starts)] += count
    return totals


def merge_clone_suffixes(totals: dict[str, int]) -> dict[str, int]:
    """Fold LLVM clone suffixes (`name (.630)`), which renumber between builds."""
    merged: dict[str, int] = defaultdict(int)
    for name, count in totals.items():
        merged[CLONE_SUFFIX.sub("", name)] += count
    return merged


def straddled(
    counts: list[tuple[int, int]], symbols: list[tuple[int, int, str]], starts: list[int]
) -> tuple[int, int]:
    """Lines of a line log whose 16 bytes belong to more than one symbol."""
    lines = value = 0
    for pc, count in counts:
        owners = {symbol_for(pc + offset, symbols, starts) for offset in range(0, 16, 4)}
        if len(owners) > 1:
            lines += 1
            value += count
    return lines, value


def canonical_code_line(physical: int) -> int:
    """Map a physical code line to the conventional cached/BIOS alias."""
    if 0x1FC0_0000 <= physical < 0x1FC8_0000:
        return physical | 0xA000_0000
    return physical | 0x8000_0000


def load_eviction_pairs(
    path: pathlib.Path,
) -> list[tuple[int, int, int, int, int]]:
    """Return (victim, incoming, set, events, stalls) temporal replacements."""
    totals: dict[tuple[int, int, int], list[int]] = defaultdict(lambda: [0, 0])
    with path.open(newline="", encoding="utf-8") as source:
        for row in csv.DictReader(source):
            if row["miss_kind"] != "tag" or int(row["victim_valid_mask"], 16) == 0:
                continue
            victim = canonical_code_line(int(row["victim_line"], 16))
            incoming = canonical_code_line(int(row["incoming_line"], 16))
            cache_set = int(row["cache_set"], 16)
            aggregate = totals[(victim, incoming, cache_set)]
            aggregate[0] += 1
            aggregate[1] += int(row["stall_cycles"])
    return [
        (victim, incoming, cache_set, counts[0], counts[1])
        for (victim, incoming, cache_set), counts in totals.items()
    ]


def symbol_for(
    pc: int, symbols: list[tuple[int, int, str]], starts: list[int]
) -> str:
    index = bisect.bisect_right(starts, pc) - 1
    while index >= 0 and starts[index] == starts[bisect.bisect_right(starts, pc) - 1]:
        start, end, name = symbols[index]
        if start <= pc < end:
            return name
        index -= 1
    if index >= 0:
        start, end, name = symbols[index]
        if start <= pc < end:
            return name
    return "<unattributed>"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("pc_lines", type=pathlib.Path)
    parser.add_argument("linker_map", type=pathlib.Path)
    parser.add_argument("--limit", type=int, default=30)
    parser.add_argument(
        "--compare",
        nargs=2,
        type=pathlib.Path,
        metavar=("BASE_LOG", "BASE_MAP"),
        help="also print per-function deltas against another build's log and map",
    )
    parser.add_argument(
        "--icache-events",
        type=pathlib.Path,
        help="exact refill CSV emitted by frontend --icache-event-log",
    )
    args = parser.parse_args()

    counts, words, value_name = load_counts(args.pc_lines)
    symbols = load_symbols(args.linker_map)
    starts = [symbol[0] for symbol in symbols]
    total = sum(count for _, count in counts) or 1

    if not words:
        shared_lines, shared_value = straddled(counts, symbols, starts)
        if shared_lines:
            print(
                f"warning: {shared_lines} lines holding {shared_value} {value_name} "
                f"({shared_value * 100.0 / total:.4f}%) span more than one symbol and "
                "are billed to one of them; rerun with --pc-log-words for exact totals\n"
            )

    by_symbol = per_symbol(counts, symbols, starts)

    # Line-level views: fold words into their I-cache line, naming every owner.
    line_totals: dict[int, int] = defaultdict(int)
    line_names: dict[int, list[str]] = defaultdict(list)
    for pc, count in sorted(counts):
        line = pc & ~0xF
        line_totals[line] += count
        name = symbol_for(pc, symbols, starts)
        if name not in line_names[line]:
            line_names[line].append(name)
    by_set: dict[int, list[tuple[int, int, str]]] = defaultdict(list)
    attributed: list[tuple[int, int, str]] = []
    for pc, count in line_totals.items():
        name = "|".join(line_names[pc])
        by_set[(pc >> 4) & 0xFF].append((pc, count, name))
        attributed.append((pc, count, name))

    print(f"hot functions ({'exact, per word' if words else 'per 16-byte line'})")
    print(f"{value_name},percent,symbol")
    for name, count in sorted(by_symbol.items(), key=lambda item: -item[1])[: args.limit]:
        print(f"{count},{count * 100.0 / total:.4f},{name}")

    if args.compare is not None:
        base_counts, base_words, _ = load_counts(args.compare[0])
        base_symbols = load_symbols(args.compare[1])
        base = merge_clone_suffixes(
            per_symbol(base_counts, base_symbols, [symbol[0] for symbol in base_symbols])
        )
        current = merge_clone_suffixes(by_symbol)
        if not (words and base_words):
            print("\nwarning: a line log on either side makes these deltas placement-sensitive")
        print("\nfunction deltas (this log minus BASE_LOG)")
        print(f"delta,percent_change,base_{value_name},{value_name},symbol")
        names = set(base) | set(current)
        ranked = sorted(names, key=lambda name: (-abs(current.get(name, 0) - base.get(name, 0)), name))
        for name in ranked[: args.limit]:
            old, new = base.get(name, 0), current.get(name, 0)
            change = f"{(new - old) * 100.0 / old:+.2f}" if old else "new"
            print(f"{new - old:+d},{change},{old},{new},{name}")

    print("\nhot lines")
    print(f"{value_name},percent,line_pc,cache_set,symbol")
    for pc, count, name in sorted(attributed, key=lambda item: -item[1])[: args.limit]:
        print(f"{count},{count * 100.0 / total:.4f},0x{pc:08x},0x{(pc >> 4) & 0xff:02x},{name}")

    pressures: list[tuple[int, int, int, list[tuple[int, int, str]]]] = []
    for cache_set, entries in by_set.items():
        entries.sort(key=lambda item: -item[1])
        set_total = sum(count for _, count, _ in entries)
        eviction_pressure = set_total - entries[0][1]
        pressures.append((eviction_pressure, set_total, cache_set, entries))
    pressures.sort(reverse=True)

    print("\nhot direct-map conflicts")
    print("pressure,total,cache_set,distinct_lines,top_lines")
    for pressure, set_total, cache_set, entries in pressures[: args.limit]:
        top = "; ".join(
            f"0x{pc:08x}:{count}:{name}" for pc, count, name in entries[:3]
        )
        print(f"{pressure},{set_total},0x{cache_set:02x},{len(entries)},{top}")

    if args.icache_events is not None:
        pairs = load_eviction_pairs(args.icache_events)
        print("\nexact temporal eviction pairs")
        print(
            "stall_cycles,events,cache_set,victim_line,victim_symbol,"
            "incoming_line,incoming_symbol"
        )
        for victim, incoming, cache_set, events, stalls in sorted(
            pairs, key=lambda item: (-item[4], -item[3], item[0], item[1])
        )[: args.limit]:
            victim_symbol = symbol_for(victim, symbols, starts)
            incoming_symbol = symbol_for(incoming, symbols, starts)
            print(
                f"{stalls},{events},0x{cache_set:02x},0x{victim:08x},"
                f"{victim_symbol},0x{incoming:08x},{incoming_symbol}"
            )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
