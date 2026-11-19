#!/usr/bin/env python3
"""encrypt-server-key.sh (gate K1): its refusals, its rollback and its success path, run against fakes.

No real systemd is needed (or used): the script runs as a copy whose fixed paths point into a temporary directory, with
fake `id`, `systemctl`, `journalctl`, `runuser`, `systemd-creds`, `shred` and `pocol` first on a PATH that holds only
the other tools it needs. The fake `systemctl start` runs the unit's effective ExecStart (the drop-in's when there is
one) with the decrypted credential in a private $CREDENTIALS_DIRECTORY, as systemd's LoadCredentialEncrypted= does,
and the fake `pocol` reports an address computed from the key source it actually read, recording which one it was.
The throwaway "key" is random bytes made here; nothing real is read or written. Linux/Unix only (bash).
"""
import hashlib
import os
from pathlib import Path
import shutil
import signal
import stat
import subprocess
import sys
import tempfile
import time
import unittest

HERE = Path(__file__).resolve().parent
SCRIPT = HERE / 'encrypt-server-key.sh'
REAL_TOOLS = ['bash', 'sh', 'sed', 'grep', 'head', 'cut', 'cat', 'seq', 'sleep', 'cmp', 'chmod', 'mv', 'mkdir', 'rm',
              'rmdir', 'find', 'tail', 'sync', 'sha256sum', 'cp', 'tr', 'ls', 'dirname', 'basename', 'env', 'printf', 'touch',
              'test']

FAKES = {
    'id': r'''#!/bin/bash
[ "$1" = -u ] && { echo "${FAKE_UID:-0}"; exit 0; }
exit 1
''',
    'runuser': r'''#!/bin/bash
# runuser -u USER -- COMMAND...
[ "$1" = -u ] && [ "$3" = -- ] || exit 2
echo "$2 ${*:4}" >> "$FAKE/runuser.log"
shift 3; exec "$@"
''',
    # A credential is the plaintext behind a fixed header; --name must match on decrypt, as systemd-creds checks.
    'systemd-creds': r'''#!/bin/bash
log="$FAKE/systemd-creds.log"; echo "$*" >> "$log"
case "$1" in
  --version) echo "systemd ${FAKE_SYSTEMD_VERSION:-255} (255.4-1)"; echo '+PAM +TPM2'; exit 0 ;;
  has-tpm2) [ -n "${FAKE_TPM2:-}" ]; exit $? ;;
  encrypt)
    name=''; for a in "$@"; do case "$a" in --name=*) name="${a#--name=}" ;; esac; done
    args=(); for a in "${@:2}"; do case "$a" in --*) ;; *) args+=("$a") ;; esac; done
    { printf 'FAKECRED %s\n' "$name"; cat "${args[0]}"; } > "${args[1]}"; chmod 0644 "${args[1]}" ;;
  decrypt)
    name=''; for a in "$@"; do case "$a" in --name=*) name="${a#--name=}" ;; esac; done
    args=(); for a in "${@:2}"; do case "$a" in --*) ;; *) args+=("$a") ;; esac; done
    [ "$(head -n 1 "${args[0]}")" = "FAKECRED $name" ] || { echo 'name mismatch' >&2; exit 1; }
    out="${args[1]:--}"
    if [ -n "${FAKE_CORRUPT_DECRYPT:-}" ]; then printf 'not the key'; exit 0; fi
    if [ "$out" = - ]; then tail -c +$(( ${#name} + 11 )) "${args[0]}"; else tail -c +$(( ${#name} + 11 )) "${args[0]}" > "$out"; fi ;;
  *) exit 2 ;;
esac
''',
    'systemctl': r'''#!/bin/bash
echo "$*" >> "$FAKE/systemctl.log"
state="$FAKE/state"; unit="$FAKE_UNIT"; dropins="$FAKE_UNIT.d"
quiet=0; [ "$2" = -q ] && { quiet=1; set -- "$1" "${@:3}"; }
case "$1" in
  is-active) [ "$(cat "$state" 2>/dev/null)" = active ]; exit $? ;;
  stop) echo inactive > "$state" ;;
  daemon-reload) ;;
  reset-failed) rm -f "$FAKE/start-limit-hit" ;;
  show) cat "$FAKE/invocation" 2>/dev/null || true ;;
  start)
    # systemd refuses a unit that hit StartLimitBurst= until reset-failed (or StartLimitIntervalSec= passes).
    [ -f "$FAKE/start-limit-hit" ] && { echo 'Job failed: start-limit-hit' >&2; exit 1; }
    exec_start="$(grep '^ExecStart=.' "$unit" | tail -n 1)"; cred=''
    for f in "$dropins"/*.conf; do
      [ -f "$f" ] || continue
      line="$(grep '^ExecStart=.' "$f" | tail -n 1)"; [ -n "$line" ] && exec_start="$line"
      c="$(sed -n 's/^LoadCredentialEncrypted=//p' "$f")"; [ -n "$c" ] && cred="$c"
    done
    inv="inv$RANDOM$RANDOM"; echo "$inv" > "$FAKE/invocation"
    if [ -n "$cred" ]; then
      dir="$FAKE/run-credentials/$inv"; mkdir -p "$dir"
      systemd-creds decrypt --name="${cred%%:*}" "${cred#*:}" "$dir/${cred%%:*}" || { echo failed > "$state"; exit 1; }
      chmod 0400 "$dir/${cred%%:*}"; chmod 0500 "$dir"; export CREDENTIALS_DIRECTORY="$dir"
    else
      unset CREDENTIALS_DIRECTORY
    fi
    # The node keeps running in reality; the fake reports its start lines and leaves it "active" when they succeed
    # (FAKE_START_DIES: a node that stops after reporting, with the credential).
    # A node that exits non-zero is restarted by Restart=on-failure until FAKE_START_LIMIT models StartLimitBurst=.
    if ${exec_start#ExecStart=} > "$FAKE/journal-$inv" 2>&1 && ! { [ -n "${FAKE_START_DIES:-}" ] && [ -n "$cred" ]; }; then echo active > "$state"
    else echo failed > "$state"; [ -n "${FAKE_START_LIMIT:-}" ] && touch "$FAKE/start-limit-hit"; true; fi ;;
  *) exit 2 ;;
esac
''',
    'journalctl': r'''#!/bin/bash
for a in "$@"; do case "$a" in _SYSTEMD_INVOCATION_ID=*) cat "$FAKE/journal-${a#*=}" 2>/dev/null ;; esac; done
[ -n "${FAKE_SILENT_JOURNAL:-}" ] && exit 0
exit 0
''',
    'shred': r'''#!/bin/bash
echo "$*" >> "$FAKE/shred.log"
[ "$1" = --remove ] && rm -f "$2"
''',
    # The node: `identity` and `start` report the address of the key source they read (sha256 of its bytes, as a
    # stand-in for the secp256k1 address), and log which source that was.
    'pocol': r'''#!/bin/bash
cmd="$1"; shift
if [ "$cmd" = help ]; then
  if [ -n "${FAKE_OLD_POCOL:-}" ]; then echo 'usage: pocol identity [--home DIR] | start [--home DIR]' >&2
  else echo 'usage: pocol identity [--home DIR] [--key-credential NAME] | start [--home DIR] [--key-credential NAME]' >&2; fi
  exit 1
fi
home=''; cred=''; backup=''
while [ $# -gt 0 ]; do case "$1" in --home) home="$2"; shift 2 ;; --key-credential) cred="$2"; shift 2 ;; --backup) backup="$2"; shift 2 ;; *) shift ;; esac; done
if [ "$cmd" = inspect-key-backup ]; then
  # A fake backup: a header line, then the throwaway key; the passphrase is a line on stdin.
  IFS= read -r pass || true
  [ "$pass" = 'right passphrase' ] || { echo 'PoCol: wrong backup passphrase' >&2; exit 1; }
  printf '{"address":"0x%s","current":null}\n' "$(tail -c 32 "$backup" | sha256sum | cut -c1-40)"; exit 0
fi
[ "$cmd" = start ] && [ -n "$cred" ] && [ -n "${FAKE_CRED_START_FAILS:-}" ] && { echo 'PoCol: key credential refused' >&2; exit 1; }
[ "$cmd" = start ] && [ -z "$cred" ] && [ -n "${FAKE_FILE_START_FAILS:-}" ] && { echo 'PoCol: cannot start' >&2; exit 1; }
# A compromised node: it swaps the key file for a symbolic link to a file only root may write.
[ "$cmd" = start ] && [ -n "$cred" ] && [ -n "${FAKE_SWAP_KEY:-}" ] && { rm -f "$home/identity.pocol-key"; ln -s "$FAKE/victim" "$home/identity.pocol-key"; }
if [ -n "$cred" ]; then
  [ -n "${CREDENTIALS_DIRECTORY:-}" ] || { echo 'PoCol: --key-credential needs $CREDENTIALS_DIRECTORY' >&2; exit 1; }
  src="$CREDENTIALS_DIRECTORY/$cred"; echo "$cmd credential" >> "$FAKE/pocol-sources"
else
  src="$home/identity.pocol-key"; echo "$cmd file" >> "$FAKE/pocol-sources"
fi
[ -f "$src" ] || { echo "PoCol: cannot read $src" >&2; exit 1; }
address="0x$(sha256sum "$src" | cut -c1-40)"
[ -n "$cred" ] && [ -n "${FAKE_OTHER_ADDRESS:-}" ] && address="0x$(printf other | sha256sum | cut -c1-40)"
[ -n "$cred" ] && [ -n "${FAKE_SILENT_JOURNAL:-}" ] && exit 0
printf '{"address":"%s","home":"%s","scope":"mainnet"}\n' "$address" "$home"
''',
}


@unittest.skipIf(os.name == 'nt' or shutil.which('bash') is None, 'bash and a Unix file system are needed')
class EncryptServerKey(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.fake = self.root / 'fake'; self.fake.mkdir()
        self.bin = self.root / 'bin'; self.bin.mkdir()
        for tool in REAL_TOOLS:
            found = shutil.which(tool)
            if found: (self.bin / tool).symlink_to(found)
        for name, body in FAKES.items():
            if name == 'pocol': continue
            (self.bin / name).write_text(body, encoding='utf-8'); (self.bin / name).chmod(0o755)
        self.opt = self.root / 'opt/pocol-standalone'; self.opt.mkdir(parents=True)
        self.state = self.root / 'var/lib/pocol-standalone'; self.state.mkdir(parents=True, mode=0o700)
        self.units = self.root / 'etc/systemd/system'; self.units.mkdir(parents=True)
        self.credstore = self.root / 'etc/credstore.encrypted'
        (self.opt / 'pocol').write_text(FAKES['pocol'], encoding='utf-8'); (self.opt / 'pocol').chmod(0o755)
        (self.opt / 'network.pocol.json').write_text('{}', encoding='utf-8')
        self.key = self.state / 'identity.pocol-key'
        self.secret = os.urandom(32)
        self.key.write_bytes(self.secret); self.key.chmod(0o600)
        self.address = '0x' + hashlib.sha256(self.secret).hexdigest()[:40]
        (self.opt / 'identity.address').write_text(self.address, encoding='utf-8')
        self.backup = self.root / 'identity.pocol-backup'
        self.backup.write_bytes(b'FAKEBACKUP\n' + self.secret)
        self.unit = self.units / 'pocol-standalone.service'
        self.exec_start = f'ExecStart={self.opt}/pocol start --home {self.state} --listen 0.0.0.0:30300 --rpc 127.0.0.1:8545'
        self.unit.write_text(f'[Service]\nUser=pocol-standalone\n{self.exec_start}\nRestart=on-failure\n', encoding='utf-8')
        (self.fake / 'state').write_text('active\n', encoding='utf-8')
        # The script under test: the committed bytes with only its fixed paths moved into the temporary root.
        text = SCRIPT.read_text(encoding='utf-8')
        for path in ('/opt/pocol-standalone', '/var/lib/pocol-standalone', '/etc/systemd/system', '/etc/credstore.encrypted'):
            self.assertIn(path, text)
            text = text.replace(path, str(self.root) + path)
        self.text = text.replace('SETTLE_SECONDS=5', 'SETTLE_SECONDS=0')
        self.script = self.root / 'encrypt-server-key.sh'
        self.set_wait(2)

    def set_wait(self, seconds):
        self.script.write_text(self.text.replace('WAIT_SECONDS=90', f'WAIT_SECONDS={seconds}'), encoding='utf-8')

    def environment(self, path=None, **env):
        environment = {'PATH': str(path or self.bin), 'FAKE': str(self.fake), 'FAKE_UNIT': str(self.unit), 'LC_ALL': 'C'}
        environment.update(env)
        return environment

    def arguments(self, args, backup):
        return ['bash', str(self.script), *args, *(['--backup', str(self.backup)] if backup else [])]

    def run_script(self, *args, path=None, backup=True, passphrase='right passphrase', **env):
        return subprocess.run(self.arguments(args, backup), capture_output=True, text=True, env=self.environment(path, **env),
                              input=passphrase + '\n', timeout=60)

    def log(self, name):
        p = self.fake / name
        return p.read_text(encoding='utf-8').splitlines() if p.exists() else []

    def dropin(self):
        return self.units / 'pocol-standalone.service.d/50-encrypted-key.conf'

    def assert_unchanged(self, result, message):
        self.assertNotEqual(result.returncode, 0, result.stdout)
        self.assertIn(message, result.stderr)
        self.assertEqual(self.key.read_bytes(), self.secret, 'the plaintext key is kept')
        self.assertFalse((self.credstore / 'pocol-identity').exists())
        self.assertFalse(self.dropin().exists())
        self.assertEqual(self.log('shred.log'), [])
        self.assertNotIn(self.secret.hex(), result.stdout + result.stderr)

    def assert_rolled_back(self, result, message):
        self.assert_unchanged(result, message)
        self.assertIn('rolling back', result.stderr)
        log = self.log('systemctl.log')
        i = len(log) - 1 - log[::-1].index('daemon-reload')
        self.assertEqual(log[i:i + 3], ['daemon-reload', 'reset-failed pocol-standalone.service', 'start pocol-standalone.service'])
        self.assertEqual((self.fake / 'state').read_text(encoding='utf-8').strip(), 'active', 'the service runs again')
        self.assertEqual(self.log('pocol-sources')[-1], 'start file', 'it restarted from the key file')
        self.assertIn(f'rolled back; pocol-standalone.service runs from the plaintext key file and reports {self.address}', result.stderr)
        self.assertNotIn('ROLLBACK FAILED', result.stderr)

    def test_success_seals_the_key_verifies_the_address_then_removes_the_plaintext(self):
        result = self.run_script()
        self.assertEqual(result.returncode, 0, result.stderr)
        cred = self.credstore / 'pocol-identity'
        self.assertEqual(stat.S_IMODE(cred.stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE(self.credstore.stat().st_mode), 0o700)
        self.assertEqual(self.dropin().read_text(encoding='utf-8').splitlines()[2:], [
            '[Service]', f'LoadCredentialEncrypted=pocol-identity:{cred}', 'ExecStart=',
            f'{self.exec_start} --key-credential pocol-identity'])
        # The address was read from the key file with the service stopped, then reported by the service from the
        # credential, and only then was the plaintext removed.
        self.assertEqual(self.log('pocol-sources'), ['identity file', 'start credential'])
        self.assertEqual(self.log('systemctl.log')[:4], ['is-active -q pocol-standalone.service', 'stop pocol-standalone.service',
                                                         'daemon-reload', 'start pocol-standalone.service'])
        self.assertEqual(self.log('shred.log'), [f'--remove {self.key}'])
        self.assertIn(f'pocol-standalone shred --remove {self.key}', self.log('runuser.log'), 'shred runs as the service user')
        self.assertIn(f'The backup opens to {self.address}', result.stdout)
        self.assertFalse(self.key.exists())
        self.assertIn(f'reports {self.address}', result.stdout)
        self.assertIn('with-key=host', ' '.join(self.log('systemd-creds.log')))
        self.assertNotIn(self.secret.hex(), result.stdout + result.stderr)
        # A second run finds no plaintext key and changes nothing.
        again = self.run_script()
        self.assertNotEqual(again.returncode, 0)
        self.assertIn('no plaintext key file', again.stderr)

    def test_keep_plaintext_and_tpm2(self):
        result = self.run_script('--keep-plaintext', backup=False, FAKE_TPM2='1')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.key.read_bytes(), self.secret)
        self.assertEqual(self.log('shred.log'), [])
        self.assertIn('--with-key=host+tpm2', ' '.join(self.log('systemd-creds.log')))
        self.assertIn('Kept', result.stdout)
        self.assertNotIn('inspect-key-backup', ' '.join(self.log('runuser.log')))

    def test_removing_the_plaintext_needs_a_backup_that_opens_to_the_key(self):
        self.assert_unchanged(self.run_script(backup=False), 'needs --backup FILE')
        self.assertEqual(self.log('systemctl.log'), [], 'the service is not touched')
        self.backup.unlink()
        self.assert_unchanged(self.run_script(), 'is not a regular file')
        self.assertEqual(self.log('systemctl.log'), [])
        # A backup of another key, or a wrong passphrase, is found with the service stopped, and rolls back.
        self.backup.write_bytes(b'FAKEBACKUP\n' + os.urandom(32))
        self.assert_rolled_back(self.run_script(), 'does not open to ' + self.address)
        self.backup.write_bytes(b'FAKEBACKUP\n' + self.secret)
        self.assert_rolled_back(self.run_script(passphrase='wrong'), 'does not open to ' + self.address)

    def test_refusals_before_anything_changes(self):
        no_creds = self.root / 'bin-without-systemd-creds'; no_creds.mkdir()
        for entry in self.bin.iterdir():
            if entry.name != 'systemd-creds': (no_creds / entry.name).symlink_to(entry.resolve() if entry.is_symlink() else entry)
        cases = [
            ({'FAKE_UID': '1000'}, (), 'Run with sudo'),
            ({}, ('--with-key', 'tpm2-absent'), 'never a null key'),
            ({}, ('--with-key', 'auto'), 'never a null key'),
            ({}, ('--unknown',), 'unknown option'),
            ({'FAKE_SYSTEMD_VERSION': '249'}, (), 'older than 250'),
            ({'FAKE_OLD_POCOL': '1'}, (), 'has no --key-credential option'),
        ]
        for env, args, message in cases:
            with self.subTest(message):
                self.assert_unchanged(self.run_script(*args, **env), message)
                self.assertEqual(self.log('systemctl.log'), [], 'the service is not touched')
        self.assert_unchanged(self.run_script(path=no_creds), 'systemd-creds is not installed')
        self.assertEqual(self.log('systemctl.log'), [])
        (self.fake / 'state').write_text('inactive\n', encoding='utf-8')
        self.assert_unchanged(self.run_script(), 'is not running')
        (self.fake / 'state').write_text('active\n', encoding='utf-8')
        (self.opt / 'network.pocol.json').unlink()
        self.assert_unchanged(self.run_script(), 'install the shared network.pocol.json')
        (self.opt / 'network.pocol.json').write_text('{}', encoding='utf-8')
        # An existing credential or drop-in is never replaced.
        self.credstore.mkdir(mode=0o700)
        (self.credstore / 'pocol-identity').write_bytes(b'existing')
        result = self.run_script()
        self.assertIn('is never replaced', result.stderr)
        self.assertEqual((self.credstore / 'pocol-identity').read_bytes(), b'existing')
        (self.credstore / 'pocol-identity').unlink()
        self.credstore.chmod(0o777)
        self.assert_unchanged(self.run_script(), 'writable by root only')
        self.credstore.chmod(0o700)
        self.dropin().parent.mkdir()
        (self.dropin().parent / '10-other.conf').write_text('[Service]\nExecStart=\nExecStart=/bin/true\n', encoding='utf-8')
        self.assert_unchanged(self.run_script(), 'already sets ExecStart=')
        (self.dropin().parent / '10-other.conf').unlink()
        self.unit.write_text(self.unit.read_text(encoding='utf-8') + 'ExecStart=/bin/true\n', encoding='utf-8')
        self.assert_unchanged(self.run_script(), 'exactly one ExecStart= line')
        self.unit.write_text(f'[Service]\n{self.exec_start} --key-credential x\n', encoding='utf-8')
        self.assert_unchanged(self.run_script(), 'already passes --key-credential')
        self.key.unlink(); self.key.symlink_to(self.root / 'elsewhere')
        self.assertIn('no plaintext key file', self.run_script().stderr)
        self.assertNotIn('stop pocol-standalone.service', self.log('systemctl.log'))

    def test_a_credential_that_does_not_decrypt_to_the_key_is_rolled_back(self):
        self.assert_rolled_back(self.run_script(FAKE_CORRUPT_DECRYPT='1'), 'does not decrypt to the key file')

    def test_a_different_address_from_the_credential_is_rolled_back(self):
        self.assert_rolled_back(self.run_script(FAKE_OTHER_ADDRESS='1'), 'not ' + self.address)

    def test_no_address_from_the_service_is_rolled_back(self):
        self.assert_rolled_back(self.run_script(FAKE_SILENT_JOURNAL='1'), 'did not report an address')

    def test_a_service_that_stops_after_reporting_is_rolled_back(self):
        self.assert_rolled_back(self.run_script(FAKE_START_DIES='1'), 'did not keep running')

    def test_a_refused_credential_stops_the_wait_and_rolls_back_past_the_start_limit(self):
        # The node exits at once with the credential; Restart= would crash-loop it into the start limit. The wait
        # stops at the first exit instead of after WAIT_SECONDS, and the rollback clears the limit before it starts.
        self.set_wait(30)
        started = time.monotonic()
        result = self.run_script(FAKE_CRED_START_FAILS='1', FAKE_START_LIMIT='1')
        self.assertLess(time.monotonic() - started, 15, 'the wait ends when the node exits')
        self.assert_rolled_back(result, 'or stopped first')

    def test_a_rollback_whose_service_does_not_come_back_says_so(self):
        result = self.run_script(FAKE_CRED_START_FAILS='1', FAKE_FILE_START_FAILS='1')
        self.assert_unchanged(result, 'ROLLBACK FAILED: pocol-standalone.service is not running')
        self.assertNotIn('rolled back;', result.stderr)

    def test_an_interruption_rolls_back(self):
        # Ctrl-C and TERM reach the whole process group; sudo passes a hang-up on to the script alone.
        self.set_wait(30)
        for sig, group in ((signal.SIGINT, True), (signal.SIGHUP, False), (signal.SIGTERM, True)):
            with self.subTest(sig.name):
                for name in ('systemctl.log', 'pocol-sources', 'runuser.log'):
                    (self.fake / name).unlink(missing_ok=True)
                answer = self.root / 'passphrase'; answer.write_text('right passphrase\n', encoding='utf-8')
                with answer.open('rb') as stdin:
                    process = subprocess.Popen(self.arguments((), True), stdin=stdin, stdout=subprocess.PIPE,
                                               stderr=subprocess.PIPE, text=True, start_new_session=True,
                                               env=self.environment(FAKE_SILENT_JOURNAL='1'))
                deadline = time.monotonic() + 20
                while 'start credential' not in self.log('pocol-sources'):
                    self.assertLess(time.monotonic(), deadline, 'the service was started with the credential')
                    time.sleep(0.05)
                time.sleep(0.3)
                (os.killpg if group else os.kill)(process.pid, sig)
                out, err = process.communicate(timeout=30)
                self.assertEqual(process.returncode, 128 + sig, err)
                self.assert_rolled_back(subprocess.CompletedProcess([], process.returncode, out, err), 'rolling back')

    def test_a_key_path_swapped_for_a_symbolic_link_is_not_shredded(self):
        victim = self.fake / 'victim'; victim.write_text('a file only root may write\n', encoding='utf-8')
        result = self.run_script(FAKE_SWAP_KEY='1')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('is no longer a regular file; nothing was removed', result.stderr)
        self.assertEqual(victim.read_text(encoding='utf-8'), 'a file only root may write\n')
        self.assertEqual(self.log('shred.log'), [])
        self.assertTrue(self.dropin().exists(), 'the service keeps running from the verified credential')

    def test_a_key_file_of_another_address_than_the_installed_one_is_refused(self):
        (self.opt / 'identity.address').write_text('0x' + '1' * 40, encoding='utf-8')
        result = self.run_script()
        self.assert_unchanged(result, 'differs from')
        self.assertEqual((self.fake / 'state').read_text(encoding='utf-8').strip(), 'active', 'the service runs again')

    def test_the_script_parses(self):
        subprocess.run(['bash', '-n', str(SCRIPT)], check=True)
        text = SCRIPT.read_text(encoding='utf-8')
        # The plaintext is removed only after the verified restart, and never by the rollback.
        self.assertLess(text.index('[ "$after" = "$before" ]'), text.index('shred --remove "$KEY"'))
        self.assertNotIn('KEY', text[text.index('rollback() {'):text.index("trap 'rc=$?")].replace('$CRED', ''))
        self.assertIn('runuser -u "$SERVICE_USER" -- shred --remove "$KEY"', text)


if __name__ == '__main__':
    unittest.main()
