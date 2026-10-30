# M1 — Website Storage Contracts: Implementation Draft 0.1

Status: ready for review, NOT approved. No production contracts are implemented in this package.
Baseline design: e8a19ecb306c7b955cddc75b050fdda95f9829c0e2b5cc06563cb27dba851aa0.

## Delivery scope and authority

This delivery covers the core of M1-spec: ChunkFactory, Website, the publisher, and T1/T16 test vectors. All baseline sections and decisions are preserved verbatim in `reference/` so cross-component requirements remain available. The extension, bridge, and recovery annexes have NOT yet been completed as standalone implementation specifications. They remain mandatory before their components are implemented. Splitting deliveries does not remove any baseline requirement or test.

Requirement labels:
- **B**: inherited from the approved design.
- **P**: proposed implementation detail requiring reviewer approval; not a previously approved decision.
- **O**: open decision to resolve before implementing the affected unit.

This English document replaces the Arabic M1 draft for implementation discussions. This is a language conversion, not a new approval or a protocol revision. The baseline files remain in their original Arabic and retain their authority; they have not been translated or modified.

## 1. ChunkFactory

**B01.** `store(bytes data)` accepts 1 through 24575 bytes. Runtime code is `0x00 || data`. The salt is Keccak-256 of data, NOT SHA3-256.

**B02.** Initialization code is:

```
61 || be16(1 + len(data)) || 80 60 0a 5f 39 5f f3 || 00 || data
```

Runtime begins at byte offset 10. Maximum runtime length is 24576 bytes.

**B03.** The chunk address is the last 20 bytes of:

```
keccak256(0xff || factory20 || salt32 || keccak256(initcode))
```

**P01.** Proposed ABI: `store(bytes) external returns (address chunk)`, nonpayable; and `predict(bytes) external view returns (address)`. The factory has no persistent storage.

**P01a.** If code already exists at the predicted address, verify that its length and hash match `0x00 || data`, then return that address without another CREATE2 operation. Reject a mismatch. Repeating CREATE2 at an occupied address does not itself provide idempotence.

**P01b.** A zero address returned by CREATE2 must not be reported as success. Do not use DELEGATECALL or invoke the chunk payload as executable code.

The vectors use the system factory address from the baseline execution section:
`0x0000000000000000000000000000000000c0c005`.
This is a fixture setting, not evidence of a deployed contract. For the anvil experiment, record the actual factory address and regenerate address-dependent vectors if it differs, or provision the fixed address through an explicitly declared test setup. Do not present such provisioning as an ordinary deployment test.

`vectors/chunks.json` contains three complete vectors without ellipses: byte `0x61`; 24575 bytes of `0xaa`; and an exact HTML file. Each includes data, salt, initialization code and its hash, runtime code, and expected address.

## 2. Manifest and files

**B04.** Manifest structure:

```
RLP([1, entryIndex, files])
file  = [path, mimeId, size, contentHash, chunks]
chunk = [addr20, len]
```

**P02.** RLP integers are nonnegative and use the shortest big-endian representation; zero is the empty byte string. `u8` and `u32` constrain the numeric range, not fixed-width padding. `entryIndex` is zero-based. Explicit reviewer confirmation is required because the baseline does not specify all these details.

**B05.** Paths are case-sensitive ASCII, at most 256 bytes long. Each segment matches `[A-Za-z0-9._~-]+` and is neither `.` nor `..`. Files are strictly ordered by path bytes, with no duplicates.

**B06.** MIME identifiers: 1 HTML, 2 CSS, 3 JS, 4 JSON, 5 PNG, 6 JPEG, 7 WebP, 8 SVG, 9 WOFF2, 10 text. The entry file must be HTML.

**B07.** Chunk length is 1..24575. The sum of chunk lengths equals file size. File size is at most 1048576 bytes. Manifest size is at most 65536 bytes. Client limits are 256 files and 4194304 bytes per site.

**P03.** The initial implementation would reject empty files until their representation is explicitly resolved. This is a NEW proposed restriction, not an implication of B07. Do not implement it before reviewer approval.

**B08.** Hash the concatenated original file bytes, excluding each chunk's runtime `0x00` prefix. Hash the complete RLP manifest to obtain `manifestHash`.

**P04.** The publisher splits files in their original byte order into chunks of at most 24575 bytes; the last chunk may be shorter. It does not implicitly compress data, change line endings, add/remove a BOM, or change file encoding.

The minimal-site vector consists of `vectors/index.html` and `vectors/manifest.json`. The manifest is 79 bytes under P02. The HTML ends with exactly one LF byte. Entry index is 0 and its path is `/index.html`.

## 3. Validation and retrieval

**P05.** Before submission, the publisher validates canonical RLP, complete input consumption, field counts, types and ranges, ordering, limits, sums, entry, and MIME. The viewer repeats these validations before execution. Reject nonminimal RLP lengths and trailing bytes.

**B09.** Retrieve each chunk's code and check the `0x00` prefix and declared length. After stripping the prefix, verify length totals and the reconstructed content hash.

**P06.** To reject chunks from another factory, recompute salt, initialization code, and address from retrieved data and the factory in the network profile, then compare addresses. Prefix and length checks alone do not authenticate the factory.

**P07.** Bind all Website and chunk reads for a site load to one specified read block. A concurrent `currentVersion` change must not mix two versions. If that snapshot cannot be read, restart the whole load or return an error; never silently fall back to `latest`. Reconcile this proposal with the browser section's LN/RP rules before approving the retrieval ABI.

**O01.** The reviewer must specify which checks belong to contracts and which belong to the publisher/viewer. Proposal: the contract checks version permissions and manifest bounds, references, and hash; the viewer revalidates all files before executing them. Do not assume the contract can retrieve and validate 4 MiB within one transaction. A published version does not imply content is safe to execute.

## 4. Path normalization

**B10.** Query and fragment are excluded from file lookup. Empty path or `/` selects the entry. `/x/` becomes `/x/index.html`. `%`, `//`, and segments `.` and `..` produce 404.

**P08.** Separate query and fragment first, then validate the path. Do not percent-decode, lowercase, or apply host filesystem normalization. Manifest paths remain explicit file paths; empty aliases and trailing slashes are not accepted there.

| Input | Proposed result |
|---|---|
| Empty or `/` | Entry file |
| `/index.html?x=1#part` | `/index.html` |
| `/docs/` | `/docs/index.html` |
| `/a//b`, `/./a`, `/a/../b`, `/%61` | 404 |
| `/INDEX.html` | Distinct from `/index.html` |

**O02.** Confirm normalization order relative to `httpsUrl` and `site_navigate` validation. M1 must not weaken navigation authorization requirements.

## 5. Website: proposed ABI and state transitions

**B11.** The contract has owner and pendingOwner. PUBLISHER can create, publish, and select versions. OWNER manages publishers, revokes noncurrent versions, and transfers ownership through two steps. Maximum publishers: 16. Maximum versions: 1024. States: draft=0, published=1, revoked=2.

**P09.** Version IDs start at 1; currentVersion=0 means no published site selected. The initial owner is nonzero and is the first registered publisher, counted toward the limit. Publishing permission is not inferred from ownership alone. Ownership transfer leaves the publisher set unchanged; the new owner changes it explicitly.

**P10.** Proposed methods:

```
createVersion(bytes32 manifestHash, uint32 manifestLen,
              address[] chunks, uint32[] lengths) returns (uint32 id)
publish(uint32 id)
setCurrent(uint32 id)
revoke(uint32 id)
setPublisher(address publisher, bool enabled)
transferOwnership(address candidate)
acceptOwnership()
```

Read methods include owner, pendingOwner, versionCount, currentVersion, getVersion, and isPublisher. This is a proposed ABI, not an implemented contract.

**P11.** createVersion creates an immutable-content draft. publish allows only draft→published. setCurrent accepts only published versions. revoke accepts only a published, noncurrent version and permanently marks it revoked. Never edit a version's manifest in place; corrections create a new version. publishedBlock is set at publish time.

**P12.** Only the nominated nonzero candidate can accept ownership. Nomination does not immediately change owner. Renomination replaces the previous candidate. Reject the zero address as publisher. Adding an existing publisher or removing an absent one does not change the count. Proposed v1 has no renounceOwnership operation.

**P13.** Do not introduce inheritance or preceding variables that shift baseline slots 0..5. Proposed layout:

| Slot | Proposed contents |
|---|---|
| 0 | owner address in the low 160 bits |
| 1 | pendingOwner address |
| 2 | uint32 versionCount, then uint32 currentVersion at byte offset 4 |
| 3 | mapping(uint32 => Version) |
| 4 | mapping(address => bool) publishers |
| 5 | uint32 publisherCount |

Version layout: manifestHash at relative slot 0; uint32 manifestLen, uint8 status, then uint64 publishedBlock packed in relative slot 1; Chunk[] at relative slot 2. Each Chunk packs address addr followed by uint32 len into one slot.

Pin and verify the compiler-produced storageLayout. These widths and offsets are proposed additions. Do not fabricate Merkle proof fixtures before they are accepted.

**O03.** Review and fix ABI errors, events, publisher enumeration, and owner/publisher interactions alongside P09–P13. Update the document and vectors before implementation if anything changes.

## 6. T1 contract acceptance tests

Use declared test addresses: A=owner, B=publisher, C=unauthorized caller, D=ownership candidate. No real funds.

| ID | Action and expected result |
|---|---|
| T1-01 | C calls createVersion, publish, or setCurrent: revert with no state change. |
| T1-02 | A grants B; B creates v1, publishes it, and selects it: check every state and slot. |
| T1-03 | A revokes current: revert. B creates/publishes/selects v2; A revokes v1: v1 becomes revoked. |
| T1-04 | A nominates D: owner remains A. C cannot accept. D accepts: owner=D, pendingOwner=0. |
| T1-05 | Store identical data twice, first across two transactions and then in one test transaction: same address and code. |
| T1-06 | Data lengths 0 and 24576 reject; 1 and 24575 accept. Compare runtime bytes exactly. |
| T1-07 | 16 publishers accepted; adding the 17th rejects. Regrant does not increment the count. |
| T1-08 | Missing, draft, and revoked versions cannot become current; the 1025th version exceeds the limit. |
| T1-09 | A nonowner cannot manage publishers, revoke versions, or transfer ownership. |
| T1-10 | Publishing, revoking, or selecting a version changes neither chunk bytes nor that version's manifest. |
| T1-11 | Resuming an interrupted publisher reuses matching deployed chunks and never publishes a partial manifest. |

Rejection means a contract revert or a client validation error depending on ownership of the requirement. Exact revert encodings await O03.

## 7. T16 manifest rejection tests

`vectors/negative-manifests.json` contains 11 literal rejected inputs: unknown MIME, CSS entry, inconsistent size, wrong hash, dot segment, non-ASCII path, duplicate path, unsorted paths, zero chunk length, trailing byte, and noncanonical RLP.

Additional mocked code-provider tests must cover: identical data at another factory's address; empty code; nonzero prefix; wrong code length; and one modified data byte. Identify the exact rejection rule without claiming arbitrary hashes can be chosen on a real network.

Generate boundary fixtures for bytes, file counts, and version counts. Small-file tests are not substitutes for these boundaries.

## 8. Publisher workflow

Validate inputs and compute a deployment plan before sending transactions. Present file and manifest byte totals, chunk counts, predicted addresses, and estimated costs separately from measured costs. Submit chunks, verify their code, submit manifest chunks, then createVersion/publish/setCurrent according to the user's choice. An earlier error must not change the current version. Save a local checkpoint without private keys. Do not deploy to a public network or send transactions without explicit authorization.

## 9. Review and implementation gate

Claude reviews the baseline, proposals P01–P13, and open decisions O01–O03, then updates the document. Codex reviews the changes, linking each finding to a rule and vector. A bare approval is insufficient: close every O item and fix the ABI, storage layout, and validation responsibility. Then Claude implements ChunkFactory, Website, and the publisher on an experimental branch; Codex runs EVM tests and reports actual results and tool versions.

There is no fixed limit on repair rounds. An affected unit does not enter implementation until its specification is resolved. M1-spec as a whole is not complete until the implementation section's required annexes are completed. A conflict with the baseline opens a documented change request rather than silently modifying that baseline.

Validation performed for this package: vector generation, matching official CREATE2 example 0, offset/length checks, and RLP decoding/round-trip verification. No contract, anvil, or Chrome execution occurred. The user's local Claude and Codex were not invoked. M0's three independent hash-library checks have not been performed.

## Sources

- `reference/FINAL_DESIGN.md`: approved baseline, particularly storage, execution, validation, implementation, and D27.
- https://eips.ethereum.org/EIPS/eip-1014 — CREATE2 address formula and occupied-address behavior.
- https://eips.ethereum.org/EIPS/eip-170 — runtime code size limit.
- https://ethereum.org/en/developers/docs/data-structures-and-encoding/rlp/ — RLP and integer representation.
