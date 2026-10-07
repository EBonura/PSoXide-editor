"""Bake Aletha's ready/fire poses and aiming gaits onto the existing 26-joint bind.
Run headless Blender with factory startup. Re-running replaces this generated study.
"""
import bpy, sys, struct, json, math, re
import numpy as np
from pathlib import Path
from mathutils import Matrix, Vector, Quaternion
from bpy_extras.anim_utils import action_get_channelbag_for_slot
out=Path(__file__).resolve().parent
project=out.parents[3]
sys.path.insert(0,str(out))
sys.path.insert(0,str(out.parent/'cybernetic_walk_transitions_review_v4'))
from decode_clip import decode
bpy.ops.wm.open_mainfile(filepath=str(project/'source_assets/characters/aletha_closed_458/Aletha-closed-458.blend'))
s=bpy.context.scene
r=next(o for o in s.objects if o.type=='ARMATURE')
r.animation_data_clear()
for b in r.pose.bones: b.matrix_basis=Matrix.Identity(4)
bpy.context.view_layer.update()
names=[b.name for b in r.data.bones]
assert names==['Hips','LeftUpperLeg','LeftLowerLeg','LeftFoot','RightUpperLeg','RightLowerLeg','RightFoot','Spine','Chest','Neck','RightShoulder','RightUpperArm','RightLowerArm','RightHand','RightThumbMetacarpal','RightThumbProximal','RightIndexProximal','RightIndexIntermediate','LeftShoulder','LeftUpperArm','LeftLowerArm','LeftHand','LeftThumbMetacarpal','LeftThumbProximal','LeftIndexProximal','LeftIndexIntermediate']
world=r.matrix_world.copy(); inv=world.inverted(); rest={n:world@r.data.bones[n].matrix_local for n in names}
# PSX bind coordinates are normalized about the model centre; Blender is Z-up.
blob=(project/'assets/models/aletha_closed_458/aletha_mirror_458.psxmdl').read_bytes()
j,parts,nv,nf,nm,*_=struct.unpack_from('<8H',blob,12);off=28+j*4+nm*8+parts*16
raw=np.array([struct.unpack_from('<3h',blob,off+i*8) for i in range(nv)],float)
mesh=bpy.data.objects['Aletha optimized']
verts=np.array([mesh.matrix_world@v.co for v in mesh.data.vertices])
C=Matrix(((1,0,0),(0,0,-1),(0,1,0)))
rv=raw[:,[0,2,1]]*np.array([1,-1,1]);unit=np.ptp(verts[:,2])/np.ptp(rv[:,2])
center=Vector(((verts.min(0)+verts.max(0))/2-(rv.min(0)+rv.max(0))/2*unit).tolist())

def apply_source(rot,trans):
    for i,n in enumerate(names):
        rr=C@Matrix(rot[i].tolist())@C.transposed()
        tt=C@Vector(trans[i].tolist())*unit+center-rr@center
        skin=rr.to_4x4();skin.translation=tt
        r.pose.bones[n].matrix=inv@skin@rest[n]
        bpy.context.view_layer.update()

def point_bone(name,target):
    b=r.pose.bones[name];m=world@b.matrix
    direction=m.to_3x3()@Vector((0,1,0))
    q=direction.rotation_difference(Vector(target)-m.translation)
    m2=(q.to_matrix()@m.to_3x3()).to_4x4();m2.translation=m.translation
    b.matrix=inv@m2;bpy.context.view_layer.update()

def arm_ik(upper, lower, hand, target, pole):
    start=(world@r.pose.bones[upper].matrix).translation
    target=Vector(target);axis=(target-start).normalized()
    a=r.data.bones[upper].length;b=r.data.bones[lower].length
    distance=min((target-start).length,(a+b)*.97)
    along=(a*a-b*b+distance*distance)/(2*distance)
    bend=(Vector(pole)-start);bend=(bend-axis*bend.dot(axis)).normalized()
    elbow=start+axis*along+bend*math.sqrt(max(0,a*a-along*along))
    point_bone(upper,elbow);point_bone(lower,start+axis*distance)
    wrist=(world@r.pose.bones[hand].matrix).translation
    point_bone(hand,wrist+Vector((0,-1,.04)))

def ready_pose(recoil):
    c=(world@r.pose.bones['Chest'].matrix).translation
    arm_ik('RightUpperArm','RightLowerArm','RightHand',
        c+Vector((-.16,-.40+recoil,.065+recoil*.4)),c+Vector((-.4,-.12,-.13)))
    hand=(world@r.pose.bones['RightHand'].matrix).translation
    arm_ik('LeftUpperArm','LeftLowerArm','LeftHand',
        hand+Vector((.085,0,-.075)),c+Vector((.4,-.13,-.17)))

text=(project/'project.ron').read_text()
def clip_path(id):
    match=re.search(r'            id: \('+str(id)+r'\),.*?psxanim_path: "([^"]+)"',text,re.S)
    return project/match[1]
sources={label:clip_path(id) for label,id in [('aim',45),('walk',73),('backward',81),('left',82),('right',83),('fire',45)]}

report={};actions={}
for label,path in sources.items():
    R,T,n,hz=decode(path)
    indices=list(range(n)) if label not in ('aim','fire') else [min(84,n-1)]*(2 if label=='aim' else 14)
    fps=hz if label not in ('aim','fire') else 30
    # Aimed locomotion retains 75% of authored walk speed.
    if label not in ('aim','fire'): fps=max(1,round(fps*.75))
    r.animation_data_clear();data=bytearray(path.read_bytes()[:20])
    struct.pack_into('<H',data,4,1);struct.pack_into('<4H',data,12,26,len(indices),fps,0)
    soles=[]
    for frame,source in enumerate(indices):
        apply_source(R[source],T[source])
        before=[(world@r.pose.bones[n].matrix).translation.copy() for n in ['LeftFoot','RightFoot']]
        recoil={0:0,1:0,2:0,3:.07,4:.05,5:.035,6:.022,7:.013,8:.006}.get(frame,0) if label=='fire' else 0
        ready_pose(recoil)
        assert max(((world@r.pose.bones[n].matrix).translation-p).length for n,p in zip(['LeftFoot','RightFoot'],before))<.0001
        for name in names:
            b=r.pose.bones[name];b.rotation_mode='QUATERNION'
            b.keyframe_insert('location',frame=frame);b.keyframe_insert('rotation_quaternion',frame=frame);b.keyframe_insert('scale',frame=frame)
            skin=(world@b.matrix)@rest[name].inverted();rot=skin.to_3x3();rr=C.transposed()@rot@C
            tt=C.transposed()@(skin.translation-center+rot@center)/unit
            flat=[max(-32768,min(32767,round(rr[row][col]*4096))) for col in range(3) for row in range(3)]
            data.extend(struct.pack('<9h3i',*flat,*[round(v) for v in tt]))
    action=r.animation_data.action;action.name='Aletha_Zenith_'+label;action.use_fake_user=True;actions[label]=action
    cb=action_get_channelbag_for_slot(action,r.animation_data.action_slot)
    for fc in cb.fcurves:
        for key in fc.keyframe_points:key.interpolation='LINEAR'
    struct.pack_into('<I',data,8,len(data)-12)
    target=project/'assets/animations/zenith_ranged_v1'/f'{label}.psxanim';target.write_bytes(data)
    report[label]={'frames':len(indices),'hz':fps,'bytes':len(data),'source':str(path.relative_to(project)),'lower_body_preserved':True}
r.animation_data.action=actions['aim'];s.frame_start=0;s.frame_end=1;s.frame_set(0)
# Socket uses bind-space coordinates, matching the skin-matrix runtime.
skin=(world@r.pose.bones['RightHand'].matrix)@rest['RightHand'].inverted()
rr=C.transposed()@skin.to_3x3()@C
bind_rotation=rr.inverted()
g=rest['RightHand'].translation
grip=C.transposed()@(g-center)/unit
muzzle=grip+bind_rotation@Vector((0,0,12000))
rotation=[round(v/(2*math.pi)*4096) for v in bind_rotation.to_euler('XYZ')]
(out/'socket.json').write_text(json.dumps({'grip':[round(v) for v in grip],
    'muzzle':[round(v) for v in muzzle],'rotation_q12':rotation},indent=2)+'\n')
# Preview the integrated cannon envelope in the same scale as its in-game model.
from cannon_mesh import cannon_mesh
cv,cf=cannon_mesh()
cm=bpy.data.meshes.new('Zenith arm cannon');cm.from_pydata(cv,[],cf);cm.update()
co=bpy.data.objects.new('Zenith arm cannon',cm);s.collection.objects.link(co)
cannon_unit=unit
co.matrix_world=(world@r.pose.bones['RightHand'].matrix)@rest['RightHand'].inverted()@Matrix.Translation(C@grip*unit+center)
# Skin-rotated local basis; cannon +Z points ahead in the ready pose.
co.matrix_world=(world@r.pose.bones['RightHand'].matrix)@rest['RightHand'].inverted()@Matrix.Translation(C@grip*unit+center)@(C@bind_rotation).to_4x4()@Matrix.Diagonal((cannon_unit,)*3+(1,))
cannon_world=co.matrix_world.copy()
co.parent=r;co.parent_type="BONE";co.parent_bone="RightHand";co.matrix_world=cannon_world
co.color=(.18,.49,.52,1)
# Save a single editable rig with all six actions, retaining the original mesh.
for ob in s.objects:
 if ob.type=='MESH':ob.hide_render=ob not in (mesh,co);ob.hide_set(ob not in (mesh,co))
mesh.hide_render=False;mesh.hide_set(False)
s.render.engine='BLENDER_WORKBENCH';s.render.resolution_x=480;s.render.resolution_y=640;s.render.resolution_percentage=100
s.display.shading.light='STUDIO';s.display.shading.color_type='OBJECT';mesh.color=(.44,.73,.80,1)
s.display.shading.show_shadows=True;s.display.shading.show_cavity=True;s.view_settings.view_transform='Standard'
s.camera.location=(3,-5,2.2);focus=Vector((0,-.15,1));s.camera.rotation_euler=(focus-s.camera.location).to_track_quat('-Z','Y').to_euler();s.camera.data.type='ORTHO';s.camera.data.ortho_scale=2.5
bpy.ops.wm.save_as_mainfile(filepath=str(out/'zenith-ranged.blend'))
s.render.filepath=str(project/'validation/zenith-ranged/ready-pose.png');bpy.ops.render.render(write_still=True)
(out/'validation.json').write_text(json.dumps(report,indent=2)+'\n')
print('RESULT',json.dumps(report))
