"""Outdoor authoring sweep, same two actors/camera/assets; only pillar count changes.
Generated projects stay under build. Retains MCP edits, disc hashes and raw timing.
"""
from pathlib import Path
import concurrent.futures, hashlib, json, os, re, shutil, subprocess, sys
from mcpc import Client
E=Path(__file__).resolve().parents[2]
ROOT=Path(os.environ.get('PSOXIDE_AREA_OUTPUT', str(E/'build/area-calibration'))).resolve()
FE=E/'target/release/frontend'
def run(cmd, log, env):
    with log.open('w') as f:subprocess.run(list(map(str,cmd)),cwd=E,env=env,stdout=f,stderr=subprocess.STDOUT,check=True)
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def create(n):
    out=ROOT/f'pillars-{n}';p=out/'project'
    if p.exists():raise RuntimeError(f'Refusing to overwrite evidence at {p}; set PSOXIDE_AREA_OUTPUT to a new directory')
    p.mkdir(parents=True)
    text=(E/'editor/projects/graybox-valley/project.ron').read_text()
    text=text.replace('../default/assets/',str(E/'editor/projects/default/assets')+'/')
    text=text.replace('assets/textures/picotron-grid-64.psxt',str(E/'editor/projects/graybox-valley/assets/textures/picotron-grid-64.psxt'))
    (p/'project.ron').write_text(text)
    c=Client(p);calls=[]
    def call(t,a={}):
        r=c.call(t,a);calls.append(dict(tool=t,args=a,result=r))
        if r.get('isError'):raise RuntimeError(r)
        return r
    try:
        size=int(re.search(r'(\d+) brushes',call('status')['content'][0]['text'])[1])
        for i in range(size):call('get_brush',{'brush':i})
        call('delete',dict(first=0,count=size))
        for nm in ['Courtyard daylight','Valley daylight','Tower daylight']:call('delete_node',{'node':nm})
        boxes=[([-8192,-256,-8192],[8192,0,8192]),([-8448,-256,-8448],[-8192,8192,8448]),([8192,-256,-8448],[8448,8192,8448]),([-8192,-256,-8448],[8192,8192,-8192]),([-8192,-256,8192],[8192,8192,8448])]
        for lo,hi in boxes:call('add_shape',dict(shape='box',min=lo,max=hi,material='Graybox / Original 64'))
        call('add_shape',dict(shape='box',min=[-8448,8192,-8448],max=[8448,8448,8448],material='DP Fog City Cube Sky'))
        for i in range(n):
            x=(-6144,-3072,3072,6144)[i%4];z=-2048+(i//4)*2048
            call('add_shape',dict(shape='box',min=[x-384,0,z-384],max=[x+384,3072,z+384],material='Graybox / Cliff 64'))
        for i in range(6+n):call('get_brush',{'brush':i})
        call('set_face_uv',dict(first=0,count=6+n,scale_percent=[400,400]))
        for nm,pos in [('Aletha (Player)',[0,0,-5120]),('Intake Custodian',[-1280,0,0]),('Intake Custodian Copy',[1280,0,1024])]:call('move_node',dict(node=nm,position=pos))
        call('add_light',dict(position=[0,7168,0],radius=24576,intensity=1.0,name='Calibration daylight'))
        call('save');r=call('audit',{'depth':'full'})
        (out/'audit.txt').write_text('\n'.join(b['text'] for b in r['content'] if b['type']=='text'))
    finally:c.close();(out/'mcp.json').write_text(json.dumps(calls,indent=2))
    env=os.environ.copy();env['EDITOR_PLAYTEST_FEATURES']='cd-stream-bench emulator-telemetry'
    print('build',n,flush=True);run([FE,'build-project-disc','--project',p],out/'build.log',env)
    cue=Path([l for l in (out/'build.log').read_text().splitlines() if l.endswith('.cue')][-1])
    (out/'manifest.json').write_text(json.dumps(dict(project_sha256=sha(p/'project.ron'),frontend_sha256=sha(FE),cue=str(cue),disc_files={f:sha(cue.parent/f) for f in re.findall(r'FILE "([^"]+)"',cue.read_text())},features=env['EDITOR_PLAYTEST_FEATURES']),indent=2))
    return out,cue

def replay(out,cue):
    env=os.environ.copy();env['PSOXIDE_EXPERIMENTAL_DMA_FIFO']='1'
    cmd=[FE,'launch','--path',cue,'--embedded-playtest','--stop-at-poll=1800','--steps=2400000000','--profile-log',out/'profile.csv','--route-log',out/'route.csv','--gpu-frame-stats-log',out/'gpu.csv','--route-screenshot-dir',out/'shots','--route-screenshot-interval=900']
    print('replay',out.name,flush=True);run(cmd,out/'run.log',env)
    print('done',out.name,flush=True)
if __name__=='__main__':
    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
        futures=[]
        for n in map(int,sys.argv[1:] or [0,4,8,12]):
            futures.append(pool.submit(replay,*create(n)))
        for f in futures:f.result()
