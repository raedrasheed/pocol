# Caps track evidence (CAPS-SUSTAINABILITY, round 2)

These logs support [../../proposals/CAPS-SUSTAINABILITY.md](../../proposals/CAPS-SUSTAINABILITY.md),
which is PROPOSED and not approved.
- They are raw test output, each with a short header naming the tree, the command and the outcome.
- They were run on Linux, in release, with the toolchain from `rust-toolchain.toml` (1.99.1), using this
  track's own target directory under the shared CPU lock.
- A TIMEOUT would be recorded as incomplete. There is none here: the 240 s limits below are test
  assertions, and they fail.

| file | tree | what it shows |
|---|---|---|
| `unfixed-bdb14bb.log` | commit bdb14bb (tests; builder unfixed) | `tests/caps.rs`, 6 of 6 FAIL. The builder offers transactions past the caps ("offers 10 transactions past the room (5, …)", "skips a1 …"). A received over-cap template is locked. The live TX_CAP node includes nothing (`cumTxs` 0 of 3). The live TXBYTES_CAP node does not reach height 8 within 240 s |
| `unfixed-ddf023a-live-trace.log` | commit ddf023a (the live tests log production; builder unfixed), exported tree | the stall traced. At each height the node's own block is rejected "16 caps", attempt 0 expires 74–78 s later, and a fallback block follows at a=1. Alongside it, the TX_CAP starvation: `apply Budget("TX_MAX")`, then an empty template |
| `unfixed-final-tests-reverted-builder.log` | the 9094f98 tree with `pocold/src/node.rs` from ddf023a | the final tests against the unfixed builder: 6 of 6 FAIL |
| `caps-tests-a0dd771.log` | commit a0dd771 | `tests/caps.rs` (6 pass; the `CAPS` lines give template sizes, counters and the live nodes' signed-block record) and `lp4-execution` `tests/caps_identity.rs` (3 pass) in the release build |
| `caps-test-build-a0dd771.log` | commit a0dd771, `--features caps-test` | `pocold` `tests/log_cap.rs` (2 pass; the `LOGCAP` lines give the receipts of the exhausted block and the reward balances 20 blocks later) and `caps_identity.rs` (3 pass, with the same devnet genesisHash as the release build) |
| `suites-release-a0dd771.log` | commit a0dd771 | the full `lp4-execution` and `pocold` suites, release build |
| `suites-caps-test-a0dd771.log` | commit a0dd771 | both full suites in the caps-test build |
| `caps-model-a0dd771.log` | commit a0dd771 | `python3 -I proposals/model/caps_model.py`: 25 checks, and output byte-identical to the committed `vectors/caps-sustainability.json` |

Not recorded: a first run at 182026d. Its test draft was superseded by bdb14bb, on the same unfixed
builder.

No exploit-shaped program was written. The tests are regression tests asserting the conformance, and
every devnet in them is in-process (no network listener).
