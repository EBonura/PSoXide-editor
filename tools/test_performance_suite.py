#!/usr/bin/env python3
"""Fail-closed replay/cache tests using a deterministic subprocess, no guest assets."""
from concurrent.futures import ThreadPoolExecutor
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import struct
from unittest.mock import patch
import contextlib
import unittest

import performance_suite as suite

DRIVER = '''import argparse,os,pathlib,sys,time
p=argparse.ArgumentParser();p.add_argument('--config-dir');p.add_argument('--steps',type=int);p.add_argument('--path');p.add_argument('--input-tape');p.add_argument('--stop-at-poll',type=int);p.add_argument('--embedded-playtest',action='store_true');a=p.parse_args();a.mode=os.environ.get('PSOXIDE_TEST_MODE','ok');a.count=os.environ['PSOXIDE_TEST_COUNT']
assert 'PSOXIDE_UNDECLARED_TEST' not in os.environ
with open(a.count,'a') as f:f.write('run\\n')
time.sleep(.03)
r=pathlib.Path(a.config_dir).parent
(r/'frames').mkdir();(r/'frames/a.ppm').write_bytes(b'frame1');(r/'frames/b.ppm').write_bytes(b'frame2');(r/'audio.wav').write_bytes(b'pcm');(r/'state.json').write_text('{"player":1}')
if a.mode=='fail':sys.exit(2)
n=a.steps if a.mode=='cap' else 10
print(f'tick={n}  cycles=20  pc=0x80010000  stopped-at={n}')
print(f'route-ticks=5  port1-polls={a.stop_at_poll}')
print('vram_fnv1a_64=0x1');print('display_fnv1a_64=0x2  w=320  h=240')
'''


class SuiteTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name); self.store = self.root/'store'
        (self.root/'driver.py').write_text(DRIVER)
        (self.root/'disc.bin').write_bytes(b'disc')
        (self.root/'disc.cue').write_text('FILE "disc.bin" BINARY\n TRACK 01 MODE2/2352\n INDEX 01 00:00:00\n')
        (self.root/'tape').write_bytes(b'PXITAPE2'+struct.pack('<II', 10, 0)+bytes(60))
        self.bindings = {k: str(self.root/v) for k,v in {'driver':'driver.py','image':'disc.cue','tape':'tape'}.items()}
        self.bindings['emulator'] = sys.executable
        self.case = {'id':'fixture','lane':'acceptance','build_provenance':{'directory':'fixture','source':'test'},
            'inputs':{k:'$'+k for k in self.bindings},
            'argv':['{input.driver}','--config-dir','{out}/config','--path','{input.image}',
                    '--input-tape','{input.tape}','--embedded-playtest','--steps','100','--stop-at-poll','5',
                    ] ,
            'environment':{'PSOXIDE_TEST_COUNT':str(self.root/'count')},
            'completion':{'poll':5},'required_outputs':['frames/a.ppm','frames/b.ppm','audio.wav'],
            'quality':{'visual':{'patterns':['frames/*.ppm'],'scope':'two fixture checkpoints'},
                       'audio':{'patterns':['audio.wav'],'scope':'fixture PCM'},
                       'gameplay':{'patterns':['state.json'],'scope':'fixture player state'}}}

    def run_case(self, case=None):
        return suite.run_case(case or self.case, self.bindings, self.store)

    def test_hit_rehash_and_no_repeat_execution(self):
        os.environ['PSOXIDE_UNDECLARED_TEST'] = 'must not leak'; self.addCleanup(os.environ.pop,'PSOXIDE_UNDECLARED_TEST',None)
        first=self.run_case();second=self.run_case()
        self.assertEqual((first['cache'],second['cache']),('miss','hit'))
        self.assertEqual((self.root/'count').read_text(),'run\n')
        self.assertEqual(suite.compare(Path(first['result']),Path(second['result']))['acceptance'],'pass')

    def test_transitive_disc_tamper_and_options_invalidate(self):
        a=suite.plan(self.case,self.bindings)['key']; (self.root/'disc.bin').write_bytes(b'DISC')
        b=suite.plan(self.case,self.bindings)['key'];self.assertNotEqual(a,b)
        other=copy.deepcopy(self.case);other['environment']['PSOXIDE_EXPERIMENTAL_DMA_FIFO']='1'
        self.assertNotEqual(b,suite.plan(other,self.bindings)['key'])
        other=copy.deepcopy(self.case);other['build_provenance']['directory']='different-path'
        self.assertNotEqual(b,suite.plan(other,self.bindings)['key'])

    def test_same_size_cached_corruption_rejected(self):
        result=self.run_case();(Path(result['result'])/'artifacts/frames/a.ppm').write_bytes(b'FRAME1')
        with self.assertRaisesRegex(ValueError,'artifact'):self.run_case()

    def test_partial_never_hit(self):
        key=suite.plan(self.case,self.bindings)['key'];p=self.store/'results'/key;p.mkdir(parents=True)
        with self.assertRaises(FileNotFoundError):self.run_case()

    def test_failure_and_cap_never_complete(self):
        for mode in ('fail','cap'):
            case=copy.deepcopy(self.case);case['environment']['PSOXIDE_TEST_MODE']=mode
            with self.assertRaises(ValueError):self.run_case(case)
            key=suite.plan(case,self.bindings)['key'];self.assertFalse((self.store/'results'/key).exists())
        self.assertEqual(len(list((self.store/'failures').iterdir())),2)

    def test_duplicate_concurrent_work_is_single_execution(self):
        with ThreadPoolExecutor(max_workers=2) as pool:results=list(pool.map(lambda _:self.run_case(),range(2)))
        self.assertEqual(sorted(x['cache'] for x in results),['hit','miss'])
        self.assertEqual((self.root/'count').read_text(),'run\n')

    def test_comparison_rejects_different_runtime_options(self):
        a=self.run_case();other=copy.deepcopy(self.case);other['environment']['PSOXIDE_EXPERIMENTAL_DMA_FIFO']='1'
        b=self.run_case(other)
        with self.assertRaisesRegex(ValueError,'environment'):suite.compare(Path(a['result']),Path(b['result']))

    def test_quick_lane_cannot_pass_quality(self):
        case=copy.deepcopy(self.case);case['lane']='quick';r=self.run_case(case)
        result=suite.compare(Path(r['result']),Path(r['result']))
        self.assertEqual(result['acceptance'],'not established')
        self.assertTrue(all(x['status']=='unavailable' for x in result['quality'].values()))

    def test_missing_multiframe_evidence_not_pass(self):
        case=copy.deepcopy(self.case);case['quality']['visual']['minimum_count']=3;r=self.run_case(case)
        self.assertEqual(suite.compare(Path(r['result']),Path(r['result']))['quality']['visual']['status'],'unavailable')

    def test_receipt_summary_tamper_rejected(self):
        r=self.run_case();p=Path(r['result'])/'receipt.json';j=json.loads(p.read_text());j['summary']['bus_cycles']=1;suite.write_json(p,j)
        with self.assertRaisesRegex(ValueError,'sealed artifact'):suite.validate_result(Path(r['result']))

    def test_literal_input_and_unknown_flag_rejected(self):
        self.case['argv'][self.case['argv'].index('--path')+1]='/unhashed/image.cue'
        with self.assertRaisesRegex(ValueError,'hashed input'):suite.plan(self.case,self.bindings)

    def test_comparison_rejects_changed_adapter_input(self):
        self.case['inputs']['adapter']='$driver'
        a=self.run_case()
        (self.root/'driver.py').write_text(DRIVER+'\n# changed semantic parser\n')
        b=self.run_case()
        with self.assertRaisesRegex(ValueError,'non-candidate input'):suite.compare(Path(a['result']),Path(b['result']))

    def test_poll_completion_rejects_tape_exhaustion(self):
        (self.root/'tape').write_bytes(b'PXITAPE2'+struct.pack('<II', 5, 0)+bytes(30))
        with self.assertRaisesRegex(ValueError, 'strictly beyond'): self.run_case()

    def test_fault_with_successful_summary_rejected(self):
        (self.root/'driver.py').write_text(DRIVER + "\nprint('[cli] step 10 failed: fixture fault')\n")
        with self.assertRaisesRegex(ValueError, 'guest fault'): self.run_case()

    def test_empty_quality_cannot_pass(self):
        self.case['quality']['visual'] = {'patterns':['missing*'], 'minimum_count':0, 'scope':'none'}
        r = self.run_case()
        with self.assertRaisesRegex(ValueError, 'evidence floor'): suite.compare(Path(r['result']),Path(r['result']))

    def test_lock_wait_input_change_rejected(self):
        self.run_case()
        @contextlib.contextmanager
        def changed(*args):
            (self.root/'disc.bin').write_bytes(b'DISC')
            yield
        with patch.object(suite, 'key_lock', changed):
            with self.assertRaisesRegex(ValueError, 'waiting for cache lock'): self.run_case()

    def test_cumulative_cycles_not_summed(self):
        out = self.root/'parsed'; out.mkdir()
        (out/'stdout.txt').write_text('tick=10 cycles=40 stopped-at=10\nroute-ticks=2 port1-polls=5\nvram_fnv1a_64=1\ndisplay_fnv1a_64=2\n')
        (out/'cycles.csv').write_text('bus_cycles,issue_cycles\n20,5\n40,6\n')
        result = suite.parse_summary(out)
        self.assertEqual(result['bus_cycles'],40)
        self.assertEqual(result['guest_cycle_columns'],{'issue_cycles':11})

    def test_opaque_wrapper_and_config_rejected(self):
        self.bindings['emulator']=self.bindings['driver'];(self.root/'driver.py').write_text('#!/usr/bin/env python3\n')
        with self.assertRaisesRegex(ValueError,'wrapper'):suite.plan(self.case,self.bindings)
        self.bindings['emulator']=sys.executable
        self.case['argv'][self.case['argv'].index('--config-dir')+1]='/existing/config'
        with self.assertRaisesRegex(ValueError,'isolated'):suite.plan(self.case,self.bindings)


if __name__=='__main__':unittest.main()
