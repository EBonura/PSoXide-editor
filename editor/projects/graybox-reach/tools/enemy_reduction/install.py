"""Install regenerated runtime models into Graybox Reach."""
from pathlib import Path
import shutil,struct
p=Path(__file__).resolve().parents[2]
s=p/'source_assets/characters/enemy_reduced';a=p/'assets/models/enemy_reduced';a.mkdir(parents=True,exist_ok=True)
for name in ['light','heavy']:shutil.copy2(s/f'{name}-reduced.psxmdl',a/f'{name}.psxmdl')
b=bytearray((a/'light.psxmdl').read_bytes());jc,pc,vc,fc,mc,*_=struct.unpack_from('<8H',b,12);part=28+4*jc+8*mc+9*16
assert struct.unpack_from('<H',b,part)[0]==9
struct.pack_into('<H',b,part+4,0);struct.pack_into('<H',b,part+8,0);(a/'light_clawless.psxmdl').write_bytes(b)
