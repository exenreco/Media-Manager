"""Export exact-build Rust symbols and embed them in an unsigned Play bundle."""
import argparse
import hashlib
import json
import re
import subprocess
import tempfile
import zipfile
from pathlib import Path


def run(tool, *args):
    return subprocess.check_output([str(tool), *map(str, args)], text=True)


def build_id(readelf, path):
    match = re.search(r'Build ID:\s*([0-9a-f]+)', run(readelf, '--notes', path))
    return match.group(1) if match else None


def export(bundle, target, ndk, output):
    tool_dir = next((ndk / 'toolchains/llvm/prebuilt').glob('*/bin'))
    readelf, objcopy = tool_dir / 'llvm-readelf', tool_dir / 'llvm-objcopy'
    with tempfile.TemporaryDirectory() as temporary:
        temporary = Path(temporary)
        with zipfile.ZipFile(bundle) as archive:
            assert not any(n.upper().endswith(('.RSA', '.SF')) and n.startswith('META-INF/') for n in archive.namelist()), 'Sign only after symbol export'
            packaged = temporary / 'libmain.so'
            packaged.write_bytes(archive.read('base/lib/arm64-v8a/libmain.so'))
        expected = build_id(readelf, packaged)
        assert expected, 'Packaged library must have a build ID'
        # Rust/Dioxus may rename the application shared object while packaging.
        candidates = list((target / 'aarch64-linux-android/release').glob('*.so'))
        candidates += list(target.glob('dx/**/release/**/*.so'))
        source = None
        for candidate in candidates:
            if build_id(readelf, candidate) == expected and '.debug_info' in run(readelf, '--sections', candidate):
                source = candidate
                break
        assert source, 'No matching unstripped application library with DWARF information found'
        symbols = temporary / 'libmain.so.dbg'
        run(objcopy, '--only-keep-debug', source, symbols)
        assert build_id(readelf, symbols) == expected
        assert '.debug_info' in run(readelf, '--sections', symbols)
        output.parent.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(output, 'w', zipfile.ZIP_DEFLATED) as archive:
            archive.write(symbols, 'arm64-v8a/libmain.so')
        entry = 'BUNDLE-METADATA/com.android.tools.build.debugsymbols/arm64-v8a/libmain.so.dbg'
        with zipfile.ZipFile(bundle, 'a', zipfile.ZIP_DEFLATED) as archive:
            assert entry not in archive.namelist(), 'Symbols already embedded'
            archive.write(symbols, entry)
        report = {'build_id': expected, 'symbol_source': str(source),
                  'symbols_sha256': hashlib.sha256(output.read_bytes()).hexdigest(),
                  'scope': 'Rust application library; vendor libraries require vendor-provided symbols'}
        output.with_suffix('.json').write_text(json.dumps(report, indent=2))
        print(json.dumps(report, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('bundle', type=Path)
    parser.add_argument('--target', type=Path, required=True)
    parser.add_argument('--ndk', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    export(args.bundle, args.target, args.ndk, args.output)
