"""Reproduce October 6 continuation; never overwrite October 3 fixtures.
Usage: python3 run.py SCENARIO [SCENARIO...]
Sequential builds share editor-playtest/generated. Each scenario retains its project and disc.
"""
import sys, os, json, re, subprocess, pathlib, shutil, hashlib
from mcpc import Client
E=pathlib.Path(__file__).resolve().parents[2]
ROOT=pathlib.Path(os.environ.get('PSOXIDE_STRESS_OUTPUT', str(E/'build/engine-stress'))).resolve()
FIXTURES=pathlib.Path(__file__).resolve().parent/'fixtures'
FE=E/'target/release/frontend'
SCENARIOS={'e0':'e0','e1':'e1','e2':'base','small2':'rsmall','scale4':'base',
           'outdoor':'base','vista8':'base','vista16':'base','patch4k':'base','patch4k_scale4':'base','outdoor_sealed':'base'}
def run(cmd, out, env=None):
    with open(out,'w') as f: subprocess.run(list(map(str,cmd)),cwd=E,env=env,stdout=f,stderr=subprocess.STDOUT,check=True)
def sha(p):
    h=hashlib.sha256()
    with open(p,'rb') as f:
        for b in iter(lambda:f.read(1048576),b''):h.update(b)
    return h.hexdigest()
def scenario(name):
    out=ROOT/name; out.mkdir(parents=True,exist_ok=True)
    project=out/'project'
    if project.exists(): raise RuntimeError(f'Refusing to overwrite {project}')
    project.mkdir()
    shutil.copy2(FIXTURES/SCENARIOS[name]/'project.ron', project/'project.ron')
    (project/'assets').symlink_to(os.path.relpath(E/'editor/projects/default/assets',project))
    c=Client(project)
    calls=[]
    def call(tool,args={}):
        result=c.call(tool,args); calls.append({'tool':tool,'args':args,'result':result})
        if result.get('isError'):raise RuntimeError(result)
        return result
    try:
        call('status')
        if name in ('scale4','patch4k_scale4'):
            for i in range(6):call('get_brush',{'brush':i})
            call('set_face_uv',{'first':0,'count':6,'scale_percent':[400,400]})
            call('save')
        if name in ('outdoor','vista8','vista16','outdoor_sealed'):
            for i in range(6):call('get_brush',{'brush':i})
            call('delete',{'first':0,'count':6})
            boxes=[([-12288,-256,-8192],[12288,0,20480]),
                   ([-12544,0,-8448],[-12288,768,20736]),
                   ([12288,0,-8448],[12544,768,20736]),
                   ([-12288,0,-8448],[12288,768,-8192]),
                   ([-12288,0,20480],[12288,768,20736])]
            for lo,hi in boxes:call('add_shape',{'shape':'box','min':lo,'max':hi,'material':'DP City / Megastructure Wall'})
            for i in range(5):call('get_brush',{'brush':i})
            call('set_face_uv',{'first':0,'count':5,'scale_percent':[400,400]})
            if name=='outdoor_sealed':
                sky_boxes=[([-12544,6144,-8448],[12544,6400,20736]),
                           ([-12544,768,-8448],[-12288,6144,20736]),
                           ([12288,768,-8448],[12544,6144,20736]),
                           ([-12288,768,-8448],[12288,6144,-8192]),
                           ([-12288,768,20480],[12288,6144,20736])]
                for lo,hi in sky_boxes:call('add_shape',{'shape':'box','min':lo,'max':hi,'material':'DP Fog City Cube Sky'})
            call('save')
    finally:c.close()
    p=project/'project.ron'; s=p.read_text()
    assert 'boot: Gameplay' in s and 'name: "Loading",\n            root:' not in s and 'world_message: Some' not in s
    if name.startswith('patch4k'):
        s,count=re.subn(r'bsp_patch_extent: 2048', 'bsp_patch_extent: 4096',s,count=1)
        assert count==1
        p.write_text(s)
    if name.startswith('vista'):
        n=int(name[5:])
        s,count=re.subn(r'far_vista: \(enabled: false, texture: None,', 'far_vista: (enabled: true, texture: Some((21)),',s,count=1)
        assert count==1
        s,count=re.subn(r'(far_vista: .*?segments: )\d+',lambda m:m[1]+str(n),s,count=1)
        assert count==1
        p.write_text(s)
    c=Client(project)
    try:
        audit=c.call('audit',{'depth':'full'})
        calls.append({'tool':'audit','args':{'depth':'full'},'result':audit})
        if audit.get('isError'):raise RuntimeError(audit)
    finally:c.close()
    (out/'mcp.json').write_text(json.dumps(calls,indent=2))
    (out/'audit.txt').write_text('\n'.join(x['text'] for x in audit.get('content',[]) if x['type']=='text'))
    shutil.copy2(p,out/'project.ron')
    env=os.environ.copy();env['EDITOR_PLAYTEST_FEATURES']='cd-stream-bench emulator-telemetry'
    print(name+': build',flush=True)
    run([FE,'build-project-disc','--project',project],out/'build.log',env)
    cue=pathlib.Path([x.strip() for x in (out/'build.log').read_text().splitlines() if x.strip().endswith('.cue')][-1])
    shutil.copy2(E/'engine/examples/editor-playtest/generated/level_manifest.cooked.rs',out/'level_manifest.cooked.rs')
    manifest={'scenario':name,'project':str(project),'cue':str(cue),'features':env['EDITOR_PLAYTEST_FEATURES'],
              'project_sha256':sha(p),'frontend_sha256':sha(FE),'editor_head':subprocess.check_output(['git','rev-parse','HEAD'],cwd=E,text=True).strip(),
              'editor_diff':subprocess.check_output(['git','diff'],cwd=E,text=True),
              'components':json.loads((E/'components.lock.json').read_text()),'cue_content':cue.read_text()}
    manifest['disc_files']={f:sha(cue.parent/f) for f in re.findall(r'FILE "([^"]+)"',cue.read_text())}
    (out/'manifest.json').write_text(json.dumps(manifest,indent=2))
    replay(out, cue)
    print(name+': done',flush=True)
def resolve_retained_cue(cue):
    """Find a retained test disc after its project was moved out of the picker."""
    if cue.exists():return cue
    archive=E/'editor/archive/local-tests'
    matches=list(archive.glob(f'*/{cue.parent.parent.name}/baked/{cue.name}'))
    if len(matches)==1:return matches[0]
    raise FileNotFoundError(f'Disc missing or archive location ambiguous: {cue}')
def replay(out, cue):
    cue=resolve_retained_cue(cue)
    name=out.name
    manifest=json.loads((out/'manifest.json').read_text())
    assert sha(FE)==manifest['frontend_sha256'], 'Frontend changed: establish a new baseline'
    for filename, expected in manifest['disc_files'].items():
        assert sha(cue.parent/filename)==expected, 'Disc changed: establish a new baseline'
    from concurrent.futures import ThreadPoolExecutor
    def one(mode):
        dest=out/mode;dest.mkdir(exist_ok=True)
        marker=dest/'complete.json'
        if marker.exists():marker.unlink()
        runenv=os.environ.copy();runenv['PSOXIDE_EXPERIMENTAL_DMA_FIFO']='1' if mode=='fifo' else '0'
        cmd=[FE,'launch','--path',cue,'--embedded-playtest','--stop-at-poll=5400','--steps=4860000000',
             '--profile-log',dest/'profile.csv','--counter-log',dest/'counter.csv',
             '--route-log',dest/'route.csv','--gpu-frame-stats-log',dest/'gpu.csv',
             '--cd-command-log',dest/'cd.log','--route-screenshot-dir',dest/'shots','--route-screenshot-interval=1800']
        (dest/'command.json').write_text(json.dumps(list(map(str,cmd)),indent=2))
        print(name+': '+mode,flush=True);run(cmd,dest/'run.log',runenv)
        (dest/'complete.json').write_text(json.dumps({'dma_fifo':runenv['PSOXIDE_EXPERIMENTAL_DMA_FIFO'],'completed':True}))
    with ThreadPoolExecutor(max_workers=2) as pool:
        list(pool.map(one,('legacy','fifo')))
if __name__=='__main__':
    for name in sys.argv[1:]:
        if name not in SCENARIOS:raise ValueError(name)
        if (ROOT/name/'manifest.json').exists():
            replay(ROOT/name,pathlib.Path(json.loads((ROOT/name/'manifest.json').read_text())['cue']))
        else:scenario(name)
