# Direct CLUT rewrite measurement

The private v2.5 battery adds step `CLUT ADDRESS REWRITE` and records
`0xA00` through `0xA18`. They observe palette behavior directly instead of
assuming the stale result in the older `0xB1` and `0xB2` checks. Those older
checks and all prior v2.5 records remain intact.

Each row uploads a red palette at `(0,504)` and a blue palette at `(0,505)`,
clears GP0's texture cache, draws once using the first address, uploads blue
to that same address, and draws again. All palettes are opaque. Texture data
at `(768,256)` stays unchanged. The raw primitive commands disable tinting,
blending and dithering. The drawing area covers VRAM, draw offset and mask
bits are zero, and drawing to the display area is enabled.

Nothing reads VRAM or draws text between the first and second draws. Both
drawn pixels and the rewritten palette are read only after an ordered
GP0(1Fh) drain. The two draws occupy separate destinations. A missing draw
therefore reads zero, and the second draw cannot hide the first result.

| Sample | Record IDs | Texture index |
| --- | --- | --- |
| 4bpp rectangle | `A00` to `A04` | 1 |
| 8bpp rectangle, shared cache line | `A05` to `A09` | 1 |
| 8bpp rectangle, upper cache line | `A0A` to `A0E` | 17 |
| 4bpp triangle | `A0F` to `A13` | 1 |
| 8bpp triangle, upper cache line | `A14` to `A18` | 17 |

Within each group the five variants are, in order:

1. Rewrite between draws, same CLUT word, without an extra mid-sequence drain.
2. Same word, with ordered drains after the first draw and after the upload.
3. GP0(01h) cache clear after the upload, then draw with the same word.
4. Draw with the changed word addressing the blue control palette.
5. Draw with the blue control word, then return to the rewritten original word.

Each record carries `first_pixel`, `second_pixel`, `descriptor_flags` in its
three 16-bit fields. Red is `001F`; blue is `7C00`. Descriptor bits 0 to 2
are the variant; bits 3 to 5 are the sample group. Error bits are 8 drain
timeout, 9 read timeout, 10 unequal adjacent readback pixels, 11 palette
readback not blue, 12 first pixel not red, and 13 command-ready timeout.
Every new device wait is bounded. A failed drain/readiness/readback resets
the command parser after recording the failure.

The dependency-free Rust tool accepts a complete PX8 page set or TTY log:

```sh
rustc --edition=2021 tools/clut-probe-report.rs -o clut-probe-report
./clut-probe-report /path/to/pages.txt
```

It verifies page CRCs, one run ID, the complete Base64 payload, its binary
CRC, unique record IDs, suite v2.5 and zero record-integrity errors. It
requires all 25 descriptors, a red first pixel, a blue rewritten palette,
and blue changed-address controls. Same-address and cache-clear observations
may be red or blue; neither is a presumed silicon verdict.

The `clut-preview` feature is a short headless renderer witness. It submits
the same measurement code and copies the offscreen matrix to the display at
`(120,96)`. Columns are the sample groups above; rows are the variants above.
Each cell has its first draw at the left and its second draw eight pixels
to the right. The control draw in variant 5 occupies the last four
pixels of its cell. The burn artifact uses no features and runs the full
battery. The preview does not replace the full PX8 console capture.

This tiny witness is diagnostic. It does not satisfy the full matrix of
new before/after diffs and crops required to accept a renderer correction.

Emulator observations establish a backend disagreement, not a silicon
result. Preserve the source, EXE and disc hashes with the console recording.
Choose a renderer correction only after the exact disc produces a complete,
valid hardware capture of these records.
