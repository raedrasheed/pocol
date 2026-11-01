# LP3 handoff at S5 design blocker

Status: S0-S4 accepted/merged according to prior evidence; S5 incomplete; LP3 NOT accepted.

Foundation at start: f933ef8a1e43a22ee6c7ca08c0b14236e086a95f, directly after requested
S4 merge c662bff29fb748594224eb48d329d9b2aff1244e. Branch: codex-lp3-s5-paramgate.
This branch contains reviewed partial R16/R20/R26 arithmetic and a draft checkpoint PR.
Do not create a duplicate PR; discover/update the existing branch PR.

Resume after resolving DESIGN-DECISION.md. Implement the remaining full S5 gate in
this branch: ordered exact R1-R26/UnknownField, no supplied-ok shortcuts, all PG-END,
PG-L and PG-RPC verdicts, measured-fact/code/alloc gating and production R22/R24
restrictions. Test each success/bypass path with independent negative controls. Do not
convert missing facts or synthetic test success into production boot permission.

Then perform a separate sequential review, preserve findings/failures and exact
source-bound evidence, check current PR head/base, review threads, required checks,
rulesets and branch protection, and merge normally into foundation only when S5 is
fully accepted. The current partial PR is not eligible for merge.

Continue S6 (TypeScript arithmetic/timing/assignment model and proper type checking),
S7 (AS1-AS19 exact traces, fresh million-case independent integer differential,
ASQ monotonicity/error, Rust noheap/TS <=512-bit evidence, required release benchmark
methodology and TJ/T0/CF/SV core coverage), S8 adversarial/mutation integration and S9
Arabic/dashboard evidence. Recheck each S0-S8 row before declaring LP3 accepted.
Real builder/system/revm execution remains LP4 and cannot be faked by core tests.

Use the GitHub connector for remote operations. Terminal proxy:8080 is unavailable;
do not bypass it or depend on a hidden token. Do not invoke Claude/coordinator,
merge into main, deploy, change billing or alter protected M1/previous evidence.
User authorization for implementation/testing/review/push/PRs/eligible foundation
merges continues; no routine permission request is needed after the design decision.
