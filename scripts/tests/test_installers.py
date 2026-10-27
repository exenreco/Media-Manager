import importlib.util
from pathlib import Path
import plistlib
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('package_installer', ROOT / 'scripts/package_installer.py')
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class InstallerTests(unittest.TestCase):
    def payload(self, folder):
        source = Path(folder) / 'payload'
        source.mkdir()
        for name in ['Media-Manager', 'media-manager-server', 'README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md']:
            (source / name).write_text('fixture')
        return source

    def test_windows_requires_bootstrapper_before_compiling(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(packaging.subprocess, 'run') as run:
            with self.assertRaises(ValueError):
                packaging.windows(Path(folder), Path(folder) / 'setup.exe', '0.1.4', 'ISCC.exe', None)
            run.assert_not_called()

    def test_windows_passes_space_containing_paths_as_single_arguments(self):
        with tempfile.TemporaryDirectory(prefix='installer spaces ') as folder:
            source = self.payload(folder)
            webview = Path(folder) / 'Microsoft Setup.exe'
            webview.touch()
            with patch.object(packaging.subprocess, 'run') as run:
                packaging.windows(source, Path(folder) / 'setup.exe', '1.0.0-beta', 'ISCC.exe', webview)
            args = run.call_args.args[0]
            self.assertIn('/DSourceDir=' + str(source.resolve()), args)
            self.assertIn('/DWebViewBootstrapper=' + str(webview.resolve()), args)
            self.assertIn('/DAppVersion=1.0.0-beta', args)
            self.assertIn('/DNumericVersion=1.0.0', args)

    def test_linux_uses_actual_dependencies_and_registers_launcher(self):
        with tempfile.TemporaryDirectory() as folder:
            source = self.payload(folder)
            seen = []
            def build(args, **kwargs):
                root = Path(args[-2])
                control = (root / 'DEBIAN/control').read_text()
                self.assertIn('Architecture: amd64', control)
                self.assertIn('Version: 1.0.0~beta\n', control)
                self.assertIn('Depends: libc6 (>= 2.39), libwebkit2gtk-4.1-0', control)
                launcher = (root / 'usr/share/applications/com.tvshowmanager.app.desktop').read_text()
                self.assertIn('Terminal=false', launcher)
                self.assertIn('Exec=media-manager\n', launcher)
                self.assertTrue((root / 'usr/bin/media-manager-server').is_file())
                seen.append(args)
            with patch.object(packaging.subprocess, 'check_output', return_value='shlibs:Depends=libc6 (>= 2.39), libwebkit2gtk-4.1-0\n'), patch.object(packaging.subprocess, 'run', side_effect=build):
                packaging.linux(source, Path(folder) / 'app.deb', '1.0.0-beta', 'x86_64')
            self.assertEqual(len(seen), 1)
            self.assertIn('--root-owner-group', seen[0])

    def test_macos_includes_server_and_disables_bundle_relocation(self):
        with tempfile.TemporaryDirectory() as folder:
            source = self.payload(folder)
            app = source / 'Media Manager.app/Contents'
            (app / 'MacOS').mkdir(parents=True)
            (app / 'Resources').mkdir()
            (app / 'MacOS/media-manager').write_text('fixture')
            commands = []
            def run(args, **kwargs):
                commands.append(args)
                if '--analyze' in args:
                    Path(args[-1]).write_bytes(plistlib.dumps([{'RootRelativeBundlePath': 'Applications/Media Manager.app', 'BundleIsRelocatable': True}]))
                if '--component-plist' in args:
                    entries = plistlib.loads(Path(args[args.index('--component-plist') + 1]).read_bytes())
                    self.assertFalse(entries[0]['BundleIsRelocatable'])
                    root = Path(args[args.index('--root') + 1])
                    self.assertTrue((root / 'Applications/Media Manager.app/Contents/MacOS/media-manager-server').is_file())
                    self.assertTrue((root / 'Applications/Media Manager.app/Contents/Resources/LICENSE').is_file())
            with patch.object(packaging.subprocess, 'run', side_effect=run):
                packaging.macos(source, Path(folder) / 'app.pkg', '0.1.4', 'aarch64')
            self.assertEqual(commands[-1][0], 'pkgbuild')


if __name__ == '__main__':
    unittest.main()
