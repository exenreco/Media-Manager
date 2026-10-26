import importlib.util
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
def module(name):
    spec = importlib.util.spec_from_file_location(name, ROOT / 'scripts' / (name + '.py'))
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value

fixtures = module('mobile_fixtures')
smoke = module('mobile_smoke')

class Fixtures(unittest.TestCase):
    def test_repeat_run_refuses_to_overwrite_and_duplicates_are_real(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / 'test-library'
            manifest = fixtures.create(root)
            self.assertEqual(manifest['Duplicates/01.mkv'], manifest['Duplicates/02.mkv'])
            self.assertNotEqual(manifest['Duplicates/01.mkv'], manifest['Duplicates/03.mkv'])
            before = (root / 'Manual/s1/01.mkv').read_bytes()
            with self.assertRaises(ValueError):
                fixtures.create(root)
            self.assertEqual((root / 'Manual/s1/01.mkv').read_bytes(), before)

    def test_absent_adb_is_not_reported_as_a_pass(self):
        from argparse import Namespace
        with patch.object(smoke.shutil, 'which', return_value=None):
            with self.assertRaisesRegex(RuntimeError, 'adb is unavailable'):
                smoke.android(Namespace(device=None), Path('.'))

    def test_unauthorized_device_is_never_installed_to(self):
        from argparse import Namespace
        with patch.object(smoke.shutil, 'which', return_value='adb'), patch.object(smoke, 'run', return_value='List of devices attached\nphone\tunauthorized\n') as run:
            with self.assertRaisesRegex(RuntimeError, 'Connect/authorize'):
                smoke.android(Namespace(device=None), Path('.'))
            run.assert_called_once_with(['adb','devices'])

    def test_ios_requires_mac_and_simulator_is_not_physical_validation(self):
        from argparse import Namespace
        with patch.object(smoke.sys, 'platform', 'win32'):
            with self.assertRaisesRegex(RuntimeError, 'requires macOS'):
                smoke.ios(Namespace(), Path('.'))

if __name__ == '__main__':
    unittest.main()
