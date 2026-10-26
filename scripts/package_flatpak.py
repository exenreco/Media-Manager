"""Package native Linux binaries against the managed GNOME 49 runtime."""
import argparse
import hashlib
from pathlib import Path
import re
import shutil
import subprocess
import tarfile

APP_ID = 'com.tvshowmanager.app'
RUNTIME = '49'
# Small libraries outside the GNOME runtime. GTK, WebKitGTK, GLib, glibc and
# graphics drivers must come from the runtime, never from the build host.
PRIVATE_LIBRARIES = (
    'libxdo.so.', 'libayatana-appindicator3.so.', 'libayatana-indicator3.so.',
    'libdbusmenu-gtk3.so.', 'libdbusmenu-glib.so.', 'libdbus-glib-1.so.',
)


def run(*args):
    subprocess.run(args, check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--version', required=True)
    parser.add_argument('--binaries', type=Path, default=Path('target/release'))
    parser.add_argument('--out', type=Path, default=Path('dist'))
    args = parser.parse_args()
    if not re.fullmatch(r'\d+\.\d+\.\d+(?:-[A-Za-z0-9.-]+)?', args.version):
        parser.error('Invalid release version')
    args.out.mkdir(parents=True, exist_ok=True)
    build = args.out / 'flatpak-build'
    repo = args.out / 'flatpak-repo'
    if build.exists():
        parser.error('Flatpak build directory already exists; use a fresh --out directory')
    run('flatpak', 'build-init', str(build), APP_ID, 'org.gnome.Sdk', 'org.gnome.Platform', RUNTIME)
    files = build / 'files'
    (files / 'bin').mkdir(parents=True, exist_ok=True)
    (files / 'lib').mkdir(exist_ok=True)
    dependencies = {}
    source_packages = set()
    source_directory = args.out / 'linux-runtime-sources'
    source_directory.mkdir()
    for source, name in [('tv-manager-app', 'media-manager'), ('tv-manager-server', 'media-manager-server')]:
        binary = args.binaries / source
        shutil.copy2(binary, files / 'bin' / name)
        (files / 'bin' / name).chmod(0o755)
        output = subprocess.check_output(['ldd', str(binary)], text=True)
        if 'not found' in output:
            raise SystemExit('Host binary has unresolved dependencies: ' + output)
        for line in output.splitlines():
            match = re.match(r'\s*(\S+) => (/\S+)', line)
            if match:
                dependencies[match[1]] = Path(match[2])
    for name, source in dependencies.items():
        if name.startswith(PRIVATE_LIBRARIES):
            shutil.copy2(source.resolve(), files / 'lib' / name)
            owner = subprocess.check_output(['dpkg-query', '-S', str(source.resolve())], text=True).split(': ', 1)[0]
            package, version = subprocess.check_output(
                ['dpkg-query', '-W', '-f=${source:Package} ${source:Version}', owner], text=True).split()
            source_packages.add((package, version))
            copyright_file = Path('/usr/share/doc') / owner.split(':')[0] / 'copyright'
            notice = source_directory / (owner.replace(':', '-') + '-copyright')
            shutil.copy2(copyright_file, notice)
            app_notice = files / 'share/doc' / APP_ID / 'runtime-libraries' / notice.name
            app_notice.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(notice, app_notice)
    for package, version in sorted(source_packages):
        subprocess.run(['apt-get', 'source', '--download-only', '--only-source', package + '=' + version],
                       cwd=source_directory.resolve(), check=True)
    # ldd inside the selected runtime catches any missing host-specific library
    # rather than silently producing a package tied to the Ubuntu build image.
    for binary in ['media-manager', 'media-manager-server']:
        result = subprocess.run(['flatpak', 'build', '--runtime', str(build), 'ldd', '/app/bin/' + binary],
                                capture_output=True, text=True, check=True)
        if 'not found' in result.stdout + result.stderr:
            raise SystemExit('GNOME runtime compatibility check failed:\n' + result.stdout + result.stderr)
    desktop = files / 'share/applications' / (APP_ID + '.desktop')
    desktop.parent.mkdir(parents=True)
    desktop.write_text('[Desktop Entry]\nType=Application\nName=Media Manager\n'
                       'Comment=Preview and organize TV shows and movies\n'
                       'Exec=media-manager\nIcon=' + APP_ID + '\nTerminal=false\nCategories=AudioVideo;Utility;\n')
    icon = files / 'share/icons/hicolor/256x256/apps' / (APP_ID + '.png')
    icon.parent.mkdir(parents=True)
    shutil.copy2('assets/branding/icons/linux-256.png', icon)
    docs = files / 'share/doc' / APP_ID
    docs.mkdir(parents=True, exist_ok=True)
    for name in ['LICENSE', 'THIRD_PARTY_NOTICES.md', 'README.md']:
        shutil.copy2(name, docs / name)
    run('flatpak', 'build-finish', str(build), '--command=media-manager',
        '--share=network', '--share=ipc', '--socket=wayland', '--socket=fallback-x11',
        '--socket=pulseaudio', '--device=dri', '--filesystem=host',
        '--talk-name=org.freedesktop.Notifications', '--env=GTK_USE_PORTAL=1')
    run('flatpak', 'build-export', str(repo), str(build), 'stable')
    archive = args.out / f'Media-Manager-{args.version}-linux-x86_64.flatpak'
    run('flatpak', 'build-bundle', '--runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo',
        str(repo), str(archive), APP_ID, 'stable')
    sources = args.out / f'Media-Manager-{args.version}-linux-runtime-sources.tar.gz'
    with tarfile.open(sources, 'w:gz') as target:
        target.add(source_directory, arcname='runtime-library-sources')
    for output in [archive, sources]:
        with output.open('rb') as source:
            digest = hashlib.file_digest(source, 'sha256').hexdigest()
        output.with_name(output.name + '.sha256').write_text(f'{digest}  {output.name}\n')
    print(archive)


if __name__ == '__main__':
    main()
