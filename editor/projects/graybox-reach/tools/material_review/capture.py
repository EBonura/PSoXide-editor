"""Capture real guest material trials at 4x internal resolution.

Only the review build receives a temporary close camera. Source is restored
in finally; the normal playable disc is rebuilt before returning.
"""
from pathlib import Path
from PIL import Image
import subprocess, os, json, hashlib
PROJECT=Path(__file__).resolve().parents[2]
ROOT=PROJECT.parents[2]
OUT=PROJECT/'validation/material-review-v3'
AUTHOR=PROJECT/'source_assets/animations/player/zenith_cannon_v2/material_trials.py'
CAMERA=ROOT/'engine/examples/editor-playtest/src/playtest_runtime.rs'
FRONTEND=ROOT/'target/release/frontend'
BASE='        let camera = self.solve_follow_camera(ctx);'
REVIEW=BASE+'''
        // Temporary material inspection camera; never used in the playable build.
        let camera = if ctx.sim_tick.as_u32() > 700 {
            let p = self.motor.position();
            let detail = (820..970).contains(&ctx.sim_tick.as_u32());
            let (x,y,z,focus_y,focus_z) = if detail { (75,74,65,60,8) } else { (115,65,90,42,0) };
            world_camera_from_position_focus(camera.projection,
                RoomPoint::new(p.x + x, p.y + y, p.z + z),
                RoomPoint::new(p.x, p.y + focus_y, p.z + focus_z))
        } else { camera };'''

def command(args,log,env=None):
    with (OUT/log).open('w') as f:
        subprocess.run(list(map(str,args)),cwd=ROOT,env=env,stdout=f,stderr=subprocess.STDOUT,check=True)

def build(log):
    command([FRONTEND,'build-project-disc','--project',PROJECT],log,
            dict(os.environ,EDITOR_PLAYTEST_FEATURES='cd-stream-bench'))

def capture(name,poll):
    command([FRONTEND,'launch','--config-dir',PROJECT/'validation/enemy-behavior/emulator-config',
             '--path',PROJECT/'baked/graybox_reach.cue','--embedded-playtest',
             '--input-tape',OUT/'review-tape.csv','--stop-at-poll',poll,'--steps','5000000000',
             '--dump-hw',OUT/(name+'.ppm'),'--dump-display',OUT/(name+'-native.ppm')],name+'.log',
            dict(os.environ,PSOXIDE_HW_DUMP_SCALE='4'))
    for suffix in ['', '-native']:
        p=OUT/(name+suffix+'.ppm'); im=Image.open(p); im.save(p.with_suffix('.png'));p.unlink()
    print('Captured '+name,flush=True)

def main():
    source=CAMERA.read_text(); assert 'Temporary material inspection' not in source
    assert source.count(BASE)==1
    try:
        CAMERA.write_text(source.replace(BASE,REVIEW))
        for variant in ['pearl','jade','satin']:
            command(['python3',AUTHOR,'--install',variant],variant+'-author.log')
            build(variant+'-build.log')
            for view,poll in [('full',800),('detail',900),('hrz',1150)]: capture(variant+'-'+view,poll)
    finally:
        CAMERA.write_text(source)
        command(['python3',AUTHOR,'--install','jade'],'selected-author.log')
        build('playable-build.log')
    capture('jade-gameplay',800)
    (OUT/'capture.json').write_text(json.dumps({'resolution':[1280,960], 'internal_scale':4,
        'renderer':'PSoXide hardware renderer; replay of real guest GPU packets',
        'camera':'temporary inspection camera for trials; restored for jade-gameplay and playable disc',
        'selected':'jade','texture':[128,128,4], 'opaque':True,
        'views':{'full':800,'detail':900,'hrz':1150},
        'playable_disc_sha256':hashlib.sha256((PROJECT/'baked/graybox_reach.bin').read_bytes()).hexdigest()},indent=2)+'\n')
if __name__=='__main__': main()
