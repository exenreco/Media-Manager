import importlib.util
from pathlib import Path
import plistlib
import tempfile
import stat
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

    def test_copied_read_only_sdk_can_be_resigned(self):
        binary = self.root / 'ios-arm64' / 'Example.framework' / 'Example'
        binary.write_bytes(b'framework')
        binary.chmod(stat.S_IRUSR)
        packaging.make_writable(binary.parent)
        self.assertTrue(binary.stat().st_mode & stat.S_IWUSR)
        binary.write_bytes(b'resigned framework')


class PlatformMetadata(unittest.TestCase):
    def test_simulator_metadata_does_not_claim_to_be_a_phone_bundle(self):
        original = {'CFBundleSupportedPlatforms': ['iPhoneOS', 'iPadOS']}
        info = packaging.platform_info(original, 'iphonesimulator', '26.0')
        self.assertEqual(info['CFBundleSupportedPlatforms'], ['iPhoneSimulator'])
        self.assertEqual(info['DTSDKName'], 'iphonesimulator26.0')
        self.assertEqual(original['CFBundleSupportedPlatforms'], ['iPhoneOS', 'iPadOS'])

    def test_device_metadata_uses_iphoneos(self):
        info = packaging.platform_info({}, 'iphoneos', '26.0')
        self.assertEqual(info['CFBundleSupportedPlatforms'], ['iPhoneOS'])
        self.assertEqual(info['UIDeviceFamily'], [1, 2])

    def test_arm64_does_not_make_a_device_binary_simulator_compatible(self):
        packaging.validate_platform(' platform 7\n', 'iphonesimulator')
        packaging.validate_platform(' platform IOSSIMULATOR\n', 'iphonesimulator')
        packaging.validate_platform(' platform IOS\n', 'iphoneos')
        with self.assertRaises(ValueError):
            packaging.validate_platform(' platform 2\n', 'iphonesimulator')
        with self.assertRaises(ValueError):
            packaging.validate_platform(' platform 7\n', 'iphoneos')


if __name__ == '__main__':
    unittest.main()
