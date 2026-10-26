"""Package a native Cargo release build; no cross-compilation or private signing keys."""
import argparse
import hashlib
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import tarfile
import zipfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=['windows', 'macos', 'linux'], required=True)
    parser.add_argument('--arch', choices=['x86_64', 'aarch64'], required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--binaries', type=Path, default=Path('target/release'))
    parser.add_argument('--out', type=Path, default=Path('dist'))
    parser.add_argument('--require-windows-signature', action='store_true')
    args = parser.parse_args()
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', args.version):
        parser.error('Invalid release version')
    name = f'Media-Manager-{args.version}-{args.platform}-{args.arch}'
    root = args.out / name
    root.mkdir(parents=True, exist_ok=True)
    suffix = '.exe' if args.platform == 'windows' else ''
    app_source = args.binaries / ('tv-manager-app' + suffix)
    server_source = args.binaries / ('tv-manager-server' + suffix)
    if args.require_windows_signature and args.platform != 'windows':
        parser.error('--require-windows-signature is only valid for Windows')
    if args.platform == 'windows':
        if args.require_windows_signature:
            subprocess.run(['pwsh', '-NoProfile', '-File', 'scripts/verify_windows_signatures.ps1', '-Binaries', str(args.binaries)], check=True)
        (root / 'WINDOWS-SIGNING.txt').write_text(
            'Both executables have verified, timestamped Authenticode signatures.\n' if args.require_windows_signature else
            'Unsigned preview: these executables do not have publisher-verified Authenticode signatures.\n')
        binary = app_source.read_bytes()
        pe_offset = struct.unpack_from('<I', binary, 0x3c)[0]
        subsystem = struct.unpack_from('<H', binary, pe_offset + 92)[0]
        if subsystem != 2:
            raise SystemExit('Refusing to package a console app: rebuild with windows_subsystem = "windows"')
    if args.platform == 'macos':
        app = root / 'Media Manager.app'
        contents = app / 'Contents'
        (contents / 'MacOS').mkdir(parents=True, exist_ok=True)
        (contents / 'Resources').mkdir(parents=True, exist_ok=True)
        shutil.copy2('assets/branding/icons/app.icns', contents / 'Resources' / 'app.icns')
        shutil.copy2(app_source, contents / 'MacOS' / 'media-manager')
        (contents / 'Info.plist').write_bytes(plistlib.dumps({
            'CFBundleName': 'Media Manager', 'CFBundleDisplayName': 'Media Manager',
            'CFBundleIdentifier': 'com.tvshowmanager.app', 'CFBundleExecutable': 'media-manager',
            'CFBundlePackageType': 'APPL', 'CFBundleVersion': args.version,
            'CFBundleIconFile': 'app.icns',
            'CFBundleShortVersionString': args.version.split('-')[0],
            'LSMinimumSystemVersion': '13.0', 'NSHighResolutionCapable': True,
        }))
        subprocess.run(['codesign', '--force', '--sign', '-', str(app)], check=True)
    else:
        shutil.copy2(app_source, root / ('Media-Manager' + suffix))
    shutil.copy2(server_source, root / ('media-manager-server' + suffix))
    for document in ['README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md']:
        shutil.copy2(document, root / document)
    if args.platform == 'linux':
        archive = args.out / (name + '.tar.gz')
        with tarfile.open(archive, 'w:gz') as target:
            target.add(root, arcname=name)
    else:
        archive = args.out / (name + '.zip')
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as target:
            for item in sorted(root.rglob('*')):
                target.write(item, item.relative_to(args.out))
    with archive.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    archive.with_name(archive.name + '.sha256').write_text(f'{digest}  {archive.name}\n')
    print(archive)


if __name__ == '__main__':
    main()
