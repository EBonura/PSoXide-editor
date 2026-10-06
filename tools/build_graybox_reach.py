#!/usr/bin/env python3
"""Author Graybox Reach through the editor MCP. No inferred FPS/face cap."""
from pathlib import Path
import json, re, shutil, sys, base64
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'benchmarks/engine-stress'))
from mcpc import Client
PROJECT=ROOT/'editor/projects/graybox-reach'
OUT=ROOT/'build/graybox-reach'
SEED=ROOT/'editor/projects/graybox-valley'
GRID,GROUND,CLIFF,SKY='Graybox / Original 64','Graybox / Ground 64','Graybox / Cliff 64','DP Fog City Cube Sky'
def main():
    if PROJECT.exists() and '--replace' not in sys.argv: raise SystemExit('Project already exists; use --replace for this generated fixture.')
    PROJECT.mkdir(exist_ok=True);OUT.mkdir(parents=True,exist_ok=True)
    for directory in ['assets','source_assets']: shutil.copytree(SEED/directory,PROJECT/directory,dirs_exist_ok=True)
    seed=(SEED/'project.ron').read_text().replace('name: "Graybox Valley"','name: "Graybox Reach"',1)
    seed=re.sub(r'orbit_radius: \d+','orbit_radius: 65000',seed,count=1)
    seed=re.sub(r'orbit_target: \([^)]*\)','orbit_target: (4096, 2048, 22528)',seed,count=1)
    (PROJECT/'project.ron').write_text(seed)
    (PROJECT/'.gitignore').write_text('.mcp-backups/\n')
    c=Client(PROJECT);log=[]
    def call(tool,args=None):
        r=c.call(tool,args or {});log.append(dict(tool=tool,args=args or {},result=r))
        if r.get('isError'):raise RuntimeError(r)
        return r
    def count():return int(re.search(r'(\d+) brushes',call('status')['content'][0]['text'])[1])
    def inspect():
        n=count()
        for i in range(n):call('get_brush',dict(brush=i))
        return n
    def box(lo,hi,mat=GRID):call('add_shape',dict(shape='box',min=lo,max=hi,material=mat))
    def ramp(lo,hi,direction):call('add_shape',dict(shape='ramp',min=lo,max=hi,direction=direction,material=GROUND))
    def group(start,name):call('group_brushes',dict(first=start,count=count()-start,name=name))
    try:
        call('delete',dict(first=0,count=inspect()))
        for name in ['Courtyard daylight','Valley daylight','Tower daylight']:call('delete_node',dict(node=name))
        courts=[([-16384,0,-16384],[16384,12544,8192]),([-16384,0,28672],[16384,12544,61440])]
        spaces=courts+[
            ([-4096,0,8192],[4096,4096,12288]),
            ([-8192,0,12288],[8192,6144,24576]),
            ([0,0,24576],[8192,4096,28672]),
            ([12288,2048,0],[24576,6144,8192]),
            ([16384,2048,8192],[24576,6144,28672]),
            ([12288,2048,28672],[24576,6144,36864]),
        ]
        # Exact integer decomposition of the solid terrain around connected
        # voids. Author each resulting box through MCP. This avoids coincident
        # clipping planes producing zero-thickness fragments in repeated carve.
        outer_lo=[-16640,-512,-16640];outer_hi=[24832,12288,61696]
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
            box([axes[0][x],axes[1][y],axes[2][z]],[axes[0][xx],axes[1][yy],axes[2][zz]],CLIFF)
        group(0,'Terrain / two outdoor basins, great hall and east gallery')
        start=count()
        for lo,hi in courts:box([lo[0],12288,lo[2]],[hi[0],12544,hi[2]],SKY)
        group(start,'Sky / open basins')
        # The raised loop has generous landings and long, walkable ramps.
        start=count()
        ramp([12288,0,-8192],[16384,2048,0],'south')
        box([12288,0,0],[16384,2048,8192],GROUND)
        box([12288,0,28672],[16384,2048,36864],GROUND)
        ramp([12288,0,36864],[16384,2048,45056],'north')
        # Half-height parapets articulate the balconies while preserving views.
        box([11776,0,1024],[12288,2560,7168])
        box([11776,0,29696],[12288,2560,35840])
        group(start,'East loop / two ramps and overlooking balconies')
        start=count()
        # Low ruins and a sloping ridge create choices across the broad arrival.
        box([-12288,0,-8192],[-8192,1024,-4096],GROUND)
        ramp([-12288,0,-12288],[-8192,1024,-8192],'south')
        box([-11264,1024,-7168],[-10240,5120,-6144])
        ramp([-16384,0,-2048],[-10240,4096,6144],'west')
        box([-6144,0,6144],[-4096,8192,8192])
        box([4096,0,6144],[6144,8192,8192])
        group(start,'Arrival basin / ruins, hillside and gate towers')
        start=count()
        # Four structural columns, broad central aisle and a west dais.
        for x in [-5120,4096]:
            for z in [15360,20480]:box([x,0,z],[x+1024,6144,z+1024])
        box([-8192,0,16384],[-6144,512,22528],GROUND)
        box([-6144,0,17408],[-5632,256,21504],GROUND)
        group(start,'Great hall / columns and west dais')
        start=count()
        # Side gallery is wide enough to fight in; buttresses break its outline.
        for z in [12288,20480]:box([23552,2048,z],[24576,6144,z+2048])
        group(start,'East gallery / interior buttresses')
        start=count()
        box([-16384,0,40960],[-8192,2048,57344],GROUND)
        ramp([-16384,0,32768],[-8192,2048,40960],'south')
        ramp([-16384,0,57344],[-8192,2048,61440],'north')
        box([-13312,2048,50176],[-11264,10240,52224])
        # Headland blocks a portion of the long sightline without partitioning
        # the whole basin into tiny rooms. Two generous paths go around it.
        ramp([-2048,0,40960],[2048,4096,47104],'south')
        box([-2048,0,47104],[2048,4096,49152],CLIFF)
        ramp([-2048,0,49152],[2048,4096,55296],'north')
        group(start,'Northern reach / raised shrine, headland and branching paths')
        start=count()
        box([-8192,0,22528],[4096,6144,24576],CLIFF)
        group(start,'Great hall / screened north exit')
        call('set_face_uv',dict(first=0,count=inspect(),scale_percent=[400,400]))
        for name,pos in [('Aletha (Player)',[0,0,-12288]),('Intake Custodian',[-5120,0,34816]),('Intake Custodian Copy',[0,0,36864])]:call('move_node',dict(node=name,position=pos))
        for name,pos,radius,color in [
            ('Arrival daylight',[0,10240,-4096],32768,[220,232,255]),
            ('Great hall amber',[0,5120,18432],12288,[255,200,144]),
            ('East gallery blue',[20480,5120,18432],20480,[176,208,255]),
            ('Northern daylight',[0,10240,45056],32768,[220,232,255])]:
            call('add_light',dict(name=name,position=pos,radius=radius,intensity=1.0,color=color))
        world=call('get_node',dict(node='World'))['content'][0]['text']
        kind=world.split('```',1)[1].split('```',1)[0].strip()
        kind=kind.replace('mode: Panorama,','mode: Cube, texture: Some((21)),',1).replace('visibility: Always','visibility: ThroughSkySurfaces',1)
        call('set_node',dict(node='World',kind=kind))
        call('set_cook_mode',dict(release=True));call('save')
        areas=[
            dict(name='Arrival basin',samples=[[0,1536,-12288],[-6144,1536,-4096],[4096,1536,4096]]),
            dict(name='Gatehouse and great hall',samples=[[0,1536,10240],[0,1536,18432],[6144,1536,23552]]),
            dict(name='Northern reach',enemies=2,samples=[[4096,1536,30720],[6144,1536,36864],[6144,1536,51200],[-10240,3584,45056]]),
            dict(name='Raised east loop',samples=[[14336,3584,4096],[20480,3584,18432],[14336,3584,32768]])]
        main_route=[[0, 0, -12288], [0, 0, 18432], [6144, 0, 18432], [6144, 0, 32768], [6144, 0, 36864], [6144, 0, 57344], [-6144, 0, 57344], [-6144, 0, 31744], [-10240, 0, 31744], [-10240, 2048, 45056], [-10240, 2048, 55296]]
        loop_route=[[0,0,-12288],[14336,0,-12288],[14336,0,-8192],[14336,2048,4096],[20480,2048,4096],[20480,2048,32768],[14336,2048,32768],[14336,0,46080],[6144,0,46080]]
        (PROJECT/'validation').mkdir(exist_ok=True)
        (PROJECT/'validation/routes.json').write_text(json.dumps(dict(areas=areas,main_route=main_route,loop_route=loop_route),indent=2))
        for tool,args,name in [('audit',dict(depth='full'),'audit'),('area_budget',dict(areas=areas),'area-budget'),('walk_test',dict(path=main_route,radius=188,height=1024,leg=128),'walk-main'),('walk_test',dict(path=loop_route,radius=188,height=1024,leg=128),'walk-loop'),('plan_view',dict(axis='top',slice=2560,width=1000,center=[4096,0,22528],extent=40960),'plan')]:
            r=call(tool,args)
            (OUT/(name+'.txt')).write_text('\n'.join(b['text'] for b in r['content'] if b['type']=='text'))
            for b in r['content']:
                if b['type']=='image':(OUT/(name+'.png')).write_bytes(base64.b64decode(b['data']))
            print(name,'done',flush=True)
        print('Saved',count(),'brushes',flush=True)
    finally:c.close();(OUT/'authoring.json').write_text(json.dumps(log,indent=2))
if __name__=='__main__':main()
