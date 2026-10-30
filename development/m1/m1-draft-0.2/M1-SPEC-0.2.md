# M1 — Website Storage Contracts: Specification Draft 0.2

Status: **review draft, NOT approved.** Nothing in this document is approved by virtue of appearing here. No production contract, node, or extension code exists for it.
Baseline: `reference/FINAL_DESIGN.md`, fingerprint `e8a19ecb306c7b955cddc75b050fdda95f9829c0e2b5cc06563cb27dba851aa0`. The baseline is unchanged and remains authoritative over this draft.
Supersedes: nothing. Draft 0.1 (`../M1-SPEC.md`) is preserved unchanged for traceability.
Inputs: draft 0.1; `../M1-CLAUDE-REVIEW.md` (findings F01–F26); `../M1-CODEX-RESPONSE.md` (dispositions and corrections).
Companion files in this directory:

| File | Content |
|---|---|
| `M1-DISPOSITIONS-0.2.md` | Disposition matrix for F01–F26, plus errata to the Claude review |
| `M1-UNRESOLVED-0.2.md` | Unresolved decisions (U01–U22), experiments (E01–E07), outstanding M0 prerequisites |
| `M1-ANNEX-CHECKLIST-0.2.md` | Completeness of every M1-spec item that the baseline requires |
| `M1-CHANGELOG-0.2.md` | Changes from draft 0.1 |
| `vectors/`, `tools/`, `results/` | Fixtures, the deterministic generator and reference model, and machine-readable results of the checks actually executed |

## 0. Conventions

Requirement labels:
- **B**: inherited from the approved baseline. Each B item cites the baseline line, written FD:Lnn.
- **P**: proposed by this draft. Requires reviewer approval and is not approved.
- **U**: an unresolved decision, listed in `M1-UNRESOLVED-0.2.md`. Wherever a P item depends on a U item, the P item gives the preferred option, which is not a resolution.
- **E**: an experiment needed before the dependent text can be frozen.

Numbering: draft 0.1 item numbers are kept. P03 is withdrawn. New items start at P14.

Fixture file names refer to `vectors/`. Every fixture carries a stable `id`. Negative fixtures also carry a `rule`, a `stage`, and an `isolation` class (§3.4).

"Mock" marks code-provider or state content that is not ordinarily deployable state, such as injected code. Mocks are declared as such in the fixtures and are never presented as deployment results.

## 1. ChunkFactory

**B01** (FD:L885–888). `store(bytes data)` accepts `1 ≤ |data| ≤ 24575`. `salt = keccak256(data)` (Keccak-256, not SHA3-256). The runtime is `0x00 ‖ data`. The operation is idempotent. The factory has no storage (D27, FD:L777, FD:L851).

**B02** (FD:L887). The initcode is `61 ‖ be16(1+|data|) ‖ 80 60 0a 5f 39 5f f3 ‖ 00 ‖ data`, with the runtime starting at byte offset 10. The maximum runtime length is 24576 bytes. Opcode trace: `PUSH2 L; DUP1; PUSH1 0x0a; PUSH0; CODECOPY; PUSH0; RETURN`.

**B03.** The chunk address is `keccak256(0xff ‖ factory20 ‖ salt32 ‖ keccak256(initcode))[12:]` (EIP-1014).

**B-EVM** (FD:L843; F21). The execution target is Cancun. `PUSH0` requires Shanghai or later. Experiments pin the anvil hardfork to Cancun and the compiler `evmVersion` to `cancun`, and record the actual tool versions once selected (E01).

**P01** (revised). The ABI is `store(bytes) external returns (address)`, nonpayable, and `predict(bytes) external view returns (address)`. The factory emits no events (U18). It contains no SSTORE, and it never uses DELEGATECALL or executes chunk payloads.

**P14** (new; F02). Explicit bounds and error precedence. `store(data)` evaluates the following in order:
1. If `|data| == 0` or `|data| > 24575`: revert `ChunkLength(uint256 length)`. This check runs before any initcode is built. It must not rely on the EVM code-size limit, because PoCol's `B_code_max = 32768` (FD:L2247) would accept a 24577-byte runtime.
2. Compute `a = predict(data)`. If `extcodesize(a) ≠ 0`:
   - if `extcodesize(a) == |data|+1` and `extcodehash(a) == keccak256(0x00‖data)`, return `a` without CREATE2;
   - otherwise revert `ChunkMismatch(address a)`.
3. `CREATE2(0, initcode, salt)`. A zero result reverts `ChunkCreateFailed()`. A result different from `a` also reverts `ChunkCreateFailed()`; this is unreachable under the EVM model.
4. Return `a`.

`predict(data)` applies rule 1 identically, then returns the address.

Proposed identifiers (computed from these signatures, not compiler output; `vectors/abi-and-slots.json`): `store(bytes)` `0xb374012b`; `predict(bytes)` `0xa64139fa`; `ChunkLength(uint256)` `0x980b4b67`; `ChunkMismatch(address)` `0x8f318868`; `ChunkCreateFailed()` `0x48c2f2c1`. Revert data for `ChunkLength(24576)` is `0x980b4b67` followed by `be256(0x6000)`.

**P01a** (clarified; F20). Ordinary repeated `store` calls are idempotent and testable without injection: the second call returns the same address and the trace shows no CREATE2. The `ChunkMismatch` branch is unreachable on a chain whose state comes only from EVM execution, assuming Keccak-256 collision resistance. That holds because the address commits to an initcode that does not depend on its environment. The branch is exercised only through a declared state-injection mock (T1-06c).

**P28** (F23 → U01). The factory address is the protocol constant `0x0000000000000000000000000000000000c0c005`. This is a proposed expansion of the elided baseline value `0x…C0C005` (FD:L777). Every address-dependent fixture is tagged with this assumption and must be regenerated if U01 resolves otherwise. On anvil, the compiled factory runtime is provisioned at that address through a declared test setup (state injection), recorded as such, and never reported as a deployment test.

**P27** (F22). Reproducible-build record. Before any implementation is reviewed, a build record must hold: compiler name and exact version; full settings JSON (optimizer, runs, `evmVersion`, `viaIR`, metadata hash setting, `bytecodeHash`); the sha256 of each source file; runtime and creation bytecode with their Keccak-256; and the compiler `storageLayout` for Website.

These values are produced by the implementation (E04). This draft neither invents them nor makes them a pre-implementation prerequisite.

The "no SSTORE" check (FD:L851) disassembles the runtime with PUSH-immediate skipping and excludes the trailing CBOR metadata whose length the last two bytes declare. It establishes only that no reachable instruction byte is `0x55`; it says nothing about behaviour reached through DELEGATECALL (excluded by P01) or about other contracts.

## 2. Manifest and files

**B04** (FD:L892). `RLP([version, entryIndex, files])`, where `file = [path, mimeId, size, contentHash, chunks]` and `chunk = [addr, len]`.

**P02** (revised; F19). Types and encodings:

| Field | RLP type | Constraint |
|---|---|---|
| top level | list of exactly 3 | |
| version | integer | must equal 1 |
| entryIndex | integer, u32 (width proposed) | `entryIndex < len(files)` |
| files | list | 1..256 items (the upper bound is the B07 client limit) |
| file | list of exactly 5 | |
| path | byte string | B05 |
| mimeId | integer, u8 | B06 |
| size | integer, u32 | `≤ 1048576` |
| contentHash | byte string of exactly 32 bytes | Keccak-256 |
| chunks | list (may be empty, see B07a) | |
| chunk | list of exactly 2 | |
| addr | byte string of exactly 20 bytes | |
| len | integer, u32 | `1..24575` |

An integer is a byte string holding the minimal big-endian value: no leading zero byte, and zero is the empty string `0x80`. A one-byte value `0x00` is therefore invalid as an integer. RLP must be canonical: single bytes below `0x80` are not wrapped, long form is used only for lengths of 56 or more, and length-of-length fields have no leading zero. A list must not appear where a string is expected, nor the reverse. No input may remain after the top-level item.

**B05** (restored; FD:L895; F03). `path = ('/' seg)+`, with `seg = [A-Za-z0-9._~-]+` and `seg ∉ {'.', '..'}`. The full path is at most 256 bytes, counting the leading `/`. Paths are case-sensitive.

Consequences: a leading `/` is required, there are no empty segments, there is no trailing `/`, and paths are ASCII. `...` and `~` are valid segments.

**B05a** (FD:L894). Files are strictly increasing by path bytes, with no duplicates.

**B06** (FD:L896). MIME IDs: 1 html, 2 css, 3 js, 4 json, 5 png, 6 jpeg, 7 webp, 8 svg, 9 woff2, 10 text. The entry file has MIME 1.

**B07** (FD:L893, FD:L904–905). Each chunk satisfies `1 ≤ len ≤ 24575`, and `Σlen = size ≤ 1048576`. The manifest is at most 65536 bytes and the contract holds at most 1024 versions. Client limits: a site has at most 256 files and at most 4 MiB.

**B07a** (F01; follows from B07 and P02, not a new rule). An empty file is `size = 0, chunks = [], contentHash = keccak256("") = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470`. Because `len ≥ 1` and `Σlen = size`, `chunks = []` holds exactly when `size = 0`. An empty file causes no chunk fetch and no empty chunk. **P03 is withdrawn.**

**P25** (F16 → U12). Site size means `Σ size` over all files, i.e. logical bytes, regardless of chunk sharing. The manifest bytes are bounded separately by B07. The same chunk address may appear several times within a file or across files (U17).

Boundary facts:
- 1048576 bytes split as `42 × 24575 + 16426`, giving 43 chunks.
- 1048577 bytes split as `42 × 24575 + 16427`, also 43 chunks. Only the length of the last chunk differs.

**B08** (FD:L892). `contentHash = keccak256(concatenation of chunk data without the 0x00 prefix)` and `manifestHash = keccak256(manifest bytes)`.

**P04** (unchanged). The publisher splits each file in byte order into maximal 24575-byte chunks, with the last possibly shorter. It applies no compression, line-ending, BOM, or encoding changes. The manifest bytes are split the same way (see P22).

## 3. Validation

### 3.1 Responsibility matrix (P22; resolves O01 only as a proposal; U07, U08)

Legend: **R** required, **—** not performed, **(U07)** performed only if U07 adopts on-chain manifest verification.

| Check | Factory | Website contract | Publisher (pre-submit) | Viewer (pre-render) |
|---|---|---|---|---|
| Chunk `1 ≤ |data| ≤ 24575` | R (P14) | — | R | R (`chunk.len`) |
| Idempotent store / mismatch | R (P14) | — | — | — |
| Role and state transitions | — | R (§5) | R (read-back) | — |
| `1 ≤ manifestLen ≤ 65536` | — | R | R | R (`manifest.length`) |
| Manifest chunk arrays equal length; canonical split (U08) | — | R | R | R (`manifest.split`) |
| Manifest chunk existence, prefix, length, factory origin | — | (U07) | R | R (`mfetch.*`) |
| `keccak256(manifest) == manifestHash` | — | (U07) | R | R (`manifest.hash`) |
| RLP canonical form, structure, widths | — | — | R | R (`decode.*`, `struct.*`) |
| Version, file count, entryIndex, paths, order, MIME, entry MIME, sizes, sums, site size | — | — | R | R (semantic rules) |
| Content chunk prefix, length, origin (P06), file hash | — | — | R (after deploy, before `createVersion`) | R (`fetch.*`, `file.hash`) |
| Execution safety of content | — | — | — | Not established by any check. A published version does not imply safe content (FD:L141 A7b) |

The contract never parses RLP and never touches content files. The viewer revalidates everything regardless of what the contract checked.

### 3.2 Stages and rule IDs (P16; F13, F14)

The viewer validates in the stage order below and reports the **first** violated rule. Within a stage, rules are evaluated in the order listed. Files are visited in manifest order, and the fields of a file and the chunks of a file in order.

Every pre-fetch stage (`manifest` length and split, `decode`, `structure`, `semantic`) completes over the whole manifest before any content chunk is fetched. A manifest rejected before the fetch stage therefore causes **zero** content `eth_getCode` calls.

| Stage | Rule IDs in precedence order |
|---|---|
| `manifest` (Website record; §4) | `manifest.length` (manifestLen not in 1..65536), `manifest.split` (lengths are not the canonical split, U08); then for each manifest chunk: `mfetch.missing`, `mfetch.prefix`, `mfetch.length`, `mfetch.origin`; then `manifest.hash` |
| `decode` | `decode.oversize` (more than 65536 bytes, checked before decoding), then while decoding: `decode.truncated`, `decode.noncanonical`, `decode.trailing` |
| `structure` (one full pass) | `struct.shape` (list arity or list/string type), `struct.int` (leading zero, or a value beyond u8/u32), `struct.width` (contentHash ≠ 32 bytes, addr ≠ 20 bytes), in field order |
| `semantic` (one full pass) | `manifest.version`, `manifest.fileCount`, `manifest.entryIndex`; then per file: `path.length`, `path.grammar`/`path.dotSegment` (per segment, left to right), `path.order`, `file.mime`, `entry.mime`, `file.size`, `chunk.len` (per chunk), `file.sum`; then `site.size` |
| `fetch` + `hash` (file-major) | for each file, for each chunk: `fetch.missing`, `fetch.prefix`, `fetch.length`, `fetch.origin`; then that file's `file.hash` |

Notes:
- `path.grammar` covers a missing leading `/`, empty segments, characters outside the segment class, and non-ASCII bytes. `path.dotSegment` covers `.` and `..`.
- A modified byte in an otherwise valid chunk is rejected by `fetch.origin` (P06) before `file.hash` (Codex correction). To isolate `file.hash`, a fixture uses valid chunk bytes at their correct address and changes only the declared hash.
- Distinct chunk addresses are fetched at most once per load (FD:L1309–1315). The rules are evaluated for each reference.

**P05** (revised). The publisher runs the same `decode`, `structure` and `semantic` rules on the manifest it builds, and the `fetch` and `hash` rules against deployed code before `createVersion`. Both sides must report identical rule IDs for every fixture.

**B09 + P06** (F06). The baseline requires the prefix and length checks and the rejection of chunks from another factory (FD:L889, FD:L907 "another factory"). The proposed mechanism (P06) re-derives the CREATE2 address from the retrieved data and the P28 factory constant, and compares it with the referenced address. The network profile has no factory field (FD:L972), and this draft adds none.

### 3.3 Retrieval limits (F25)

- **B** (FD:L905): at most 8 parallel light connections to the local node. On −32021, retry after `retryAfterMs`.
- **U14:** the baseline phrase "timeout 10s" does not say whether it is a per-request or a whole-load deadline. Preferred reading: a per-request timeout of 10 s for each `eth_getCode` / `eth_getProof`. Until U14 resolves, implementations must not introduce a whole-load deadline.

### 3.4 Fixture semantics (F13 correction)

- A negative fixture **must be rejected**, with its `rule` as the first error at its `stage`, while all rules are enabled.
- `isolation: "full"`: when only that rule is disabled, the reference validator **accepts** the input. This proves the fixture isolates the rule.
- `isolation: "next"`: when that rule is disabled, the first error becomes `nextRuleWhenDisabled`. These inputs cannot become globally valid by removing one rule; they demonstrate precedence instead.
- `isolation: "none"`: the rule cannot meaningfully be disabled (e.g. arity, truncation). The fixture asserts rejection only.
- `expected.fetchCalls`: 0 for every pre-fetch rejection. For fetch-stage fixtures it is the exact number of distinct `eth_getCode` calls.

## 4. Retrieval, snapshot and version selection

### 4.1 Read mechanism (P21; F08 → U06)

Website state is read with storage-slot keys (P13 layout, §5.5), not with `eth_call`. Chunks are read with `eth_getCode`. Slot keys for versions 1, 2 and 1024 are in `vectors/abi-and-slots.json`, for example:

```
Version[1].base       = 0xa15bc60c955c405d20d9149c709e2460f1c2d9a497496a7f46004d1772c3054c
Version[1].chunks[0]  = 0x126fa0859c3b6c65087899d2a6ef4db9ca4209343a9d5478bc96a6404374bc07
```

These are computed from P13, not from compiler output. E04 must compare them with the compiler `storageLayout`.

### 4.2 Snapshot consistency (P20; F07 → U05)

Requirement: all Website state used by one load must come from one state root. Endpoint checks of the block hash before and after the reads are a **detector only**. They cannot exclude an A → B → A sequence of states served at the same height (Codex correction).

Preferred mechanism: **proof-bound reads.**
1. **Anchor.** Obtain one header with `number N`, `hash H` and `stateRoot R`:
   - in LN, from `eth_getBlockByNumber('latest', false)`;
   - in RP, the anchor must be the head header verified by the RP window (FD:L985–1021).
2. **Round 1.** `eth_getProof(website, [slot 2], N)`. Verify the account proof and the storage proof against `R`. This yields `versionCount` and `currentVersion`.
3. **Round 2.** `eth_getProof(website, [base, base+1, base+2, elem0, elem1, elem2], N)` for the selected version. With the canonical split (U08) there are at most 3 manifest chunks, so all element slots are requested up front, and unused slots prove the value 0. Verify against the same `R`.
4. If any proof fails to verify against `R`, or the node returns −32017 (outside `K_eff`, FD:L548), restart from step 1 with a new anchor. Allow at most 3 restarts. After that, show an explicit "inconsistent or unavailable state" error and render zero frames.
5. Chunk code (manifest and content) is read with `eth_getCode(addr, 'latest')`. Chunk code cannot change after creation, and P06 binds code to address. A chunk missing at `latest` gives `fetch.missing` / `mfetch.missing` ("content unavailable"), never a partial render.

Assumptions and dependencies (all unverified; U05, E03):
- (a) pocold serves `eth_getProof` in the Ethereum MPT format and verification against a header's `stateRoot` is possible.
- (b) Extension-internal use of `eth_getProof` is permitted. D95 forbids it only to sites (FD:L882, FD:L1204). Its heavy-method cost under D88 must be accounted for.
- (c) The method is available within `K_eff ≤ 32`.
- This draft does not assume EIP-1898 support.

Trust:
- In LN, proof-bound reads give a single-state view of a trusted node.
- In RP, they bind state to a PoW-checked header, but there is still no execution verification. A consistent lie remains possible (D13, FD:L6288), and the RP label rules apply unchanged.

Alternatives, if U05 rejects proofs:
- (B) Pin reads by block number with `eth_getStorageAt`, plus a before/after hash detector and the immutability of version content (P11). This is documented as a detector, not a guarantee.
- (C) An atomic node snapshot facility. This is a node change requiring a change request.

### 4.3 Version selection (P17; F04 → U02, U03)

| Request | Condition at the anchor `R` | Result |
|---|---|---|
| default | `currentVersion == 0` | "No published site", no frame |
| default | `currentVersion = n`, status published | Load n |
| default | `currentVersion = n`, status ≠ published | `state.invariant` error, no frame (impossible for a conforming contract) |
| `@v<n>` malformed (§6.2) | — | `malformedSelector`, no RPC |
| `@v<n>` | `n > versionCount` | "Version not found at this height" (T25 (f2), FD:L4627), no frame |
| `@v<n>` | draft | Refuse, no frame (P; the baseline is silent; U03) |
| `@v<n>` | published, current | Load |
| `@v<n>` | published, not current | Load with a persistent "not the current version" banner (P) |
| `@v<n>` | revoked | **U02.** The baseline says revoke prevents *default* display (FD:L914). Preferred: an interstitial that names the version as revoked and needs an explicit click, then a persistent banner. Alternative: refuse; that is a stricter rule and needs a change request. |

**P30.** `site_navigate`, link navigation and sub-resource loads resolve against the manifest of the version already loaded in the session, without a new anchor (consistent with FD:L1311). A user-initiated reload re-runs §4.2 for the original request.

## 5. Website contract (proposed ABI, state machine, errors, events)

### 5.1 State and roles (B11, P09)

- **B** (FD:L899–904): `owner` and `pendingOwner`; statuses `draft=0`, `published=1`, `revoked=2`; PUBLISHER may call `createVersion`, `publish` and `setCurrent`; OWNER manages publishers (at most 16), revokes non-current versions, and transfers ownership in two steps; at most 1024 versions.
- **P09:**
  - Version IDs start at 1, and `currentVersion = 0` means none.
  - `constructor(address initialOwner)` rejects zero with `ZeroAddress()`.
  - The initial owner is registered as the first publisher and counts toward the 16.
  - Publishing rights are never inferred from ownership.
  - An ownership transfer leaves the publisher set unchanged (U10). The publisher tool must warn after a transfer if the previous owner is still a publisher.
- **Invariant:** `currentVersion == 0`, or `status(currentVersion) == published`.

### 5.2 Transition and error table (P24; F12, O03 → U11)

Each check fails with the first error in the precedence column; the "effects" column applies only on success. A revert changes no state.

| Call | Precedence: authorization → arguments → existence → state | Effects | Event |
|---|---|---|---|
| `constructor(o)` | `o == 0` → `ZeroAddress()` | owner = o; publishers[o] = true; publisherCount = 1 | `OwnershipTransferred(0, o)`, `PublisherChanged(o, true)` |
| `createVersion(h, len, chunks, lens)` | not a publisher → `NotPublisher(caller)`; `versionCount == 1024` → `VersionLimit()`; `len ∉ 1..65536` → `ManifestLength(len)`; `|chunks| ≠ |lens|` → `ChunkArrays(|chunks|, |lens|)`; non-canonical split → `ManifestSplit(i)` (U08); (U07) chunk invalid → `ManifestChunkInvalid(i)`, hash → `ManifestHashMismatch()` | id = ++versionCount; Version[id] = {h, len, draft, 0, chunks} | `VersionCreated(id, caller, h, len)` |
| `publish(id)` | `NotPublisher`; `id == 0 ∨ id > versionCount` → `UnknownVersion(id)`; status ≠ draft → `WrongStatus(id, status)` | status = published; publishedBlock = block.number | `VersionPublished(id, block.number)` |
| `setCurrent(id)` | `NotPublisher`; `UnknownVersion`; status ≠ published → `WrongStatus`; `id == currentVersion` → `AlreadyCurrent(id)` (U11; alternative: silent no-op) | prev = currentVersion; currentVersion = id | `CurrentVersionChanged(prev, id)` |
| `revoke(id)` | not owner → `NotOwner(caller)`; `UnknownVersion`; `id == currentVersion` → `CurrentVersion(id)`; status ≠ published → `WrongStatus` | status = revoked (permanent) | `VersionRevoked(id)` |
| `setPublisher(p, true)` | `NotOwner`; `p == 0` → `ZeroAddress()`; already enabled → **no-op, no event**; `publisherCount == 16` → `PublisherLimit()` | publishers[p] = true; publisherCount++ | `PublisherChanged(p, true)` |
| `setPublisher(p, false)` | `NotOwner`; `p == 0` → `ZeroAddress()`; absent → **no-op, no event** | publishers[p] = false; publisherCount-- (may reach 0; the owner may remove itself) | `PublisherChanged(p, false)` |
| `transferOwnership(c)` | `NotOwner`; `c == owner` → `InvalidCandidate(c)` (U11); `c == 0` is allowed and **cancels** the nomination (U11) | pendingOwner = c | `OwnershipNominated(owner, c)` |
| `acceptOwnership()` | `caller ≠ pendingOwner` → `NotPendingOwner(caller)` (also covers pendingOwner = 0, because the caller is never 0) | prev = owner; owner = caller; pendingOwner = 0; publishers unchanged (U10) | `OwnershipTransferred(prev, caller)` |
| `getVersion(id)` | `UnknownVersion(id)` | returns (manifestHash, manifestLen, status, publishedBlock, chunks[], lengths[]) | — |
| `owner()`, `pendingOwner()`, `versionCount()`, `currentVersion()`, `isPublisher(a)`, `publisherCount()` | — | — | — |

There is no `renounceOwnership` and no `setCurrent(0)`. With no other published version, the baseline gives no on-chain way to take a site down; changing that would need a change request (noted, not proposed).

Authorization is checked first, so an unauthorized caller always gets the authorization error, whatever the state (T1-01).

### 5.3 Proposed identifiers (computed, not compiler output)

The full set is in `vectors/abi-and-slots.json`. As an independent check of the selector tooling, `owner()` `0x8da5cb5b`, `transferOwnership(address)` `0xf2fde38b`, `acceptOwnership()` `0x79ba5097` and `pendingOwner()` `0xe30c3978` match their widely published values (`results/run-results.json`).

Event signatures, with `indexed` marking proposed indexed parameters:
- `VersionCreated(uint32 indexed id, address indexed publisher, bytes32 manifestHash, uint32 manifestLen)`
- `VersionPublished(uint32 indexed id, uint64 publishedBlock)`
- `CurrentVersionChanged(uint32 indexed previous, uint32 indexed current)`
- `VersionRevoked(uint32 indexed id)`
- `PublisherChanged(address indexed publisher, bool enabled)`
- `OwnershipNominated(address indexed owner, address indexed candidate)`
- `OwnershipTransferred(address indexed previous, address indexed next)`

Publisher enumeration (O03): the mapping cannot be enumerated from state. Clients reconstruct the set from `PublisherChanged` events, bounded by 16 live entries, and check each candidate with `isPublisher`.

### 5.4 Version content (P11, P22)

- `createVersion` makes an immutable-content draft. A version's manifest is never edited in place; corrections create a new version. `publishedBlock` is set by `publish`.
- **P22 / U08** (new contract restriction, recorded explicitly as Codex asked): the manifest chunk arrays must be the canonical split of `manifestLen`. That means `⌈manifestLen/24575⌉` chunks (1 to 3), every one 24575 bytes except the last. This bounds storage writes and makes identical manifests map to identical chunk addresses. It is not implied by P04, which governs only the publisher.
- **U07:** whether `createVersion` also verifies manifest chunk existence, prefix, length, factory origin and `keccak256 == manifestHash` on chain. If adopted, Website stores the factory address as an `immutable`, which occupies no slot. My estimate of the cost (about 50–150k gas for 64 KiB) is **unmeasured** (E02).

### 5.5 Storage layout (P13, unchanged in substance)

No inheritance and no preceding variables.

| Slot | Contents |
|---|---|
| 0 | `owner`, low 160 bits |
| 1 | `pendingOwner` |
| 2 | `versionCount` bits 0–31, `currentVersion` bits 32–63 |
| 3 | `mapping(uint32 => Version)` |
| 4 | `mapping(address => bool)` publishers |
| 5 | `publisherCount` (uint32) |

`Version`:
- `base = keccak256(be256(id) ‖ be256(3))` holds `manifestHash`.
- `base+1` holds `manifestLen` (bits 0–31), `status` (bits 32–39) and `publishedBlock` (bits 40–103).
- `base+2` holds the length of `Chunk[]`. Element i is at `keccak256(be256(base+2)) + i`, with `addr` in bits 0–159 and `len` in bits 160–191.

The compiler `storageLayout` must match this table (E04). No Merkle-proof fixtures are created before P13 and U05 are accepted.

## 6. Paths, navigation and references

### 6.1 Lookup normalization (B10, P08; FD:L897)

Applied to a path component that has already had its query and fragment removed, in this order:
1. `''` or `'/'` selects the entry file.
2. Any `%` gives 404 (`nav.percent`).
3. Any `//` gives 404 (`nav.emptySegment`).
4. A component not starting with `/` gives 404 (`nav.grammar`).
5. Any `.` or `..` segment gives 404 (`nav.dotSegment`).
6. A trailing `/` has `index.html` appended.
7. A result longer than 256 bytes gives 404 (`nav.length`).
8. Any segment outside the segment class gives 404 (`nav.grammar`).
9. Exact, case-sensitive lookup in the session manifest; a path not present gives 404.

There is no percent-decoding, no case folding, no host filesystem normalization, and no implicit directory: `/docs` gives 404 unless the file `/docs` exists.

### 6.2 Omnibox (P18; F18 → U13)

The input format is `pocol <profile>/<0xaddr>[/path][@v<n>]` (FD:L967). The part after the address is processed as follows:
1. Cut at the first `?` or `#`, whichever comes first. The rest is the query/fragment and is excluded from lookup.
2. In the remaining component only, a trailing `@v<digits>` is a version selector. It is malformed if:
   - the digits have a leading zero, including `@v0`;
   - the value exceeds 2³²−1;
   - the component ends in `@v` with no digits.
   A malformed selector gives a `malformedSelector` error with no RPC.
3. Remove the selector, then apply §6.1. Any other `@` stays in the path and gives 404.

Examples: `/a.html@v2?x=1` selects v2. `/a.html?x=@v2` does not. `@v3` and `/@v3` select the entry of v3. `/a@v1@v2` gives 404.

Whether the query and fragment are exposed to the page is outside M1 (U19).

### 6.3 `site_navigate` (B; FD:L1164, FD:L1193)

The baseline `pathStr` schema applies first: printable ASCII `0x21–0x7e`, length 1..2048, leading `/`; otherwise −32602. Then the query and fragment are cut (§6.2 step 1) and §6.1 is applied against the session's loaded version (P30). **There is no version-selector interpretation**: `/a.html@v2` gives 404. BR18b–BR18d (FD:L2728–2731) hold unchanged.

### 6.4 Document-relative references (P19; F05 → U04)

Stage R applies to references found by the loader in the attributes that the sanitizer rewrites (`src`, `href`, `srcset`, `url()`, `@import`) and in intercepted links (FD:L1340–1354). The base is the **manifest path** of the document containing the reference, e.g. `/docs/index.html` when `/docs/` was requested. Classification, in this order:

| Reference form | Classification and handling |
|---|---|
| `''` | The same document |
| `#…` | Fragment-only. No navigation and no fetch |
| `scheme:…` (`[A-Za-z][A-Za-z0-9+.-]*:`) | Not local. The existing browser rules apply: an `https:` link goes to `site_openExternal`; a resource scheme is subject to the CSP (e.g. `img-src data:`); everything else is not followed. Never resolved as a filename |
| `//…` | Network-path reference. Not followed and not loaded (P) |
| `/…` | Absolute path. §6.1 on the literal component, with no dot-segment removal, so `/a/../b.html` gives 404 (matches BR18c) |
| `?…` | Query-only. The base document |
| anything else | Relative path. Cut the query/fragment, merge with the base directory, apply RFC 3986 `remove_dot_segments`. Climbing above the root gives 404 (`ref.aboveRoot`; P, stricter than RFC 3986, which clamps). Then apply §6.1 |

The existing T12 relative-CSS case does not prove support for every parent-directory form. `vectors/path-cases.json` lists the forms covered, including `../`, `./`, `..`, above-root, query-only, fragment-only, scheme and network-path. Browser confirmation is experiment E06.

## 7. Publisher workflow and recovery (P23, P26; F10, F24 → U09, U16)

### 7.1 Phases

1. **Plan:** validate local files, build the manifest (P04 split), and compute predicted addresses. Show file and manifest byte totals, chunk counts, and estimated costs. Estimated and measured costs are shown separately.
2. **Content chunks:** for each planned chunk without verified code, send `store`.
3. **Confirm:** wait `CONF_DEPTH` confirmations (U16). Then read the code at each address and run the `fetch.*` rules.
4. **Manifest chunks:** the same as steps 2–3 for the canonical manifest split.
5. **Find or create the draft** (P23): search for an existing version equal on `(manifestHash, manifestLen, chunks[], lengths[])`, comparing array lengths and every element, with status draft or published.
   - If found, reuse it, taking the lowest matching ID.
   - Otherwise call `createVersion`.
6. **Publish:** only if the user chose to publish and the status is draft.
7. **Select:** only if the user chose `setCurrent`, and only after the version is published.

An error in any phase leaves `currentVersion` unchanged.

Residual risks, stated rather than hidden:
- A reorg after phase 3 can still remove a chunk, because a pre-submit code check cannot guarantee that no reorg follows. Re-running the publisher re-stores the missing chunk at the same address (B01 idempotence). Until then the viewer reports content unavailable.
- Concurrent publishers may both create identical drafts. The contract does not prevent this.
- Draft reuse mitigates accidental exhaustion of the 1024 versions. It does not stop a malicious authorized publisher from creating 1024 distinct versions.

### 7.2 Transaction reconciliation

On restart, for each recorded transaction:
- query its receipt;
- if it is absent and the account nonce for that slot is still unused, rebroadcast the same signed transaction, or replace it at the same nonce with a higher fee and identical calldata.
- Because `store` is idempotent, a duplicated or replaced `store` is harmless. `createVersion` is never re-sent without first repeating the phase 5 search.

### 7.3 Checkpoint schema (`pocol-m1-publisher-checkpoint/1`, JSON, never contains keys)

```json
{
  "schema": "pocol-m1-publisher-checkpoint/1",
  "chainId": 0, "genesisHash": "0x…", "factory": "0x…", "website": "0x…", "sender": "0x…",
  "options": {"publish": true, "select": true, "confirmations": 0},
  "plan": {
    "manifestHash": "0x…", "manifestLen": 0,
    "manifestChunks": [{"address": "0x…", "len": 0}],
    "contentChunks": [{"address": "0x…", "len": 0}],
    "files": [{"path": "/…", "size": 0, "keccak256": "0x…"}]
  },
  "phase": "plan|contentChunks|confirm|manifestChunks|draft|publish|select|done",
  "txs": [{"purpose": "store|createVersion|publish|setCurrent", "ref": "0x…|id",
           "nonce": 0, "hash": "0x…", "status": "sent|mined|replaced|dropped",
           "blockNumber": null, "blockHash": null}],
  "draftId": null
}
```

The checkpoint is advisory. A lost or stale checkpoint is recovered by reading chain state, through the phase 5 search and code checks, never by trusting the file.

## 8. Tests

### 8.1 T1, contract and publisher

Declared accounts: A = owner, B = publisher, C = unauthorized, D = candidate. "Slot diff" means every slot of §5.5 for every version that exists, compared before and after. Error encodings are those of §5.2. Each test also names the deliberately broken variant that must fail it.

| ID | Preconditions | Action | Expected | Must catch |
|---|---|---|---|---|
| T1-01 | A owns; B is a publisher; v1 draft; v2 published | C calls `createVersion(ver-minimal)`, `publish(1)`, `setCurrent(2)` | each reverts `NotPublisher(C)`; empty slot diff | the authorization check removed |
| T1-02 | fresh deployment | A `setPublisher(B,true)`; B `createVersion(ver-minimal)`, `publish(1)`, `setCurrent(1)` | slots equal `abi-and-slots.json` `t1_02_expectedState`, with A, B and the publish block bound at run time; events in order | status not written; wrong packing |
| T1-03 | after T1-02 | A `revoke(1)`; B creates, publishes and selects v2; A `revoke(1)` | first call `CurrentVersion(1)`; then v1 status 2; `VersionRevoked(1)` | revoking the current version allowed |
| T1-04 | A owns | A `transferOwnership(D)`; C `acceptOwnership()`; D `acceptOwnership()` | owner stays A after the nomination; C gets `NotPendingOwner(C)`; then owner = D, pendingOwner = 0 | a one-step transfer |
| T1-05 | factory provisioned | `store(x)` twice in separate transactions, then twice within one transaction | same address; `predict(x)` equals it; the trace of each repeat has no CREATE2 | CREATE2 repeated |
| T1-06a | anvil default limit | `store` with lengths 0 and 24576 | revert data `ChunkLength(0)` / `ChunkLength(24576)`, byte-exact; no code at the predicted address | a missing explicit check (would fail T1-06b) |
| T1-06b | anvil with code-size limit ≥ 32768 (mirrors `B_code_max`) | `store` with length 24576 | `ChunkLength(24576)`; no code | relying on EIP-170 |
| T1-06c | **mock:** declared state injection at `predict(x)` | (i) inject `0x00‖x`, then `store(x)`; (ii) inject `0x00‖x'` (same length); (iii) inject a different length | (i) returns the address with no CREATE2; (ii) and (iii) `ChunkMismatch` | the mismatch branch skipped |
| T1-06d | — | `store` with lengths 1 and 24575 (`chunks.json`) | runtime bytes equal the vectors exactly | — |
| T1-07 | fresh deployment, count 1 (A) | grant 15 distinct addresses; grant a 16th; regrant an existing one; remove an absent one | count 16; then `PublisherLimit()`; regrant and removal are no-ops (count 16, no event) | an off-by-one cap; a counting regrant |
| T1-08 | v1 draft, v2 revoked | `setCurrent` on 0, 3 (missing), 1, 2; create versions up to 1024; create one more | `UnknownVersion`, `UnknownVersion`, `WrongStatus(1,0)`, `WrongStatus(2,2)`; the 1025th gives `VersionLimit()` | — |
| T1-09 | A owns | C calls `setPublisher`, `revoke`, `transferOwnership` | `NotOwner(C)` each; empty slot diff | — |
| T1-10 | v1 published | `publish`, `setCurrent`, `revoke` on other versions | slot diff for v1 limited to the documented fields (status, publishedBlock, slot 2); chunk code byte-identical | in-place edits |
| T1-11 | publisher crash points K1 (after k of n chunks), K2 (after the manifest chunks), K3 (after `createVersion` is mined, checkpoint deleted), K4 (after `publish`) | restart the publisher | no chunk sent twice except idempotent re-stores; `versionCount` increases by exactly 1 overall; the same draft ID is published; `currentVersion` unchanged until the select phase | duplicate drafts; partial manifests |
| T1-12 | after T1-04 (owner D; A still a publisher) | A `setCurrent(n)`; D `setPublisher(A,false)`; A `setCurrent(n)` | first succeeds (U10 as written); then `NotPublisher(A)`; the publisher tool showed the warning | — |
| T1-13 | table-driven | every row of §5.2 × {authorized, unauthorized} × {each status} | the result or the first error per the precedence column; empty slot diff on revert | wrong precedence |
| T1-14 | — | `createVersion` with: `len` 0, 65537; arrays of 1 vs 2; split `[40, 39]` for 79 bytes; split `[24575, 24575, 16386]` for 65536 | `ManifestLength`, `ManifestLength`, `ChunkArrays(1,2)`, `ManifestSplit(0)`; the last succeeds and its gas is recorded as measured | — |
| T1-15 | — | `transferOwnership(0)` after nominating D; `transferOwnership(A)` by A | pendingOwner = 0, and D can no longer accept; `InvalidCandidate(A)` (both U11) | — |

T1-12, T1-14 and T1-15 encode preferred U10, U08 and U11 options. If a decision differs, these rows change before any implementation.

### 8.2 T16, manifest and retrieval fixtures

| File | Content |
|---|---|
| `manifest-positive.json` | 11 accepted sites: minimal (draft 0.1), empty file, shared chunk, case-distinct paths, 256-byte path, `...`/`~` segments, a 24575-byte chunk, a 1048576-byte file (43 chunks), 256 files, a 4194304-byte site (4 × 1 MiB, shared chunks), a manifest of exactly 65536 bytes |
| `manifest-negative.json` | 62 rejected inputs covering every rule ID in §3.2 for the decode, structure, semantic, fetch and hash stages. Includes the 11 draft 0.1 cases (3 retained as documented multi-fault `next` fixtures, prefixed `neg-draft01-`) and single-fault replacements |
| `version-records.json` | Manifest-stage cases: valid records for the minimal and 65536-byte manifests (the latter is the `createVersion` input for T1-02/T1-14), plus wrong hash, missing chunk, non-canonical split, `manifestLen = 0` |
| `code-table.json` | Honest chunk code by address. Fixtures reference it by address. Mocks are declared inline as `providerOverrides` with `mock: true` |
| `path-cases.json` | Hand-written expectations for §6: 30 omnibox, 11 `site_navigate`, 17 document-relative cases |
| `abi-and-slots.json` | Proposed selectors, event topics, slot keys, and the T1-02 expected state |

Boundary pairs, each accept at the limit and reject one past it:

| Quantity | Accept | Reject |
|---|---|---|
| Chunk length | 24575 | 24576 |
| File size | 1048576 | 1048577 (both 43 chunks) |
| Manifest size | 65536 | 65537 |
| Files | 256 | 257 |
| Site size | 4194304 | 4194305 |
| Path length | 256 | 257 |

The version (1024/1025) and publisher (16/17) boundaries are contract tests T1-07 and T1-08.

Acceptance:
- A conforming publisher-side validator and a conforming viewer-side validator must each reproduce `expected.firstRule` and `expected.stage` for every negative fixture, and `expected.fetchCalls` where it is given.
- They must accept every positive fixture, and must produce the same results for every `path-cases.json` entry.
- The reference model in `tools/` passes all of these (`results/run-results.json`). Agreement with the reference model is evidence about the fixtures, not about any implementation.

## 9. Implementation gate (restored; F26 → U15)

By default the full baseline gate applies (FD:L4955–5000). M1-spec is complete only when every item in `M1-ANNEX-CHECKLIST-0.2.md` is complete and reviewed.

ChunkFactory, Website and the publisher do not enter implementation until all of the following hold:
- (a) the checklist is complete, **or** an explicit, recorded owner-level decision (U15) authorizes staged implementation;
- (b) every U item that affects the unit is resolved;
- (c) the outstanding M0 prerequisites (`M1-UNRESOLVED-0.2.md` §3) are done.

Reviewer agreement and the number of review rounds do not satisfy this gate. A conflict with the baseline opens a documented change request; the baseline is never edited silently.

## 10. Evidence for this draft

Executed, with results in `results/run-results.json`: 212 checks, all passing, using only the Python 3.11 standard library. They cover:
- the Keccak self-test, including the EIP-1014 example 0;
- well-known selectors;
- byte-identical reproduction of all four draft 0.1 vector files by this independent tooling (draft 0.1 was generated with pycryptodome and pyrlp; Codex reported reproducing it);
- generator determinism across two runs;
- integrity of the committed fixtures against a fresh generation;
- the code table being entirely factory-derived;
- the outcome of every positive and negative fixture, every isolation claim, and the fetch-count claims;
- rule coverage;
- the boundary arithmetic;
- all path cases against their hand-written expectations.

Not executed: EVM/anvil, Solidity compilation, gas, browser or extension, pocold, `eth_getProof` behaviour, and the M0 K1–K3 hash libraries. No compiler storage layout exists yet.
