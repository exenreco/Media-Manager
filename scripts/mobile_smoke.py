"""Install and launch a mobile preview, retaining explicit launch-only evidence.

Requires an already booted Android device/emulator or iOS Simulator. This does not
claim physical-device or feature acceptance. Never uninstalls or clears app data.
"""
import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import time


def run(args, *, binary=False):
    result = subprocess.run([str(a) for a in args], capture_output=True,
                            text=not binary, timeout=180)
    if result.returncode:
        detail = result.stderr if not binary else result.stderr.decode(errors='replace')
        raise RuntimeError(f'{args[0]} command failed: {detail}')
    return result.stdout


def android(args, evidence):
    adb = shutil.which('adb')
    if not adb:
        raise RuntimeError('adb is unavailable; install Android SDK Platform-Tools')
    rows = run([adb, 'devices']).splitlines()[1:]
    devices = [row.split()[0] for row in rows if len(row.split()) > 1 and row.split()[1] == 'device']
    if args.device:
        if args.device not in devices:
            raise RuntimeError('Selected Android device is absent, offline, or not authorized')
        device = args.device
    elif len(devices) == 1:
        device = devices[0]
    else:
        raise RuntimeError('Connect/authorize one Android device, or select it with --device')
    cmd = [adb, '-s', device]
    package = args.package
    if not package:
        sdk = os.environ.get('ANDROID_HOME') or os.environ.get('ANDROID_SDK_ROOT')
        candidates = sorted(Path(sdk).glob('build-tools/*/aapt*')) if sdk else []
        aapt = shutil.which('aapt') or next((str(p) for p in reversed(candidates) if p.is_file() and p.name in ('aapt','aapt.exe')), None)
        if not aapt:
            raise RuntimeError('Supply --package or install Android build-tools to inspect the APK')
        badging = run([aapt, 'dump', 'badging', args.app])
        match = re.search(r"^package: name='([^']+)'", badging, re.M)
        if not match:
            raise RuntimeError('Could not identify the APK application ID')
        package = match.group(1)
    if not re.fullmatch(r'[A-Za-z][A-Za-z0-9_.]+', package):
        raise ValueError('Invalid Android package name')
    installation = run([*cmd, 'install', '-r', args.app])
    if 'Success' not in installation:
        raise RuntimeError('APK installation did not report success')
    resolved = run([*cmd, 'shell', 'cmd', 'package', 'resolve-activity', '--brief', package])
    components = [line.strip() for line in resolved.splitlines() if line.strip().startswith(package + '/')]
    if len(components) != 1:
        raise RuntimeError('Could not resolve one app launcher activity')
    launch = run([*cmd, 'shell', 'am', 'start', '-W', '-n', components[0]])
    (evidence / 'launch.txt').write_text(launch, encoding='utf-8')
    if re.search(r'(?im)^\s*(error|exception)', launch):
        raise RuntimeError('Android reported a launch error')
    time.sleep(10)
    pid = run([*cmd, 'shell', 'pidof', package]).strip()
    if not pid:
        raise RuntimeError('App exited during the startup observation')
    log = run([*cmd, 'logcat', '-d', '--pid', pid.split()[0], '-t', '500'])
    (evidence / 'app-log.txt').write_text(log, encoding='utf-8')
    if re.search(r'FATAL EXCEPTION|Fatal signal|ANR in ' + re.escape(package), log):
        raise RuntimeError('Crash/ANR detected in app diagnostics')
    screen = run([*cmd, 'exec-out', 'screencap', '-p'], binary=True)
    (evidence / 'startup.png').write_bytes(screen)
    return {'platform':'android', 'package':package, 'process_running':True,
            'environment':'emulator' if device.startswith('emulator-') else 'connected-device'}


def ios(args, evidence):
    if sys.platform != 'darwin' or not shutil.which('xcrun'):
        raise RuntimeError('iOS Simulator validation requires macOS and Xcode')
    inventory = json.loads(run(['xcrun','simctl','list','devices','booted','--json']))
    devices = [device for group in inventory['devices'].values() for device in group if device.get('isAvailable') and device['state'] == 'Booted']
    if args.device:
        devices = [device for device in devices if device['udid'] == args.device]
    if len(devices) != 1:
        raise RuntimeError('Boot/select exactly one iOS Simulator with --device')
    device = devices[0]['udid']
    with (args.app / 'Info.plist').open('rb') as source:
        info = plistlib.load(source)
    executable = args.app / info['CFBundleExecutable']
    if not executable.resolve().is_relative_to(args.app.resolve()):
        raise RuntimeError('Invalid simulator app executable path')
    commands = run(['xcrun', 'otool', '-l', executable])
    if not re.search(r'(?m)^\s*platform\s+(?:IOSSIMULATOR|7)\s*$', commands):
        raise RuntimeError('App executable is not built for iOS Simulator')
    if info.get('CFBundleSupportedPlatforms') != ['iPhoneSimulator']:
        raise RuntimeError('Simulator platform metadata is incorrect; rerun iOS framework packaging')
    bundle = info['CFBundleIdentifier']
    run(['xcrun','simctl','install',device,args.app])
    launched = run(['xcrun','simctl','launch',device,bundle])
    (evidence / 'launch.txt').write_text(launched, encoding='utf-8')
    match = re.search(r': (\d+)\s*$', launched)
    if not match:
        raise RuntimeError('Simulator did not return an app process ID')
    time.sleep(10)
    os.kill(int(match.group(1)), 0)
    run(['xcrun','simctl','io',device,'screenshot',evidence / 'startup.png'])
    return {'platform':'ios', 'package':bundle, 'process_running':True, 'environment':'simulator'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--platform', choices=['android','ios'], required=True)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--device')
    parser.add_argument('--package', help='Optional Android application ID; otherwise inferred using aapt')
    parser.add_argument('--evidence', type=Path, default=Path('target/mobile-validation'))
    args = parser.parse_args()
    args.evidence.mkdir(parents=True, exist_ok=True)
    report = {'scope':'install-launch-smoke-only', 'physical_feature_acceptance':'not_run', 'status':'not_run'}
    try:
        args.app = args.app.resolve(strict=True)
        report.update((android if args.platform == 'android' else ios)(args, args.evidence))
        report['status'] = 'passed'
    except Exception as error:
        report.update(status='failed_or_unavailable', error=str(error))
    (args.evidence / 'result.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(report, indent=2))
    return 0 if report['status'] == 'passed' else 1


if __name__ == '__main__':
    raise SystemExit(main())
