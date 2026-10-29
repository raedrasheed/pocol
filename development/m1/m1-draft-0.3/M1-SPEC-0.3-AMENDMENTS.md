# M1 — Specification Draft 0.3: Amendments to Draft 0.2

Status: **review draft, NOT approved.** No production contract, node or extension code exists. Nothing was deployed and no transaction was sent.
Baseline: `../reference/FINAL_DESIGN.md`, design ID `e8a19ecb306c7b955cddc75b050fdda95f9829c0e2b5cc06563cb27dba851aa0`. The baseline is unchanged and authoritative. For the design ID and the raw file SHA-256, see R3-09.

**How to read this document.** Draft 0.3 is draft 0.2 (`../m1-draft-0.2/M1-SPEC-0.2.md`, unchanged) plus the amendments below. Each amendment names the 0.2 section it replaces or extends. Text that is not amended stays in force, with the same B/P/U/E labels. Draft 0.2 files are not edited. Revision IDs (R3-nn) are stable so that reviews can cite them.

| ID | Coordinator issue | Subject | 0.2 sections affected |
|---|---|---|---|
| R3-01 | C03 | RLP decoding is total and iterative; child-boundary precedence | §3.2 `decode` row, §3.4 |
| R3-02 | C04 | Lexical `@v` selector validation | §6.2 step 2 |
| R3-03 | C05 | Actual requests, unique keys and the shared session ChunkCache | §3.2 notes, §3.4 `fetchCalls`, §8.2 |
| R3-04 | C06 | Snapshot mechanism not frozen; mock scenarios; baseline reconciliation | §4.2 |
| R3-05 | C09 | Integer widths: manifest version u8; version IDs uint32 | §2 P02, §5.2, §5.3 |
| R3-06 | C09 | T1-10 and T1-12 corrected; T1-16 added | §8.1 |
| R3-07 | C01 | Annex batch 1 (partial): X1 preimage, V1 RW/HC transcription | `M1-ANNEX-INVENTORY-0.3.md` |
| R3-08 | C01, F26 | M1-spec acceptance separated from implementation experiments | §9 |
| R3-09 | C08 | Design-ID provenance statement and probe | header, §10 |
| R3-10 | C02 | Tooling that runs under the coordination interpreter | §10, `README.md` |

---

## R3-01 — RLP decoding (C03; replaces the `decode` row of 0.2 §3.2)

**P31 (new). Totality.** For every input of at most 65536 bytes, the decoder returns either one decoded item or exactly one `decode.*` rule. It never fails with an unhandled condition such as stack exhaustion, an exception or a timeout. `decode.oversize` is checked first, before any decoding, as in 0.2.

**P32 (new). No depth rule.** RLP nesting depth is not limited by any rule. The byte limit already bounds it: the deepest canonical input of at most 65536 bytes is 21916 nested empty lists, exactly 65536 bytes long (`vectors/rlp-adversarial.json`, `depthFacts`). Such input is valid RLP. It is rejected later by `struct.shape`, because the manifest schema needs at most 5 list levels. No depth limit is introduced, because that would reject valid RLP, which the task forbids without a baseline-compatible proposal. An implementation must decode depth 21916 correctly. It meets this by using an explicit stack, or by proving that its native stack is large enough. The baseline already requires the same property of the extension's JSON parser: "iterative, not recursive, linear cost" (FD:L1245). This draft applies that property to RLP as a proposal; the baseline does not state it for RLP.

*Alternative, not adopted (would need a decision):* schema-guided decoding that rejects a list at depth 6 or more as `struct.shape` during decoding. It would bound memory at 5 frames. It would also change first-error precedence, because a structure error could be reported before a later `decode.*` error in the same input. That breaks the 0.2 rule that `decode` completes before `structure`.

**P33 (new). Boundaries and precedence inside `decode`.** Decoding is one left-to-right pass. Each item is checked against the **extent of its innermost enclosing item**: the declared body of its parent list, or the whole input at the top level. Checks for one item header, in order:

1. a header byte exists before the end of the enclosing extent, else `decode.truncated`;
2. for long forms, the length-of-length bytes lie inside the enclosing extent, else `decode.truncated`;
3. the length has no leading zero byte, else `decode.noncanonical`;
4. a long form encodes a length of at least 56, else `decode.noncanonical`;
5. the body lies inside the enclosing extent, else `decode.truncated`. A child that crosses its parent's end is rejected here, before any of its body is read;
6. for a string of length 1, the byte is at least 0x80, else `decode.noncanonical`.

Children are decoded in order, depth first. After the top-level item, any remaining input is `decode.trailing`. The first violation in this order is reported.

**Correction to the 0.2 tooling.** The 0.2 decoder bounded a child by the end of the *input*, not the end of its parent. It reported a different first rule when the crossing child also had a non-canonical body. The precedence fixtures in `vectors/rlp-adversarial.json` record the 0.2 behaviour for each case:

| Fixture (literal hex) | 0.3 first rule | 0.2 tooling |
|---|---|---|
| `c18105` | `decode.truncated` | `decode.noncanonical` |
| `c1b801` | `decode.truncated` | `decode.noncanonical` |
| `c2c28100` | `decode.truncated` | `decode.noncanonical` |
| `c2c28080` | `decode.truncated` | `decode.truncated` |
| `c58105` | `decode.truncated` | `decode.truncated` |
| 1201 nested lists (3391 B, the Codex probe size) | `struct.shape` | uncaught `RecursionError` |
| 21916 nested lists (65536 B) | `struct.shape` | uncaught `RecursionError` |

Also included: deep input followed by a trailing byte (`decode.trailing`); deep input truncated by one byte (`decode.truncated` at offset 0, before descending); a non-canonical item at depth 1201 (`decode.noncanonical` at offset 3392); a deep version field inside an otherwise valid manifest (`struct.shape`); and 21917 levels at 65540 bytes (`decode.oversize`). Every one requires zero `eth_getCode` requests. `run_checks_03.py` decodes the 65536-byte case with the Python recursion limit set to 120, to show that the 0.3 model does not depend on native recursion.

The existing 0.2 decode fixtures keep their first rules under R3-01. `run_checks_03.py` re-checks every 0.2 negative fixture against the 0.3 model.

## R3-02 — `@v` selector (C04; replaces 0.2 §6.2 step 2)

2. In the remaining component only, a trailing `@v` followed by one or more ASCII digits `0-9` is a version selector. Unicode digits are not digits here. The digit string is validated **lexically, before any conversion to an integer**. It is `malformedSelector` if any of the following holds:
   - its first character is `0`, which covers `@v0` and every leading zero;
   - it has more than 10 characters;
   - it has exactly 10 characters and compares greater than `4294967295` as a string.

   If none holds, its value is at most 2³²−1 and is converted. A component ending in `@v` with no digits is also `malformedSelector`. A malformed selector sends no RPC and renders no frame.

Rationale: an unbounded conversion is neither total nor portable. Draft 0.2 raised Python's `ValueError` at 4301 or more digits (Codex probe). Fixed-width parsers in other languages wrap, so `@v4294967297` or `@v18446744073709551617` would select version 1. Fixtures (`vectors/selector-cases.json`, 17 cases):
- the u32 boundary: 4294967295 is accepted, 4294967296 is malformed;
- wrap traps for u32 and u64;
- 10- and 11-digit cases;
- leading zeros;
- 4301 and 5000 digits;
- 5000 digits followed by a query, and 5000 digits inside the query only;
- an Arabic-Indic digit and a full-width digit.

All 0.2 path cases are re-run unchanged.

## R3-03 — Actual requests and the session ChunkCache (C05; amends 0.2 §3.2 notes, §3.4, §8.2)

**B (made explicit).**
- The site session owns a ChunkCache (FD:L1300, FD:L4932).
- The cache key is (chunk address, length) (FD:L1309).
- `ChunkFetcher.fetch(addr, len, session)` looks up the ChunkCache first, and requests only on a miss (FD:L4942).
- Navigation within a stored version fetches no chunk again (FD:L1311).
- Each chunk is fetched once while it stays in memory (FD:L1317).
- Manifest chunks and content chunks are both chunks fetched with `eth_getCode` (FD:L1217), so they share the session cache.

**P34 (new).**
- Only a chunk that passed every `fetch.*`/`mfetch.*` rule is inserted into the cache. A failed response is never cached.
- A cache hit re-applies no rule. That is safe because the key fixes the address and the length, and the code at an address cannot change.
- The fetch-stage rules are still reported in reference order: a reference served from the cache cannot fail.

**Counting terms (replace the ambiguous `fetchCalls`):**

| Term | Meaning |
|---|---|
| `requests` | Actual `eth_getCode` invocations by the load, counted at the mock endpoint, every call |
| `uniqueKeys` | Distinct (address, length) keys referenced before the load ended |
| `references` | Chunk references visited, manifest chunks included |

The 0.2 field `fetchCalls` is read as `requests` from now on. The 0.2 model counted unique addresses while it re-invoked the provider for every reference (Codex probe: reported 1, actual 2). So the 0.2 figures are not evidence of at-most-once fetching. The 0.3 model counts invocations at the provider.

Expectations (`vectors/fetch-requests.json`, derived by hand):
- every 0.2 positive fixture now carries `requests`, `uniqueKeys` and `references`, for example `pos-site-4194304` with 2, 2 and 172;
- a no-cache mutant must give `requests = references` on each;
- `fr-shared-manifest-content`: a file whose content equals manifest chunk 1. Under the specified shared cache it makes 4 requests. A mutant with separate manifest and content caches makes 5; a mutant with no cache makes 104;
- `fr-same-address-different-length`: makes 2 requests. The length is part of the key (FD:L1309), so an address-only cache, which would make 1 request, is distinguishable;
- `fr-session-reload`: a second load in the same session makes 0 additional requests.

## R3-04 — Snapshot consistency (C06; replaces the status of 0.2 §4.2)

**The snapshot guarantee is NOT frozen in draft 0.3.** The P20 text of 0.2 §4.2 stays as the *preferred proposal*, and U05 stays open. Draft 0.3 adds two things: an exact baseline reconciliation, which finds a conflict, and literal mock scenarios that define the required client behaviour.

**Baseline reconciliation (new findings):**
1. The extension sends only methods listed in the RECV_LIMIT table: "a method not listed is not sent" (FD:L1274). `eth_getProof` is not in that table (FD:L1262–1273). `eth_getStorageAt` is (4096 B, exact, FD:L1262). **So the preferred P20 mechanism contradicts the baseline as written.** It needs a change request; see `M1-OPEN-0.3.md` BLK-01.
2. `HttpTransport` may be imported only by RpcReadClient, WalletSubmit, LogClient, HeaderNetCheck, NetworkProfiles and ChunkFetcher (FD:L4934, "only"). No listed module reads Website state. Under any mechanism (P20, or alternative B with `eth_getStorageAt`), the Website-record reader must live in one of these modules or be added by change request. Preferred: a function in the ChunkFetcher module, with no new importer. This is recorded as P35, pending review.
3. D95 forbids `eth_getProof` to *sites* (FD:L882, FD:L1204). That ban does not itself forbid extension-internal use, but item 1 does.
4. Cost: `eth_getProof` is heavy (RESP_MAX class, FD:L1960). Its node-wide budget is HEAVY_ACTIVE = 4 and HEAVY_QUEUE = 16 (FD:L1966); the per-connection token bucket is 20/s with capacity 20 (FD:L1967).
   - P20 uses at most 4 attempts per load: one anchor and two proofs each, so at most 4 light and 8 heavy calls.
   - Whether a full 4 MiB load fits the bucket is unmeasured (E03). Besides the proofs, such a load makes up to 3 manifest-chunk requests and up to ⌊4194304/24575 + 256⌋ = 426 content-chunk requests (256 files, each adding at most one partial chunk).

**Mock scenarios** (`vectors/snapshot-mock-scenarios.json`, executed against the abstract model `tools/snapshot_model.py`):

| ID | Sequence | Required result |
|---|---|---|
| SNAP-01 | A | Render A; 3 calls; 0 restarts |
| SNAP-02 | A → B (reorg between rounds) | Round-2 proof from B fails against R_A; re-anchor; render B only; 6 calls |
| SNAP-03 | A → B → A | B response rejected; re-anchor; render A only |
| SNAP-03D | A → B → A under alternative B (block-hash detector) | Must produce a **mixed** render: v1 content from B shown as current, which was never current in any state. This is the executable proof that B is a detector only |
| SNAP-04 / -11 | −32017 once / four times | Re-anchor and render / `inconsistent` with zero frames after 3 restarts |
| SNAP-05 / -06 | Forged proofs ×4 / flapping A, B ×4 | `inconsistent`, zero frames, 8 / 12 calls |
| SNAP-07 | Proof verifies but covers other slots | Treated as a verification failure |
| SNAP-08 | Single consistent root, current version revoked | `stateInvariant`, no restart, zero frames |
| SNAP-09, -10, -12, -13 | `@v` not found, `@v` non-current, no site, `@v` draft | §4.3 rows, with the call counts given |

These fixtures fix the behaviour. They do not show that pocold serves verifiable proofs (E03, assumption (a) in 0.2 §4.2). The abstract model treats a proof as binding to exactly one root, which is the property E03 must establish for real MPT proofs.

**What can be frozen without the change request** (P36, proposed). If the owner rejects CR-M1-01, the baseline-compatible mechanism is alternative B: `eth_getStorageAt` at block number N with a before/after hash check. Its guarantee must then be stated as conditional: "consistent unless block N is reorganized during the load and reorganized back" (SNAP-03D). The viewer must not claim single-state consistency in that case.

## R3-05 — Integer widths (C09; amends 0.2 P02 and §5)

| Quantity | Width | Encoding / location | Rule on excess |
|---|---|---|---|
| Manifest `version` | **u8** | RLP integer | `struct.int` (leading zero or value > 255), then `manifest.version` unless the value is 1 |
| Manifest `entryIndex`, `size`, chunk `len` | u32 | RLP integer | `struct.int` (unchanged) |
| Manifest `mimeId` | u8 | RLP integer | `struct.int` (unchanged) |
| Website version ID (`id`, `versionCount`, `currentVersion`) | **uint32** | ABI `uint32`; slot 2 bits 0–31 and 32–63 | Values 0..1024 by `VersionLimit()`; 0 means none |
| `@v<n>` selector | 1..2³²−1 | Lexical (R3-02) | `malformedSelector` |
| `manifestLen` | uint32 (values ≤ 65536) | ABI `uint32`; base+1 bits 0–31 | `ManifestLength` |
| `publishedBlock` | uint64 | base+1 bits 40–103 | — |
| `status` | uint8 | base+1 bits 32–39 | — |

The 0.2 model already used u8 for `version`. The 0.2 prose did not say so. Fixtures (`vectors/version-width.json`):
- `neg-version-256`, `neg-version-257` and `neg-version-leading-zero` each give `struct.int`, then `manifest.version` when that rule is disabled;
- `neg-version-255` gives `manifest.version`.

`neg-version-257` also catches a validator that truncates to u8, because 257 mod 256 = 1.

**P37 (new). ABI width precedence.** A call whose `uint32` argument word has any of bits 32–255 set reverts with **empty revert data** during ABI decoding. That happens before the authorization check, for every caller. Truncation, for example 2³²+1 read as 1, is forbidden. With Solidity ≥ 0.8 this is the compiler's ABI-decoder behaviour, but it is unverified until E01 (T1-16).

## R3-06 — T1 corrections (C09; replaces rows T1-10 and T1-12 of 0.2 §8.1, adds T1-16)

"Slot diff" compares every slot of §5.5, slots 0–5, and the publisher mapping entries of every declared account. "Bits" refers to the packing of §5.5.

| ID | Preconditions | Action | Expected | Must catch |
|---|---|---|---|---|
| T1-10 | Owner A, publisher A. v1 published at block P1 and current; v2 draft; v3 published, not current | (a) `publish(2)`; (b) `setCurrent(3)`; (c) `revoke(1)`; (d) `setCurrent(2)`. Diff after each step | (a) Only Version[2].base+1 bits 32–39 (0→1) and 40–103 (0→block). (b) Only slot 2 bits 32–63 (1→3). (c) Only Version[1].base+1 bits 32–39 (1→2); its manifestLen bits 0–31 and publishedBlock bits 40–103 stay equal to P1. (d) Only slot 2 bits 32–63 (3→2). In every step, Version[1].base, base+2 and all element slots are byte-identical; versionCount bits 0–31 do not change; `extcodehash` of every manifest chunk is unchanged | An in-place edit of manifest fields; publishedBlock rewritten by `revoke` or `setCurrent`; status written into the wrong word or bits |
| T1-12 | Owner A; A is a publisher (constructor). v1 and v2 published; currentVersion = 1. Then A `transferOwnership(D)` and D `acceptOwnership()`. The publisher tool shows the U10 warning that A is still a publisher | (1) A `setCurrent(2)`; (2) D `setPublisher(A,false)`; (3) A `setCurrent(1)` | (1) Succeeds: `CurrentVersionChanged(1,2)`. (2) `PublisherChanged(A,false)`; publisherCount decreases by 1. (3) Reverts with exactly `NotPublisher(A)`; empty slot diff; currentVersion stays 2 | An ownership transfer that silently strips the old owner's publisher right (step 1 fails); an ineffective removal or a missing authorization check (step 3 succeeds and slot 2 changes) |
| T1-16 | v1 published, not current; v2 current | Raw calldata `setCurrent` with argument word `2^32 + 1`, sent by A (publisher) and by C (unauthorized) | Both revert with empty revert data (ABI decoding, P37); empty slot diff; for C the error is **not** `NotPublisher` | A decoder that truncates to uint32 (it would select v1) |

Why T1-12 changed: in 0.2 the step "A `setCurrent(n)`" had no precondition that n was not current. Under U11 (`AlreadyCurrent`) the first call could revert for that reason, and a mutant without an authorization check could revert with `AlreadyCurrent` in step 3, so it could pass a test that checked only "reverts". In 0.3 both targets are published and not current at the time of the call, and step 3 requires the exact error. `AlreadyCurrent` itself is tested only in T1-13, with the target current as a stated precondition.

Why T1-10 changed: the 0.2 wording "slot diff for v1 limited to (status, publishedBlock, slot 2)" could be read as allowing v1's publishedBlock to change, and it did not separate the four actions. publishedBlock is written once, by `publish`, and never again.

## R3-07 — Annex batch 1 (C01; partial)

See `M1-ANNEX-INVENTORY-0.3.md` §3 and `vectors/annex-batch1.json`:
- **X1:** the literal RLP preimage of the netKey vector, `0xf28c506f436f6c2d6e65742d7631830bdb29a0‖0x11×32` (51 bytes). The encoding assumption for chainId is labelled P. The Keccak output is computed and recorded by the runner, but not asserted until the K1–K3 libraries agree (E05).
- **V1:** RW1–RW6 and HC1–HC7 transcribed from FD:L2646–2647, with the formulas and the exhaustive and random checks. The message texts and the remaining vectors of FD:L5017 are scheduled for batch 11.

## R3-08 — M1-spec acceptance vs implementation experiments (C01, F26; replaces 0.2 §9)

**M1-spec acceptance** (document gate; the full baseline list FD:L4955–5020 applies with no carve-out). M1-spec is accepted only when every item in `M1-ANNEX-INVENTORY-0.3.md` §2 (core C1–C4 and all 37 annex rows) is **Complete**. Complete means:
1. the specification text and its literal fixtures exist;
2. every claim that can be computed in the specification tooling has been **executed**, with machine-readable results;
3. Codex has reviewed it;
4. every U item that affects it is decided by its named decider;
5. every genuine blocker that affects it is closed.

**Implementation-phase experiments are not M1-spec acceptance conditions:** E01 EVM/anvil, E02 gas, E03 pocold `eth_getProof`, E04 compiler ABI/layout, E06 browser, E07 the viewer on mocked state, and the M0 tool pins. They are gates for implementing the affected unit, and their results may reopen the spec. This **does not weaken** annex completeness. An annex item whose *content* the baseline defines by a measurement or an experiment is not "Complete" until the specification states the experiment, its literal inputs and its pass criterion. The outcome belongs to the implementation phase.

| Kind of evidence | Required for M1-spec acceptance | Required before implementing the unit |
|---|---|---|
| Specification text and literal fixtures | Yes, for all rows | Yes |
| Reference-model checks executed (Python, standard library) | Yes | Yes |
| E05 K1–K3 cross-check of hashes in fixtures | Yes; fixture freeze (M0 prerequisite) | Yes |
| E01, E02, E04 (EVM, gas, compiler) | No | Yes, for ChunkFactory and Website |
| E03 (pocold proofs), E06/E07 (browser, viewer) | No; but U05 must be decided | Yes, for the viewer (M2) |

The implementation gate of 0.2 §9 (a)–(c) is unchanged.

## R3-09 — Design-ID provenance (C08)

- The approved design ID `e8a19ecb…a851aa0` appears **inside** `FINAL_DESIGN.md` at FD:L3 ("البصمة", fingerprint). So it cannot be the plain SHA-256 of that file's bytes: that would require a SHA-256 fixed point. The recorded raw SHA-256 `d54b5aee…a194a480e` therefore differs from it by construction. The difference is **not evidence of corruption**, and the two values are not contradictory.
- `reference/baseline.json` gives `"source": "FINAL_DESIGN.md and state(3).json"`. No `state(3).json` exists in this workspace (searched `D:\PoCol-Development`), so its derivation cannot be tested here.
- `tools/design_id_probe.py` tests stated candidate preimages with SHA-256, Keccak-256 and SHA3-256: the file without line 3, the file with the ID blanked, line-ending variants, every other reference file, and a concatenation of the section files. **It has not been run in this turn.** A negative result leaves the derivation open; it would not show the ID is wrong. Owner question Q-C08 is in `M1-OPEN-0.3.md`.
- Documents should call the value the "approved design ID", not a "fingerprint" of the bytes. This is a wording change only; the 0.2 files are not edited.

## R3-10 — Tooling that runs under the coordination interpreter (C02)

- `coordination/runtime/python311` is an embedded Python 3.11.1 whose `python311._pth` puts only `python311.zip` and the runtime directory on `sys.path`. Under it, `python run_checks.py` in `m1-draft-0.2/tools` cannot import its sibling modules.
- `tools/run_checks_03.py` sets its own path. It loads the unchanged 0.2 modules `keccak`, `m1abi` and `gen_fixtures` from `../m1-draft-0.2/tools`, and the 0.3 modules from its own directory.
- `tools/rerun_02.py` runs the unchanged 0.2 `run_checks.main()` with only the 0.2 tools on the path. It redirects the output to `results/rerun-0.2/`, so the committed 0.2 results are not overwritten.
- **None of these scripts was executed in this turn.** The tool set of this session had no command execution. Commands are in `README.md`.
