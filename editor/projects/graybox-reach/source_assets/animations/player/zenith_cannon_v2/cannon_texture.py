"""Four opaque cannon finishes in one 128x128, 4bpp atlas.

The approved B palette is reused byte-for-byte. Scarf textures and CLUTs are
never written here. The four 32-pixel strips are dark core, jade shell,
pale edges, and muzzle. Runtime normal lookup stays inside each face's strip.
"""
from pathlib import Path
import json,math
from PIL import Image
from material_trials import psxt
S=Path(__file__).resolve().parent; P=S.parents[3]
colours=json.loads((S/'palette.json').read_text())['palette_rgb']
im=Image.new('P',(128,128));im.putpalette([c for rgb in colours for c in rgb]+[0]*(768-48))
limits=[(1,5),(5,11),(10,15),(13,15)]
pixels=[]
for y in range(128):
 for x in range(128):
  band=x//32;u=(x%32)/31;v=y/127
  sheen=max(0,min(1,.14+.78*math.exp(-((u-.35)/.58)**2-((v-.28)/.82)**2)))
  lo,hi=limits[band];pixels.append(round(lo+(hi-lo)*sheen))
im.putdata(pixels);im.save(S/'cannon-finish-128-4bit.png',bits=4)
(P/'assets/models/zenith_projector/iris-crystal.psxt').write_bytes(psxt(im))
print('Cannon atlas: 128x128, 4bpp, 16 unchanged B colours, all opaque')
