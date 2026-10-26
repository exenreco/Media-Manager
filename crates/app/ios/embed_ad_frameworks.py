"""Finish a Dioxus iOS bundle with Google's binary SDKs, then re-sign it on macOS."""
import argparse
from pathlib import Path
import plistlib
import shutil
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
    for name in ['GoogleMobileAds', 'UserMessagingPlatform']:
        matches = list(args.artifacts.resolve(strict=True).rglob(name + '.xcframework'))
        if len(matches) != 1:
            parser.error(f'Expected one {name}.xcframework under --artifacts; found {len(matches)}')
        source = select_slice(matches[0], args.sdk, args.arch)
        target = frameworks / (name + '.framework')
        if not target.resolve().is_relative_to(app):
            parser.error('Framework destination escapes app bundle')
        shutil.copytree(source, target, dirs_exist_ok=True)
        run('xcrun', 'lipo', str(target / name), '-verify_arch', args.arch)
    # SDK frameworks are loaded from the packaged bundle, not SwiftPM build paths.
    load_commands = run('xcrun', 'otool', '-l', str(plugin))
    if 'path @loader_path/.. ' not in load_commands:
        run('xcrun', 'install_name_tool', '-add_rpath', '@loader_path/..', str(plugin))
    with (app / 'Info.plist').open('rb') as source:
        info = plistlib.load(source)
    with Path(__file__).with_name('Info.plist').open('rb') as source:
        info.update(plistlib.load(source))
    with (app / 'Info.plist').open('wb') as target:
        plistlib.dump(info, target)
    for framework in sorted(frameworks.glob('*.framework')):
        run('codesign', '--force', '--sign', args.identity, str(framework))
    run('codesign', '--force', '--sign', args.identity,
        '--preserve-metadata=identifier,entitlements', str(app))
    run('codesign', '--verify', '--deep', '--strict', str(app))
    print(f'Embedded Google SDKs and verified signatures: {app}')


if __name__ == '__main__':
    main()
