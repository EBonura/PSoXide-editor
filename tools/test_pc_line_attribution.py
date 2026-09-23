#!/usr/bin/env python3
"""Regression tests for tools/pc_line_attribution.py.

The case that matters: one 16-byte I-cache line holding the tail of a cold
function and the head of a hot one. A line log bills the whole line to the
cold function; a word log (frontend `--pc-log-words`) must not.
"""

from __future__ import annotations

import contextlib
import importlib.util
import io
import sys
import tempfile
import unittest
from pathlib import Path

TOOLS = Path(__file__).resolve().parent

spec = importlib.util.spec_from_file_location("pc_line_attribution", TOOLS / "pc_line_attribution.py")
assert spec is not None and spec.loader is not None
tool = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tool)

# `cold` owns 0x80010000..0x80010008, `hot` starts mid-line at 0x80010008.
MAP = (
    "     VMA      LMA     Size Align Out     In      Symbol\n"
    "80010000 80010000        8     4                 cold\n"
    "80010008 80010008       18     4                 hot (.12)\n"
)
WORDS = "pc,instructions,percent\n0x80010008,90,0\n0x80010004,10,0\n0x80010010,5,0\n"
LINES = "line_pc,instructions,percent\n0x80010000,100,0\n0x80010010,5,0\n"


class PcLineAttributionTest(unittest.TestCase):
    def setUp(self) -> None:
        self.dir = tempfile.TemporaryDirectory()
        root = Path(self.dir.name)
        self.map = root / "link.map"
        self.words = root / "words.csv"
        self.lines = root / "lines.csv"
        self.map.write_text(MAP)
        self.words.write_text(WORDS)
        self.lines.write_text(LINES)

    def tearDown(self) -> None:
        self.dir.cleanup()

    def run_tool(self, *args: str) -> str:
        out = io.StringIO()
        argv = sys.argv
        sys.argv = ["pc_line_attribution.py", *args]
        try:
            with contextlib.redirect_stdout(out):
                self.assertEqual(tool.main(), 0)
        finally:
            sys.argv = argv
        return out.getvalue()

    def test_word_log_splits_a_shared_line_exactly(self) -> None:
        symbols = tool.load_symbols(self.map)
        counts, words, value = tool.load_counts(self.words)
        self.assertTrue(words)
        self.assertEqual(value, "instructions")
        totals = tool.per_symbol(counts, symbols, [s[0] for s in symbols])
        self.assertEqual(totals, {"cold": 10, "hot (.12)": 95})

    def test_line_log_is_flagged_as_straddling(self) -> None:
        output = self.run_tool(str(self.lines), str(self.map))
        self.assertIn("warning: 1 lines holding 100 instructions", output)
        self.assertIn("100,95.2381,cold", output)

    def test_compare_reports_the_line_log_phantom(self) -> None:
        output = self.run_tool(
            str(self.words), str(self.map), "--compare", str(self.lines), str(self.map)
        )
        self.assertIn("+90,+1800.00,5,95,hot", output)
        self.assertIn("-90,-90.00,100,10,cold", output)
        self.assertIn("0x80010000,0x00,cold|hot (.12)", output)


if __name__ == "__main__":
    unittest.main()
