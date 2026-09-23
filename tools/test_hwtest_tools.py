#!/usr/bin/env python3
"""Regression tests for the hardware-test host tools.

Two kinds of check. Archived captures in docs/hardware-refs must keep parsing
to the same summary, so a parser edit cannot silently orphan a console run.
And the tables the tools keep by hand (record names, work counts, block
flags, memory-control names) must agree with the guest source they mirror.
"""

from __future__ import annotations

import base64
import binascii
import importlib.util
import struct
import re
import sys
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent
REFS = REPO / "docs" / "hardware-refs"
GUEST_SRC = REPO / "engine" / "examples" / "hardware-tests" / "src"


def load_tool(filename: str):
    name = filename.removesuffix(".py").replace("-", "_")
    spec = importlib.util.spec_from_file_location(name, TOOLS / filename)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    # Dataclasses resolve their module through sys.modules while executing.
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


report = load_tool("hwtest-report.py")
verifier = load_tool("verify-hwtest-machine-code.py")
audio_report = load_tool("hwtest-audio-report.py")

# file -> (schema, suite minor, timing records, whole-binary CRC)
ARCHIVED = {
    "px7-emulator-v1.5.txt": ("PX7", 5, 151, 0x5245A296),
    "px7-emulator-v1.6.txt": ("PX7", 6, 151, 0x056F8EC8),
    "px7-emulator-v1.7.txt": ("PX7", 6, 151, 0x39FFC9DD),
    "px7-emulator-v1.8.txt": ("PX7", 8, 151, 0xEE090F6D),
    "px7-silicon-2026-07-26.txt": ("PX7", 4, 131, 0xB7EFA355),
    "px7-silicon-2026-07-31-v1.6-partial.txt": ("PX7", 6, 55, 0x4AFD3764),
    "px7-silicon-2026-07-31-v1.7-full.txt": ("PX7", 6, 151, 0xB0A8FC88),
    "px7-silicon-v1.5-2026-07-26.txt": ("PX7", 5, 151, 0x0A28B12F),
    "px8-emulator-v1.14.txt": ("PX8", 14, 0, 0x6D93820A),
    "px8-emulator-v1.15.txt": ("PX8", 15, 0, 0x964DA405),
    "px8-emulator-v1.16.txt": ("PX8", 16, 0, 0x624AEC02),
    "px8-emulator-v1.17.txt": ("PX8", 17, 0, 0x7B1CED9C),
    "px8-emulator-v1.18.txt": ("PX8", 18, 0, 0x1D984FC5),
    "px8-emulator-v1.19.txt": ("PX8", 19, 0, 0xC7704842),
    "px8-emulator-v1.20.txt": ("PX8", 20, 0, 0x752E799B),
    "px8-silicon-2026-08-07-v1.17-full.txt": ("PX8", 17, 151, 0x5C8F0460),
    "px8-silicon-2026-09-17-v1.22-full.txt": ("PX8", 22, 179, 0xE0F65995),
    "px8-silicon-2026-09-17-v1.22-perf-sweep.txt": ("PX8", 22, 87, 0x7F53654B),
    "px8-silicon-2026-09-17-v1.22-perf-ab.txt": ("PX8", 22, 108, 0x0A7B83CE),
    "px8-silicon-2026-09-17-v1.23-perf-sweep.txt": ("PX8", 23, 116, 0x1C081CF8),
    "px8-silicon-2026-09-17-v1.23-perf-ab.txt": ("PX8", 23, 137, 0x8C571D68),
}


def guest_source() -> str:
    return "\n".join(path.read_text(encoding="utf-8") for path in sorted(GUEST_SRC.glob("*.rs")))


class ArchivedCaptureTests(unittest.TestCase):
    def test_every_archived_capture_is_listed(self) -> None:
        on_disk = {path.name for path in REFS.glob("px[78]-*.txt")}
        # Baselines for the suite version under development come and go.
        listed = set(ARCHIVED)
        self.assertFalse(listed - on_disk, "listed capture missing from docs/hardware-refs")

    def test_archived_captures_parse_to_the_same_summary(self) -> None:
        for name, (schema, minor, records, crc) in ARCHIVED.items():
            with self.subTest(capture=name):
                capture = report.parse_capture(report.payloads_from_paths([str(REFS / name)]))
                self.assertEqual(capture.schema, schema)
                self.assertEqual(capture.suite_minor, minor)
                self.assertEqual(len(capture.records), records)
                self.assertEqual(capture.binary_crc, crc)

    def test_a_corrupted_page_is_rejected(self) -> None:
        page = report.payloads_from_paths([str(REFS / "px8-emulator-v1.20.txt")])[0]
        body, crc = page.rsplit("/C:", 1)
        flipped = body[:-1] + ("A" if body[-1] != "A" else "B")
        with self.assertRaises(ValueError):
            report.parse_capture([f"{flipped}/C:{crc}"])


class TableSyncTests(unittest.TestCase):
    def test_labels_and_work_cover_the_same_ids(self) -> None:
        self.assertEqual(set(report.LABELS), set(report.WORK_BY_ID))
        self.assertNotIn(report.PX7_RECORD_UNUSED, report.LABELS)

    def test_literal_guest_records_match_the_host_tables(self) -> None:
        source = guest_source()
        literal = re.findall(r"sample_timing\(\s*(0x[0-9A-Fa-f]{2,3}),\s*(\d+|0x[0-9A-Fa-f]+),", source)
        self.assertGreater(len(literal), 100, "guest timing call sites not found")
        for id_text, work_text in literal:
            record_id, work = int(id_text, 16), int(work_text, 0)
            with self.subTest(record=f"{record_id:02X}"):
                self.assertIn(record_id, report.LABELS)
                self.assertEqual(report.WORK_BY_ID[record_id], work)

    def test_perf_probe_table_matches_the_host_tables(self) -> None:
        # perf_probes.rs drives its records from one table of
        # `probe(0xID, work, ...)` rows instead of literal call sites.
        rows = re.findall(r"\b(?:ab_probe|probe|case)\(\s*(0x[0-9A-Fa-f]{2,3}),\s*(\d+),", guest_source())
        self.assertGreater(len(rows), 40, "perf probe table not found")
        for id_text, work_text in rows:
            record_id, work = int(id_text, 16), int(work_text)
            with self.subTest(record=f"{record_id:02X}"):
                self.assertIn(record_id, report.LABELS)
                self.assertEqual(report.WORK_BY_ID[record_id], work)

    def test_record_slots_hold_the_largest_scope(self) -> None:
        source = guest_source()
        slots = int(re.search(r"const TIMING_RECORD_COUNT: usize = (\d+);", source).group(1))
        table = {
            name: int(re.search(rf"const {name}: \[\w+; (\d+)\]", source).group(1))
            for name in ("SAFE", "LEVERS", "EXTENDED", "SHAPES", "RISKY", "CASES")
        }
        dma_pairs = 6
        retired = sum(1 for label in report.LABELS.values() if label.startswith("v122_only_"))
        # What is left is the standing battery, which has not changed size.
        standing = len(report.LABELS) - sum(table.values()) - dma_pairs - retired
        self.assertEqual(standing, 151)
        # The standard scope takes the standing battery, SAFE and LEVERS.
        self.assertLessEqual(standing + table["SAFE"] + table["LEVERS"], slots)
        self.assertLessEqual(sum(table.values()) + dma_pairs, slots)

    def test_no_label_claims_an_unused_slot_marker(self) -> None:
        self.assertNotIn(0xFF, report.LABELS)
        self.assertNotIn(0xFFFF, report.LABELS)

    def test_list_busy_labels_match_the_battery(self) -> None:
        # Cases are named by index on the host, so the labels must start where
        # the guest's list_busy_probes entries start and cover all of them.
        source = (GUEST_SRC / "main.rs").read_text(encoding="utf-8")
        battery = source[source.index("const TESTS: [TestSpec; TEST_COUNT]") :]
        runs = re.findall(r"run: ([\w:]+),", battery)
        busy = [index for index, run in enumerate(runs) if run.startswith("list_busy_probes::")]
        self.assertEqual(busy[0], report.LIST_BUSY_FIRST_CASE)
        self.assertEqual(len(busy), len(report.LIST_BUSY_LABELS))
        self.assertEqual(busy, list(range(busy[0], busy[0] + len(busy))))

    def test_block_flags_match_photo_rs(self) -> None:
        photo = (GUEST_SRC / "photo.rs").read_text(encoding="utf-8")
        for name in ("STATUS", "FAILURES", "OBSERVED", "TIMING", "MEMCTL", "PRECISION"):
            with self.subTest(block=name):
                shift = int(re.search(rf"pub const {name}: u8 = 1 << (\d+);", photo).group(1))
                self.assertEqual(getattr(report, f"PX8_BLOCK_{name}"), 1 << shift)

    def test_every_memory_control_value_has_a_name(self) -> None:
        count = int(re.search(r"const MEMORY_CONTROL_REGISTERS: \[u32; (\d+)\]", guest_source()).group(1))
        self.assertLessEqual(count, len(report.MEMORY_CONTROL_NAMES))


class MachineCodeVerifierTests(unittest.TestCase):
    START, END = 0x3400_0000 | (7 << 1), 0x3400_0001 | (7 << 1)

    def test_a_span_is_found_and_digested_without_its_markers(self) -> None:
        body = [0x0109_0019, 0x0000_5012]
        spans, layout = verifier.discover([0, self.START, *body, self.END, 0x3400_8003])
        self.assertEqual(spans, {7: (1, 4)})
        self.assertEqual(layout, {3: 5})
        self.assertNotEqual(verifier.digest(body), verifier.digest(body[::-1]))

    def test_broken_marker_pairs_are_errors(self) -> None:
        for words in (
            [self.START],  # never closed
            [self.END],  # never opened
            [self.START, self.START, self.END],  # reopened
            [self.START, self.END, self.START, self.END],  # id reused
        ):
            with self.subTest(words=words), self.assertRaises(verifier.AuditError):
                verifier.discover(words)

    def test_the_pinned_baseline_parses(self) -> None:
        newest = max(
            REFS.glob("hwtest-machine-code-v*.txt"),
            key=lambda path: tuple(int(part) for part in re.findall(r"\d+", path.stem)),
        )
        rows = verifier.parse_baseline(newest)
        self.assertIn("07", rows)
        self.assertEqual(rows["07"][0], "timed_multu_mflo")


class ProbePayloadTests(unittest.TestCase):
    @staticmethod
    def payload(body: bytes, suffix: int | None = None) -> str:
        crc = binascii.crc32(body) & 0xFFFF_FFFF
        binary = body + struct.pack("<I", crc)
        shown = crc if suffix is None else suffix
        return f"PA1/{base64.b64encode(binary).decode()}/C:{shown:08X}"

    def test_a_consistent_payload_decodes(self) -> None:
        binary, crc = audio_report.probe_binary(self.payload(b"PA1B" + bytes(12)), "PA1", 20)
        self.assertEqual(binary[:4], b"PA1B")
        self.assertEqual(crc, binascii.crc32(binary[:-4]) & 0xFFFF_FFFF)

    def test_length_and_crc_disagreements_are_rejected(self) -> None:
        good = b"PA1B" + bytes(12)
        with self.assertRaises(ValueError):
            audio_report.probe_binary(self.payload(good), "PA1", 24)
        with self.assertRaises(ValueError):
            audio_report.probe_binary(self.payload(good, suffix=0), "PA1", 20)


if __name__ == "__main__":
    unittest.main()
