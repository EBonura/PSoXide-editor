from pathlib import Path
import numpy as np,json,struct,shutil
from PIL import Image
P=Path(__file__).resolve().parents[2]
O=P/'source_assets/characters/aletha_closed_458'
# Octahedral front-hemisphere normal lookup. The CPU supplies view-normal X/Y over L1.
y,x=np.mgrid[0:128,0:128];nx=x/127*2-1;ny=y/127*2-1;nz=-np.maximum(0,1-np.abs(nx)-np.abs(ny));length=np.maximum(np.sqrt(nx*nx+ny*ny+nz*nz),1e-6);nx/=length;ny/=length;nz/=length
rx=-2*nz*nx;ry=-2*nz*ny;rz=1-2*nz*nz
# A cool luminous field with dark reflected planes and soft white ribbons.
value=.53+.19*(-ry)+.12*rz+.10*rx
value-=.36*np.exp(-((rx+.22)/.32)**2-((ry-.28)/.65)**2)
value+=.40*np.exp(-((rx+.55)/.23)**2-((ry+.22)/.9)**2)
value+=.27*np.exp(-((rx-.62)/.20)**2-((ry+.18)/.8)**2)
value+=.20*np.exp(-((ry+.52)/.18)**2)
value=np.clip(value,0,1)
# Sixteen opaque BGR555 colours. The upper entries become neutral reflection whites.
palette=np.array([(8,16,24),(16,27,38),(28,42,55),(43,59,73),(61,79,94),(82,101,116),(104,124,139),(125,146,160),(146,167,180),(166,187,199),(185,205,215),(202,220,229),(216,232,239),(230,241,246),(244,251,253),(255,255,255)],dtype=np.uint8)
# Ordered dither in the source map only; no added geometry or overlay.
bayer=np.array([[0,8,2,10],[12,4,14,6],[3,11,1,9],[15,7,13,5]])/16-.5
indices=np.clip(np.rint(value*15+bayer[y%4,x%4]*.75),0,15).astype(np.uint8)
words=[(int(r)>>3)|((int(g)>>3)<<5)|((int(b)>>3)<<10) for r,g,b in palette]
words=[w or 0x8000 for w in words]
packed=(indices[:,::2]|(indices[:,1::2]<<4)).tobytes();clut=struct.pack('<16H',*words)
blob=b'PSXT'+struct.pack('<HHI',1,0,16+len(packed)+len(clut))+struct.pack('<BBHHHII',4,0,128,128,16,len(packed),len(clut))+packed+clut
assert len(blob)==8252
tex=P/'assets/textures/aletha_mirror_128_4bit.psxt';tex.parent.mkdir(parents=True,exist_ok=True);tex.write_bytes(blob)
img=Image.fromarray(indices,'P');img.putpalette(palette.flatten().tolist()+[0]*(768-48));img.save(O/'reflection-map.png');img.resize((512,512),Image.Resampling.NEAREST).save(O/'reflection-map-preview.png')
