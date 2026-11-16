#!/usr/bin/env bash
# Owner-run: sign a PoCol release directory with the owner's ssh-ed25519 release key (owner decision 2026-11-16).
# For a Linux or macOS signing machine; on Windows use Sign-PoColRelease.ps1. deploy/release/README.md is the procedure.
#
#     bash deploy/release/sign-release.sh --key <offline path>/pocol-release DIR
#
# --key is the private key file (ssh-keygen asks for its passphrase), or its .pub when an ssh-agent holds the key.
# DIR comes from assemble_release.py: the release files and SHA256SUMS. This script
#   - refuses a key inside this repository, a public key that is not the committed allowed_signers key, an existing
#     .sig in DIR, a SHA256SUMS that is not strict or does not match the files, and any file SHA256SUMS does not list;
#   - signs each listed file, then SHA256SUMS, with `ssh-keygen -Y sign -n pocol-release`, writing FILE.sig;
#   - runs verify_release.py on DIR against the committed allowed_signers and prints the key's fingerprint, to compare
#     with the one published on pocol.net.
# It never copies, prints, converts or generates a key. --allowed-signers is for the tests only.
set -euo pipefail
umask 022
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/../.." && pwd)"
NAMESPACE=pocol-release
ALLOWED="$HERE/allowed_signers"
KEY=''
DIR=''
die() { echo "REFUSED: $*" >&2; exit 1; }
while [ $# -gt 0 ]; do
  case "$1" in
    --key) KEY="${2:-}"; shift 2 ;;
    --allowed-signers) ALLOWED="${2:-}"; shift 2 ;;
    -*) die "unknown option $1" ;;
    *) [ -z "$DIR" ] || die 'one release directory only'; DIR="$1"; shift ;;
  esac
done
[ -n "$KEY" ] && [ -n "$DIR" ] || die 'usage: sign-release.sh --key KEY DIR'
command -v ssh-keygen >/dev/null || die 'ssh-keygen (OpenSSH 8.2 or later) is required'
command -v python3 >/dev/null || die 'python3 is required (verify_release.py)'
[ -f "$KEY" ] || die "no key file at $KEY"
[ -d "$DIR" ] || die "no release directory at $DIR"
KEY_ABS="$(cd "$(dirname "$KEY")" && pwd -P)/$(basename "$KEY")"
REPO_REAL="$(cd "$REPO" && pwd -P)"
case "$KEY_ABS/" in "$REPO_REAL"/*) die "the key $KEY is inside the repository; keep it offline, outside any checkout" ;; esac
case "$KEY_ABS" in *.pub) PUB="$KEY_ABS" ;; *) PUB="$KEY_ABS.pub" ;; esac
[ -f "$PUB" ] || die "no public key at $PUB (keep the .pub beside the key)"
# The key must be the committed release key, before anything is written.
python3 -B -c '
import sys, importlib.util
spec = importlib.util.spec_from_file_location("v", sys.argv[1]); v = importlib.util.module_from_spec(spec); spec.loader.exec_module(v)
try:
    _, kind, key = v.read_allowed(sys.argv[2])
except v.NotVerified as e:
    print(e, file=sys.stderr); sys.exit(1)
parts = open(sys.argv[3], encoding="ascii").read().split()
sys.exit(0 if parts[:2] == [kind, key] else 1)
' "$HERE/verify_release.py" "$ALLOWED" "$PUB" || die "the key is not the one in $ALLOWED (commit its public line first, by a reviewed PR)"
cd "$DIR"
[ -f SHA256SUMS ] || die 'no SHA256SUMS (run assemble_release.py first)'
if compgen -G '*.sig' >/dev/null; then die 'a .sig already exists in the release directory; never overwrite a signature'; fi
if grep -q $'\r' SHA256SUMS; then die 'SHA256SUMS has CR line endings'; fi
if grep -qvE '^[0-9a-f]{64}  [A-Za-z0-9._-]+$' SHA256SUMS; then die 'SHA256SUMS has a line that is not <sha256>  <name>'; fi
sha256sum --check --strict --quiet SHA256SUMS || die 'SHA256SUMS does not match the files'
mapfile -t LISTED < <(cut -c67- SHA256SUMS)
for entry in *; do
  [ "$entry" = SHA256SUMS ] && continue
  found=0; for name in "${LISTED[@]}"; do [ "$name" = "$entry" ] && found=1; done
  [ "$found" = 1 ] || die "$entry is not listed in SHA256SUMS"
done
for name in "${LISTED[@]}" SHA256SUMS; do
  ssh-keygen -q -Y sign -f "$KEY_ABS" -n "$NAMESPACE" "$name"
done
cd - >/dev/null
python3 -B "$HERE/verify_release.py" "$DIR" --allowed-signers "$ALLOWED"
echo 'Release key fingerprint (compare with the one published on pocol.net):'
ssh-keygen -lf "$PUB"
