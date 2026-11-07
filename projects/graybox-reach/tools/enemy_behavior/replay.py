#!/usr/bin/env python3
"""Generate poll-clock encounter tapes and optionally replay the built Graybox disc.

Usage: python3 replay.py [--run] [--case observe|circle|retreat|reset|blocked]
The emulator inputs drive the actual controller; host policy scenarios are tested
separately by psx-game-runtime's entities::tactics tests.
"""
from pathlib import Path
import argparse, csv, json, subprocess, collections
P=Path(__file__).resolve().parents[2]
R=P.parents[2]
O=P/'validation/enemy-behavior'
GOALS=['None','Hold','Approach','CircleLeft','CircleRight','Retreat','WaitRetry','Reposition','ReturnHome']
def tape(case):
    rows=[]
    for frame in range(3960 if case == 'blocked' else 2760):
        buttons=0; lx=ly=128
        # Reset after boot/streaming, then use a fixed input epoch.
        if frame in (360,):buttons=2049 if case=='blocked' else 1025
        t=frame-360
        if case=='circle' and 180<=t<700:lx=230
        if case=='retreat' and 180<=t<700:ly=255
        if case=='reset' and t==900:buttons=1025
        rows.append([frame,buttons,128,128,lx,ly])
    path=O/(case+'.csv')
    with path.open('w') as f:
        f.write('psoxide-tape,v2,clock=pad_poll,start_poll=0\n')
        w=csv.writer(f);w.writerow(['frame','buttons','right_x','right_y','left_x','left_y']);w.writerows(rows)
    return path

def summarize(log):
    rows=[]
    for line in log.read_text().splitlines():
        if 'enemy-study,' in line:
            part=line.split('enemy-study,',1)[1].rstrip(',')
            try: row=list(map(int,part.split(',')))
            except ValueError:continue
            if len(row)==16:rows.append(row)
    header=['tick','player_x','player_y','player_z','enemy_x','enemy_y','enemy_z','yaw','state','clip','goal','result','generation','remaining','running','retries']
    with log.with_suffix('.trace.csv').open('w') as f:
        w=csv.writer(f);w.writerow(header);w.writerows(rows)
    summary={'samples':len(rows),'goals':dict(collections.Counter(GOALS[r[10]] for r in rows)),
        'states':dict(collections.Counter(r[8] for r in rows)), 'clips':dict(collections.Counter(r[9] for r in rows))}
    log.with_suffix('.summary.json').write_text(json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))

def main():
    a=argparse.ArgumentParser();a.add_argument('--run',action='store_true');a.add_argument('--case',choices=['observe','circle','retreat','reset','blocked'],default='observe');a.add_argument('--summarize',type=Path)
    a.add_argument('--disc', type=Path, default=P/'baked/graybox_reach.cue')
    args=a.parse_args();O.mkdir(exist_ok=True)
    if args.summarize:summarize(args.summarize);return
    path=tape(args.case)
    if not args.run:print(path);return
    log=O/(args.case+'.log');shots=O/(args.case+'-shots')
    with log.open('w') as f:
        subprocess.run([str(R/'target/release/frontend'),'launch','--config-dir',str(O/'emulator-config'),'--path',str(args.disc.resolve()),'--embedded-playtest','--input-tape',str(path),'--stop-at-poll',str(3960 if args.case == 'blocked' else 2760),'--steps','10000000000','--guest-debug-log','--route-screenshot-dir',str(shots),'--route-screenshot-interval','240','--dump-hash'],cwd=R,stdout=f,stderr=subprocess.STDOUT,check=True)
    summarize(log)
if __name__=='__main__':main()
