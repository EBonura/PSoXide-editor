"""Export the saved editable walk's current bone animation to PSXA v2."""
import bpy,json,struct,math
from pathlib import Path
import numpy as np
from mathutils import Matrix
OUT=Path(__file__).resolve().parent
bpy.ops.wm.open_mainfile(filepath=str(OUT/'light-walk.blend'));rig=bpy.data.objects['Mantis walk rig'];names=json.loads(rig['joint_order']);C=Matrix(((.0001,0,0,0),(0,0,-.0001,0),(0,.0001,0,0),(0,0,0,1)));CI=C.inverted();frames=[]
for fi in range(1,int(rig['runtime_frames'])+1):
 bpy.context.scene.frame_set(fi);bpy.context.view_layer.update();frames.append([np.array(CI@rig.pose.bones[name].matrix@rig.data.bones[name].matrix_local.inverted()@C) for name in names])
frames=np.array(frames);shift=max(0,math.ceil(math.log2(max(1,np.abs(frames[:,:,:3,3]).max())/32760)));data=bytearray(struct.pack('<4H',len(names),len(frames),bpy.context.scene.render.fps,shift))
for frame in frames:
 for mat in frame:data+=struct.pack('<9h3h',*np.rint(mat[:3,:3].T*4096).astype(int).ravel(),*np.rint(mat[:3,3]/2**shift).astype(int))
previous=(OUT/'walk.psxanim').read_bytes();out=bytearray(previous[:12]);struct.pack_into('<H',out,4,2);struct.pack_into('<I',out,8,len(data));out+=data;(OUT/'walk.psxanim').write_bytes(out);print('RESULT exported',len(frames),'frames',len(names),'joints')
