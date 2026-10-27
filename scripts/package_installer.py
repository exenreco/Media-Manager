"""Build native installers from the output of package_desktop.py."""
import argparse
import hashlib
from pathlib import Path
import re
import shutil
import subprocess
import tempfile


def checksum(path):
    with path.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    path.with_name(path.name + '.sha256').write_text(f'{digest}  {path.name}\n')


def windows(source, output, version, compiler, webview):
    if not webview or not webview.is_file():
        raise ValueError('Windows installer requires the verified WebView2 bootstrapper')
    script = Path(__file__).resolve().parent / 'installers/windows.iss'
    subprocess.run([compiler, '/DAppVersion=' + version,
                    '/DSourceDir=' + str(source.resolve()),
                    '/DOutputDir=' + str(output.parent.resolve()),
                    '/DOutputName=' + output.stem,
                    '/DWebViewBootstrapper=' + str(webview.resolve()), str(script)], check=True)


def macos(source, output, version, arch):
    # The server and notices live inside the app so uninstalling the app removes
    # the entire payload. No daemon, login item or global PATH entry is installed.
    with tempfile.TemporaryDirectory(prefix='media-manager-pkg-') as temporary:
        root = Path(temporary) / 'root'
        app = root / 'Applications/Media Manager.app'
        shutil.copytree(source / 'Media Manager.app', app, symlinks=True)
        contents = app / 'Contents'
        shutil.copy2(source / 'media-manager-server', contents / 'MacOS/media-manager-server')
        for name in ['README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md']:
            shutil.copy2(source / name, contents / 'Resources' / name)
        subprocess.run(['codesign', '--force', '--sign', '-', str(contents / 'MacOS/media-manager-server')], check=True)
        subprocess.run(['codesign', '--force', '--sign', '-', str(app)], check=True)
        # Analyze explicitly to disable relocation: upgrades always target /Applications.
        components = Path(temporary) / 'components.plist'
        subprocess.run(['pkgbuild', '--analyze', '--root', str(root), str(components)], check=True)
        import plistlib
        entries = plistlib.loads(components.read_bytes())
        for entry in entries:
            entry['BundleIsRelocatable'] = False
            entry['BundleOverwriteAction'] = 'upgrade'
        components.write_bytes(plistlib.dumps(entries))
        subprocess.run(['pkgbuild', '--root', str(root), '--component-plist', str(components),
                        '--identifier', 'com.tvshowmanager.app.installer', '--version', version.split('-')[0],
                        '--ownership', 'recommended', '--install-location', '/', str(output)], check=True)


def linux(source, output, version, arch):
    deb_arch = {'x86_64': 'amd64', 'aarch64': 'arm64'}[arch]
    with tempfile.TemporaryDirectory(prefix='media-manager-deb-') as temporary:
        work = Path(temporary)
        root = work / 'root'
        binaries = root / 'usr/bin'
        binaries.mkdir(parents=True)
        for old, new in [('Media-Manager', 'media-manager'), ('media-manager-server', 'media-manager-server')]:
            shutil.copy2(source / old, binaries / new)
            (binaries / new).chmod(0o755)
        desktop = root / 'usr/share/applications/com.tvshowmanager.app.desktop'
        desktop.parent.mkdir(parents=True)
        desktop.write_text('[Desktop Entry]\nType=Application\nName=Media Manager\n'
                           'Comment=Preview and organize TV shows and movies\n'
                           'Exec=media-manager\nIcon=com.tvshowmanager.app\nTerminal=false\n'
                           'Categories=AudioVideo;Utility;\n', encoding='utf-8')
        icon = root / 'usr/share/icons/hicolor/256x256/apps/com.tvshowmanager.app.png'
        icon.parent.mkdir(parents=True)
        shutil.copy2('assets/branding/icons/linux-256.png', icon)
        docs = root / 'usr/share/doc/media-manager'
        docs.mkdir(parents=True)
        for name in ['README.md', 'LICENSE', 'THIRD_PARTY_NOTICES.md']:
            shutil.copy2(source / name, docs / name)
        # Resolve dependencies from both ELF binaries on the native build host,
        # including its glibc floor. Do not hard-code a misleading distro range.
        debian = work / 'debian'
        debian.mkdir()
        (debian / 'control').write_text('Source: media-manager\nSection: video\nPriority: optional\n'
                                      'Maintainer: Media Manager <exenreco@users.noreply.github.com>\n\n'
                                      'Package: media-manager\nArchitecture: any\nDescription: Media library manager\n')
        result = subprocess.check_output(['dpkg-shlibdeps', '-O',
                    '-e' + str((binaries / 'media-manager').resolve()),
                    '-e' + str((binaries / 'media-manager-server').resolve())], cwd=work, text=True)
        depends = next(line.split('=', 1)[1] for line in result.splitlines() if line.startswith('shlibs:Depends='))
        if not depends.strip():
            raise ValueError('No native runtime dependencies were discovered')
        control = root / 'DEBIAN/control'
        control.parent.mkdir()
        size = sum(p.stat().st_size for p in root.rglob('*') if p.is_file()) // 1024 + 1
        control.write_text(f'Package: media-manager\nVersion: {version}\nArchitecture: {deb_arch}\n'
                           'Section: video\nPriority: optional\n'
                           'Maintainer: Media Manager <exenreco@users.noreply.github.com>\n'
                           'Homepage: https://github.com/exenreco/Media-Manager\n'
                           f'Installed-Size: {size}\nDepends: {depends}\n'
                           'Description: Preview and organize TV shows and movies\n'
                           ' Review proposed names and artwork before changing your library.\n', encoding='utf-8')
        subprocess.run(['dpkg-deb', '--root-owner-group', '--build', str(root), str(output.resolve())], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=['windows', 'macos', 'linux'], required=True)
    parser.add_argument('--arch', choices=['x86_64', 'aarch64'], required=True)
    parser.add_argument('--version', required=True)
    parser.add_argument('--out', type=Path, default=Path('dist'))
    parser.add_argument('--iscc', default='ISCC.exe')
    parser.add_argument('--webview-bootstrapper', type=Path)
    args = parser.parse_args()
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', args.version):
        parser.error('Invalid release version')
    if args.platform == 'windows' and args.arch != 'x86_64':
        parser.error('Windows installer currently packages the x64 build only')
    stem = f'Media-Manager-{args.version}-{args.platform}-{args.arch}'
    source = args.out / stem
    if not source.is_dir():
        parser.error('Run package_desktop.py first; missing ' + str(source))
    extension = {'windows': '-setup.exe', 'macos': '.pkg', 'linux': '.deb'}[args.platform]
    output = args.out / (stem + extension)
    if args.platform == 'windows':
        windows(source, output, args.version, args.iscc, args.webview_bootstrapper)
    elif args.platform == 'macos':
        macos(source, output, args.version, args.arch)
    else:
        linux(source, output, args.version, args.arch)
    # Windows checksum is generated again by CI after optional publisher signing.
    checksum(output)
    print(output)


if __name__ == '__main__':
    main()
