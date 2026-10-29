# LP3 S4 implementation and separate review — 2026-10-17

## Scope and exact revisions

Fetched foundation and verified owner-specified merge `866666e2f129c26575409d07d8b7b4fa11899f78`, parents 30daceb7 and be134089, tree identical to the S3 PR head. New source branch `codex-lp3-s4-assignment`, draft target `work/lp3-consensus-foundation`. Tested implementation **71cbbcc5cfed12eda204e29579f31cf672e7d972**. Later evidence/publication commits do not change this implementation.

Read repository development/README.md and accepted LP3-MILESTONE.md; no AGENTS.md exists under /workspace. The current owner's Codex-only/no-coordinator instruction supersedes historical workflow text. No accepted specification was amended. Reference hashes are in RESULTS.json.

S4 is membership from parent state, sealed-parent nonce seed/tickets, bounded/unbounded nonceMax, partition/owner computation and proposer Draw. This increment exposes pure library APIs. A ParentView supplies height, seal and ActList records from an already validated parent; ShareCount supplies parent-ledger counts. The APIs do not authenticate those facts, execute registry transitions, make signatures, or issue a state-assignment certificate. No S5/ParamGate work or boot authorization was added.

## Implementation and separate source review

- Parent height must be h-1. ActList fits supplied M_max (u16); populated identities must be unique and nonreserved. A_h filters tombs and uses lastShare>=h-W_act OR graceStart>=h-W_act. If empty, fallback uses all nontomb ActList members. Saturating the cutoff at zero is equivalent over nonnegative block heights. No extra lastShare/graceStart<=parent-height rule is invented: the function applies the stated formula to supplied values; actual state validity belongs to the parent validator. Empty results are representable; assignment/Draw return EmptyMembership so the caller can stop.
- Membership stores a snapshot of child height and parent seal privately. Nonce assignment cannot accidentally accept a different seal/height as a separate parameter. seed=SHA256(PoColNonce || seal_p || be64(h) || be32(a)); ticket=SHA256(seed || id20). Golden preimages are checked with independent Python hashes. Tickets sort ascending; distinct-ID hash collisions return TicketCollision because M1 supplies no collision tie policy. Under accepted A1, this is a fail-closed defensive case, not an invented consensus tie rule.
- nonceMax=min(2^64,c*floor(2^256/(target+1))) in bounded mode; 2^64 in unbounded mode. target=0/c=0 are invalid input parameters. Existing exact work() is reused only for target>0, avoiding its target=0 clamp. U512 product is capped before narrowing. Tests straddle exact 2^64, 2^64-1, minimum/maximum target and c=255.
- Ranges use [floor(i*nMax/n),floor((i+1)*nMax/n)) rather than subtracting one from an empty range. Private endpoints and n<=65535 ensure multiplication fits u128, bounds tile [0,nMax), and inclusive_end is None for empty ranges. Full 2^64 is representable; every u64 nonce is eligible in unbounded mode. owner returns None out of range and the unique matching owner otherwise.
- Draw sums counts for A_h* only, on exactly [h-1-W_w,h-2], excluding the parent block h-1. Missing/zero histories cause uniform fallback only when all weights are zero; zero-weight members otherwise cannot win. Repeated (height,id) entries are rejected because ShareLedger is a map, not an additive event stream. Counts are u64 and sums checked u128; valid typed window/member bounds fit this representation. Members remain in sorted address order; choose uses first cumulative sum strictly greater than index.
- Rejection sampling accepts x<floor(2^256/W)*W and returns x mod W. It computes R=2^256 mod W using existing U512 division, so its inclusive last accepted digest is U256::MAX-R. All 256 digest bits participate. x_k=SHA256(PoColProposer || seal_p || be64(h) || be32(0) || be32(k)), starting at zero. Rejected hashes advance a checked be32 counter; exhaustion returns an error, never modulo-fallback or wrap. Private test-only injection exercises rejection and exhaustion; production Draw always uses SHA256.

Separate review after the implementation commit inspected each formula, narrowing/overflow edge, immutable-state boundary and hash preimage against consensus.md §§2/5/6 and execution.md recordShares(h-1) (including its winner +1 semantics upstream). The independent Python oracle imports no repository model, uses hashlib and unlimited integers, and compares complete membership/seed/ticket/range/owner/weight/choice/Draw outputs plus direct sampler boundaries. No blocking implementation finding remained. This is a separate Codex review pass, not a different external reviewer identity or formal GitHub approval.

## Fresh evidence and accurate outcomes

| Check | Outcome |
|---|---|
| Rust 1.58.1/Cargo 1.58.0 offline locked build and rustfmt | PASS |
| Regression: 50 lib, 5 adversarial, 8 assignment, 12 fixtures, 16 GenesisSpec, 6 S2, 6 S3 | PASS: 103 tests |
| New S4 surface | PASS: 8 integration tests plus 1 unit rejection/retry/exhaustion control (already inside the counts above) |
| Seeded partition properties | PASS: 256 configurations within one integration test; small nonce spaces enumerated |
| Independent Python/hashlib/native integer comparison | PASS: 697 complete scenarios, 7977 sampler cases, zero mismatches |
| Input limits and domain edges | PASS: ActList size 65535 and 65536 rejection; empty sets; reserved ID neighbors; h=1/2/u64::MAX; u32 attempts; 2^64 nonce endpoints; weight-window/CDF/rejection boundaries |
| State/history enumeration permutations | PASS: 40 extra corpus variants |
| Eight isolated single-fault controls | PASS: all caught by failed assertions, not compiler failures |
| Fixture CLI | PASS: window 22, ASERT 1027, hashes 3677, K1-K3 |
| M1 and all prior evidence, .github and dependencies | PASS: unchanged from foundation; preservation.json |
| Initial reviewer probe compilation | FAIL, corrected: tool error only; original source/log retained |
| New Windows execution and Windows-only storage/crash checks | BLOCKED BY PLATFORM; not counted PASS |
| Heavy unchanged S1/S3 suites, live registry/ledger authentication, EVM/boot, S5 | NOT RUN; outside this increment or unchanged evidence reused |
| Real SHA collision generation / actual 2^32 rejected hashes | NOT RUN; private injection tests error paths, A1 assumed |

The original review tool failed on a JSON macro array expression and ambiguous integer comparison under Rust 1.58.1. After correction it compiled and all oracle scenarios matched. Raw initial compiler failure is retained as oracle-compile-initial-FAIL.log with oracle_probe-initial-FAIL.rs. Passed implementation regressions were not rerun for this tool-only fix. The command ledger retains the interrupted attempt and resumption (including a harmless repeated rustc version query). No unmodified-source test failure at 71cbbcc5cfed12eda204e29579f31cf672e7d972 is observed. Initial precommit test development also corrected an empty-slice type-inference assertion before this tested commit; that was not a runtime test pass.

Mutations: exclusive rather than inclusive membership cutoff; allow tombs; little-endian attempt seed; ceil/gap partition; include h-1 counts; non-strict cumulative boundary; modulo without rejection; wrong 64-bit cap. Each scratch checkout has a freshly created unique target directory inside its temporary root, deleted afterwards. Two variants run in parallel with two Cargo build jobs each; five CPUs and available RAM/disk were checked first. Raw mutation cargo exits 101 are expected failures of faulty code; successful detection is PASS. Author and clean review targets are separate and never used by mutants.

## Reproduction and preservation

Use the saved scripts oracle.py, oracle_probe.rs and verify.py under /workspace/pocol-s4 with a fresh /workspace/pocol-s4/evidence directory and repository at /workspace/Proof-of-Collaboration. verify.py is the reproduction runner and records exact commands, working directories, unique targets, raw exits and durations. It includes a source/helper-bound resumption guard added after the interrupted tool-compilation attempt; a changed source/helper requires fresh evidence. On a clean rerun no initial helper failure is expected. The original failed probe is kept separately for reproducing that diagnostic. Source-bound marker added for future resumption does not replace the recorded fresh regression and oracle executions.

Tool homes: CARGO_HOME=/workspace/pocol-tools/cargo, RUSTUP_HOME=/workspace/pocol-tools/rustup. Unchanged checksum-verified vendored dependencies and lockfile are used. The standalone Rust probe links serde_json from the clean target as an existing dependency; no manifest/lockfile edits. Only scripts, text logs/JSON and checksums are published; no keys, binaries or build caches.

preservation.json records every tracked prototype's SHA256 and protected-tree comparison against the merge base, including all M1 and LP1/LP2/S1/S2/S3 evidence. Existing source (other than lib.rs exposing assignment) and every prior acceptance artifact are retained unchanged. The appended Arabic dashboard is the only modified existing documentation.

## Acceptance limits

Technical result: the documented S4 pure-core criteria are met under accepted A1; **no known unmet arithmetic/API criterion was found** in this pass. This remains a draft increment for review/owner acceptance, not complete LP3. Explicitly unverified: live parent-state/ledger provenance, node/EVM/signing integration, new Windows execution, external reviewer/GitHub approval, and behavior beyond A1 such as real hash collisions. Successful computation must never be presented as proof of parent state or permission to boot. Counter exhaustion has an explicit error path; no invented biased fallback or collision tie policy is accepted. No merge and no S5 were performed.
