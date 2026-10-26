import hashlib
import importlib.util
from pathlib import Path
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('package_desktop', ROOT / 'scripts/package_desktop.py')
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class WindowsDistributionTests(unittest.TestCase):
    def package(self, temporary, signed=False):
        folder = Path(temporary)
        binaries = folder / 'bin'
        binaries.mkdir()
        executable = bytearray(256)
        executable[:2] = b'MZ'
        struct.pack_into('<I', executable, 60, 64)
        executable[64:68] = b'PE\0\0'
        struct.pack_into('<H', executable, 156, 2)
        for name in ['tv-manager-app.exe', 'tv-manager-server.exe']:
            (binaries / name).write_bytes(executable)
        arguments = ['package', '--platform', 'windows', '--arch', 'x86_64',
                     '--version', '0.1.3', '--binaries', str(binaries), '--out', str(folder / 'dist')]
        if signed:
            arguments.append('--require-windows-signature')
        return folder / 'dist', arguments

    def test_unsigned_preview_is_labeled_and_checksum_covers_final_archive(self):
        with tempfile.TemporaryDirectory() as temporary:
            out, arguments = self.package(temporary)
            with patch.object(sys, 'argv', arguments):
                packaging.main()
            archive = next(out.glob('*.zip'))
            with zipfile.ZipFile(archive) as content:
                status = next(n for n in content.namelist() if n.endswith('/WINDOWS-SIGNING.txt'))
                self.assertIn(b'Unsigned preview', content.read(status))
            checksum = archive.with_name(archive.name + '.sha256').read_text().split()[0]
            self.assertEqual(hashlib.sha256(archive.read_bytes()).hexdigest(), checksum)

    def test_failed_signature_verification_prevents_archive_publication(self):
        with tempfile.TemporaryDirectory() as temporary:
            out, arguments = self.package(temporary, signed=True)
            with patch.object(sys, 'argv', arguments), patch.object(packaging.subprocess, 'run',
                    side_effect=subprocess.CalledProcessError(1, 'signature verification')):
                with self.assertRaises(subprocess.CalledProcessError):
                    packaging.main()
            self.assertEqual(list(out.glob('*.zip')), [])
            self.assertEqual(list(out.glob('*.sha256')), [])


if __name__ == '__main__':
    unittest.main()
