#!/usr/bin/env python3
"""Run the offline JNI probe through adb on one connected ARM64 Android device.

Requires an explicitly supplied bundle and a fresh report path. Uses only a
random /data/local/tmp directory. Does not install an APK, access wallet data,
start consensus, create a key, change device settings or collect logcat.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import sys
import uuid
import zipfile

from build_device_probe import COMPATIBILITY, LIBRARIES, NATIVE_ARTIFACT, NATIVE_COMMIT, NATIVE_RUN, require_arm64_16k

FILES = ('probe.jar', *LIBRARIES, 'build-info.json')
REMOTE = '/data/local/tmp/pocol-jni-probe-'


def verify_bundle(bundle):
    value = json.loads((bundle / 'manifest.json').read_text(encoding='utf-8-sig'))
    if (value.get('schema') != 'pocol-jni-device-bundle-v1'
            or value.get('probe_source_tree_dirty') is not False
            or not isinstance(value.get('probe_source_commit'), str)
            or not re.fullmatch(r'[0-9a-f]{40}', value['probe_source_commit'])
            or value.get('compatibility_sha256') != COMPATIBILITY
            or value.get('native_source_commit') != NATIVE_COMMIT
            or value.get('native_run') != NATIVE_RUN or value.get('native_artifact_id') != NATIVE_ARTIFACT
            or value.get('abi') != 'arm64-v8a' or value.get('min_android_api') != 26
            or set(value.get('files', {})) != set(FILES)):
        raise ValueError('Unexpected device bundle provenance or file allowlist')
    for name in FILES:
        path = bundle / name
        if path.is_symlink() or not path.is_file():
            raise ValueError('Bundle member must be a regular file: ' + name)
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != value['files'][name]:
            raise ValueError('Bundle member hash changed: ' + name)
        if name in LIBRARIES:
            require_arm64_16k(data)
    with zipfile.ZipFile(bundle / 'probe.jar') as jar:
        if jar.namelist() != ['classes.dex'] or not jar.read('classes.dex').startswith(b'dex\n'):
            raise ValueError('Probe JAR must contain exactly one DEX program')
    info = json.loads((bundle / 'build-info.json').read_text(encoding='utf-8-sig'))
    if info.get('compatibilitySha256') != COMPATIBILITY or info.get('features') != []:
        raise ValueError('Unexpected native build identity')
    return value


def selected_device(output, wanted):
    devices = []
    for line in output.splitlines()[1:]:
        fields = line.split()
        if len(fields) >= 2 and fields[1] == 'device':
            devices.append(fields[0])
    if wanted:
        if wanted not in devices:
            raise ValueError('The selected device is not connected and authorized')
        selected = wanted
    elif len(devices) != 1:
        raise ValueError('Connect and authorize one device, or choose --serial explicitly')
    else:
        selected = devices[0]
    if selected.startswith('emulator-') or ':' in selected or selected.endswith('._tcp'):
        raise ValueError('This physical-device procedure requires a local USB device')
    return selected


def safe_remote(path):
    if not re.fullmatch(re.escape(REMOTE) + r'[0-9a-f]{32}', path):
        raise ValueError('Refusing a remote path outside the isolated probe directory')
    return path


def probe_result(output):
    values = [line[len('POCOL_JNI_RESULT '):] for line in output.splitlines()
              if line.startswith('POCOL_JNI_RESULT ')]
    if len(values) != 1:
        raise ValueError('Missing or ambiguous native probe result')
    value = json.loads(values[0])
    if value != {'schema': 'pocol-jni-probe-v1', 'passed': 9, 'failed': 0}:
        raise ValueError('One or more JNI probe cases did not pass')
    return value


def run(adb, serial, bundle, manifest, report):
    def call(arguments, timeout=30, check=True):
        result = subprocess.run([adb, '-s', serial, *arguments], capture_output=True,
                                text=True, encoding='utf-8', errors='replace', timeout=timeout)
        if check and result.returncode:
            raise RuntimeError('adb operation failed: ' + arguments[0])
        return result
    def shell(arguments, timeout=30, check=True):
        return call(['shell', shlex.join(arguments)], timeout, check)
    properties = {}
    for name in ('ro.product.manufacturer', 'ro.product.model', 'ro.build.version.release',
                 'ro.build.version.sdk', 'ro.product.cpu.abilist', 'ro.kernel.qemu', 'ro.boot.qemu'):
        properties[name] = shell(['getprop', name]).stdout.strip()
    properties['page_size'] = shell(['getconf', 'PAGE_SIZE']).stdout.strip()
    report['device'] = properties
    report['physical_device_confirmation'] = 'requires operator confirmation; emulator flags alone are insufficient'
    if ('arm64-v8a' not in properties['ro.product.cpu.abilist'].split(',')
            or not properties['ro.build.version.sdk'].isdigit()
            or int(properties['ro.build.version.sdk']) < 26
            or any(properties[name] == '1' for name in ('ro.kernel.qemu', 'ro.boot.qemu'))):
        raise ValueError('An ARM64 Android 8+ physical USB device is required')
    path = safe_remote(REMOTE + uuid.uuid4().hex)
    report['remote_directory'] = path
    try:
        shell(['mkdir', '-m', '700', path])
        for name in ('probe.jar', *LIBRARIES):
            call(['push', str((bundle / name).resolve()), path + '/' + name], timeout=120)
        hashes = shell(['sha256sum', *(path + '/' + name for name in ('probe.jar', *LIBRARIES))]).stdout
        found = {}
        for line in hashes.splitlines():
            fields = line.split()
            if len(fields) == 2:
                found[fields[1]] = fields[0]
        for name in ('probe.jar', *LIBRARIES):
            if found.get(path + '/' + name) != manifest['files'][name]:
                raise ValueError('Device transfer hash mismatch: ' + name)
        # -Djava.library.path resolves the actual JNI library; LD_LIBRARY_PATH
        # resolves its real libpocold.so dependency. No stub is ever bundled.
        arguments = ['env', 'LD_LIBRARY_PATH=' + path, 'CLASSPATH=' + path + '/probe.jar',
                     '/system/bin/app_process64', '-Xcheck:jni', '-Xmx128m',
                     '-Djava.library.path=' + path, '/system/bin',
                     'org.pocol.node.JniDeviceProbe', path]
        result = shell(arguments, timeout=120, check=False)
        report['probe_exit_code'] = result.returncode
        report['probe_stdout'] = result.stdout[-16384:]
        report['probe_stderr'] = result.stderr[-8192:]
        if result.returncode:
            raise RuntimeError('Android ART/JNI probe failed; inspect the saved probe output')
        report['probe_result'] = probe_result(result.stdout)
        report['runtime'] = 'Android app_process64 / ART with real ARM64 Rust and JNI libraries'
    finally:
        # This path is generated internally and validated again before deletion.
        try:
            cleanup = shell(['rm', '-rf', '--', safe_remote(path)], check=False)
            report['temporary_directory_removed'] = cleanup.returncode == 0
        except (OSError, RuntimeError, subprocess.TimeoutExpired):
            report['temporary_directory_removed'] = False


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    parser.add_argument('--adb', default=shutil.which('adb'))
    parser.add_argument('--serial')
    args = parser.parse_args()
    if args.out.exists() or not args.out.parent.is_dir():
        parser.error('--out must be new and its parent directory must exist')
    if not args.adb:
        parser.error('Android platform-tools/adb is required; provide --adb or put it on PATH')
    report = {'schema': 'pocol-jni-device-report-v1', 'phase': 'preflight',
              'started_at': datetime.now(timezone.utc).isoformat()}
    try:
        with args.out.open('x', encoding='utf-8') as output:
            output.write(json.dumps(report) + '\n')
    except OSError:
        parser.error('Unable to create the new report file')
    code = 0
    try:
        manifest = verify_bundle(args.bundle)
        report['bundle'] = manifest
        devices = subprocess.run([args.adb, 'devices'], capture_output=True, text=True,
                                 encoding='utf-8', errors='replace', timeout=30, check=True)
        serial = selected_device(devices.stdout, args.serial)
        report['phase'] = 'device probe'
        run(args.adb, serial, args.bundle, manifest, report)
        report['phase'] = 'complete'
    except (ValueError, OSError, KeyError, TypeError, RuntimeError, subprocess.SubprocessError, zipfile.BadZipFile) as error:
        report['phase'] = 'stopped'
        report['blocked'] = str(error) if isinstance(error, (ValueError, RuntimeError)) else type(error).__name__
        print('Stopped: ' + report['blocked'], file=sys.stderr)
        code = 1
    report['finished_at'] = datetime.now(timezone.utc).isoformat()
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    if code == 0:
        print('PASS: 9 ART/JNI probe groups. Report: ' + str(args.out))
    return code


if __name__ == '__main__':
    sys.exit(main())
