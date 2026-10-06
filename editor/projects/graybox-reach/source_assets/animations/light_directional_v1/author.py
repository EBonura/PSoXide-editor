"""Directional stalking cycles; run with Blender --background --factory-startup.

Keep the 22-joint runtime rig, v5 torso asymmetry, and existing skin weights.
Feet are authored in travel space, with exact linear ground contact and a
Hermite return. The side cycles lead with the outward foot without crossing.
"""
from pathlib import Path
import ast
import json
import math
import struct
import bpy
import numpy as np
from mathutils import Matrix, Vector

OUT = Path(__file__).resolve().parent
PROJECT = OUT.parents[2]
HZ = 15
# Walk speed cooks to 2 units/tick; spacing_speed_percent=50 gives 1 unit/tick.
# Cooking divides vertex/pose positions by 16 but retains the model scale.
# Q12 113 * instance 417/256 rounds to 184.
SCALE = 184 / 4096 / 16
SPEED = 1 * 60
DUTY = .58
FLOOR = -20000.
SPECS = {
    'walk_backward': {'frames': 20, 'direction': [0, 0, -1], 'lift': 1200., 'lead': 0},
    'strafe_left': {'frames': 16, 'direction': [-1, 0, 0], 'lift': 1000., 'lead': 1},
    'strafe_right': {'frames': 16, 'direction': [1, 0, 0], 'lift': 1150., 'lead': 0},
}

def load_functions(path, names):
    tree = ast.parse(path.read_text())
    selected = ast.Module(body=[n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name in names], type_ignores=[])
    exec(compile(selected, str(path), 'exec'), globals())

load_functions(PROJECT/'tools/enemy_reduction/validate_render.py', {'model', 'animation', 'deform'})
load_functions(PROJECT/'source_assets/animations/light_stalk_v2/author.py', {'turn', 'ik'})
rig = np.load(PROJECT/'source_assets/characters/light_body_v1/bind.npz')
bind, parents = rig['bind'], rig['parents']
inverse_bind = np.linalg.inv(bind)
mesh_data = model(PROJECT/'assets/models/light_body_v1/light.psxmdl')
source = np.load(PROJECT/'source_assets/animations/light_walk_v5/walk.npz')['skin'] @ bind
plant_source = np.load(PROJECT/'source_assets/animations/light_walk_v1/rig.npz')['globals']
legs = []
for side, hip, knee, ankle, toe, frame in [(1,14,15,16,17,10), (-1,18,19,20,21,21)]:
    g = plant_source[frame]
    p = deform(mesh_data, [(a[:3,:3], a[:3,3]) for a in g @ inverse_bind])
    indices = [i for i in range(len(p)) if mesh_data['owner'][i] in [ankle,toe]]
    sole = np.array([p[indices,0].mean(), p[indices,1].min(), p[indices,2].mean()])
    relative = g[[ankle,toe]].copy()
    relative[:,:3,3] -= sole
    lengths = [np.linalg.norm(g[knee,:3,3]-g[hip,:3,3]), np.linalg.norm(g[ankle,:3,3]-g[knee,:3,3])]
    legs.append((side,hip,knee,ankle,toe,relative,lengths))

def body_at(k, spec):
    phase = (k/spec['frames'] + spec['lead']*.5) % 1
    sample = phase * len(source)
    a, b, t = source[int(sample)%24], source[(int(sample)+1)%24], sample%1
    g = a.copy()
    for j in range(22):
        qa = Matrix(a[j,:3,:3].tolist()).to_quaternion()
        qb = Matrix(b[j,:3,:3].tolist()).to_quaternion()
        g[j,:3,:3] = np.array(qa.slerp(qb,t).to_matrix())
        g[j,:3,3] = a[j,:3,3]*(1-t)+b[j,:3,3]*t
    # Retain v5's layered cannon/claw lag and target-facing head. Small
    # directional bracing comes from the hips, countered in the neck.
    d = spec['direction']
    if d[0]:
        lean = -3.5*d[0]
        turn(g,0,lean,'Z',range(22))
        turn(g,4,-lean*.6,'Z',[4,5])
        # Carry the pelvis between the opening/closing feet; retaining the
        # forward gait's lateral sway here would pull against the sidestep.
        center = np.mean([feet_at(k,i,spec)[0][0,0,3] for i in range(2)])
        g[:,0,3] += center - np.mean(g[[14,18],0,3])
    else:
        turn(g,1,-3,'X',range(1,14))
        turn(g,4,2,'X',[4,5])
    return g

def feet_at(k, index, spec):
    side,hip,knee,ankle,toe,relative,lengths = legs[index]
    u = (k/spec['frames']-(index-spec['lead'])*.5)%1
    travel = SPEED/SCALE*spec['frames']/HZ
    front, back = travel*DUTY/2, -travel*DUTY/2
    if u < DUTY:
        along, lift = front-travel*u, 0.
    else:
        t = (u-DUTY)/(1-DUTY)
        along = ((2*t**3-3*t*t+1)*back+(t**3-2*t*t+t)*(-travel)*(1-DUTY)
                 +(-2*t**3+3*t*t)*front+(t**3-t*t)*(-travel)*(1-DUTY))
        lift = spec['lift']*math.sin(math.pi*t)**2
    if spec['direction'][0]:
        center = np.array([-2300+side*6000, FLOOR, 400+side*900])
    else:
        center = np.array([source[0,hip,0,3]+side*1000, FLOOR, -800+side*550])
    center += np.array(spec['direction'])*along
    center[1] += lift
    foot = relative.copy()
    foot[:,:3,3] += center
    return foot, u < DUTY

def pack(path, poses):
    poses = np.concatenate([poses,poses[:1]])
    shift = max(0,math.ceil(math.log2(max(1,np.abs(poses[:,:,:3,3]).max())/32760)))
    payload = bytearray(struct.pack('<4H',22,len(poses),HZ,shift))
    for frame in poses:
        for m in frame:
            values = [*np.rint(m[:3,:3].T*4096).astype(int).ravel(), *np.rint(m[:3,3]/2**shift).astype(int)]
            assert all(-32768 <= x <= 32767 for x in values)
            payload += struct.pack('<12h',*values)
    header = bytearray((PROJECT/'assets/animations/light_walk_v5/walk.psxanim').read_bytes()[:12])
    struct.pack_into('<H',header,4,2)
    struct.pack_into('<I',header,8,len(payload))
    path.write_bytes(header+payload)
    return shift

def main():
    result = {}
    for name,spec in SPECS.items():
        # One constant offset avoids frame-dependent hip snapping.
        drop = 0.
        for k in np.arange(0,spec['frames'],.25):
            g = body_at(k,spec)
            for i,(_,hip,_,_,_,_,lengths) in enumerate(legs):
                foot,_ = feet_at(k,i,spec)
                h,f = g[hip,:3,3], foot[0,:3,3]
                horizontal = np.sum((h[[0,2]]-f[[0,2]])**2)
                limit = .945*sum(lengths)
                assert horizontal < limit**2, (name,k,i,horizontal,limit**2)
                drop = max(drop,h[1]-f[1]-math.sqrt(limit**2-horizontal))
        frames, contacts, extensions = [],[],[]
        for k in range(spec['frames']):
            g = body_at(k,spec)
            g[:,1,3] -= drop
            planted = []
            for i,(side,hip,knee,ankle,toe,_,_) in enumerate(legs):
                feet,contact = feet_at(k,i,spec)
                extensions.append(ik(g,hip,knee,ankle,feet[0,:3,3],np.array([side*(.02 if spec['direction'][0] else .12),-.12,1.])))
                g[ankle],g[toe] = feet
                planted.append(contact)
            frames.append(g@inverse_bind)
            contacts.append(planted)
        frames = np.array(frames)
        contacts = np.array(contacts)
        # Enter a sidestep with the trailing foot collected under the body,
        # then reach outward. Starting at maximum width caused a pose pop.
        start_phase = spec['frames']//2 if spec['direction'][0] else 0
        frames = np.roll(frames,-start_phase,axis=0)
        contacts = np.roll(contacts,-start_phase,axis=0)
        shift = pack(OUT/f'{name}.psxanim',frames)
        np.savez(OUT/f'{name}.npz',skin=frames,bind=bind,parents=parents,contact=contacts,direction=spec['direction'])
        result[name] = dict(spec,sample_hz=HZ,stored_frames=len(frames)+1,period_seconds=len(frames)/HZ,
                           duty=DUTY,start_phase_frames=start_phase,pelvis_lowering=drop,max_leg_extension=max(extensions),translation_shift=shift)
    result['calibration'] = {'cooked_speed_per_tick':1,'ticks_per_second':60,'visual_scale_q12':184,'cook_position_divisor':16,
                             'visual_scale_q8':417,'authored_walk_speed':28,'model_q12_before_cook':113,
                             'assumption':'Steady straight travel. Circling radius correction and turning can introduce contact slip.'}
    (OUT/'authoring.json').write_text(json.dumps(result,indent=2)+'\n')
    print('RESULT',json.dumps(result))

if __name__ == '__main__':
    main()
