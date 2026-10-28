"""Package Google SDK resources/dynamic frameworks, then re-sign the iOS app."""
import argparse
from pathlib import Path
import plistlib
import re
import shutil
import stat
import struct
import subprocess
import sys


def select_slice(container, sdk, arch):
    with (container / 'Info.plist').open('rb') as source:
        libraries = plistlib.load(source)['AvailableLibraries']
    candidates = [item for item in libraries
                  if item['SupportedPlatform'] == 'ios'
                  and item.get('SupportedPlatformVariant', '') == ('simulator' if sdk == 'iphonesimulator' else '')
                  and arch in item['SupportedArchitectures']]
    if len(candidates) != 1:
        raise ValueError(f'Expected one {sdk}/{arch} slice in {container}')
    item = candidates[0]
    result = (container / item['LibraryIdentifier'] / item['LibraryPath']).resolve()
    if not result.is_relative_to(container.resolve()) or not result.is_dir():
        raise ValueError('Invalid framework slice path')
    return result


def run(*args):
    result = subprocess.run(args, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"{' '.join(args)} failed:\n{result.stdout}{result.stderr}")
    return result.stdout


def make_writable(root):
    # SwiftPM artifacts may be read-only. Change only the copied app framework,
    # so codesign can replace its vendor signature and write its resource seal.
    for item in [root, *root.rglob('*')]:
        if item.is_symlink():
            continue
        mode = item.stat().st_mode | stat.S_IRUSR | stat.S_IWUSR
        if item.is_dir():
            mode |= stat.S_IXUSR
        item.chmod(mode)


def is_static_archive(binary):
    with binary.open('rb') as source:
        header = source.read(8)
        if header == b'!<arch>\n':
            return True
        layouts = {b'\xca\xfe\xba\xbe': ('>', False), b'\xbe\xba\xfe\xca': ('<', False),
                   b'\xca\xfe\xba\xbf': ('>', True), b'\xbf\xba\xfe\xca': ('<', True)}
        if header[:4] not in layouts:
            return False
        endian, wide = layouts[header[:4]]
        count = struct.unpack(endian + 'I', header[4:])[0]
        if not 1 <= count <= 32:
            raise ValueError('Invalid universal framework architecture count')
        size = 32 if wide else 20
        offsets = []
        for _ in range(count):
            row = source.read(size)
            if len(row) != size:
                raise ValueError('Truncated universal framework header')
            offsets.append(struct.unpack(endian + ('IIQQII' if wide else 'IIIII'), row)[2])
        archives = []
        for offset in offsets:
            source.seek(offset)
            archives.append(source.read(8) == b'!<arch>\n')
        if any(archives) and not all(archives):
            raise ValueError('Mixed static/dynamic framework slices')
        return all(archives)


def copy_static_resources(source, app, name):
    # SwiftPM already links these archives into DioxusSwiftPlugins. An archive
    # inside Frameworks is not a loadable Mach-O dylib and breaks installation.
    target = app / 'Frameworks' / (name + '.framework')
    if not target.resolve().is_relative_to(app.resolve()):
        raise ValueError('Framework destination escapes app bundle')
    if target.exists():
        shutil.rmtree(target)
    resources = app / (name + 'Resources.bundle')
    if not resources.resolve().is_relative_to(app.resolve()):
        raise ValueError('Resource destination escapes app bundle')
    resources.mkdir(exist_ok=True)
    for item in source.iterdir():
        if item.name in (name, 'Info.plist', 'Headers', 'PrivateHeaders', 'Modules', '_CodeSignature'):
            continue
        destination = resources / item.name
        if item.is_dir():
            shutil.copytree(item, destination, dirs_exist_ok=True)
        else:
            shutil.copy2(item, destination)
    with (resources / 'Info.plist').open('wb') as output:
        plistlib.dump({'CFBundleIdentifier': 'com.mediamanager.resources.' + name,
                      'CFBundleName': name + 'Resources', 'CFBundlePackageType': 'BNDL',
                      'CFBundleVersion': '1'}, output)
    return resources


def platform_info(info, sdk, version):
    if sdk not in ('iphoneos', 'iphonesimulator'):
        raise ValueError('Unsupported iOS SDK')
    result = dict(info)
    # Cargo prerelease labels are valid app versions, but Apple bundle fields
    # require numeric versions. The release asset keeps the beta label.
    for key in ('CFBundleVersion', 'CFBundleShortVersionString'):
        value = result.get(key)
        if isinstance(value, str) and re.fullmatch(r'\d+\.\d+\.\d+-[A-Za-z0-9.-]+', value):
            result[key] = value.split('-', 1)[0]
    result['CFBundleSupportedPlatforms'] = ['iPhoneSimulator' if sdk == 'iphonesimulator' else 'iPhoneOS']
    result['DTPlatformName'] = sdk
    result['DTPlatformVersion'] = version
    result['DTSDKName'] = sdk + version
    result.setdefault('UIDeviceFamily', [1, 2])
    return result


def validate_platform(commands, sdk):
    expected = r'(?:IOSSIMULATOR|7)' if sdk == 'iphonesimulator' else r'(?:IOS|2)'
    if not re.search(r'(?m)^\s*platform\s+' + expected + r'\s*$', commands):
        raise ValueError('Executable platform does not match the requested iOS SDK')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--artifacts', type=Path, required=True, help='SwiftPM .build/artifacts directory')
    parser.add_argument('--sdk', choices=['iphoneos', 'iphonesimulator'], required=True)
    parser.add_argument('--arch', choices=['arm64', 'x86_64'], default='arm64')
    parser.add_argument('--identity', required=True, help='Apple signing identity; use - for simulator ad-hoc signing')
    args = parser.parse_args()
    if sys.platform != 'darwin':
        parser.error('Embedding/signing requires macOS and Xcode')
    if args.sdk == 'iphoneos' and args.identity == '-':
        parser.error('Physical-device bundles require an Apple signing identity')
    app = args.app.resolve(strict=True)
    if app.suffix != '.app' or not (app / 'Info.plist').is_file():
        parser.error('--app must name an existing iOS .app bundle')
    frameworks = app / 'Frameworks'
    frameworks.mkdir(exist_ok=True)
    if not frameworks.resolve().is_relative_to(app):
        parser.error('Frameworks must remain inside the app bundle')
    plugin = frameworks / 'DioxusSwiftPlugins.framework' / 'DioxusSwiftPlugins'
    if not plugin.is_file():
        parser.error('Dioxus did not build the Swift plugin; rebuild with Dioxus CLI 0.7.10')
    for name in ['GoogleMobileAds', 'UserMessagingPlatform', 'VLCKit']:
        matches = list(args.artifacts.resolve(strict=True).rglob(name + '.xcframework'))
        if len(matches) != 1:
            parser.error(f'Expected one {name}.xcframework under --artifacts; found {len(matches)}')
        source = select_slice(matches[0], args.sdk, args.arch)
        target = frameworks / (name + '.framework')
        if not target.resolve().is_relative_to(app):
            parser.error('Framework destination escapes app bundle')
        run('xcrun', 'lipo', str(source / name), '-verify_arch', args.arch)
        if is_static_archive(source / name):
            copy_static_resources(source, app, name)
            print(f'Preserved {name} resources; static SDK code is linked into the Swift plugin')
        else:
            shutil.copytree(source, target, dirs_exist_ok=True)
            make_writable(target)
    # SDK frameworks are loaded from the packaged bundle, not SwiftPM build paths.
    load_commands = run('xcrun', 'otool', '-l', str(plugin))
    if 'path @loader_path/.. ' not in load_commands:
        run('xcrun', 'install_name_tool', '-add_rpath', '@loader_path/..', str(plugin))
    with (app / 'Info.plist').open('rb') as source:
        info = plistlib.load(source)
    with Path(__file__).with_name('Info.plist').open('rb') as source:
        info.update(plistlib.load(source))
    executable = (app / info['CFBundleExecutable']).resolve(strict=True)
    if not executable.is_relative_to(app):
        parser.error('App executable must remain inside the bundle')
    validate_platform(run('xcrun', 'otool', '-l', str(executable)), args.sdk)
    version = run('xcrun', '--sdk', args.sdk, '--show-sdk-version').strip()
    info = platform_info(info, args.sdk, version)
    with (app / 'Info.plist').open('wb') as target:
        plistlib.dump(info, target)
    run(sys.executable, str(Path(__file__).resolve().parents[3] / 'scripts/install_ios_icon.py'),
        '--app', str(app), '--sdk', args.sdk)
    for framework in sorted(frameworks.glob('*.framework')):
        run('codesign', '--force', '--sign', args.identity, str(framework))
    run('codesign', '--force', '--sign', args.identity,
        '--preserve-metadata=identifier,entitlements', str(app))
    run('codesign', '--verify', '--deep', '--strict', str(app))
    print(f'Embedded Google SDKs and verified signatures: {app}')


if __name__ == '__main__':
    main()
