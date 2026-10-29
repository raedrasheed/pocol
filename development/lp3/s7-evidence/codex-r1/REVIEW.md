# S7 numerical evidence and pure SV core — partial acceptance checkpoint

Base: accepted S6 merge 772cb1f843c9214b9a8575856d4b34e38b7976ae.
Verdict: numerical/resource gates PASS; **S7 incomplete; do not merge this increment**.
CF-DECISION.md documents the genuine missing canonical test binding. T0's final
core-scope audit and S8/S9 remain pending. LP3 is NOT accepted.

## Implementation and separate review

Rust AsertOut now carries the actual internal delta_t/e/s/f/F trace. The arithmetic
algorithm is unchanged; early exits expose factor=None and still avoid multiplication
or huge shifts. A named integration test checks all AS1-AS19 targets and trace fields,
including floor-negative, exact factor constants and early saturation boundaries.

Rust and TS add pure SV share/split arithmetic with 512-bit Rust intermediates and
bounded counts (<=256 explicit shares plus implicit winner). Duplicate/zero/excess
counts and invalid split parameters reject. Tomb and account inputs are supplied
facts; neither model authenticates parent state, pays real accounts, executes Escrow,
or certifies execution. SV1-SV5 and SV7/SV7s arithmetic is exercised. SV6 invalid-pot
transition and all actual settlement/finalization remain NOT RUN/LP4. A trivial
numeric identity is deliberately not counted as executing SV6.

The sequential separate Codex review checked traces, narrowing, ceil/floor behavior,
U512 products/division, sum conservation, burn precedence, maximal 256-bit amounts,
bounded input lengths, reproducibility and source preservation. Source is recorded
at local commit 72c32ae with full SHA256 manifest. This is not another external
reviewer identity or a formal GitHub approval.

## Actually executed results

- 120 Rust regression tests passed; all prior schema/profile/timing/assignment/gate
  cases included, plus named AS traces and four SV arithmetic tests. Node's nine
  named groups and strict TypeScript 5.9.3 checking passed locally.
- Independent Python unlimited-integer oracle compared **1,000,000 full trace/target
  cases** with both Rust and TS: zero mismatches, all 601 integer s values [-300,300]
  represented, maximum intermediate width 512. Streaming batches of 10000 bound memory.
  Full-input/output hashes, every batch hash and exponent counts are in million.json.
- ASQ: all 65536 fractions, TS actual polynomial output and independent Decimal-60
  reference, identical factor digest. Maximum relative error
  0.00011729562395226757019219105560017978303453583450443093433 < 0.0002.
  F and target are nondecreasing across the fraction grid; 1536 adjacent exponent
  boundaries for three representative targets were checked. Monotonic floor/clip
  operations preserve monotonicity between those verified factors/boundaries.
- Rust allocation counter: 1069 calls, zero allocations, positive allocating probe
  observed, maxShiftBits=512; debug and release runs passed. This is actual counter
  evidence, not an inference from lack of Vec in the function.
- Final benchmark harnesses ran sequentially after other authored CPU jobs completed:
  10000 warmup, 11 rounds x100000 calls, median ns/call; raw min/max batch means retained.
  Rust AS1 median 78.22946 ns; max AS14-AS19 median 91.97339 ns (ratio <=1.176).
  TS AS1 median 1836.45895 ns; max extreme median 1747.3309 ns (ratio <=0.952).
  Both satisfy measured median <=1ms and <=10xAS1 under this stated methodology.
  These shared-host batch means are not a worst-case individual-call proof or a
  Windows/other-device result. See BENCH-METHOD.json and final logs.
- Independent settlement oracle: 4000 random/boundary distributions matched Rust
  and TS, including maximal pot, zero/low/full counts, burn routes and conservation.
- Eight isolated Rust mutation controls detected wrong trace, truncation, polynomial,
  denominator, tomb/account route, round burn and split divisor through assertions,
  after each complete faulty library/test compiled successfully.

The million run used the same ASERT algorithms throughout. Unrelated SV functions
were appended to the TS module while that run was in progress; the exact ASERT prefix
is verified unchanged from accepted S6. million-source-note.json records this limit
and prefix digest; it is not represented as a second million run. Exploratory timing
logs could overlap other jobs and are not the final acceptance statistic. Final
benchmarks were rerun in sequence after the workloads. Earlier root-directory fmt
tool invocations lacked a manifest; corrected explicit-manifest/cwd checks passed.

## Scope that must remain visible

Deterministic input jitter/TJ1-TJ4 evidence is preserved from S3/S6. No physical
consensus scheduler was executed; callbacks/timers are LP6 by the accepted boundary.
Header/codec evidence from LP1 is not blanket acceptance of literal T0 V1-V26;
an explicit final T0/core audit remains pending. CF1-CF6 have no canonical definition
in the preserved inputs; proposed local labels cannot be counted as original passes.
S8 integrated adversarial acceptance and S9 final matrix audit remain pending.

M1 and all old evidence/workflows/dependencies are unchanged. The approved amendment's
original general R18/R21 and caps/64 minimum obligations remain DEFERRED LP4, NOT PASSED.
Actual EVM/SYS/builder/retention measurements/proofs and boot remain unperformed.

Reproduce numerical checks with Rust 1.58.1 cached locked dependencies, Node and the
committed exact TypeScript compiler tools:

    python3 development/lp3/s7-evidence/codex-r1/verify.py /absolute/fresh/evidence

This reproducer does not resolve the named CF acceptance question or accept LP3.
