"""Synthetic fixtures only: never opens a user's wallet or network.

Run after `npm run build` with: python3 -m unittest tests/test_linux_export.py
Requires Python cryptography and Node.js for the actual extension importer check.
"""
import base64
import importlib.util
import json
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("linux_export", ROOT / "tools/export-studio-wallet.py")
exporter = importlib.util.module_from_spec(spec)
spec.loader.exec_module(exporter)

# Public disposable fixture. It must never be used for a real wallet.
SYNTHETIC_SECRET = bytes.fromhex("20" * 32)
SYNTHETIC_PASSWORD = "synthetic-test-only-كلمة-123"


@unittest.skipUnless(sys.platform.startswith("linux"), "Linux file guards")
class LinuxExportTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="pocol-export-synthetic-")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.key = self.root / "synthetic.pocol-key"
        self.key.write_bytes(SYNTHETIC_SECRET)
        self.key.chmod(0o600)

    def test_actual_extension_importer_preserves_key_and_address(self):
        secret = exporter.read_native_key(self.key)
        box = exporter.encrypt_export(secret, SYNTHETIC_PASSWORD)
        output = self.root / "encrypted.json"
        exporter.write_export(output, box)
        self.assertEqual(stat.S_IMODE(output.stat().st_mode), 0o600)
        self.assertEqual(self.key.read_bytes(), SYNTHETIC_SECRET)
        self.assertEqual(stat.S_IMODE(self.key.stat().st_mode), 0o600)
        self.assertEqual(box["schema"], "pocol-studio-key-v1")
        self.assertEqual(len(base64.b64decode(box["ciphertext"])), 48)
        self.assertNotIn(SYNTHETIC_SECRET.hex(), output.read_text())
        program = r"""
import assert from 'node:assert/strict';
import fs from 'node:fs';
import { webcrypto } from 'node:crypto';
globalThis.crypto ??= webcrypto;
const { importKeyfile } = await import('./dist/lib/studio-keyfile.js');
const { addressOfSecret } = await import('./dist/lib/crypto.js');
const { toHex } = await import('./dist/lib/hex.js');
const input = JSON.parse(fs.readFileSync(0, 'utf8'));
const encoded = new TextEncoder().encode(JSON.stringify(input.box));
const original = Uint8Array.from({ length: 32 }, () => 0x20);
const restored = await importKeyfile(encoded, input.password);
assert.deepEqual(restored, original);
assert.equal(toHex(addressOfSecret(restored)), toHex(addressOfSecret(original)));
await assert.rejects(importKeyfile(encoded, 'wrong-password-only'));
for (const field of ['ciphertext', 'mac', 'salt', 'iv']) {
  const corrupted = { ...input.box };
  const bytes = Buffer.from(corrupted[field], 'base64'); bytes[0] ^= 1;
  corrupted[field] = bytes.toString('base64');
  await assert.rejects(importKeyfile(new TextEncoder().encode(JSON.stringify(corrupted)), input.password));
}
restored.fill(0); original.fill(0);
console.log('Synthetic key identity/address and authentication checks passed.');
"""
        result = subprocess.run(["node", "--input-type=module", "-e", program], cwd=ROOT,
                                input=json.dumps({"box": box, "password": SYNTHETIC_PASSWORD}),
                                text=True, capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("authentication checks passed", result.stdout)
        self.assertNotIn(SYNTHETIC_SECRET.hex(), result.stdout)
        secret[:] = b"\0" * len(secret)

    def test_source_guards_and_read_only_mode(self):
        self.key.chmod(0o400)
        self.assertEqual(exporter.read_native_key(self.key), SYNTHETIC_SECRET)
        self.key.chmod(0o640)
        with self.assertRaisesRegex(exporter.ExportError, "0600 or 0400"):
            exporter.read_native_key(self.key)
        self.key.chmod(0o600)
        link = self.root / "symbolic-key"
        link.symlink_to(self.key)
        with self.assertRaisesRegex(exporter.ExportError, "not be a symbolic link"):
            exporter.read_native_key(link)
        hardlink = self.root / "hardlink-key"
        os.link(self.key, hardlink)
        with self.assertRaisesRegex(exporter.ExportError, "one hard link"):
            exporter.read_native_key(self.key)
        hardlink.unlink()
        with patch.object(exporter.os, "geteuid", return_value=os.geteuid() + 1):
            with self.assertRaisesRegex(exporter.ExportError, "owner"):
                exporter.read_native_key(self.key)
        foreign = SimpleNamespace(st_mode=stat.S_IFREG | 0o600, st_uid=65534, st_nlink=1, st_size=32)
        with patch.object(exporter.os, "geteuid", return_value=0):
            with self.assertRaisesRegex(exporter.ExportError, "Root does not bypass"):
                exporter.check_key_stat(foreign)
        with self.assertRaisesRegex(exporter.ExportError, "regular"):
            exporter.read_native_key(self.root)
        fifo = self.root / "fifo-key"
        os.mkfifo(fifo, 0o600)
        with self.assertRaisesRegex(exporter.ExportError, "regular"):
            exporter.read_native_key(fifo)

    def test_invalid_size_and_secp256k1_scalars(self):
        for invalid in (b"x" * 31, b"x" * 33, b"\0" * 32, exporter.SECP256K1_ORDER.to_bytes(32, "big")):
            self.key.write_bytes(invalid)
            with self.assertRaises(exporter.ExportError):
                exporter.read_native_key(self.key)
        self.key.write_bytes(b"POCOL-DPAPI-USER-V1\0" + b"x" * 32)
        with self.assertRaisesRegex(exporter.ExportError, "Windows DPAPI"):
            exporter.read_native_key(self.key)

    def test_output_never_overwrites_and_fsyncs(self):
        output = self.root / "encrypted.json"
        with patch.object(exporter.os, "fsync", wraps=os.fsync) as fsync:
            exporter.write_export(output, {"ciphertext": "synthetic encrypted placeholder"})
            fsync.assert_called_once()
        original = output.read_bytes()
        with self.assertRaisesRegex(exporter.ExportError, "never overwritten"):
            exporter.write_export(output, {"changed": True})
        self.assertEqual(output.read_bytes(), original)
        link = self.root / "output-link.json"
        link.symlink_to(self.key)
        with self.assertRaisesRegex(exporter.ExportError, "never overwritten"):
            exporter.write_export(link, {"changed": True})
        self.assertEqual(self.key.read_bytes(), SYNTHETIC_SECRET)

    def test_password_must_be_hidden_interactive_and_confirmed(self):
        with patch.object(exporter.sys.stdin, "isatty", return_value=False):
            with self.assertRaisesRegex(exporter.ExportError, "interactively"):
                exporter.prompt_password()
        for values, expected in [(["short", "short"], "10 characters"),
                                 (["long-test-password", "different-test-password"], "do not match")]:
            with patch.object(exporter.sys.stdin, "isatty", return_value=True), patch.object(exporter.getpass, "getpass", side_effect=values):
                with self.assertRaisesRegex(exporter.ExportError, expected):
                    exporter.prompt_password()
        with patch.object(exporter.sys.stdin, "isatty", return_value=True), patch.object(exporter.getpass, "getpass", side_effect=[SYNTHETIC_PASSWORD] * 2):
            self.assertEqual(exporter.prompt_password(), SYNTHETIC_PASSWORD)
        with patch.object(exporter.sys.stdin, "isatty", return_value=True), patch.object(exporter.getpass, "getpass", side_effect=exporter.getpass.GetPassWarning()):
            with self.assertRaisesRegex(exporter.ExportError, "hidden password"):
                exporter.prompt_password()
        process = subprocess.run([sys.executable, str(ROOT / "tools/export-studio-wallet.py"),
                                  "--key-path", str(self.key), "--output", str(self.root / "no-pipe.json")],
                                 input=SYNTHETIC_PASSWORD + "\n", text=True, capture_output=True, timeout=5)
        self.assertNotEqual(process.returncode, 0)
        self.assertFalse((self.root / "no-pipe.json").exists())
        self.assertNotIn(SYNTHETIC_PASSWORD, process.stdout + process.stderr)


if __name__ == "__main__":
    unittest.main()
