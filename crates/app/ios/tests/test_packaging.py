import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest

spec = importlib.util.spec_from_file_location('packaging', Path(__file__).parents[1] / 'embed_ad_frameworks.py')
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class FrameworkSlices(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / 'Example.xcframework'
        self.root.mkdir()
        self.libraries = []
        for identifier, arches, variant in [
            ('ios-arm64', ['arm64'], ''),
            ('ios-arm64_x86_64-simulator', ['arm64', 'x86_64'], 'simulator'),
        ]:
            (self.root / identifier / 'Example.framework').mkdir(parents=True)
            row = dict(LibraryIdentifier=identifier, LibraryPath='Example.framework',
                       SupportedPlatform='ios', SupportedArchitectures=arches)
            if variant:
                row['SupportedPlatformVariant'] = variant
            self.libraries.append(row)
        self.save()

    def save(self):
        (self.root / 'Info.plist').write_bytes(plistlib.dumps(dict(AvailableLibraries=self.libraries)))

    def tearDown(self):
        self.temp.cleanup()

    def test_distinguishes_arm64_device_and_simulator(self):
        self.assertEqual(packaging.select_slice(self.root, 'iphoneos', 'arm64').parent.name, 'ios-arm64')
        self.assertEqual(packaging.select_slice(self.root, 'iphonesimulator', 'arm64').parent.name,
                         'ios-arm64_x86_64-simulator')

    def test_refuses_unsupported_architecture(self):
        with self.assertRaises(ValueError):
            packaging.select_slice(self.root, 'iphoneos', 'x86_64')

    def test_refuses_slice_paths_outside_xcframework(self):
        self.libraries[0]['LibraryPath'] = '../../'
        self.save()
        with self.assertRaises(ValueError):
            packaging.select_slice(self.root, 'iphoneos', 'arm64')


if __name__ == '__main__':
    unittest.main()
