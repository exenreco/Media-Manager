"""Check that symbol export refuses mismatched or stripped build inputs."""
import contextlib
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

sys.path.insert(0, str(Path(__file__).parents[1]))
from export_android_symbols import export


class SymbolChecks(unittest.TestCase):
    def check_export(self, source_id='aabb', dwarf=True):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'ndk/toolchains/llvm/prebuilt/linux/bin').mkdir(parents=True)
            source = root / 'target/aarch64-linux-android/release/libapp.so'
            source.parent.mkdir(parents=True)
            source.write_bytes(b'native library')
            bundle = root / 'app.aab'
            with zipfile.ZipFile(bundle, 'w') as archive:
                archive.writestr('base/lib/arm64-v8a/libmain.so', b'packaged library')

            def tool(tool, *args):
                if '--only-keep-debug' in args:
                    Path(args[-1]).write_bytes(b'debug data')
                    return ''
                if '--notes' in args:
                    return 'Build ID: ' + (source_id if Path(args[-1]) == source else 'aabb')
                return '.debug_info' if dwarf else '.dynsym'

            output = root / 'native-debug-symbols.zip'
            with patch('export_android_symbols.run', side_effect=tool), contextlib.redirect_stdout(io.StringIO()):
                export(bundle, root / 'target', root / 'ndk', output)
            with zipfile.ZipFile(output) as archive:
                self.assertEqual(archive.namelist(), ['arm64-v8a/libmain.so'])
                self.assertEqual(archive.read('arm64-v8a/libmain.so'), b'debug data')
            with zipfile.ZipFile(bundle) as archive:
                self.assertEqual(archive.read('BUNDLE-METADATA/com.android.tools.build.debugsymbols/arm64-v8a/libmain.so.dbg'), b'debug data')

    def test_exports_and_embeds_matching_symbols(self):
        self.check_export()

    def test_rejects_different_build(self):
        with self.assertRaisesRegex(AssertionError, 'No matching'):
            self.check_export(source_id='ccdd')

    def test_rejects_stripped_library(self):
        with self.assertRaisesRegex(AssertionError, 'No matching'):
            self.check_export(dwarf=False)


if __name__ == '__main__':
    unittest.main()
