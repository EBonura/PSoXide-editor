#!/usr/bin/env python3
"""Read-only Quake v9 chain completion and canonical presentation-window adapter.

Contract mirrors quake-psx host/quake-build/main.rs at 24908826:
read_probe_version, validate_e1m1_chain_probe, full_level_render_metrics.
No asset paths, guest writes, or benchmark-building side effects.
"""
import csv
import json
from pathlib import Path
import struct
import sys

FIELDS = '''magic version complete phase failure_code failure_map failure_entity
failure_detail total_frames maps_loaded maps_validated transitions weapon_selected
weapon_fired weapon_animated monster_present monster_animated monster_state_bounds
monster_attack monster_pain monster_death boss current_map route_index last_health
state_ranges valid_state_ranges map_loads stage_frames shock_count intermission_state
player_state weapon_pickups target_edges'''.split()
EXPECTED = dict(version=9, failure_code=0, complete=1, phase=0x51,
                maps_loaded=6, maps_validated=6, current_map=2, route_index=60,
                map_loads=2, transitions=1, player_state=0x7fff)

def inspect(out):
    ram = (out / 'ram.bin').read_bytes()
    magic = struct.pack('<II', 0x51505358, 9)
    hits = [i for i in range(0, len(ram) - 136 + 1, 4)
            if ram[i:i + 8] == magic]
    if len(hits) != 1:
        raise ValueError(f'expected exactly one aligned complete v9 probe; found {len(hits)}')
    probe = dict(zip(FIELDS, struct.unpack_from('<34I', ram, hits[0])))
    failures = {k: {'actual': probe[k], 'expected': v}
                for k, v in EXPECTED.items() if probe[k] != v}
    if probe['weapon_selected'] & 7 != 7:
        failures['mover_sound_mask'] = probe['weapon_selected']
    if probe['target_edges'] < 4:
        failures['target_edges_minimum4'] = probe['target_edges']
    with (out / 'route.csv').open() as f:
        route = list(csv.DictReader(f))
    with (out / 'cd.csv').open() as f:
        cd = list(csv.reader(f))[1:]
    reads = [int(r[0]) for r in cd if len(r) >= 2 and r[1] == '0x06']
    if len(reads) < 2:
        raise ValueError('fewer than two ReadN sessions')
    # Rust max_by_key chooses the last pair on ties.
    _, (start, end) = max(enumerate(zip(reads, reads[1:])),
                         key=lambda x: (max(0, x[1][1] - x[1][0]), x[0]))
    flips = [int(r['bus_cycles']) for r in route
             if int(r['display_start_changed']) == 1
             and start < int(r['bus_cycles']) < end]
    if len(flips) < 2 or flips[-1] <= flips[0]:
        raise ValueError('invalid gameplay presentation interval')
    polls = int(route[-1]['port1_polls'])
    if polls <= 0:
        failures['controller_polls'] = polls
    elapsed = flips[-1] - flips[0]
    return {'complete': not failures,
            'evidence': {'probe': probe, 'ram_offset': hits[0],
                         'failures': failures, 'final_polls': polls,
                         'readn_window_cycles': [start, end]},
            'gameplay': {'status': 'observed' if not failures else 'unavailable',
                         'basis': 'guest v9 completion probe, not pixel validation',
                         'scope': 'fixed-step diagnostic E1M1 route and natural E1M2 transition'},
            'metrics': {'presentations': len(flips),
                        'presentation_intervals': len(flips) - 1,
                        'elapsed_bus_cycles': elapsed,
                        'fps_x1000': (len(flips) - 1) * 33868800 * 1000 // elapsed,
                        'fps': (len(flips) - 1) * 33868800 / elapsed,
                        'host_seconds_is_performance': False}}

if __name__ == '__main__':
    try:
        if len(sys.argv) != 3:
            raise ValueError('usage: quake_chain_adapter.py RUN_DIR RESOLVED_INPUTS_JSON')
        json.loads(Path(sys.argv[2]).read_text())  # Runner binds and hashes inputs.
        result = inspect(Path(sys.argv[1]))
    except (OSError, ValueError, KeyError, IndexError, struct.error) as error:
        result = {'complete': False, 'evidence': {'error': str(error)},
                  'gameplay': {'status': 'unavailable'}, 'metrics': {}}
    print(json.dumps(result, sort_keys=True))
    sys.exit(0 if result['complete'] else 1)
