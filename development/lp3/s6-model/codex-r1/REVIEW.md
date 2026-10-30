# S6 TypeScript reference model and separate sequential review

Base: accepted amended S5 merge 18cf7ab3f0140715db4365c8f893d46d2084a3f4.
Scope: M3 reference arithmetic/timing/assignment plus full GenesisSpec identity.
No signing, private keys, state authentication, EVM, scheduler, real node or boot.
The approved LP3 amendment and all original deferred LP4 obligations remain explicit.

The model uses bigint for consensus integers, checked input widths and <=512-bit
ASERT intermediates. It exposes dt/e/s/f/F traces, saturates before extreme shifts,
implements inclusive timing/strict boundary predicates, parent membership, sealed
nonce tickets, half-open partitions, owner lookup, exact weights and unbiased Draw.
Draw always hashes real preimages; a separate generic retry sampler supports explicit
injected-digest rejection/counter tests without claiming real SHA preimages.

Genesis framing is iterative and bounded by the same MAX_SPEC_BYTES as the Rust
schema. Complete 52-field schema data is transcribed from the immutable accepted
schema, not inferred from fixtures. Decode follows structure/version/count/int/length/
range/order/reserved priority. Encode is canonical; identity uses an independently
implemented Keccak sponge with K1-K3 and the frozen three-library GSV1 hash. This is
not SHA3-256. Full valid corpus reencoding/hash identity is compared to Rust and the
preserved Python specification oracle. Error comparisons claim codes, not identical
human diagnostic details for every framing error. Identity bootable is always false.

## Actually executed evidence

- Eight named test groups: AS1-AS19 targets/trace controls, 1027 frozen ASERT rows,
  timing/TJ1-4, assignment/hash/partition edges, weighted/rejection/counter controls,
  Keccak/GSV1 identity, full schema negatives, all L4n/order supplements and 26 deep/
  shallow variants of the 13 accepted malformed-structure cases at depth 1500.
- Separate Python/native-integer ASERT: 12,000 cases s in [-300,300], zero TS trace
  mismatches, all Rust outputs matched, maximum intermediate 512 bits.
- Independent Python/hashlib assignment: 697 complete scenarios and 7977 sampler
  cases, zero mismatches. Includes 65535 members, over-bound rejection, u64 endpoints,
  permutations, exact weight/CDF/rejection boundaries and supplied-parent errors.
- Timing: 10,000 seeded boundary/error cases, zero mismatches.
- Genesis: 1200 cases (866 valid, 334 invalid) agreed with preserved independent
  Python and Rust; valid bytes and Keccak hashes matched exactly.
- Twelve isolated single-fault controls all detected through failed assertions:
  negative truncation, polynomial, biased sample, parent weights, exclusive cutoff,
  partition gap, reserved id acceptance, equality deadline, timestamp limit, final
  retry counter, schema widths and SHA3 padding instead of Keccak.
- Proper TypeScript 5.9.3 strict checking passed both in GitHub Actions on Node
  22.18.0 and locally using the checksum-verified compiler artifact from that run.
  Node 24.19 local stripping runs are additional runtime checks, not type checking.
  package-lock pins all three tool packages. Licenses and artifact digest/run are
  in TOOL-PROVENANCE.json; no compiler/node_modules/cache is published in the repo.

Review occurred sequentially after the authored implementation commit 577488c.
The Python arithmetic/timing calculations import neither authored model. The Genesis
reference is the preserved, independent M1 specification tool, read-only. The S4
Python/hashlib oracle was reused unchanged in arithmetic, adapting only invocation
and output paths. It remains independent of both TS and Rust. This is a separate
Codex pass, not another external reviewer identity or a formal GitHub approval.

## Initial failures retained, not passed

The first CI run 39038444826 at d4e301b failed strict type checking: a never-returning
arrow helper did not narrow the RLP union, cascading into implicit-any errors. A named
never-returning function fixed the narrowing without relaxing strict/noEmit checks.
Node 22 tests in that failed job were NOT RUN. Subsequent CI passed. Sanitized exact
type errors are in typecheck-initial-FAIL.log; the complete run remains on GitHub.

An initial local node --test invocation reported only a file-level test in this
environment; it is not counted as seven checks. Direct execution through node:test
was used and its explicit seven/eight counts checked thereafter. The deep corpus
adapter initially forgot gsv1Append and failed G-extraTop; the adapter was fixed to
faithfully apply placements, edits and final truncation, with every byte length
checked. Original failing test source/log are preserved.

The first final-counter mutant threw the expected runtime error, but the review
detector required AssertionError; adding an explicit doesNotThrow assertion made the
intended success contract and mutant detection unambiguous. The initial ledger is
preserved, and all twelve final controls were rerun successfully. No unmodified
final-source implementation failure is observed. Earlier source/type/test failures
must not be relabeled as successful checks.

## Reproduction and acceptance limits

Build the unchanged Rust binary using Rust 1.58.1/offline locked dependencies, and
install the exact committed TS lockfile or use the recorded compiler artifact. Run:

    python3 development/lp3/s6-model/codex-r1/verify.py /absolute/fresh/output

The runner records versions, commands, raw exits, source digests, proper type checking,
eight tests and all independent comparisons/mutations. A final reproduction of this
published runner passed; raw records are in reproduce-final. The Rust binary used is
the unchanged S5 source build; no real EVM or chain execution is implied.

S6 reference scope is technically satisfied with no unresolved implementation finding.
Check the latest PR head's CI/review/protection requirements before normal foundation
merge. S7 million-case acceptance, ASQ/ASB release gates, remaining T0/CF/SV scope and
S8/S9 final audit are pending; this document does not accept LP3.
