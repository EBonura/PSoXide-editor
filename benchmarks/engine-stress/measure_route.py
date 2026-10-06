"""Measure displayed cadence without guest instrumentation; enrich with telemetry if present.
Normal Play builds intentionally omit emulator-telemetry. Missing counters are unknown,
not zero. Performance acceptance must use the normal guest; instrumented stage costs
are diagnostic and may perturb the workload being measured.
"""
import csv, json, pathlib, statistics, sys
HZ=33868800
VBLANK=571240

def read(p):
    with p.open() as f:return [{k:int(v,16) if v.startswith('0x') else int(v or 0) for k,v in row.items()} for row in csv.DictReader(f)]
def pct(xs,p):return sorted(xs)[int((len(xs)-1)*p)] if xs else None

def measure(root,first,last):
    routes=[r for r in read(root/'route.csv') if first<=r['port1_polls']<=last]
    if len(routes)<3:raise ValueError('insufficient route samples')
    if routes[-1]['port1_polls'] < last-4:raise ValueError('requested poll window was not completed')
    flips=[r for r in routes if r['display_start_changed']]
    intervals=[b['bus_cycles']-a['bus_cycles'] for a,b in zip(flips,flips[1:])]
    if len(intervals)<30:raise ValueError('insufficient displayed frames')
    long=sum(i>2*VBLANK+100 for i in intervals)
    result=dict(first_poll=routes[0]['port1_polls'],last_poll=routes[-1]['port1_polls'],display_intervals=len(intervals),
        display_fps=round(len(intervals)*HZ/sum(intervals),3),
        interval_ms_p95=round(pct(intervals,.95)/HZ*1000,3),interval_ms_max=round(max(intervals)/HZ*1000,3),
        intervals_over_two_vblanks=long,cadence_pass=long==0,
        telemetry_available=False,primitive_overflows=None,measured_pass=None)
    lo=routes[0]['bus_cycles'];hi=routes[-1]['bus_cycles'];profile=root/'profile.csv'
    rows=[r for r in read(profile) if lo<=r['start_bus_cycles'] and r['end_bus_cycles']<=hi and r['render']>0] if profile.exists() else []
    if rows:
        overflows=sum(r['room_submit_primitive_overflows'] for r in rows)
        result.update(telemetry_available=True,render_samples=len(rows),primitive_overflows=overflows,
            render_cycles_p95=pct([r['visual_render_task'] for r in rows],.95),
            mean_cycles={k:round(statistics.mean(r[k] for r in rows)) for k in ['visual_render_task','room','player','model_instances','update']},
            max_actor_draws=max(r['model_instance_draws'] for r in rows),measured_pass=long==0 and overflows==0,
            camera_position_telemetry_available=any(r['camera_x_biased']!=1000000 or r['camera_y_biased']!=1000000 or r['camera_z_biased']!=1000000 for r in rows),
            camera_ranges={k:[min(r[k] for r in rows),max(r[k] for r in rows)] for k in ['camera_x_biased','camera_y_biased','camera_z_biased','player_view_yaw_q12']})
    return result
if __name__=='__main__':
    print(json.dumps(measure(pathlib.Path(sys.argv[1]),int(sys.argv[2]),int(sys.argv[3])),indent=2))
