# Independent keys and enrollment: results, 29 October 2026

This increment advances K1/J1 in [the public launch plan](PLAN.md). It is **not a
public launch approval**. No remote servers, public RPC services, real assets or
economic mainnet were deployed. The production profile remains rejected.

## Implemented

- `pocol-key`: OS-random secp256k1 secret generation, exclusive file creation,
  durable writes, public-address-only output and strict Unix file checks.
- Miner/account key-file inputs for the node, transaction signer and site publisher;
  duplicate, missing and conflicting key sources fail explicitly.
- `init-testnet`: constructs genesis from public addresses and allocations, accepts
  an explicit experimental registration-per-block cap, preserves reward allocations,
  derives roots and runs existing admission before writing output. No private keys
  are required or emitted. Public configuration reads are bounded to 256 KiB.
- Negative controls and real four-node integration, plus an exact-head GitHub workflow
  whose artifacts include only public evidence, never the generated secret directory.

## Local evidence

| Check | Result | Scope |
| --- | --- | --- |
| Final release build | PASS | Rust 1.99.1, offline, locked, all pocold binaries |
| CLI key/genesis controls | 5 tests PASS | Includes permissions 0600/0400 and rejected 0644/0700/0000, links, overwrite, malformed keys, conflicting options, duplicate/unknown configuration fields and reserved identities |
| Existing Rust library suite | 51 PASS | Library regression only, not the complete node suite |
| First integration | 5 checks PASS | Independent keys, transfer, non-genesis miner enrollment/mining, duplicate registration rejection, shared roots and restart |
| Expanded integration | 6 checks PASS | Adds independent-key website publishing and byte-identical verified fetch on four nodes |
| Existing translation manifest | PASS | No historical translation evidence silently changed |

The expanded integration compared a common block at height 22, after the fourth
miner registered and produced blocks. Its input is a fresh simnet with a synthetic
difficulty fixture and R_max=1. It deposits the existing profile's 100-token bond
(10^18 base units per test token); this is an exercised test parameter, not an adopted
mainnet economy. A duplicate registration reverted and membership remained four.

Evidence is in `evidence/integration-1/` and `evidence/integration-2/`, with a binary
SHA-256 in each result. These runs used their recorded candidate binaries. After the
second run, permission validation was tightened to exactly 0600/0400 and public
configuration reading was bounded; the final build and all CLI controls were rerun.
`final-source-sha256.json` and `final-binary.sha256` identify that final local candidate.
The exact-commit CI integration is separate evidence; it must be observed passing,
not inferred from earlier local binary results. No counts here are summed as a
single independent security-coverage measure.

On continuation, exact-head CI for `873fa2fdf8064a85a88603818a4ca7c3d29a81c4`
was observed **PASS**, including the build, key/genesis controls and expanded
integration: [Actions run 40869875650](https://github.com/palcertcom/pocol/actions/runs/40869875650).
This resolves that pending validation only. Subsequent compatibility/operations work
is reported separately in [OPERATIONS.md](OPERATIONS.md).

The new enrollment path used the existing registry implementation. No Solidity,
consensus, fork-choice or production-admission rule was replaced or disabled.
The candidate profile changes genesis identity through its public inputs and
explicit R_max; it must not be used to reopen an old data directory.

## Remaining limits

Key files are plaintext, not encrypted wallets or HSMs. Directory protection,
backup and recovery remain operator obligations; Windows key ACL support is absent.
The original private bundle generator still uses disposable labels. Public service
packaging, Internet exposure controls, K_eff/resource obligations, versioned constants,
worst-case calibration, network lifetime/economic decisions, actual multi-host trials
and external security review remain open. Local late enrollment is not a proof of
Sybil resistance, fairness, network-scale liveness or eligibility churn behavior.

The earlier private-network CI failures were investigated separately: receipt races
and a 30-second observation shorter than the 60-second mining attempt. Their original
logs and corrective evidence are retained in [the private preparation report](../private-testnet/README.md).
Extending an operational observation to 180 seconds does not establish a ten-second
block latency guarantee.
