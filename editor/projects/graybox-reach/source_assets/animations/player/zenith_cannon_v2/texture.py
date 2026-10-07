"""Create a 128-square indexed 4-bit gradient atlas: sixteen scarf-derived colours."""
from pathlib import Path
from PIL import Image
import math,json
P=Path(__file__).resolve().parent
colours=['081d26','0f303a','194850','255e65','32767a','459092','58aca7','6cc4b8','6ce0c6','8aead4','b0f3e1','d1f8ed','edfff7','f8fffd','416b73','6b969e']
palette=[tuple(bytes.fromhex(c)) for c in colours]
im=Image.new('P',(128,128));im.putpalette([n for c in palette for n in c]+[0]*(768-48))
indices=[]
for y in range(128):
 for x in range(128):
  if y>=112:
   value=12 if x<64 else 1
  else:
   # Wide reflection bands, softly staggered over length, without grain.
   value=round(3+8*(.5+.5*math.cos(2*math.pi*(x/128+.22*y/112)))**1.4)
   value=max(0,min(13,value+round(1.2*math.sin(math.pi*y/112))))
  indices.append(value)
im.putdata(indices);im.save(P/'iris-gradient-128-4bit.png',bits=4)
assert Image.open(P/'iris-gradient-128-4bit.png').getextrema()[1]<16
(P/'palette.json').write_text(json.dumps({'size':[128,128],'bits_per_pixel':4,'scarf_ZTH_rgb':[108,224,198],'palette_rgb':palette,'indices_used':len(set(indices))},indent=2)+'\n')
# Identical indices, alternate CLUT: HRZ scarf and transition only, never a gun.
amber=['26130a','402011','603019','834022','a65029','c76332','e3793d','f79850','ff713a','ffba78','ffd59e','ffe6c4','fff5e4','fffdf6','815c48','b38c70']
hrz=im.copy();hrz.putpalette([v for c in amber for v in bytes.fromhex(c)]+[0]*(768-48))
hrz.save(P/'scarf-hrz-gradient-128-4bit.png',bits=4)
assert list(hrz.getdata())==list(im.getdata())
(P/'palette-hrz.json').write_text(json.dumps({'usage':['HRZ scarf','stance-change fragments'],'cannon':False,'size':[128,128],'bits_per_pixel':4,'palette_rgb':[list(bytes.fromhex(c)) for c in amber]},indent=2)+'\n')
