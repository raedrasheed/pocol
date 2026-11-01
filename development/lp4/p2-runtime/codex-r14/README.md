# P2 independent execution continuation — acceptance remains incomplete

Publication prerequisite resolved in R13 at ecc7eda6405b7437d24bf831c32d89f55e3fb106.
Both specifically authorized R12 archives are published unchanged. No repeated owner
consensus/publication approval is needed. M1, LP3, previous receipts, runtime Rust
source/dependencies, resource limits and foundation are unchanged here.

## Actual independent executions

New explicit py-evm profile lp4-create01-modexp-admission adds STANDARD Cancun admission
checks missing from its unsigned/spoof path: EIP2681/3607/1559/3860/4844, intrinsic/block
gas and maximal gas+value+blob affordability. These checks use actual state/transaction
inputs, never expected fixture outcomes or success flags. They do not authenticate
fixture senders or prove signed-wire validation. Old profiles remain separate.

Full20661-case Python run:19486 state/log MATCH,1173 expected-rejection PRESENCE ONLY,
2 BLOCKED_TIMEOUT (loopMul variants0/1) at300s/case;1129.705s. All19486 state/log pairs
exactly equal already published approved R4 receipts, verified case by case.20 actual
validator equality/negative checks and8 Python mutation controls passed. These8 are
interpreted comparator controls, not native compiled mutants. Optional test-only pinned
blake2b-py0.3.2 wheel matches pure-Python compression on160 deterministic boundary
vectors. It accelerates the engine's standard Blake2 path without changing gas or rules.
Historical Python requirements and runtime Cargo dependencies remain unchanged.

Independent native Geth v1.14.12, upstream commit293a300d64be3d9a1c2cc92c26fcff4089deadcd,
ran ALL20661 cases in161.939s:19488 actual state-root/native-log assertions and1173
expected-rejection-presence checks, no failures or exclusions. Both Python-blocked
10-million-iteration cases execute and match here; Python's individual blocks remain
NOT PASSED. Every native case identity/index/expected-exception matches R4 and every
actual valid-transaction root equals that published reference. Native Geth itself
asserts expected logs. Exact rejection/wire equivalence remains a separate limitation.
Two separately compiled Geth source-overlay mutants (MUL→ADD, remove storage collision)
both fail meaningful native state-root assertions after successful compilation.

The first Geth harness attempt failed parsing null (100 files with ZERO Cancun cases).
It is retained as NOT PASSED; native exit was not recorded before that parser error.
The fixed parser accepts null ONLY after proving that file has zero required cases;
a fresh COMPLETE native rerun with exit0/process-output hashes supplies the result.
No compile/import/parser failure is credited as a successful mutation or case.

## Source and tool identity

Official Go1.23.2 release SHA verified. Geth EVM/CLI/source files1998 are unchanged;
only local go.mod/go.sum metadata uses SAME-version official GitHub mirrors because
public source proxies/vanity endpoints returned403. No proxy/policy bypass. Recomputed
canonical Go h1 proves all12 ACTUALLY USED mirrors byte-identical to ORIGINAL pinned
module contents;2 unused declared mappings are explicitly not credited. All86 linked
external module source hashes match original Geth go.sum; actual binary embedded module
set equals target closure. Successful build ran OFFLINE with -buildvcs=false; source/
compiler/binary hashes and failed attempts are retained. This is explicit metadata
adaptation, not a claim of an untouched build file or reproducible upstream binary.

Geth CLI is an external GPL-3.0-or-later reference tool; its library is LGPL-3.0-or-later.
No Geth binary or library is distributed/linked into PoCol here. Source notice inventory
is not a full terms/release audit. Missing bloomfilter v2 packaged notice is supplied
from its exact pinned upstream root MIT grant and original source attribution.
The optimized Python test artifact/source checksum and MIT grant are retained separately.
Full runtime/compiler/test-tool/distribution provenance acceptance remains NOT PASSED.

## Evidence and CI boundaries

Complete new raw engine receipts/stdout/stderr are preserved LOCALLY, with their hashes.
The public equivalence certificates reference ALREADY PUBLISHED R4 per-case roots/logs;
all equalities were individually checked, not assumed from a total. Reproduction uses
pinned public inputs, source/tool identities and exact commands. No new large raw archive
is uploaded or split; no alternate route for a rejected payload is used. The originally
approved R12 payloads remain published. Local preservation receipt records complete bytes.

Run39144253769 at d955756: three jobs SUCCESS, independent old-profile job FAILURE
(73 mismatches,6 timeouts). Native40 per OS, compiled17Registry/16PAY/7CREATE/2rejection
controls,1172 numeric-category+1adapter checks verified. Legacy16PASS/8BLOCKED/2NOT RUN.
All4 ZIP digests were checked. These OLD HEAD checks do not pass the new source head.
New CI retains strict Python timeout/error failure and adds full native Geth execution
and compiled controls; no criterion/failed Python check is silently removed or passed.

PoCol deviation/wire/caps acceptance, full provenance, actual GenesisBuilder/system
execution/settlement/SV6/finalization, general R18/R21/caps64 and persistence obligations
remain mandatory. P2/LP4 and all26 ledger obligations NOT PASSED. Boot false, PR24 draft;
no foundation/main merge, deployment, billing/protection change, Claude or coordinator.
