# Separate sequential author review — R3, not independent-human approval

Reviewed after implementation and scoped tests. No Claude/coordinator/subagents.

## R2 hosted results

Run39407652538 source6ce5b5acab41c57da0ca1eb4ba62682185c6e359,
tree e0c6989a6ad99c48a97935edd1891261289e3e07: five successful jobs.
All five artifact ZIP digests/bytes verified against GitHub. Final artifact
12259641359 SHA256 d268f4a1c285ef7a65cee9a18669f949f19c2015e8a781073ad256cfbf2a465b.
All2807 frozen source hashes and20661 unique expected identities verified;
19488 primary roots/logs exactly equal frozen post values and published R4 bytes;
1173 expected rejections remain second-engine presence-only. No extra/missing/
excluded/blocked/incomplete/mismatch/engine error; incomplete.jsonl is empty.
Linux and Windows64 Rust tests each pass. Legacy16PASS/8BLOCKED/2NOT RUN retained;
job exit0 does not pass those ten checks. Full raw ZIPs/logs archived locally.
These results do not verify later R3 source or full P2/P3/LP4 acceptance.

## Profile/RPC/runtime review

M1 execution.md binds CHAINID to cfg.chainId; accepted LP3 distinguishes network
777001 from synthetic CI1 chain777902. Fixed777902 wrappers remain historical
controls. New signed/profile path uses the same immutable, schema/hash-bound LP3
ProfileIdentity for signing-domain recovery and EVM config. No arbitrary caller
chain/sender/success flag substitutes for it. Five accepted LP3 source modules
are byte-identical copies, enforced by CI cmp, with the existing four P1 copies.
Pinned original fixture hash checked;9 independently signed actual executions
cover legacy155/2930/1559 at777001/777902/max-u64, with complete state roots,
receipts and gas compared to unmodified CancunVM. No new runtime dependency/lock.

The experimental pool holds exact authenticated envelopes, enforces per-profile
TX_MAX, raw quota equality/+1, idempotent duplicates and actual native validation.
Duplicates are revalidated too; reorg recheck clears old entries before validation.
Only currently executable snapshot transactions are queued (no future nonce queue
or replacement); this is documented conservative service policy, not consensus.
The quota counts RAW envelopes, not all decoded/allocator memory: complete64MiB
memory proof remains mandatory P7/M7. No full pool/resource acceptance claimed.

Actual strict JSON/C19 router and loopback HTTP exercise eth_sendRawTransaction
wrong-chain/type/unprotected−32015, chain identity, result hash and malformed input;
no caller snapshot or rejected pool writes. Bounded derived LP1 transport accepts
only literal loopback binding/Host and rejects Origin because no extension is
configured. No default server binary, external listener, activation or broadcasting.
M7 RpcGuard/pacing/batches/extension setup/full RPC remain unimplemented. Native
profile USER path also binds chain and gas limit to the same identity; complete
stored-header/context/phase/final-root admission is still P5/P9, not implied.

## Signed negative comparison

Every one of the1173 frozen GST rejection identities is reconstructed as real
signed wire data from PUBLIC synthetic fixture keys, which are used only in memory.
Exported input is explicitly whitelisted raw/pre-account/env data; no original
transaction/key object, credential or real account is exported. Rust actually
recovers the sender:1014 execution categories+independent input predicates,
157 PoCol type3 rejections (not original upstream category),1 priority wire rule,
1 actual over-u256 wire integer. Public per-case certificate includes all1173
identities, indexes, expected labels, wire hashes and actual outcomes in one file.

Unmodified signed py-evm accepted73 negatives and rejected1100; its signed audit
remains NOT PASSED. No supplemental checker is mislabeled unmodified. Added a
separate Geth1.14.12 path with actual UnmarshalBinary/TransactionToMessage/recovered
sender, standard Cancun state transition, actual error sentinels and rollback.
The state transition alone accepted2 oversized-blob cases: preserved this failure,
then added Geth's actual VerifyEIP4844Header with an explicitly synthetic parent
witness. This is the missing independent header layer, not a source patch or
recovered original parent. Final independent Geth rejects all1173; every1014
supported native execution case matches its expected error category, sender/hash
and restored snapshot. Global original20661 upstream controls remain unchanged.
This still does not claim full transaction error precedence or full PoCol apply.

All1998 Geth source files match the pinned module ZIP; only pre-existing go.mod/
go.sum mirror metadata differ locally, per earlier independent mirror proof.
The new handwritten test helper builds in a separate module, not in the protected
upstream source. Tool/helper/binary digests and build recipe recorded; no Geth
runtime linkage or binary distribution. Go compiler/source proofs remain explicit.

## Tests, controls and findings

86 final Rust tests passed, no ignored/filtered full-suite checks or warnings.
Re-ran13 compiled wire and6 USER controls after profile changes;10 new profile/
pool/RPC controls and3 independent Go signed/header controls compiled and failed
meaningful assertions. Build/import/link errors are never successful controls.

Preserved failures/fixes: moved common fixture include paths under support correctly;
local loopback was initially denied by the worker sandbox, then actual tests passed
with the supported additional-network permission, not a skip. A consumer was started
before signed-input production completed: the strict length mismatch failed; rerun
only after producer exit0 covered1173. Full Rust link failed when compiler caches
filled the32GiB workspace; removed ONLY reproducible incremental cache (12GiB),
retained every source/input/log/receipt/archive, and reran full tests successfully
with incremental caching disabled. No tests or resource limits were reduced.

CODE-CAP-DECISION.md records a genuine unresolved limit/accounting interpretation:
CP B_code_max32768 versus Cancun runtime24576, and revm's coupled initcode limit.
All16 diagnostic rows are NOT approved rules/PASS. No code-size override or new
CODE_CAP accounting has been installed. A consolidated owner answer is required;
other profile/signed/PAY/source/evidence work remains authorized.

All26 LP4 obligations, P2/P3/LP4 remain NOT PASSED; boot false. No M1/LP3/prior
approved decision or ceiling changes. PR25 stays stacked draft; never merge into
P2, retarget foundation only after accepted P2 merge and then satisfy all P3/GitHub
requirements. No main/deploy/billing/protection bypass. R3 hosted checks still
must run on its own source; R2 success cannot be inherited as R3 PASS.

Source-only whitespace check passed. The original rust-tests.log retains its
SHA-bound final blank line; whole staged whitespace check reports that one EOF
warning. The imported evidence bytes were not normalized or silently hidden.
