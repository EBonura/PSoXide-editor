import tempfile
from pathlib import Path
import struct
import unittest
from quake_chain_adapter import inspect, FIELDS, EXPECTED

class Contract(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.out = Path(self.tmp.name)
        p = dict.fromkeys(FIELDS, 0)
        p.update(EXPECTED, magic=0x51505358, weapon_selected=7, target_edges=4)
        self.data = bytearray(struct.pack('<34I', *(p[k] for k in FIELDS)))
        self.write()
        (self.out/'route.csv').write_text('bus_cycles,display_start_changed,port1_polls\n100,1,1\n200,1,2\n300,1,3\n')
        (self.out/'cd.csv').write_text('cycle,command\n50,0x06\n350,0x06\n')
    def write(self):
        (self.out/'ram.bin').write_bytes(self.data)
    def test_full_contract_and_interval_denominator(self):
        r=inspect(self.out)
        self.assertTrue(r['complete'])
        self.assertEqual(r['metrics']['presentations'],3)
        self.assertEqual(r['metrics']['presentation_intervals'],2)
        self.assertEqual(r['metrics']['elapsed_bus_cycles'],200)
    def test_each_required_field_rejects(self):
        original=self.data[:]
        for key in EXPECTED:
            with self.subTest(key=key):
                self.data=original[:]
                struct.pack_into('<I',self.data,FIELDS.index(key)*4,EXPECTED[key]+1)
                self.write()
                if key=='version':
                    with self.assertRaises(ValueError):inspect(self.out)
                else:self.assertFalse(inspect(self.out)['complete'])
    def test_sound_and_edge_masks(self):
        for key,value in [('weapon_selected',3),('target_edges',3)]:
            struct.pack_into('<I',self.data,FIELDS.index(key)*4,value);self.write()
            self.assertFalse(inspect(self.out)['complete'])
    def test_duplicate_probe_rejects(self):
        (self.out/'ram.bin').write_bytes(self.data*2)
        with self.assertRaises(ValueError):inspect(self.out)
    def test_truncated_probe_rejects(self):
        (self.out/'ram.bin').write_bytes(self.data[:-1])
        with self.assertRaises(ValueError):inspect(self.out)
    def test_missing_gameplay_window_rejects(self):
        (self.out/'cd.csv').write_text('cycle,command\n50,0x06\n')
        with self.assertRaises(ValueError):inspect(self.out)

if __name__=='__main__':unittest.main()
