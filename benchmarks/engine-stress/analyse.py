"""Analyse the final 1,200 telemetry rows; align emulator counters by bus-cycle window.
FPS uses emulator display-start changes, not guest render calls. Stage timings overlap;
never add parent and child stages. GPU cost is the emulator's estimate, not silicon timing.
"""
import csv, json, pathlib, statistics, re, os
R=pathlib.Path(os.environ.get('PSOXIDE_STRESS_OUTPUT', str(pathlib.Path(__file__).resolve().parents[2]/'build/engine-stress'))).resolve()
HZ=33868800
STAGES=['visual_render_task','update','room','player','model_instances','sky','far_vista','present','ot_wait','world_flush']
COUNTS=['room_surfaces_considered','tri_primitives','model_instance_draws','room_submit_primitive_overflows','cd_room_chunk_loads','visual_deadline_misses']
def read(p):
    with p.open() as f:return [{k:int(v,16) if v.startswith('0x') else int(v or 0) for k,v in row.items()} for row in csv.DictReader(f)]
def pct(v,p):return sorted(v)[min(len(v)-1,int((len(v)-1)*p))] if v else 0
def analyse(path):
    log=(path/'run.log').read_text()
    polls=re.search(r'port1-polls=(\d+)',log)
    assert polls and 5400<=int(polls[1])<=5404, f'{path}: poll target not reached'
    rows=read(path/'profile.csv')
    if len(rows)<1200:raise ValueError(f'{path}: incomplete telemetry ({len(rows)})')
    rows=rows[-1200:]; lo=rows[0]['start_bus_cycles'];hi=rows[-1]['end_bus_cycles']
    assert not any(r.get('cd_room_chunk_loads',0) for r in rows), f'{path}: streaming in measured window'
    render=[r for r in rows if r['render']>0]
    assert render, path
    routes=read(path/'route.csv');routes=[r for r in routes if lo<=r['bus_cycles']<=hi]
    bytick={r['route_tick']:r for r in routes}
    gpu=[r for r in read(path/'gpu.csv') if r['route_tick'] in bytick]
    assert len(routes)>2 and gpu
    # Full route intervals only, avoiding the partially overlapping first interval.
    routes=routes[1:];ticks={r['route_tick'] for r in routes};gpu=[r for r in gpu if r['route_tick'] in ticks]
    wall=sum(r['bus_cycle_delta'] for r in routes)
    flips=[r for r in routes if r['display_start_changed']]
    intervals=[(b['bus_cycles']-a['bus_cycles'])/HZ*1000 for a,b in zip(flips,flips[1:])]
    audit=(path.parent/'audit.txt').read_text()
    faces=re.search(r'(\d+) world faces',audit)
    stages={k:round(statistics.mean(r.get(k,0) for r in (rows if k=='update' else render))) for k in STAGES}
    counts={k:{'mean':round(statistics.mean(r.get(k,0) for r in render),2),'max':max(r.get(k,0) for r in rows),'sum':sum(r.get(k,0) for r in rows)} for k in COUNTS}
    return {'scenario':path.parent.name,'dma':path.name,'profile_rows':len(rows),'render_calls':len(render),
        'bus_start':lo,'bus_end':hi,'seconds':round(wall/HZ,3),'first_poll':routes[0]['port1_polls'],'last_poll':routes[-1]['port1_polls'],
        'display_flips':len(flips),'display_fps':round(len(flips)*HZ/wall,2),
        'frame_interval_ms_p50':round(pct(intervals,.5),2),'frame_interval_ms_p95':round(pct(intervals,.95),2),
        'render_cycles_p95':pct([r['visual_render_task'] for r in render],.95),
        'world_faces':int(faces[1]) if faces else None,'stages':stages,'counts':counts,
        'gpu_cycles_per_flip':round(sum(g['gpu_cycles'] for g in gpu)/max(1,len(flips))),
        'gpu_busy_fraction':round(sum(g['gpu_cycles'] for g in gpu)/wall,3),
        'gpu_textured_tris_per_flip':round(sum(g['textured_tris'] for g in gpu)/max(1,len(flips)),1),
        'gpu_textured_quads_per_flip':round(sum(g['textured_quads'] for g in gpu)/max(1,len(flips)),1),
        'camera_ranges':{k:[min(r[k] for r in render),max(r[k] for r in render)] for k in ('camera_x_biased','camera_y_biased','camera_z_biased','player_view_yaw_q12')}}
results=[]
for p in sorted(R.glob('*/*/profile.csv')):
    if not (p.parent/'complete.json').exists():continue
    results.append(analyse(p.parent))
if not results:raise SystemExit('No completed stress runs found in '+str(R))
(R/'summary.json').write_text(json.dumps(results,indent=2))
print('| Scenario | DMA | faces | display fps | p95 interval ms | render kcy | room kcy | models kcy | vista kcy | GPU busy | redraws |')
print('|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|')
for r in results:
 s=r['stages'];print(f"| {r['scenario']} | {r['dma']} | {r['world_faces']} | {r['display_fps']:.2f} | {r['frame_interval_ms_p95']:.2f} | {s['visual_render_task']/1000:.1f} | {s['room']/1000:.1f} | {s['model_instances']/1000:.1f} | {s['far_vista']/1000:.1f} | {r['gpu_busy_fraction']:.1%} | {r['counts']['room_submit_primitive_overflows']['sum']} |")
