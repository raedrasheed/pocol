# Public-network compatibility and operations, 29 October 2026

The owner requested completion of a functioning public network. This increment
implements additional prerequisites; **the complete launch goal is not fulfilled**.
No actual server addresses/access were supplied. No Internet service, paid hosting,
real-value currency or production profile was launched. Remaining software/protocol
obligations are still open in [PLAN.md](PLAN.md); servers alone would not close them.

## Compatibility and hostile input

Experimental wire v2 appends `protocolVersion` and a 32-byte compatibility digest to
the seven-field Hello, making nine fields. Old handshakes and different versions or
digests fail closed, on first contact and later status updates. A rejected connection
is removed immediately; already queued frames and a second valid-looking Hello
cannot reauthorize it. Only admitted peers receive gossip. The first frame is limited
to 512 bytes; peer names are 1..64 printable ASCII bytes without spaces/control codes.

`pocold build-info` exposes a reproducible source fingerprint, and `pocol_status` /
`pocol_getParams` expose it with protocolVersion. The fingerprint commits to the UTF-8
path and raw bytes of all Rust source files in the three crates, their Cargo manifests,
the pocold lockfile/build script, embedded system/website artifacts and the caps/crash/
I/O-fault feature flags. Paths use bytewise lexical order with 64-bit big-endian
lengths, SHA-256 and domain `pocol-testnet-source-compatibility-v1\0`. It excludes Git
commit metadata, absolute build paths, timestamps and secrets. Raw text requires a
release checkout with preserved line endings.

This conservative identity intentionally changes even for some non-consensus edits.
It is not the unresolved D-5 normative constants digest, a binary reproducibility
proof, peer authentication or remote attestation. A malicious peer can advertise any
digest. Strict coordination is required for upgrades; no implicit wire downgrade or
state-binding rewrite is provided. Block validity, economics, H_END and the existing
production-admission refusals are unchanged.

Inspection also found an unconditional loopback-source filter in the P2P listener.
It prevented the older private-server package from accepting inter-host connections;
its loopback rehearsal could not detect that. The listener now honors the explicitly
configured bind address and accepts remote TCP peers through the same bounded
handshake. Native RPC remains loopback-only. A required CI control connects through
the runner's own non-loopback interface, without changing routes or contacting an
external host. That is narrower evidence than an actual multi-host trial.

## Operations implementation

`deploy/public-testnet/` contains a public-address inventory validator, one-genesis
packager, independent-key Linux service, checksum-gated fresh installer, offline
backup/restore, cross-node progress/root monitoring and a dedicated TLS gateway
configuration. `--join` prepares a new observer or independent miner with published
seeds. Secret keys stay outside bundles/state; systemd credentials expose only the
host's key to its service user. State binds to network, release, node and miner.

Backups hold both the operator and Rust journal locks, reject active processes and
links, checksum/fsync the complete stopped state and restore only into a new path.
The gateway has per-IP and global request/connection limits, fixed body/time limits,
TLS verification prerequisites and preserved Origin. It does not fix unbounded
precompile cancellation or complete RpcGuard memory accounting. Monitoring checks
compatible peers, healthy history, finite-lifetime proximity and common confirmed
hash/state/receipt roots while all nodes advance.

## Evidence and limits

Local runs (evidence under `evidence/operations/`):

| Check | Result | Scope |
| --- | --- | --- |
| Rust library | 51 PASS | Existing library regressions |
| Real-socket network/RPC | 55 PASS | Includes incompatible initial/repeated Hello, pipelined transaction fencing and bounded handshake/name checks |
| Key/genesis CLI | 5 PASS | Independent generation, file checks and admission controls |
| Final consensus and caps regression | 93 + 11 PASS | Seven existing expanded consensus measurements remain ignored; none added or disabled |
| Operations controls | 7 PASS | Inventory, links/checksums, both backup locks, restore tampering/identity, independent digest reproduction, URL and duplicate-JSON controls |
| Packaged four-node integration | 9 PASS | Independent transfers, late registration/mining, duplicate rejection, site publish/fetch, live-backup refusal, stopped-state restore and observation |
| systemd unit syntax | PASS locally | Unit verification only; actual service activation/credential ownership still needs a target host |
| Nginx TLS/rate test | Pending exact-head CI | Nginx is absent locally; the workflow installs it and runs the loopback test, without claiming a local pass |
| Separate systemd service UID and root restore | Pending exact-head CI | Runs the service from a restrictive-mode bundle and restores root-made state with service UID/GID; uses loopback fixtures only |

The packaged integration binary SHA-256 is
`33b3c8e23d4bfb762526853863f562f856daa36296a8015a099f5f6237d6296b`;
its source compatibility fingerprint is
`64e7fef3cbd46c7e7af6c3442962ecf3291678f233b45a60158f15f0528cf950`.
Restore compared the common block at height 27. The following 180-second observation
ended with all four heads at 59 and identical confirmed roots at height 53. These are
four processes on one machine, not four-host acceptance or a sustained soak.

The independent digest control initially failed: Rust Path component ordering differed
from bytewise string ordering. The failure is retained in evidence; the build script
was corrected and the independent control then passed. The 51/55 Rust run preceded
that ordering correction; the packaged integration and final controls used the corrected
build. Exact-head CI reruns the Rust suites on the final submitted source.

A wider consensus check then found a legacy fixture that injected messages from a
nonexistent peer. Its original failure is retained. Handler fixtures now establish a
real compatible transport connection before injecting deterministic events; their
original validity, penalty, pool and recovery assertions remain. The non-loopback
filter correction followed the recorded packaged run, so final-source integration
and the actual non-loopback-interface control are required in CI. Local interface
enumeration was unavailable (`netlink: Operation not permitted`); no route/security
configuration was changed to bypass that restriction.

Automated review of the first submitted operations commit found two real service
permission defects: a 077 preparation umask could leave public bundles unreadable
by the service UID, and root-created restored state was not owned by that UID. The
installer now normalizes the verified public tree to directories/binaries 0755 and
other files 0644. Root restore requires an explicit service owner and applies UID/GID
with 0700 directories/0600 files. The runbook also updates StateDirectory alongside
the restored data path under ProtectSystem=strict. The added CI drill starts the
actual separate-UID service, checks LoadCredential key access, restores and restarts.

Exact-head CI at `f2be434ff8a27aa10e813b0b05fbc482c952b9f4` passed all Rust,
Python, packaged integration, non-loopback listener and Nginx TLS/rate checks, but
the new service drill failed. The systemd credential's metadata did not satisfy
the strict Rust key-file checks. The original failure is retained in
`evidence/operations/initial-systemd-credential-failure.log` (workflow run
[40883255893](https://github.com/palcertcom/pocol/actions/runs/40883255893)).
The unit now materializes a service-owned 0600 copy in a private 0700
RuntimeDirectory, removed on stop, without relaxing the Rust checks or copying a
secret into persistent state. The drill additionally checks owner, mode, link
count and removal on stop; this correction still requires exact-head CI.
The [systemd credential/runtime directory semantics](https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html)
are distinct from the application's own key-file metadata policy.

A subsequent final-core local integration passed all nine checks with binary
`7b1bc349966d8a42d501575e7de452d6baf9ecd96985b482d36ec27d59415e23`
and compatibility fingerprint
`bfdf1d05559199057c3dfb83969839fe3fd723a195d4e332ed17ec485b36e4ca`.
Its public evidence is retained separately in `evidence/operations/integration-final/`.
It includes the listener correction but does not exercise systemd credentials.

The operations integration used the recorded packaged files. Subsequent preparer-only
validation additionally rejects multicast/reserved/duplicate-host addresses and
noncanonical seed ports. Those controls are covered by the final unit tests and CI;
they do not change the node fingerprint. Public evidence contains no keys, state
backups, generated TLS private keys or complete release binaries.

The historical English-translation manifest is preserved. Its current-file hash
check reports five deliberately changed code/test files from this increment; that
is expected snapshot drift, not a new translation attestation. Its language scan
found no raw Arabic text outside the unchanged thesis, with original-source hashes,
reference numbers and translated archive checks unchanged. The complete result is
retained as `translation-snapshot-drift.json` rather than relabeled PASS.

## Remaining launch work

1. Resolve bounded historical-state/K_eff accounting and near-cap tests; complete
   deferred resource/gas derivations and the missing RpcGuard mechanisms.
2. Adopt a concrete public profile and normative decisions, measure target hardware
   and worst-case execution, and define lifetime/cumulative-cap behavior. The present
   profile still closes/ends at 19,000/20,000; this is not a perpetual network.
3. Supply operator-controlled hosts/endpoints, independent public keys/allocations,
   clock/network measurements and a shared genesis schedule; perform multi-host
   partition/heal, recovery, overload, release/rollback and soak acceptance.
4. Obtain an independent security review, publish a reviewed release and limitations,
   then activate/verify the public endpoints. Existing automated review is not that review.
