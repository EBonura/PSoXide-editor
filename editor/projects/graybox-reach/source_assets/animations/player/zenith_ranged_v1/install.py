"""Install the generated study and a 48-triangle integrated arm cannon."""
from pathlib import Path
import json,struct,re
out=Path(__file__).resolve().parent;project=out.parents[3]
template=(project.parent/'default/assets/models/sword1_light/sword1_light.psxmdl').read_bytes()
j,p,v,f,m,*_=struct.unpack_from('<8H',template,12);po=28+j*4+m*8
from cannon_mesh import cannon_mesh
vertices,faces=cannon_mesh()
b=bytearray(template[:po]);struct.pack_into('<H',b,26,98) # Aletha 70 Q12, visual scale 360/256
struct.pack_into('<HH',b,16,len(vertices),len(faces))
b+=struct.pack('<6H',0,0,len(vertices),0,len(faces),0)+template[po+12:po+16]
for v in vertices:b+=struct.pack('<3hBB',*v,255,0)
for f in faces:
 for i,(u,v) in zip(f,[(24,24),(88,24),(88,88)]):b+=struct.pack('<HBB',i,u,v)
b+=bytes((len(faces)+3)//4);struct.pack_into('<I',b,8,len(b)-12)
(project/'assets/models/zenith_projector/projector.psxmdl').write_bytes(b)
path=project/'project.ron';s=path.read_text()
if 'name: "Zenith Projector"' in s:
 socket=json.loads((out/'socket.json').read_text());vec=lambda v:'('+', '.join(map(str,v))+')'
 s=re.sub(r'(name: "zenith_grip", joint: 13, translation: )\([^)]*\)(, translation_space: BindSpace, rotation_q12: )\([^)]*\)',lambda m:m[1]+vec(socket['grip'])+m[2]+vec(socket['rotation_q12']),s)
 s=re.sub(r'(name: "Zenith Projector Muzzle", joint: 13, capsule: \(start: )\([^)]*\)(, end: )\([^)]*\)',lambda m:m[1]+vec(socket['muzzle'])+m[2]+vec(socket['muzzle']),s)
 path.write_text(s)
 print('Updated integrated arm cannon and sockets')
 raise SystemExit(0)
ids=list(range(215,221));mapping=list(zip(['RangedAim','RangedWalk','RangedBackward','RangedLeft','RangedRight','RangedAttack'],['aim','walk','backward','left','right','fire'],ids))
def resource_block(id):
 a=s.index('            id: ('+str(id)+'),');b=s.index('\n        /*[',a);return a,b
# Modify the current animation set without touching unrelated project edits.
a,b=resource_block(61);part=s[a:b]
for key in ['action_clips','weapon_appearance_tracks']:
 pattern=r'(                '+key+r': \[)(.*)(\],)'
 match=re.search(pattern,part);assert match
 body=match[2]
 # Tuple entries are balanced; remove only the two old Zenith sword bindings.
 entries=[];depth=0;start=0
 for i,ch in enumerate(body):
  depth+=ch=='(';depth-=ch==')'
  if ch==',' and depth==0:entries.append(body[start:i].strip());start=i+1
 entries.append(body[start:].strip())
 entries=[e for e in entries if not e.startswith(('(action: VertLightAttack,','(action: VertHeavyAttack,'))]
 for action,stem,id in mapping:
  if key=='action_clips':
   entries.append(f'(action: {action}, clip: ({id}), options: Some((looping: {str(stem!="fire").lower()}, in_place: false, speed_q8: 256, frame_start: 0, frame_end: 65535, push_distance: 0, push_frame_start: 0, push_frame_end: 0)))')
  else:entries.append(f'(action: {action}, weapon: (222), character_socket: "zenith_grip", fully_visible_frame: 0, hidden_frame: 65535, transition_frames: 0, trail: None)')
 part=part[:match.start(2)]+', '.join(entries)+part[match.end(2):]
s=s[:a]+part+s[b:]
socket=json.loads((out/'socket.json').read_text());vec=lambda v:'('+', '.join(map(str,v))+')'
a,b=resource_block(27);part=s[a:b];part=part.replace('attachments: [','attachments: [(name: "zenith_grip", joint: 13, translation: '+vec(socket['grip'])+', translation_space: BindSpace, rotation_q12: '+vec(socket['rotation_q12'])+'), ');s=s[:a]+part+s[b:]
a,b=resource_block(62);part=s[a:b];line=next(l for l in part.splitlines() if 'combat_capsules:' in l);body=line.split('combat_capsules: [',1)[1].rsplit('],',1)[0]
entries=[];depth=0;start=0
for i,ch in enumerate(body):
 depth+=ch=='(';depth-=ch==')'
 if ch==',' and depth==0:entries.append(body[start:i].strip());start=i+1
entries.append(body[start:].strip());entries=[e for e in entries if 'action: VertLightAttack' not in e and 'action: VertHeavyAttack' not in e]
entries.append('(name: "Zenith Projector Muzzle", joint: 13, capsule: (start: '+vec(socket['muzzle'])+', end: '+vec(socket['muzzle'])+', radius: 48), role: ProjectileEmitter(action: RangedAttack, charge_start_frame: 0, active_start_frame: 2, active_end_frame: 3, projectile: None, speed: 256, lifetime_ticks: 120, min_range: 0, max_range: 24000, damage: 28, poise_damage: 0, tint_rgb: (112, 232, 208)))')
part=part.replace(line,'                combat_capsules: ['+', '.join(entries)+'],');s=s[:a]+part+s[b:]
resources=''
for action,stem,id in mapping:
 resources+=f'''        (id: ({id}), name: "Aletha / Zenith {stem}", data: AnimationClip((psxanim_path: "assets/animations/zenith_ranged_v1/{stem}.psxanim", skeleton: Some((23)), source: None, bake: ModelNative, role: Generic, looping: {str(stem!="fire").lower()}, tags: ["zenith_ranged_v1"], calibration: (in_place: false, offset: (0, 0, 0))))),
'''
resources+='''        (id: (221), name: "Zenith Projector Body", data: Model((model_path: "assets/models/zenith_projector/projector.psxmdl", source_path: None, texture_path: Some("../default/assets/models/weapon_energy/weapon_energy_noise_v1.psxt"), skeleton: Some((24)), world_height: 128, collision_radius: 48, scale_q8: (256, 256, 256), default_visual_yaw_q12: 0, attachments: []))),
        (id: (222), name: "Zenith Projector", data: Weapon((class: Ranged, model: Some((221)), default_character_socket: "zenith_grip", grip: (name: "grip", translation: (0, 0, 0), rotation_q12: (0, 0, 0)), hitboxes: [], arc_reach: 0, arc_half_angle_degrees: 0, damage: 28, poise_damage: 0))),
'''
pos=s.rindex('    ],\n    next_resource_id:');s=s[:pos]+resources+s[pos:];s=s.replace('next_resource_id: 215','next_resource_id: 223');path.write_text(s)
print('Installed 6 animation clips and 36-triangle projector')
