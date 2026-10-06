#!/usr/bin/env python3
"""Incrementally expand the existing Reach via editor MCP; preserve all resources.
Run once against the 52-brush pre-expansion project. Evidence stays out of picker.
"""
from pathlib import Path
import base64, json, re, shutil, sys
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'benchmarks/engine-stress'))
from mcpc import Client
PROJECT=ROOT/'editor/projects/graybox-reach'
OUT=ROOT/'build/graybox-reach/expansion'
CLIFF,GROUND,GRID,SKY='Graybox / Cliff 64','Graybox / Ground 64','Graybox / Original 64','DP Fog City Cube Sky'

def terrain_boxes(outer_lo,outer_hi,spaces):
    axes=[sorted({outer_lo[a],outer_hi[a]}|{max(outer_lo[a],min(outer_hi[a],p[a])) for v in spaces for p in v}) for a in range(3)]
    nx,ny,nz=[len(a)-1 for a in axes]
    cells=set()
    for x in range(nx):
        for y in range(ny):
            for z in range(nz):
                mid=[(axes[a][i]+axes[a][i+1])/2 for a,i in enumerate([x,y,z])]
                if not any(all(lo[a]<mid[a]<hi[a] for a in range(3)) for lo,hi in spaces):cells.add((x,y,z))
    while cells:
        x,y,z=min(cells);xx=x+1;zz=z+1;yy=y+1
        while (xx,y,z) in cells:xx+=1
        while zz<nz and all((i,y,zz) in cells for i in range(x,xx)):zz+=1
        while yy<ny and all((i,yy,k) in cells for i in range(x,xx) for k in range(z,zz)):yy+=1
        for i in range(x,xx):
            for j in range(y,yy):
                for k in range(z,zz):cells.remove((i,j,k))
        yield [axes[0][x],axes[1][y],axes[2][z]],[axes[0][xx],axes[1][yy],axes[2][zz]]

def main():
    OUT.mkdir(parents=True,exist_ok=True)
    c=Client(PROJECT);log=[]
    def call(tool,args=None):
        r=c.call(tool,args or {});log.append(dict(tool=tool,args=args or {},result=r))
        if r.get('isError'):raise RuntimeError(r)
        return r
    def count():return int(re.search(r'(\d+) brushes',call('status')['content'][0]['text'])[1])
    def box(lo,hi,mat=CLIFF):call('add_shape',dict(shape='box',min=lo,max=hi,material=mat))
    def group(start,name):call('group_brushes',dict(first=start,count=count()-start,name=name))
    def evidence(tool,args,name):
        r=call(tool,args)
        text='\n'.join(b['text'] for b in r['content'] if b['type']=='text')
        (OUT/(name+'.txt')).write_text(text)
        for b in r['content']:
            if b['type']=='image':(OUT/(name+'.png')).write_bytes(base64.b64decode(b['data']))
        print(name+': '+text[:700],flush=True)
        return text
    try:
        assert count()==52,'Expected unexpanded Reach; refusing to duplicate geometry.'
        if not (OUT/'before-project.ron').exists():shutil.copy2(PROJECT/'project.ron',OUT/'before-project.ron')
        old=json.loads((PROJECT/'validation/routes.json').read_text())
        evidence('area_budget',dict(areas=old['areas']),'before-areas')
        # Only open the two existing boundary slabs. Higher brush index first.
        call('carve',dict(first=5,count=1,min=[4096,0,61440],max=[12288,4096,61696],material=CLIFF))
        call('carve',dict(first=2,count=1,min=[-16640,2048,49152],max=[-16384,6144,57344],material=CLIFF))
        start=count()
        court=([-12288,0,86016],[20480,12544,114688])
        spaces=[
            # Exclude the entire old shell from new terrain, including its floor.
            ([-16640,-512,-16640],[24832,12544,61696]),court,
            ([4096,0,61440],[12288,4096,69632]),
            ([4096,0,69632],[20480,4096,73728]),
            ([16384,0,73728],[20480,4096,86016]),
            ([-24576,2048,49152],[-16384,6144,73728]),
            ([-32768,2048,69632],[-16384,8192,81920]),
            ([-32768,2048,77824],[-24576,6144,98304]),
            ([-32768,2048,94208],[-12288,6144,102400]),
        ]
        for lo,hi in terrain_boxes([-33024,-512,48896],[24832,12288,114944],spaces):box(lo,hi)
        group(start,'Expansion / far court, bent gate and covered western return')
        start=count();box([-12288,12288,86016],[20480,12544,114688],SKY)
        group(start,'Sky / far court')
        start=count()
        call('add_shape',dict(shape='ramp',min=[-12288,0,87040],max=[-8192,2048,94208],direction='south',material=GROUND))
        box([-12288,0,94208],[-8192,2048,102400],GROUND)
        # A central ruin gives the otherwise broad outdoor space a landmark.
        # One solid screen, with wide passage on both sides, also shortens views.
        box([-2048,0,98304],[6144,6144,100352],GRID)
        box([10240,0,106496],[16384,1024,112640],GROUND)
        call('add_shape',dict(shape='ramp',min=[10240,0,102400],max=[16384,1024,106496],direction='south',material=GROUND))
        box([12288,1024,109568],[14336,8192,111616],GRID)
        group(start,'Far court / ramped west landing, screen ruin and beacon dais')
        call('set_face_uv',dict(first=0,count=count(),scale_percent=[400,400]))
        for name,pos,radius,color in [
            ('Far court daylight',[4096,10240,100352],32768,[220,232,255]),
            ('Western hall amber',[-24576,7168,75776],24576,[255,200,144]),
            ('Bent gate blue',[12288,3584,73728],16384,[176,208,255]),
        ]:call('add_light',dict(name=name,position=pos,radius=radius,intensity=1.0,color=color))
        areas=old['areas']+[
            dict(name='Bent northern gate',enemies=0,samples=[[8192,1536,65536],[8192,1536,71680],[18432,1536,81920]]),
            dict(name='Far court',enemies=0,samples=[[16384,1536,90112],[8192,1536,100352],[4096,1536,110592],[-10240,3584,98304]]),
            dict(name='Covered western return',enemies=0,samples=[[-20480,3584,53248],[-20480,3584,65536],[-24576,3584,75776],[-28672,3584,90112],[-20480,3584,98304]]),
        ]
        route=[[6144,0,57344],[8192,0,57344],[8192,0,65536],[8192,0,71680],[18432,0,71680],[18432,0,90112],[8192,0,94208],[8192,0,106496],[4096,0,110592],[-6144,0,110592],[-6144,0,86528],[-10240,0,86528],[-10240,2048,98304],[-28672,2048,98304],[-28672,2048,75776],[-20480,2048,75776],[-20480,2048,53248],[-14336,2048,53248]]
        validation=dict(areas=areas,expansion_loop=route,main_route=old['main_route'],loop_route=old['loop_route'])
        (OUT/'routes.json').write_text(json.dumps(validation,indent=2))
        audit=evidence('audit',dict(depth='full'),'audit')
        evidence('area_budget',dict(areas=areas),'area-budget')
        evidence('walk_test',dict(path=route,radius=188,height=1024,leg=128),'walk-expansion')
        for key in ['main_route','loop_route']:evidence('walk_test',dict(path=old[key],radius=188,height=1024,leg=128),'walk-'+key)
        evidence('plan_view',dict(axis='top',slice=3072,width=1200,center=[-4096,0,49152],extent=69632),'plan')
        # Keep exact staged geometry as reviewable output even if a validation
        # needs correction; the backup permits restoring the unexpanded scene.
        call('save')
        print('SAVED',count(),'brushes',flush=True)
    finally:
        c.close();(OUT/'authoring.json').write_text(json.dumps(log,indent=2))
if __name__=='__main__':main()
