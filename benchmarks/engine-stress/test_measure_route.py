"""Normal Play must remain measurable without enabling the guest profiler."""
import csv, tempfile, unittest
from pathlib import Path
from measure_route import measure, HZ, VBLANK

class DisplayCadenceTests(unittest.TestCase):
    def fixture(self, root):
        with (root/'route.csv').open('w') as f:
            writer=csv.DictWriter(f,fieldnames=['port1_polls','bus_cycles','display_start_changed'])
            writer.writeheader()
            for frame in range(64):
                writer.writerow(dict(port1_polls=100+2*frame,bus_cycles=frame*2*VBLANK,display_start_changed=1))

    def test_normal_guest_cadence_needs_no_profile_and_counters_stay_unknown(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);self.fixture(root);r=measure(root,100,226)
            self.assertAlmostEqual(r['display_fps'],HZ/(2*VBLANK),places=3)
            self.assertTrue(r['cadence_pass'])
            self.assertFalse(r['telemetry_available'])
            self.assertIsNone(r['primitive_overflows'])
            self.assertIsNone(r['measured_pass'])

    def test_empty_profile_does_not_force_instrumentation(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);self.fixture(root);(root/'profile.csv').write_text('guest_frame,render\n')
            self.assertFalse(measure(root,100,226)['telemetry_available'])

    def test_incomplete_poll_window_cannot_pass(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);self.fixture(root)
            with self.assertRaises(ValueError):measure(root,100,400)

if __name__=='__main__':unittest.main()
