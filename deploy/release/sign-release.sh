#!/usr/bin/env bash
# Owner-run: sign a PoCol release directory with the owner's ssh-ed25519 release key (owner decision 2026-11-16).
# For a Linux or macOS signing machine (bash 3.2 or later, OpenSSH 8.2 or later, python3 3.8 or later; no GNU tools
# needed); on Windows use Sign-PoColRelease.ps1. deploy/release/README.md is the procedure.
#
#     bash deploy/release/sign-release.sh --key <offline path>/pocol-release DIR
#
# --key is the private key file (ssh-keygen asks for its passphrase), or its .pub when an ssh-agent holds the key.
# DIR comes from assemble_release.py: the release files and SHA256SUMS. This script
#   - refuses a key inside this repository, a public key that is not the committed allowed_signers key, an existing
#     .sig in DIR, a SHA256SUMS that is not strict or does not match the files, and any entry SHA256SUMS does not list
#     (hidden ones such as .DS_Store included), before it writes anything;
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
# The release directory, before anything is written, in Python (portable to stock macOS: bash 3.2 builtins only, no
# GNU coreutils; hidden entries such as .DS_Store are listed too): the strict SHA256SUMS of verify_release.py, every listed hash, no
# existing .sig, and nothing SHA256SUMS does not list. It prints the listed names, one per line.
NAMES="$(python3 -B -c '
import hashlib, os, sys, importlib.util
spec = importlib.util.spec_from_file_location("v", sys.argv[1]); v = importlib.util.module_from_spec(spec); spec.loader.exec_module(v)
d = sys.argv[2]
def refuse(why):
    print("REFUSED: " + why, file=sys.stderr); sys.exit(1)
entries = sorted(os.listdir(d))
if v.SUMS not in entries:
    refuse("no SHA256SUMS (run assemble_release.py first)")
if any(e.endswith(v.SIG) for e in entries):
    refuse("a .sig already exists in the release directory; never overwrite a signature")
path = os.path.join(d, v.SUMS)
if os.path.islink(path) or not os.path.isfile(path):
    refuse("SHA256SUMS is not a regular file")
try:
    rows = v.parse_sums(open(path, "rb").read())
except v.NotVerified as e:
    refuse(str(e))
listed = [n for _, n in rows]
for e in entries:
    if e != v.SUMS and e not in listed:
        refuse(e + " is not listed in SHA256SUMS")
for digest, name in rows:
    f = os.path.join(d, name)
    if os.path.islink(f) or not os.path.isfile(f):
        refuse(name + " is listed in SHA256SUMS but is not a regular file")
    if hashlib.sha256(open(f, "rb").read()).hexdigest() != digest:
        refuse("SHA256SUMS does not match the files (" + name + ")")
print("\n".join(listed))
' "$HERE/verify_release.py" "$DIR")" || exit 1
LISTED=()
while IFS= read -r name; do LISTED+=("$name"); done <<EOF_NAMES
$NAMES
EOF_NAMES
[ "${#LISTED[@]}" -gt 0 ] || die 'SHA256SUMS lists no file'
cd "$DIR"
for name in "${LISTED[@]}" SHA256SUMS; do
  ssh-keygen -q -Y sign -f "$KEY_ABS" -n "$NAMESPACE" "$name"
done
cd - >/dev/null
python3 -B "$HERE/verify_release.py" "$DIR" --allowed-signers "$ALLOWED"
echo 'Release key fingerprint (compare with the one published on pocol.net):'
ssh-keygen -lf "$PUB"
