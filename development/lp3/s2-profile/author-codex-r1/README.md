# LP3 S2 — typed CP and non-bootable profile identity (Codex author round 1)

Status: implemented and author-tested; **not independent acceptance**. Codex performed both implementation and a separate self-review pass; no Claude/coordinator or second reviewer was invoked. S0/S1 acceptance and all earlier evidence remain unchanged. This is a draft increment, not full LP3.

## Selection and exact source

The owner confirmed PR #10 merged. Fresh Git fetch verifies merge commit `d17273af35880f0e1d8a438e529e69c22a18f731` on `work/lp3-consensus-foundation`, parents fd7a17f and bd2ae28, with a tree identical to the accepted PR head. API metadata remains unavailable. MERGE-VERIFICATION.json records the evidence.

`development/LP3-MILESTONE.md` lists S2 as the next incomplete criterion after accepted S0/S1. Implemented source commit: `a9f0460335fa9594b943e125dd77d780042eefec`. Source branch: `codex-lp3-s2-profile`; intended draft PR target: `work/lp3-consensus-foundation`.

## Implementation

- ConsensusParams exposes all 52 CP values with their specified u8/u16/u32/u64/U256 widths, and checked conversion from/to wire-order values. Schema width/minimum/enum/gamma checks precede narrowing. This reuses accepted CP_SCHEMA; no schema or accepted decoder change.
- ForkSchedule requires initial (1,0), strictly increasing heights and only implemented versions. v1 is the only implemented version; TestFork-v2 is rejected even in simnet. Querying at activation boundaries uses the last entry at or before the height. Repeated v1 entries at distinct heights are allowed; no new protocol version is invented.
- ProfileIdentity bounds input, decodes/binds full GenesisSpec hash and advertised chainId, gates runtime/profile kinds, and validates the fork schedule. Named network identity is chainId 777001 per governance.md; simnet/synthetic identities cannot reuse it, and production Node rejects either non-network kind. Synthetic denotes local fixture classification, not a new consensus profile format. Kind enums are explicit internal API types; no external JSON loader is added.
- Validated fields are private and exposed by immutable references. Every successful result reports bootable=false. allocRoot/sysCodeHash are hash-bound identities only; no caller-provided boolean can assert builder verification.

Scope: these are library APIs, not a new CLI/startup path or full ParamGate. Cross-field R1–R26, measurements, real alloc/system-code builder outputs, and executable boot permission are deferred to their existing gates. S2 is proposed for review; contribution 1 still needs timing/membership/nonce/Draw APIs.

## Verification and review

Rust 1.58.1 / Cargo 1.58.0, offline locked unchanged dependencies. commands.json records exact commands, directories, exit codes and durations. `cargo build`, six S2 integration tests, and `cargo fmt -- --check` passed. Tests cover all CP widths/minima/upper bounds, exact roundtrip at maximum widths, gamma coupling, schedule shape/unsupported versions/boundaries, 30 kind/node/chain combinations, hash/chain/root tampering, empty/oversize input and non-bootability. Existing S1-heavy tests were not repeated: their source is unchanged and they do not exercise this new module.

Five single-fault controls in temporary copies were all detected by assertion failures (not compiler failures): allow production simnet, allow duplicate heights, accept unsupported versions, bypass genesis hash, claim bootable=true. See mutations.json and mutant logs. These are negative-control successes, not five unexplained test failures. Temporary code was never copied into the repository.

Self-review checked the 52-field mapping against consensus.md/CP_SCHEMA, narrowing only after bounds validation, immutable validated schedule/profile state, R22 gate matrix, hash/chain binding and absence of a boot authorization path. No remaining implementation finding was identified in this pass. This is not an independent review; acceptance remains pending.

source-and-preservation.json records the exact source hashes and verifies no changes to M1, LP1/LP2 evidence, S1 evidence, owner decisions, or .github relative to the merge base. Main's strict workflow was left untouched; reconciliation before eventual main integration remains required. Cargo manifests/lockfile are unchanged. Windows execution of this new increment was not performed.

## Reproduction and remaining work

With the saved environment toolchain variables active, from prototypes/lp1-node run `cargo +1.58.1 test --offline --locked --test profile`, `cargo +1.58.1 build --offline --locked`, and `cargo +1.58.1 fmt -- --check`. verify.py is the actual author-run orchestration script; its absolute root/output paths describe this cloud instance. Mutation copies are created under /workspace/pocol-s2.

Independent S2 review and any resulting corrections remain pending. S3–S8 and full contribution/LP3 acceptance remain incomplete. No new increment or main integration is authorized by this report. GitHub API remains proxy-blocked; a pushed branch is not proof a draft PR was created.
