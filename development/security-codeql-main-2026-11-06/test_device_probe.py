"""Offline bundle/selection guards and a host-only rehearsal using a fake C ABI.

The host rehearsal exercises the actual C JNI bridge and Java probe, not the
Rust engine or Android ART. Only an adb/device report can establish the latter.
"""
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile
import unittest
import zipfile

import build_device_probe as build
import run_device_probe as runner


def arm64_elf(alignment=16384):
    data = bytearray(120)
    data[:6] = b'\x7fELF\x02\x01'
    struct.pack_into('<HH', data, 16, 3, 183)
    struct.pack_into('<Q', data, 32, 64)
    struct.pack_into('<HH', data, 54, 56, 1)
    struct.pack_into('<I', data, 64, 1)
    struct.pack_into('<Q', data, 64 + 48, alignment)
    return bytes(data)


class DeviceGuards(unittest.TestCase):
    def test_multiple_or_unauthorized_devices_are_not_selected_implicitly(self):
        for text in ['List of devices attached\n', 'List of devices attached\na\tunauthorized\n',
                     'List of devices attached\na\tdevice\nb\tdevice\n']:
            with self.assertRaises(ValueError): runner.selected_device(text, None)
        text = 'List of devices attached\na\tdevice\nb\tdevice\n'
        self.assertEqual(runner.selected_device(text, 'b'), 'b')

    def test_network_adb_and_known_emulators_are_not_physical_usb_evidence(self):
        for serial in ['emulator-5554', '192.0.2.1:5555', 'adb-example._adb-tls-connect._tcp']:
            with self.assertRaises(ValueError):
                runner.selected_device('List of devices attached\n' + serial + '\tdevice\n', None)

    def test_cleanup_guard_cannot_target_wallet_or_parent_directories(self):
        for path in ['/data/local/tmp', '/data/data/org.pocol.node', runner.REMOTE + '../wallet',
                     runner.REMOTE + 'a' * 31, runner.REMOTE + 'a' * 32 + '/..']:
            with self.assertRaises(ValueError): runner.safe_remote(path)
        self.assertEqual(runner.safe_remote(runner.REMOTE + 'a' * 32), runner.REMOTE + 'a' * 32)

    def test_incomplete_or_failed_java_result_is_not_passed(self):
        valid = 'POCOL_JNI_RESULT {"schema":"pocol-jni-probe-v1","passed":9,"failed":0}\n'
        self.assertEqual(runner.probe_result(valid)['passed'], 9)
        for text in ['', valid + valid, valid.replace('"failed":0', '"failed":1'),
                     valid.replace('"passed":9', '"passed":8')]:
            with self.assertRaises(ValueError): runner.probe_result(text)

    def test_elf_guard_rejects_wrong_abi_truncation_and_small_pages(self):
        build.require_arm64_16k(arm64_elf())
        for data in [b'', arm64_elf()[:100], arm64_elf(4096), arm64_elf(16385),
                     arm64_elf().replace(b'\xb7\x00', b'\x3e\x00', 1)]:
            with self.assertRaises(ValueError): build.require_arm64_16k(data)

    def make_bundle(self, path):
        with zipfile.ZipFile(path / 'probe.jar', 'x') as jar: jar.writestr('classes.dex', b'dex\n035\x00test')
        for name in build.LIBRARIES: (path / name).write_bytes(arm64_elf())
        (path / 'build-info.json').write_text(json.dumps({'features': [], 'compatibilitySha256': build.COMPATIBILITY}))
        manifest = {'schema': 'pocol-jni-device-bundle-v1', 'compatibility_sha256': build.COMPATIBILITY,
                    'probe_source_commit': 'a' * 40, 'probe_source_tree_dirty': False,
                    'native_source_commit': build.NATIVE_COMMIT, 'native_run': build.NATIVE_RUN,
                    'native_artifact_id': build.NATIVE_ARTIFACT, 'abi': 'arm64-v8a', 'min_android_api': 26,
                    'files': {name: build.sha256(path / name) for name in runner.FILES}}
        (path / 'manifest.json').write_text(json.dumps(manifest))
        return manifest

    def test_bundle_tampering_is_rejected_before_adb(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory); self.make_bundle(path)
            runner.verify_bundle(path)
            (path / 'libpocold.so').write_bytes(arm64_elf() + b'changed')
            with self.assertRaisesRegex(ValueError, 'hash changed'): runner.verify_bundle(path)

    def test_bundle_does_not_allow_unexpected_file_targets(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory); manifest = self.make_bundle(path)
            manifest['files']['../wallet'] = 'a' * 64
            (path / 'manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, 'allowlist'): runner.verify_bundle(path)

    def test_fake_artifact_provenance_is_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory); manifest = self.make_bundle(path)
            manifest['native_artifact_id'] += 1
            (path / 'manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, 'provenance'): runner.verify_bundle(path)

    def test_uncommitted_probe_build_is_not_accepted_for_device_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory); manifest = self.make_bundle(path)
            manifest['probe_source_tree_dirty'] = True
            (path / 'manifest.json').write_text(json.dumps(manifest))
            with self.assertRaisesRegex(ValueError, 'provenance'): runner.verify_bundle(path)


# Intentionally fake mobile ABI for a host rehearsal. fopen checks that the real
# JNI conversion reaches the actual Unicode filename, rather than merely echoing.
STUB = r'''
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static char *copy(const char *s) { size_t n=strlen(s)+1;char *p=malloc(n);if(p)memcpy(p,s,n);return p; }
char *pocol_mobile_identity(const char *s) { (void)s;abort(); }
char *pocol_mobile_import(const char *h,const char *p) { (void)h;(void)p;abort(); }
char *pocol_mobile_upgrade(const char *h,const char *p) { (void)h;(void)p;abort(); }
char *pocol_mobile_run(const char *h) { (void)h;abort(); }
void pocol_mobile_stop(void) { abort(); }
char *pocol_mobile_status(void) { return copy("{\"state\":\"starting\"}"); }
char *pocol_mobile_command(const char *s) {
    return copy(strlen(s)>4096 ? "{\"error\":\"Command exceeds 4 KiB\"}" : "{\"error\":\"Invalid wallet command\"}");
}
char *pocol_mobile_inspect_network(const char *path) {
    FILE *f=fopen(path,"rb");
    if(f) { fclose(f);return copy("{\"error\":\"invalid network invitation\"}"); }
    size_t n=strlen(path)+100;char *out=malloc(n);
    if(out)snprintf(out,n,"{\"error\":\"open %s: No such file or directory\"}",path);
    return out;
}
void pocol_mobile_free(char *s) { free(s); }
'''


class HostHarnessRehearsal(unittest.TestCase):
    def test_real_bridge_and_probe_rehearsal_is_explicitly_host_only(self):
        configured = os.environ.get('POCOL_TEST_JAVA_HOME')
        javac = Path(configured) / 'bin/javac' if configured else shutil.which('javac')
        cc = shutil.which('cc')
        if not javac or not Path(javac).is_file() or not cc:
            self.skipTest('Host JDK and C compiler required; set POCOL_TEST_JAVA_HOME')
        jdk = Path(javac).resolve().parents[1]
        if not (jdk / 'include/jni.h').is_file(): self.skipTest('JDK JNI headers required')
        with tempfile.TemporaryDirectory(prefix='pocol-probe-host-') as directory:
            work = Path(directory); (work / 'stub.c').write_text(STUB)
            source = build.ROOT / 'prototypes/pocol-android/app/src/main'
            subprocess.run([cc, '-shared', '-fPIC', '-Wall', '-Wextra', '-Werror',
                '-I' + str(jdk / 'include'), '-I' + str(jdk / 'include/linux'),
                str(source / 'cpp/bridge.c'), str(work / 'stub.c'), '-o', str(work / 'libpocol_bridge.so')], check=True)
            subprocess.run([str(javac), '--release', '8', '-encoding', 'UTF-8', '-d', str(work),
                str(source / 'java/org/pocol/node/Native.java'),
                str(build.HERE / 'android/JniDeviceProbe.java')], check=True)
            result = subprocess.run([str(jdk / 'bin/java'), '-Xcheck:jni', '-Xmx128m',
                '-Djava.library.path=' + str(work), '-cp', str(work),
                'org.pocol.node.JniDeviceProbe', str(work)], capture_output=True, text=True,
                encoding='utf-8', errors='replace', timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(runner.probe_result(result.stdout)['passed'], 9)
            # No wallet key or data can be created by this rehearsal's ABI.
            self.assertEqual(sorted(p.name for p in work.iterdir()), ['libpocol_bridge.so', 'org', 'stub.c'])


if __name__ == '__main__':
    unittest.main()
