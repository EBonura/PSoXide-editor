import bpy,json
from pathlib import Path
out=Path(__file__).resolve().parent;report={}
for variant in ['A','B']:
 bpy.ops.wm.open_mainfile(filepath=str(out/('new_'+variant+'.blend')));s=bpy.context.scene;r=next(o for o in s.objects if o.type=='ARMATURE');points=[]
 for f in range(29,36):
  s.frame_set(f);points.append({side:(r.matrix_world@r.pose.bones[side+'Foot'].matrix).translation.copy() for side in ['Left','Right']})
 report[variant]={side:[(b[side]-a[side]).length for a,b in zip(points,points[1:])] for side in ['Left','Right']}
 assert min(report[variant]['Left'][1:5])>.02,report
p=out/'validation.json';d=json.loads(p.read_text());d['handoff_audit']={'join_frame':32,'step_frame_pairs':[[f,f+1] for f in range(29,35)],'foot_displacement_metres_per_frame':report,'scope':'Source joined preview. Current engine crossfade not applied.'}
settles={}
for key,entry in [('walk_fwd_winddown',0),('walk_fwd_winddown_mirror',17)]:
 samples=d['transitions'][key]['body_samples'];first='left_foot' if entry==0 else 'right_foot';second='right_foot' if entry==0 else 'left_foot'
 def dist(a,b):return sum((x-y)**2 for x,y in zip(a,b))**.5
 first_drift=max(dist(samples[14][first],v[first]) for v in samples[14:]);second_travel=sum(dist(a[second],b[second]) for a,b in zip(samples[12:26],samples[13:27]))
 assert first_drift<.001 and second_travel>.06,(key,first_drift,second_travel)
 settles[key]={'first_foot_after_contact_max_drift_metres':first_drift,'second_foot_adjustment_travel_metres':second_travel}
d['stop_audit']=settles;p.write_text(json.dumps(d,indent=2)+'\n');print('RESULT',json.dumps({'handoff':report,'stop':settles}))
