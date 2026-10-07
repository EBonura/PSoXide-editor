"""Assert control and movement evidence from the diagnostic gameplay recording."""
from pathlib import Path
import re,json,collections
p=Path(__file__).resolve().parents[2]/'validation/zenith-ranged'
s=(p/'video/capture.log').read_text();rows=[];events=[]
for line in s.splitlines():
    if 'player-weapon,' in line:
        rows.append(list(map(int,line.split('player-weapon,')[1].strip(',').split(','))))
    if 'player projectile:' in line:
        events.append((int(re.search(r'guest f(\d+)',line)[1])-1,line.split('player projectile:')[1]))
starts=[int(re.search(r'guest f(\d+)',l)[1])-1 for l in s.splitlines() if 'player ranged:start' in l]
assert len(starts)==10
assert sum(e=='release' for _,e in events)==10
assert sum(e=='hit' for _,e in events)==10
assert not any(t<740 or t>=1600 for t,_ in events)
assert all(r[5]==0 for r in rows if r[1]==0)
assert any(r[2]==1 and r[10]==1 and r[1]==0 and 570<=r[0]<=690 for r in rows)
for action in [37,38,39,40]:
    assert any(r[6]==action for r in rows),action
moving_shots=0
for shot in starts:
    near=[r for r in rows if shot-12<=r[0]<=shot+16]
    # The final forward approach stops at the enemy collider; exclude standing shots.
    if near and all(r[6] in (37,38,39,40) for r in near):
        assert len({(r[7],r[9]) for r in near})>1
        moving_shots+=1
assert moving_shots>=5
report={'shots':len(starts),'releases':sum(e=='release' for _,e in events),
        'hits':sum(e=='hit' for _,e in events),'locked_R2_without_L2_fires':0,
        'all_four_aimed_walk_actions_seen':True,'verified_moving_shots':moving_shots,
        'minimum_enemy_zenith_hp':min(r[12] for r in rows),
        'actions_seen':dict(collections.Counter(r[6] for r in rows))}
(p/'gameplay-validation.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
