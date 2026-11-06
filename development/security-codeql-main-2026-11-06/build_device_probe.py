#!/usr/bin/env python3
"""Bundle a DEX probe and the real ARM64 libraries from the verified unsigned APK.

Build tools only; no device or signer is accessed. CI downloads the exact native
artifact first. The bundle records artifact provenance separately from its own
hashes; a successful build is not evidence of execution on Android.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
NATIVE_COMMIT = '03190bed86aacc8e9fdaa895c5be349031bcceb5'
NATIVE_RUN = 42208222279
NATIVE_ARTIFACT = 13497272552
COMPATIBILITY = '116dda56f203c46bb31e84ab3022254160db2ee486e21f209ec1de2df81a9973'
LIBRARIES = ('libpocold.so', 'libpocol_bridge.so')


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require_arm64_16k(data):
    if (len(data) < 64 or data[:6] != b'\x7fELF\x02\x01'
            or struct.unpack_from('<HH', data, 16) != (3, 183)):
        raise ValueError('Expected a little-endian ARM64 ELF64 library')
    offset = struct.unpack_from('<Q', data, 32)[0]
    size, count = struct.unpack_from('<HH', data, 54)
    if size < 56 or not count or offset + size * count > len(data):
        raise ValueError('Invalid ELF program headers')
    loads = []
    for index in range(count):
        row = offset + index * size
        if struct.unpack_from('<I', data, row)[0] == 1:
            alignment = struct.unpack_from('<Q', data, row + 48)[0]
            if alignment < 16384 or alignment & (alignment - 1):
                raise ValueError('Native library is not aligned for 16 KiB pages')
            file_offset, virtual_address, _, file_size = struct.unpack_from('<QQQQ', data, row + 8)
            if file_offset + file_size > len(data) or file_offset % 16384 != virtual_address % 16384:
                raise ValueError('Invalid or unaligned native load segment')
            loads.append(alignment)
    if not loads:
        raise ValueError('Native library has no loadable segments')


def verified_build_info(path):
    value = json.loads(path.read_text(encoding='utf-8-sig'))
    if value.get('compatibilitySha256') != COMPATIBILITY or value.get('features') != []:
        raise ValueError('Unexpected native compatibility identity or build features')
    inventory = value.get('sourceSha256')
    if not isinstance(inventory, dict) or not inventory:
        raise ValueError('Missing native source inventory')
    for name, digest in inventory.items():
        if not name or '\\' in name or name.startswith('/') or any(p in ('', '.', '..') for p in name.split('/')):
            raise ValueError('Unsafe source inventory path')
        if sha256(ROOT / 'prototypes' / name) != digest:
            raise ValueError('Native source inventory changed: ' + name)
    # Also check bridge/Java/build inputs absent from the Rust inventory. Require
    # the full product source and standalone build tree to match the native run.
    subprocess.run(['git', 'diff', '--exit-code', '--quiet', NATIVE_COMMIT, '--',
                    'prototypes', 'deploy/standalone'], cwd=ROOT, check=True)
    return value


def build(apk, build_info, android_sdk, java_home, out):
    verified_build_info(build_info)
    out.mkdir()  # Refuse an existing output directory.
    java = java_home / 'bin'
    tools = android_sdk / 'build-tools/35.0.0'
    platform = android_sdk / 'platforms/android-35/android.jar'
    env = {**os.environ, 'JAVA_HOME': str(java_home), 'PATH': str(java) + os.pathsep + os.environ.get('PATH', '')}
    with tempfile.TemporaryDirectory(prefix='pocol-dex-') as directory:
        scratch = Path(directory); classes = scratch / 'classes'; classes.mkdir()
        native = ROOT / 'prototypes/pocol-android/app/src/main/java/org/pocol/node/Native.java'
        probe = HERE / 'android/JniDeviceProbe.java'
        subprocess.run([str(java / 'javac'), '--release', '8', '-encoding', 'UTF-8', '-d', str(classes),
                        str(native), str(probe)], check=True, env=env)
        jar = scratch / 'classes.jar'
        subprocess.run([str(java / 'jar'), 'cf', str(jar), '-C', str(classes), '.'], check=True, env=env)
        subprocess.run([str(tools / 'd8'), '--release', '--min-api', '26', '--lib', str(platform),
                        '--output', str(scratch), str(jar)], check=True, env=env)
        with zipfile.ZipFile(out / 'probe.jar', 'x', compression=zipfile.ZIP_DEFLATED) as dex:
            dex.write(scratch / 'classes.dex', 'classes.dex')
    with zipfile.ZipFile(apk) as archive:
        for name in LIBRARIES:
            path = 'lib/arm64-v8a/' + name
            if archive.namelist().count(path) != 1:
                raise ValueError('Missing or duplicate native APK library: ' + name)
            data = archive.read(path); require_arm64_16k(data)
            (out / name).write_bytes(data)
    shutil.copyfile(build_info, out / 'build-info.json')
    commit = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    dirty = subprocess.check_output(['git', 'status', '--porcelain', '--', str(HERE.relative_to(ROOT))],
                                    cwd=ROOT, text=True).strip()
    probe_sources = [native, probe, HERE / 'build_device_probe.py', HERE / 'run_device_probe.py']
    manifest = {'schema': 'pocol-jni-device-bundle-v1', 'probe_source_commit': commit,
                'probe_source_tree_dirty': bool(dirty),
                'probe_sources': {str(p.relative_to(ROOT)): sha256(p) for p in probe_sources},
                'native_source_commit': NATIVE_COMMIT, 'native_run': NATIVE_RUN,
                'native_artifact_id': NATIVE_ARTIFACT, 'compatibility_sha256': COMPATIBILITY,
                'apk_sha256': sha256(apk), 'min_android_api': 26, 'abi': 'arm64-v8a',
                'device_execution': 'not performed by build',
                'files': {p.name: sha256(p) for p in sorted(out.iterdir())}}
    (out / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n', encoding='utf-8')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--apk', type=Path, required=True)
    parser.add_argument('--build-info', type=Path, required=True)
    parser.add_argument('--android-sdk', type=Path, default=Path(os.environ.get('ANDROID_HOME', '.')))
    parser.add_argument('--java-home', type=Path, default=Path(os.environ.get('JAVA_HOME', '.')))
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    build(args.apk, args.build_info, args.android_sdk, args.java_home, args.out)
    print('Built DEX/ARM64 device bundle; no Android execution claimed.')


if __name__ == '__main__':
    main()
