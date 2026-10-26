"""Compile the native iOS icon catalog before signing the app bundle."""
from pathlib import Path
import plistlib
import subprocess
import tempfile


def install_icon(app, sdk):
    catalog = Path(__file__).resolve().parents[1] / 'assets/branding/icons/AppIcons.xcassets'
    with tempfile.TemporaryDirectory() as temporary:
        partial = Path(temporary) / 'icon-info.plist'
        subprocess.run([
            'xcrun', 'actool', str(catalog), '--compile', str(app),
            '--platform', sdk, '--minimum-deployment-target', '15.0',
            '--target-device', 'iphone', '--target-device', 'ipad',
            '--app-icon', 'AppIcon', '--output-partial-info-plist', str(partial),
        ], check=True)
        info_path = app / 'Info.plist'
        info = plistlib.loads(info_path.read_bytes())
        info.update(plistlib.loads(partial.read_bytes()))
        info_path.write_bytes(plistlib.dumps(info))


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--sdk', choices=['iphoneos', 'iphonesimulator'], required=True)
    args = parser.parse_args()
    install_icon(args.app.resolve(), args.sdk)
