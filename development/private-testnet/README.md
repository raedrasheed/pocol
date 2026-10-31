# Private testnet preparation evidence — 29 October 2026

Deployment tooling: [runbook and commands](../../deploy/private-testnet/README.md).

**Outcome: preparation and a local three-process rehearsal passed. No servers were
provisioned, accessed or deployed. No remote-host, firewall, VPN, clock-bound,
systemd lifecycle or production-network acceptance is claimed.**

The build used Rust 1.99.1, `cargo build --offline --locked --release --bins` for
`prototypes/pocold`. The application source is unchanged from the English translation
revision `d3dfb142e8bbe35b03fd20f809ed3f7f2a8335d2`. This change adds deployment
packaging, operational checks and their tests; it does not alter consensus, genesis
validation, the production boot gate, or the underlying deterministic key scheme.

## Completed checks

- Eight Python unit tests, including negative inputs for public/wildcard/metadata
  addresses, duplicate names, unsafe labels, wrong chainIds, invalid ports, manifest
  tampering, missing manifest coverage, path escape and false health success.
- Private-IP example successfully generated a single shared genesis, passed the
  existing experimental admission and produced three verified bundles. This only
  generated files; the private example IPs were not contacted.
- Six real-process rehearsal checks: connections on three loopback IPs; duplicate
  data-directory process refusal; a successful alice-to-bob test transfer on all
  nodes; network identity/progress/common-root check; wrong data-network binding
  refusal; restart/rejoin retaining the common block.
- The observed common block was height 3; heights progressed from 8 to 9 during
  the 30-second observation. This is a short smoke test, not a throughput result.
- Generated systemd unit syntax passed `systemd-analyze verify`; installer shell
  syntax passed `bash -n`. Installation and systemd execution on target hosts are
  untested. The installer was not run here as a deployment.
- The existing translation-integrity verifier still passes all 290 manifest files.

Recorded results: [result.json](evidence/result.json), [health.json](evidence/health.json),
[unit tests](evidence/unit.log), and per-node logs in `evidence/`. The binary digest
is in result.json; packaging/check script digests are in
[source-sha256.json](evidence/source-sha256.json). Rehearsal chainId is 777911, distinct
from the editable private-network example 777910. Both are simnet profiles with
synthetic difficulty calibration and existing documented admission waivers.

## Remaining before a distributed pilot

Supply dedicated server inventory and authorized SSH access, verify the private
network and firewall, assess timing on the actual machines, and prepare a fresh
common genesis near the scheduled start. Then deploy, record host separation,
run transfers and a website round-trip, test host restart and controlled peer
disconnection/reconnection, and observe convergence before the finite closing height.

The new GitHub workflow runs the local packaging rehearsal on the exact PR head.
Its results, when available, remain separate from this saved local evidence.

## CI follow-up: receipt convergence

The [first GitHub run](https://github.com/palcertcom/pocol/actions/runs/40864420878)
failed the receipt-block equality assertion immediately after each moving tip had
included the test transaction. This timing did not establish a shared canonical
inclusion: concurrent miners can include it in transient sibling blocks. The
rehearsal now waits, with a 240-second limit, for the **same** inclusion hash on
all nodes, for that hash to be canonical on each, and for six subsequent blocks
on each node. Equality and success/balance assertions remain required. Persistent
divergence still fails; no failure is converted to success by skipping a check.

A fresh local rerun passed all six checks; its common comparison height was 9,
with all heads advancing from 10 to 15 during the observation. See
[R2 results](evidence/r2/result.json) and [rehearsal source digest](evidence/r2/rehearsal.sha256).
The original evidence and the failed CI run remain separate. A passing local rerun
is not itself a passing GitHub run.

## CI follow-up: operational observation window

The [second GitHub run](https://github.com/palcertcom/pocol/actions/runs/40867325498)
passed the confirmed transfer, then correctly failed its 30-second no-progress
check. The downloaded artifact matched GitHub's SHA-256
`28fb1863659e94ffbe081a0f9ef02a944094866a9bb10f93c99dee90fe9f8569`.
Its three node logs show range exhaustion at height 9 and an attempt duration of
60 seconds; the observation ended before that attempt could expire. These logs
are preserved under `evidence/r3/failed-ci-node*.log`.

The operational observation is now explicitly 180 seconds (three attempt durations),
not a claim of a ten-second block-latency bound. Every node must still advance;
the identity and common-root requirements remain. A fresh local rehearsal passed,
with all heads progressing 8 to 27 and common comparison height 21. See
[R3 result](evidence/r3/result.json). That run used the in-development independent-key
binary whose digest is in the result, via the unchanged label-based path. GitHub
must separately verify the private-branch binary. No failed run is relabeled passed.
