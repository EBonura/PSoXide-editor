"""Per-foot world-space slip while planted: foot sole centroid + motor travel, frame to frame."""
import sys,json;sys.path.insert(0,sys.path[0]);sys.path.insert(0,sys.path[0]+'/../knockdown_v1');sys.dont_write_bytecode=True
import numpy as np
from psxdecode import model
from dashtravel import travel
from pathlib import Path
P=Path(__file__).resolve().parents[4]
npz,kind=sys.argv[1],sys.argv[2]
if kind=='light':m=model(P/'graybox-reach/assets/models/light_body_v1/light.psxmdl');feet={'L(lead)':[16,17],'R(trail)':[20,21]};floor=-20000.;D=27400
else:m=model(P/'grid-dungeon/assets/models/enemy_reduced/heavy.psxmdl');feet={'L(lead)':[19,20,21],'R(trail)':[24,25,26]};floor=-15883.35;D=27480
import os
ACT=int(os.environ.get("ACT","14"));DIR=int(os.environ.get("DIR","1"));D=float(os.environ.get('DIST',D))
own=np.array([m['owner'][i] for i in range(len(m['v']))])
V=np.load(npz)['verts'].astype(float);n=len(V)-1
out={}
for name,js in feet.items():
    idx=np.where(np.isin(own,js))[0];rows=[];prev=None;base_low=V[0][idx][:,1].min()-floor
    for k in range(n+1):
        v=V[k][idx];low=v[:,1].min()-floor;sole=v[v[:,1]<v[:,1].min()+400]
        w=np.array([sole[:,0].mean()+travel(k,D,ACT)*DIR,sole[:,2].mean()])
        planted=low<base_low+150
        slip=float(np.linalg.norm(w-prev[1])) if (planted and prev is not None and prev[0]) else None
        rows.append((k,round(low),planted,None if slip is None else round(slip)));prev=(planted,w)
    out[name]=rows
    pl=[r[3] for r in rows if r[3] is not None]
    print(name,'planted frames',[r[0] for r in rows if r[2]],'slip per planted frame',[r[3] for r in rows if r[3] is not None],'max',max(pl) if pl else None)
