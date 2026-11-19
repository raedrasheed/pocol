#!/usr/bin/env bash
# Opt-in (gate K1): move this server's miner key out of the plaintext key file into a systemd credential that is
# encrypted at rest. Run as root on a server installed by install-linux.sh, once its service runs:
#
#   sudo bash encrypt-server-key.sh --backup FILE [--with-key host|tpm2|host+tpm2]
#   sudo bash encrypt-server-key.sh --keep-plaintext [--backup FILE] [--with-key host|tpm2|host+tpm2]
#
# Steps, each checked before the next: stop the service; read the address of the plaintext key
# (`pocol identity`); check that the passphrase-encrypted backup FILE opens to that address (`pocol
# inspect-key-backup`, its passphrase read from the terminal); seal the key file with `systemd-creds encrypt --name=pocol-identity` into
# /etc/credstore.encrypted/pocol-identity (default --with-key: host+tpm2 when systemd finds a usable TPM2, else host);
# check that it decrypts to the same bytes (through a pipe, never a file); write the drop-in
# pocol-standalone.service.d/50-encrypted-key.conf (LoadCredentialEncrypted= and `pocol start ... --key-credential
# pocol-identity`); start the service and wait for it to report its address. Only when that address equals the one
# read before, and the service is still running, is the plaintext key file removed (shred, as the service user),
# unless --keep-plaintext. Without a checked --backup the plaintext is never removed. Any failure before that, and an
# interruption (Ctrl-C, a hang-up, TERM), rolls back: the credential and the drop-in are removed and the service starts
# again from the plaintext key file, which is never removed then; the rollback says whether the service runs again.
# Nothing here prints or copies the key.
#
# Make the passphrase-encrypted backup first (`pocol export-key`, README "Encrypted key backup"), readable by the
# service user: a credential sealed to this host's key or TPM2 cannot be decrypted on another machine, or after a
# change of the PCR state it is sealed to (with tpm2: firmware or Secure Boot changes), so it is no backup.
set -euo pipefail
umask 077

SERVICE=pocol-standalone.service
SERVICE_USER=pocol-standalone
OPT=/opt/pocol-standalone
STATE=/var/lib/pocol-standalone
UNIT=/etc/systemd/system/pocol-standalone.service
DROPIN_DIR=/etc/systemd/system/pocol-standalone.service.d
DROPIN="$DROPIN_DIR/50-encrypted-key.conf"
CREDSTORE=/etc/credstore.encrypted
NAME=pocol-identity
CRED="$CREDSTORE/$NAME"
KEY="$STATE/identity.pocol-key"
# How long the restarted service has to report its address, and how long it must then keep running.
WAIT_SECONDS=90
SETTLE_SECONDS=5

die() { echo "encrypt-server-key: $*" >&2; exit 1; }
with_key=''
keep_plaintext=0
backup=''
while [ $# -gt 0 ]; do
  case "$1" in
    --with-key) [ $# -ge 2 ] || die 'missing value for --with-key'; with_key="$2"; shift 2 ;;
    --keep-plaintext) keep_plaintext=1; shift ;;
    --backup) [ $# -ge 2 ] || die 'missing value for --backup'; backup="$2"; shift 2 ;;
    -h|--help) sed -n '2,23p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown option $1" ;;
  esac
done

# --- Checks that change nothing ---
[ "$(id -u)" = 0 ] || die 'Run with sudo; nothing changed.'
command -v systemd-creds >/dev/null 2>&1 || die 'systemd-creds is not installed (LoadCredentialEncrypted= needs systemd 250 or later); nothing changed.'
version="$(systemd-creds --version 2>/dev/null | sed -n '1s/^systemd \([0-9][0-9]*\).*/\1/p')"
[ -n "$version" ] && [ "$version" -ge 250 ] || die "systemd ${version:-of unknown version} is older than 250, which LoadCredentialEncrypted= needs; nothing changed."
for tool in systemctl journalctl runuser cmp shred; do
  command -v "$tool" >/dev/null 2>&1 || die "$tool is not installed; nothing changed."
done
case "$with_key" in
  '') if systemd-creds has-tpm2 -q >/dev/null 2>&1; then with_key=host+tpm2; else with_key=host; fi ;;
  host|tpm2|host+tpm2) ;;
  # A null key (tpm2-absent, auto-initrd) encrypts nothing, and auto may pick one.
  *) die "--with-key must be host, tpm2 or host+tpm2 (never a null key); nothing changed." ;;
esac
[ -x "$OPT/pocol" ] && [ -f "$UNIT" ] || die "no pocol-standalone installation (install-linux.sh) at $OPT; nothing changed."
[ -f "$OPT/network.pocol.json" ] || die "install the shared network.pocol.json in $OPT and start $SERVICE first; nothing changed."
{ [ -f "$KEY" ] && [ ! -L "$KEY" ]; } || die "no plaintext key file at $KEY (already moved into a credential?); nothing changed."
if [ -n "$backup" ]; then
  case "$backup" in /*) ;; *) backup="$PWD/$backup" ;; esac
  { [ -f "$backup" ] && [ ! -L "$backup" ]; } || die "--backup $backup is not a regular file; nothing changed."
  runuser -u "$SERVICE_USER" -- test -r "$backup" || die "--backup $backup is not readable by $SERVICE_USER; nothing changed."
elif [ "$keep_plaintext" = 0 ]; then
  die 'removing the plaintext key file needs --backup FILE (the passphrase-encrypted backup, checked against the key); or pass --keep-plaintext; nothing changed.'
fi
for path in "$CRED" "$CRED.new" "$DROPIN" "$DROPIN.new"; do
  { [ ! -e "$path" ] && [ ! -L "$path" ]; } || die "$path exists; an existing credential or drop-in is never replaced; nothing changed."
done
if [ -e "$CREDSTORE" ] || [ -L "$CREDSTORE" ]; then
  { [ -d "$CREDSTORE" ] && [ ! -L "$CREDSTORE" ] && [ -z "$(find "$CREDSTORE" -maxdepth 0 -perm /022)" ]; } \
    || die "$CREDSTORE must be a real directory writable by root only; nothing changed."
fi
# `pocol help` prints the usage (and exits 1: it is no command).
case "$("$OPT/pocol" help 2>&1 || true)" in
  *--key-credential*) ;;
  *) die "$OPT/pocol has no --key-credential option; install a release that has it first; nothing changed." ;;
esac
[ "$(grep -c '^ExecStart=' "$UNIT" || true)" = 1 ] || die "$UNIT must have exactly one ExecStart= line; nothing changed."
exec_start="$(grep '^ExecStart=' "$UNIT")"
case "$exec_start" in
  "ExecStart=$OPT/pocol start "*) ;;
  *) die "$UNIT does not start $OPT/pocol start as install-linux.sh writes it; nothing changed." ;;
esac
case "$exec_start" in *--key-credential*) die "$UNIT already passes --key-credential; nothing changed." ;; esac
if [ -d "$DROPIN_DIR" ] && grep -qs -e '^ExecStart=' -e '^LoadCredential' "$DROPIN_DIR"/*.conf; then
  die "a drop-in in $DROPIN_DIR already sets ExecStart= or a credential; nothing changed."
fi
systemctl is-active -q "$SERVICE" || die "$SERVICE is not running; start it first (this restarts it and compares the address it reports); nothing changed."

address_in() { sed -n 's/.*"address":"\(0x[0-9a-f]\{40\}\)".*/\1/p' | head -n 1; }

# Sets $reported to the address the run of the service with this invocation ID reports, waiting up to WAIT_SECONDS;
# empty when it reports none, or stops or restarts first. (It runs in this shell, not in $(...), so an interruption
# is handled within a second.)
wait_for_address() {
  local invocation="$1"
  reported=''
  for _ in $(seq 1 "$WAIT_SECONDS"); do
    reported="$(journalctl -q -o cat "_SYSTEMD_INVOCATION_ID=$invocation" 2>/dev/null | address_in || true)"
    [ -n "$reported" ] && return 0
    # A node that exits (Restart= then restarts it as another invocation) never reports: stop waiting.
    { systemctl is-active -q "$SERVICE" && [ "$(systemctl show -p InvocationID --value "$SERVICE")" = "$invocation" ]; } || return 0
    sleep 1
  done
}

# --- From here a failure, or an interruption, rolls back to the plaintext key file ---
armed=0 made_cred=0 made_dropin=0 before='' reported=''
rollback() {
  trap '' INT HUP TERM
  set +e
  echo 'encrypt-server-key: rolling back; the plaintext key file is kept and the service starts from it again.' >&2
  systemctl stop "$SERVICE"
  if [ "$made_dropin" = 1 ]; then rm -f "$DROPIN" "$DROPIN.new"; rmdir "$DROPIN_DIR" 2>/dev/null; fi
  if [ "$made_cred" = 1 ]; then rm -f "$CRED" "$CRED.new"; fi
  systemctl daemon-reload
  # A crash-looping credential start can have hit the unit's start limit, which would refuse this start.
  systemctl reset-failed "$SERVICE"
  if systemctl start "$SERVICE"; then
    local again
    again="$(systemctl show -p InvocationID --value "$SERVICE")"
    wait_for_address "$again"
    sleep "$SETTLE_SECONDS"
    if systemctl is-active -q "$SERVICE" && [ "$(systemctl show -p InvocationID --value "$SERVICE")" = "$again" ] \
      && { [ -z "$before" ] || [ "$reported" = "$before" ]; }; then
      echo "encrypt-server-key: rolled back; $SERVICE runs from the plaintext key file${reported:+ and reports $reported}." >&2
      return
    fi
  fi
  echo "encrypt-server-key: ROLLBACK FAILED: $SERVICE is not running from the plaintext key file. The key file is kept;" >&2
  echo "encrypt-server-key: see journalctl -u $SERVICE, then: systemctl reset-failed $SERVICE; systemctl start $SERVICE" >&2
}
trap 'rc=$?; if [ "$rc" != 0 ] && [ "$armed" = 1 ]; then armed=0; rollback; fi; exit "$rc"' EXIT
# An EXIT trap sees status 0 when bash dies of a signal; exit with the signal's status instead, so it rolls back.
trap 'exit 130' INT
trap 'exit 129' HUP
trap 'exit 143' TERM
fail() { echo "encrypt-server-key: $*" >&2; exit 1; }

echo "Stopping $SERVICE to read its key file's address."
armed=1
systemctl stop "$SERVICE"
# Checked again with the service (which may write its state directory) stopped.
{ [ -f "$KEY" ] && [ ! -L "$KEY" ]; } || fail "$KEY is no longer a regular file."
before="$(runuser -u "$SERVICE_USER" -- "$OPT/pocol" identity --home "$STATE" | address_in || true)"
[ -n "$before" ] || fail 'pocol identity did not report the address of the plaintext key file.'
if [ -f "$OPT/identity.address" ] && [ "$(cat "$OPT/identity.address")" != "$before" ]; then
  fail "the key file's address $before differs from $OPT/identity.address."
fi
echo "Key file address: $before"
if [ -n "$backup" ]; then
  echo "Checking the backup $backup; enter its passphrase."
  opened="$(runuser -u "$SERVICE_USER" -- "$OPT/pocol" inspect-key-backup --home "$STATE" --backup "$backup" | address_in || true)"
  [ "$opened" = "$before" ] || fail "the backup $backup does not open to $before${opened:+ (it holds $opened)}; make it with pocol export-key."
  echo "The backup opens to $before."
fi

[ -d "$CREDSTORE" ] || mkdir -m 0700 "$CREDSTORE"
made_cred=1
systemd-creds encrypt --with-key="$with_key" --name="$NAME" "$KEY" "$CRED.new"
chmod 0600 "$CRED.new"
mv "$CRED.new" "$CRED"
# The credential must decrypt to the key file's bytes; the plaintext goes through a pipe only.
systemd-creds decrypt --name="$NAME" "$CRED" - | cmp -s - "$KEY" || fail "$CRED does not decrypt to the key file."
echo "Sealed $CRED (--with-key=$with_key)."

made_dropin=1
mkdir -p -m 0755 "$DROPIN_DIR"
{
  echo '# Written by encrypt-server-key.sh (gate K1): the miner key is the encrypted credential below; the node never'
  echo '# reads identity.pocol-key with --key-credential, and refuses to start without the credential.'
  echo '[Service]'
  echo "LoadCredentialEncrypted=$NAME:$CRED"
  echo 'ExecStart='
  echo "$exec_start --key-credential $NAME"
} > "$DROPIN.new"
chmod 0644 "$DROPIN.new"
mv "$DROPIN.new" "$DROPIN"
systemctl daemon-reload

echo "Starting $SERVICE with the credential."
systemctl start "$SERVICE" || fail "$SERVICE did not start with the credential (journalctl -u $SERVICE)."
invocation="$(systemctl show -p InvocationID --value "$SERVICE")"
[ -n "$invocation" ] || fail "$SERVICE has no invocation to read its address from."
wait_for_address "$invocation"
after="$reported"
[ -n "$after" ] || fail "$SERVICE did not report an address within $WAIT_SECONDS seconds, or stopped first (journalctl -u $SERVICE)."
[ "$after" = "$before" ] || fail "$SERVICE reported $after with the credential, not $before."
sleep "$SETTLE_SECONDS"
{ systemctl is-active -q "$SERVICE" && [ "$(systemctl show -p InvocationID --value "$SERVICE")" = "$invocation" ]; } \
  || fail "$SERVICE did not keep running with the credential (journalctl -u $SERVICE)."
echo "$SERVICE runs from the credential and reports $after."

# --- Verified: the plaintext key file is no longer needed ---
armed=0
if [ "$keep_plaintext" = 1 ]; then
  echo "Kept $KEY (--keep-plaintext). Remove it yourself: shred --remove $KEY"
else
  # The running service may write its state directory: the path is checked again, and shred (which follows a
  # symbolic link) runs as the service user, so it can reach only what that user can already write.
  { [ -f "$KEY" ] && [ ! -L "$KEY" ]; } || die "$KEY is no longer a regular file; nothing was removed. The service runs from the credential; check the state directory."
  runuser -u "$SERVICE_USER" -- shred --remove "$KEY"
  sync
  [ ! -e "$KEY" ] || die "could not remove $KEY; remove it yourself."
  echo "Removed $KEY."
fi
leftovers="$(find "$STATE" -maxdepth 1 -name 'identity.pocol-key?*' 2>/dev/null || true)"
if [ -n "$leftovers" ]; then
  echo 'These older plaintext key files remain (left by key restores); remove them once no backup needs them:'
  echo "$leftovers"
fi
echo 'shred cannot reach copies a journaling or copy-on-write file system, an SSD or a backup kept; see README "Encrypted server key".'
