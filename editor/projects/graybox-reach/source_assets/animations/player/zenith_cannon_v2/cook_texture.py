"""Cook the 16-colour iris atlas, preserving bright opaque crystal facets.

Run texture.py first. PS1 average blending affects only palette entries with
bit 15 set; the near-white highlights therefore retain their contrast.
"""
from pathlib import Path
import struct
from PIL import Image

source = Path(__file__).resolve().parent
output = source.parents[3] / 'assets/models/zenith_projector/iris-crystal.psxt'
image = Image.open(source / 'iris-gradient-128-4bit.png')
assert image.mode == 'P' and image.size == (128, 128)
indices = image.tobytes()
assert max(indices) < 16
pixels = bytes(indices[i] | indices[i + 1] << 4 for i in range(0, len(indices), 2))
palette = image.getpalette()
words = []
for index in range(16):
    r, g, b = palette[index * 3:index * 3 + 3]
    semi = 0x8000 if index < 10 or index >= 14 else 0
    words.append((r >> 3) | (g >> 3) << 5 | (b >> 3) << 10 | semi)
clut = struct.pack('<16H', *words)
blob = b'PSXT' + struct.pack('<HHIBBHHHII',
    1, 0, 16 + len(pixels) + len(clut), 4, 0, 128, 128, 16, len(pixels), len(clut))
output.write_bytes(blob + pixels + clut)
print(f'{output}: 128x128, 4 bpp, 16 palette entries, {len(blob + pixels + clut)} bytes')
