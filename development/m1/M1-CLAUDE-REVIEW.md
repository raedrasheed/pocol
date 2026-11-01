# M1 Specification Review — Claude

Reviewed package: `M1-SPEC.md` (Implementation Draft 0.1), `README.txt`, `VALIDATION.json`, `generate_vectors.py`, `vectors/*`.
Baseline: `reference/FINAL_DESIGN.md`, fingerprint `e8a19ecb306c7b955cddc75b050fdda95f9829c0e2b5cc06563cb27dba851aa0` (matches `reference/baseline.json` and the M1-SPEC header).
Review date: 2026-10-06.

**Status of this document.** This is a specification review only. It approves nothing. Every P item stays proposed and every O item stays open until the gate in M1-SPEC §9 closes it. No contract was implemented, no dependency was installed, nothing was deployed, and no transaction was sent. `M1-SPEC.md`, `reference/` and `vectors/` were not modified.

Location conventions: `SPEC:Lnn` is a line in `M1-SPEC.md`. `FD:Lnn` is a line in `reference/FINAL_DESIGN.md`, whose text is Arabic. Quotations from FD below are my English renderings, not authoritative text.

Severity scale:
- **High**: contradicts the baseline, or leaves a security-relevant or implementation-blocking behaviour undefined.
- **Medium**: an ambiguity or gap that would let two conforming implementations diverge, or a test that can pass for the wrong reason.
- **Low**: clarity, completeness or hygiene.

---

## 1. Summary

| ID | Sev | Area | Title |
|---|---|---|---|
| F01 | High | P03 / B07 | P03 forbids empty files, but the baseline grammar already admits them |
| F02 | High | B01 / T1-06 | On PoCol `B_code_max = 32768`, so the factory itself must reject `|data| > 24575`; anvil masks this |
| F03 | High | B05 / P08 | B05 drops the baseline path grammar `('/' seg)+` |
| F04 | High | P07 / P11 | Explicit version selection `@v<n>` is unspecified for draft, revoked and missing versions |
| F05 | High | P08 / O02 | Resolving relative references conflicts with the 404-on-dot-segment rule |
| F06 | Medium | P06 | The factory address P06 needs has no source in the network profile; P06 actually implements a baseline T16 requirement |
| F07 | Medium | P07 | Pinning reads to a block number is unsafe under reorg and ignores the historical-state window |
| F08 | Medium | P07 / P13 | The retrieval ABI is unresolved: `eth_getStorageAt` slot reads vs. `eth_call` |
| F09 | Medium | O01 / P10 | Contract/client responsibility split; the manifest chunk array in `createVersion` is unbounded |
| F10 | Medium | B11 / P11 | Drafts cannot be removed and count toward 1024, so retries and resumes can exhaust version IDs |
| F11 | Medium | P09 | After ownership transfer the old owner keeps publish rights; T1-07 is ambiguous about counting the owner |
| F12 | Medium | P10–P12 | Undefined edge transitions (no-op calls, zero nomination, self-removal) |
| F13 | Medium | T16 vectors | 3 of 11 negative vectors contain several faults at once and cannot prove that a specific rule is enforced |
| F14 | Medium | T16 | Baseline T16 cases and P02/B05 rules have no literal fixtures |
| F15 | Medium | §7 | The boundary fixtures that §7 requires are absent |
| F16 | Medium | B07 | "4194304 bytes per site" is not defined (sum of sizes, unique bytes, or including the manifest) |
| F17 | Medium | T1 | T1-01, T1-02 and T1-11 are not executable as written, or can pass for the wrong reason |
| F18 | Medium | P08 / O02 | Parse order among omnibox `@v<n>`, query, fragment and `%` is unspecified |
| F19 | Low | P02 | Encoding details are missing: version literal, `entryIndex` width, fixed byte-string lengths |
| F20 | Low | P01a | The mismatch branch is unreachable on a real chain; it can only be tested through declared state injection |
| F21 | Low | B02 | The initcode uses `PUSH0` (Shanghai+); the hardfork and compiler EVM version are not pinned |
| F22 | Low | P01 | The factory's own runtime bytecode is not pinned, although it is a genesis system contract |
| F23 | Low | §1 | Expanding `0x…C0C005` is an inference from an elided baseline value |
| F24 | Low | §8 | The publisher workflow lacks confirmation depth, a checkpoint schema, and a definition of the T25 chunk count |
| F25 | Low | B07 | Baseline retrieval limits (8 connections, 10 s, −32021 retry) are omitted |
| F26 | Low | Scope / §9 | The per-unit implementation gate interprets the baseline and needs a recorded decision |

Section 2 shows the vector verification results. Section 3 gives each finding in full. Section 4 maps every P and O item to findings, with a recommended disposition that is not an approval. Section 5 lists what I could not verify.

---

## 2. Vector verification (independent recomputation)

`pycryptodome` and `rlp` are not installed, and the brief forbids installing them, so `generate_vectors.py` was **not run**. Instead I wrote a separate pure-Python Keccak-256 and a strict RLP decoder in the session scratchpad, outside the project, and recomputed every field. The Keccak implementation was checked against `keccak256("") = c5d2…a470` and against the EIP-1014 example 0 (`0x4d1a2e2b…bf38`). It is a fourth implementation, not one of M0's three libraries.

| Check | Result |
|---|---|
| `chunks.json` ×3: salt, initcode, initcodeHash, runtime, runtimeLen, dataLen, CREATE2 address with factory `0x…00c0c005` | All match |
| Initcode layout: `PUSH2 L; DUP1; PUSH1 0x0a; PUSH0; CODECOPY; PUSH0; RETURN`, runtime at offset 10 | Correct by opcode trace. Initcode sizes are 12, 24586 and 122 bytes, all below the EIP-3860 limit of 49152 |
| `index.html`: 111 bytes, one trailing LF, no CR, identical to the `html` chunk data | Match |
| `manifest.json`: length 79, `hash`, `fileHash`; chunk address in the manifest equals the `html` vector address `0xbffcaeb4…ad3b` | Match |
| Positive manifest under a strict validator (canonical RLP, shapes, ranges, order, sum, hash, prefix, length) | No errors |
| Negative manifests: every one is rejected | Yes. Which rules fire is shown below |

Rules triggered by each negative vector under my validator:

```
unknown-mime       -> mime range, entry-not-html                      (2 faults)
entry-css          -> entry-not-html
wrong-size         -> sum != size
wrong-content-hash -> content hash
dot-segment        -> segment '.'
non-ascii          -> non-ASCII, segment regex                        (same rule family; acceptable)
duplicate-path     -> order
unsorted-path      -> order
zero-chunk-length  -> chunk len range, code length, sum != size       (3 faults)
trailing-byte      -> trailing input
noncanonical-rlp   -> non-canonical single byte; also not a list      (2 faults)
```

---

## 3. Findings

### F01 — High — P03 forbids empty files, but the baseline admits them

**Source.** SPEC:L65 (P03), SPEC:L63 (B07); FD:L892–893.

**Explanation.** The baseline manifest grammar is `chunks [[addr20, len u32]…]` with `1 ≤ len ≤ 24575` and `Σlen = size ≤ 1 MiB`. An empty file is therefore representable without ambiguity: `size = 0`, `chunks = []`, `contentHash = keccak256("") = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470`. P03 calls this representation "unresolved" and proposes rejecting empty files. That would be a restriction of baseline behaviour. Under §9 ("a conflict with the baseline opens a documented change request"), it cannot be adopted as a P item. Empty files are common on real sites (`.nojekyll`, placeholder CSS, empty JSON fixtures). Rejecting them would also break T12-style site imports for no stated security reason.

**Proposed resolution.** Withdraw P03. State explicitly that `chunks = []` if and only if `size = 0`. If the authors still want the restriction, open a change request against FD storage with a rationale.

**Acceptance test.**
- Positive vector: manifest with `/index.html` (entry) plus `/empty.css` (`mime=2, size=0, hash=c5d2…a470, chunks=[]`). The publisher accepts it, the viewer accepts it, the blob is 0 bytes, and the reference implementation makes zero `eth_getCode` calls for that file.
- Negative vector: `size=0` with one chunk `len=1` is rejected for sum mismatch or chunk-without-content.
- Negative vector: `size=1` with `chunks=[]` is rejected for sum mismatch.

---

### F02 — High — The factory must enforce the 24575-byte bound itself

**Source.** SPEC:L19 (B01), SPEC:L39 (P01b), SPEC:L154 (T1-06); FD:L2247 (`B_code_max=32768`), FD:L2318 (R9 `B_code_max ≥ 24577`), FD:L886.

**Explanation.** On anvil, EIP-170 caps runtime at 24576 bytes. A 24576-byte `data` produces a 24577-byte runtime, so `CREATE2` fails and returns 0. T1-06 would then pass through P01b even if the factory contained no length check at all. PoCol's chain parameter is `B_code_max = 32768`, so on PoCol the same call **succeeds** and deploys an out-of-spec chunk. Every conforming client would then reject that chunk. The bound in B01 must be an explicit precondition in the factory, not something inherited from the EVM.

**Proposed resolution.** Add to P01: `store` reverts with a specific error such as `ChunkLength(len)` when `len == 0 || len > 24575`, before building the initcode. T1-06 must assert that error, not just "a revert".

**Acceptance test.**
- T1-06a: on anvil with default settings, lengths 0 and 24576 revert with `ChunkLength`. The revert data is compared byte for byte once O03 fixes encodings.
- T1-06b: the same calls on anvil with the code-size limit raised to at least 32768 (to mirror PoCol `B_code_max`) still revert with `ChunkLength`, and no code exists at `predict(data)` afterwards.
- A mutation test that deletes the explicit check must fail T1-06b.

---

### F03 — High — B05 omits the baseline path grammar

**Source.** SPEC:L59 (B05), SPEC:L89 (P08, last sentence); FD:L895.

**Explanation.** The baseline defines a path as `('/' seg)+`: a leading `/` is required, every segment is non-empty, and there is no trailing slash. B05 only restates the segment regex. P08 then reintroduces "empty aliases and trailing slashes are not accepted" as a proposal. Text that is already baseline is being presented as a P item. An implementer reading B05 alone could accept `index.html`, `/`, `/a/` or `/a//b` as manifest paths.

**Proposed resolution.** Restore the grammar in B05 verbatim: `path = ('/' seg)+`, `seg = [A-Za-z0-9._~-]+`, `seg ∉ {'.', '..'}`, `len(path) ≤ 256` bytes including the leading `/`, case-sensitive. Then remove the duplicated clause from P08. Also state whether segments such as `...` and `~` are valid (the grammar says yes).

**Acceptance test.** Manifest negative vectors, each with a single fault: `index.html` (no slash), `/`, `/a/`, `//a`, `/a//b`, `/..`, `/a/..`, a 257-byte path, and `/a b`. Positive vectors: a 256-byte path, `/...`, and `/~a/b-c_d.e`.

---

### F04 — High — Explicit version selection (`@v<n>`) is unspecified

**Source.** SPEC:L81 (P07), SPEC:L122 (P11); FD:L967 (omnibox `…[/path][@v<n>]`), FD:L914 ("revoke prevents the *default* display"), FD:L4622 / L4627 (T25 (e) "fetch v2 and v1@v1"; (f2) "version not found at this height").

**Explanation.** The baseline lets a user load a non-current version explicitly, and says revocation only blocks *default* display. M1-SPEC never says what the viewer does for `@v<n>` when version n is a draft, published but not current, revoked, `> versionCount`, or 0. This is security-relevant. Revocation exists to stop people viewing compromised content, and drafts have never been approved by `publish`. It also affects P07: which block the reads pin to, and how `publishedBlock` is used.

**Proposed resolution.** Add a normative table, as an O item for the reviewers, with this recommended content:

| Status of n | Viewer behaviour |
|---|---|
| 0 or > versionCount at the snapshot | "Version not found at this height" (T25 f2 wording), no frame |
| draft | Refuse, no frame |
| published (current or not) | Load; banner names the version if it is not current |
| revoked | Refuse by default; any override is a separate decision recorded as a change request |

**Acceptance test.** Fixture site with v1 revoked, v2 draft, v3 current and v4 nonexistent. `@v1` gives the revoked refusal, `@v2` the draft refusal, `@v3` loads, `@v4` gives "not found", and `@v0` gives "not found". Every refusal has zero chunk fetches and zero frames.

---

### F05 — High — Relative-reference resolution vs. the 404 rule for dot segments

**Source.** SPEC:L87 (B10), SPEC:L89–97 (P08), SPEC:L99 (O02); FD:L897, FD:L1193, FD:L1340–1344 (the sanitizer rewrites `src`/`href`/`url()`/`@import`), FD:L1354 (link interception), FD:L2730 (BR18c `'/a/../b.html'` gives 404), FD:L2643 (T12 includes "relative css" and "folders").

**Explanation.** Two different strings reach path handling:
1. **Document-relative references** inside HTML and CSS (`../style.css` from `/docs/page.html`). The loader must resolve these against the current file path, or T12 "relative css / folders" sites break.
2. **Literal paths** passed to `site_navigate`, where BR18c requires `/a/../b.html` to give 404.

If both go through WHATWG `URL`, case 2 is collapsed to `/b.html` before normalization and BR18c fails. If neither resolves dot segments, case 1 gives 404 and T12 fails. Neither the spec nor P08 says which step resolves relative references, or whether an anchor `<a href="/a/../b.html">` (absolute, but containing dot segments) is treated like case 1 or case 2.

**Proposed resolution.** Make it two normative stages:
- **Stage R (loader only):** resolve a relative reference against the current manifest path with RFC 3986 §5.2 merge plus `remove_dot_segments`, without percent-decoding. Only references that do not begin with `/` qualify.
- **Stage N (viewer and `site_navigate`):** P08 normalization on the literal result. Absolute references and `site_navigate` arguments skip Stage R, so `/a/../b.html` gives 404 everywhere.

The reviewers must also decide whether a relative reference that climbs above the root (`../../x` from `/a.html`) gives 404 or clamps to `/x`. I recommend 404.

**Acceptance test.** Fixture site `/docs/page.html` containing `<link href="../style.css">`, `<img src="img/a.png">`, `<a href="/a/../b.html">`, `<a href="../../x.html">`. Expected results: the CSS loads `/style.css`, the image loads `/docs/img/a.png`, the first anchor gives 404, and the second gives 404 under the recommendation. BR18c passes unchanged.

---

### F06 — Medium — P06 has no factory-address source, and it is really a baseline requirement

**Source.** SPEC:L79 (P06), SPEC:L41–43; FD:L907 (T16 includes "another factory"), FD:L972 (profile fields), FD:L1309 (ChunkCache treats the address as a content key), FD:L777 (`ChunkFactory: 0x…C0C005`).

**Explanation.** The baseline T16 already requires rejecting chunks from another factory, and FD:L1309 relies on the address being a content key. P06 is the only mechanism in the package that achieves this, so its requirement is **B**, and only its mechanism is P. P06 says "the factory in the network profile", but the baseline profile `{profileId, name, chainId, genesisPre, genesisHash, forkSchedule, endpoint, trustLevel}` has no factory field. Adding one is a change to the baseline. Phase A runs on anvil, where the factory normally lands at a different address.

**Proposed resolution.** Relabel the requirement B (FD T16) and keep the mechanism as P06. Make the factory address a protocol constant `0x0000000000000000000000000000000000c0c005` (subject to F23). For anvil, provision the compiled factory runtime at that address through an explicitly declared test setup, as §1 already allows. Do not extend the profile unless a change request is filed. Define a machine rule ID, e.g. `chunkFactoryMismatch`.

**Acceptance test.**
- A mock code provider serves the `html` chunk bytes (`0x00 ‖ html`) at `address(create2(factory=0x…c0c006, …))`, and a manifest references that address. Expected: rejection with `chunkFactoryMismatch`, the content-hash check is never reached, and no ChunkCache entry is created.
- On the anvil setup, `extcodehash(0x…c0c005)` equals the pinned factory runtime hash (see F22).

---

### F07 — Medium — Pinning to a block number is not reorg-safe

**Source.** SPEC:L81 (P07); FD:L938 (`K_eff ≤ 32`), FD:L548 (outside the window gives −32017), FD:L1161 (bridge tag grammar rejects EIP-1898 objects), FD:L6288 (RP allows consistent lies).

**Explanation.** "One specified read block" by number does not stop mixing across a reorg: reads at height N before and after a reorg come from different blocks. Historical state is only served within `K_eff ≤ 32`, so a slow load can hit −32017 partway through. P07 does not say whether the pin is `latest`, `safe` or `finalized` at load start. In RP mode, pinning adds consistency but no authenticity. Chunk code is immutable and tied to its content by P06, so only the Website slot reads actually need the pin.

**Proposed resolution.** Pin by `(number, blockHash)` taken at load start. Read Website slots at that number, then re-check `eth_getBlockByNumber(N).hash` after the last Website read, and restart on mismatch. Use EIP-1898 internally only if pocold supports it, which I could not verify. On −32017 or a hash mismatch, restart the whole load, at most 3 times, then show an explicit error. Never fall back to `latest`. State that in RP the pin gives consistency only.

**Acceptance test.** With a mocked RPC:
- (a) `currentVersion` changes between the slot reads and the chunk reads: the load shows exactly one version's bytes.
- (b) The block hash at N changes mid-load: the load restarts and the restart counter increments.
- (c) −32017 on the second Website read: restart, and after 3 failures an explicit error with zero frames.
- (d) Request logs contain no `latest` tag after the pin.

---

### F08 — Medium — The retrieval ABI is unresolved

**Source.** SPEC:L81 (P07), SPEC:L126–139 (P13); FD:L4958 (M1-spec must contain "Website slots and permissions"), FD:L1170 (`eth_getStorageAt` is light), FD:L1960 (`eth_call` is heavy, D88).

**Explanation.** The baseline asks M1-spec for the slot layout, which implies the viewer reads Website state with `eth_getStorageAt`. The `eth_call getVersion` route is classified as heavy. M1-SPEC defers the choice, yet P13 widths and offsets matter only if clients read slots. Without the choice, nobody can say how many requests a load costs or what the retrieval test fixtures look like.

**Proposed resolution.** Decide on slot reads. Add literal slot-key derivations in the baseline's MinerRegistry style:
- `Version[id].base = keccak256(be256(id) ‖ be256(3))`
- packed word at `base+1`: `manifestLen` at bytes 0–3, `status` at byte 4, `publishedBlock` at bytes 5–12, counting from the low-order end
- chunk array length at `base+2`; element i at `keccak256(be256(base+2)) + i`, holding `addr` in the low 20 bytes and `len` in the next 4

Reviewer-computed, **conditional on P13 being accepted and not checked against solc**:
`Version[1].base = 0xa15bc60c955c405d20d9149c709e2460f1c2d9a497496a7f46004d1772c3054c`; first chunk word of version 1 at `0x126fa0859c3b6c65087899d2a6ef4db9ca4209343a9d5478bc96a6404374bc07`.

**Acceptance test.** After T1-02, run `eth_getStorageAt` on each derived key. The decoded values must equal `getVersion(1)`, and the compiler's `storageLayout` output must match the pinned JSON byte for byte. A unit test feeds the decoder the literal word values and checks the decoded fields.

---

### F09 — Medium — O01 split; unbounded manifest chunk array

**Source.** SPEC:L83 (O01), SPEC:L107–120 (P10); FD:L904 (manifest ≤ 64 KiB, versions ≤ 1024).

**Explanation.** P10 accepts `address[] chunks, uint32[] lengths` but sets no bound and no canonical split. Lengths of 1..24575 summing to `manifestLen ≤ 65536` allow up to 65536 entries, one storage slot each. Gas caps this in practice, but behaviour then depends on gas rather than the spec. The O01 proposal leaves the meaning of "references and hash" open.

By my estimate (not measured), the contract can cheaply verify the manifest. That means `EXTCODECOPY` of at most 3 chunks totalling ≤ 64 KiB, `keccak256` over the bytes compared with `manifestHash`, and a CREATE2 recomputation per chunk to prove factory origin. Roughly 50–150k gas, well under the 8M block gas limit. It cannot reasonably parse the RLP or validate up to 4 MiB of content.

**Proposed resolution** (for the O01 decision):
- **Contract.** Role and state checks. `1 ≤ manifestLen ≤ 65536`. `chunks.length == lengths.length == ceil(manifestLen / 24575)`, with every chunk 24575 bytes except the last (canonical split, consistent with P04). Each chunk's code is `0x00 ‖ data` with the declared length and was created by the configured factory. `keccak256(concat) == manifestHash`.
- **Publisher and viewer.** Everything in P05 and B09, including all content files.
- State in the contract NatSpec that a published version is not validated content.

**Acceptance test.** `createVersion` reverts for each of: array length mismatch; `manifestLen = 0`; `manifestLen = 65537`; a non-canonical split (`[24574, 2]` for 24576 bytes); a chunk address with no code; a chunk from another factory, where anvil's state injection installs `0x00‖data` at a non-factory address; and `manifestHash` off by one bit. It succeeds for a 65536-byte manifest split as `[24575, 24575, 16386]`. Gas for that call is recorded as measured.

---

### F10 — Medium — Version IDs can be exhausted by drafts

**Source.** SPEC:L103 (B11), SPEC:L122 (P11), SPEC:L159 (T1-11).

**Explanation.** Drafts count toward the 1024 limit and cannot be revoked or deleted under P11. Each interrupted publish that dies after `createVersion` can leave an orphan draft. A buggy or compromised publisher can permanently exhaust the site's version space, and recovery means deploying a new Website contract at a new address. T1-11 does not cover crashes that happen after `createVersion`.

**Proposed resolution.** Require the publisher to look for an existing draft with the same `(manifestHash, manifestLen, chunks)` and reuse it. A checkpoint is not enough, because it can be lost. Optionally the contract could reject a second draft with an identical `manifestHash`; that is an O03 decision. Document that exhaustion is permanent.

**Acceptance test.**
- Kill the publisher after `createVersion` is mined and restart it: `versionCount` grows by exactly 1 overall, and the same draft ID is published.
- Create 1024 versions; the 1025th `createVersion` reverts, and state is unchanged.

---

### F11 — Medium — Owner and publisher interactions

**Source.** SPEC:L105 (P09), SPEC:L155 (T1-07); FD:L902–903.

**Explanation.** (a) P09 makes the initial owner the first publisher, counted toward 16. T1-07's "16 publishers accepted" does not say whether the owner is one of the 16, so it means either 15 or 16 grants. (b) After `transferOwnership`/`acceptOwnership`, the old owner A keeps publish and `setCurrent` rights until the new owner removes them. A site sale or key rotation therefore leaves the old key able to replace live content. That may be acceptable, but it must be deliberate and tested.

**Proposed resolution.** Keep P09's rules if the reviewers accept them. Rewrite T1-07 as "owner plus 15 grants, then the 16th grant rejects", and add a warning to the publisher UI. Alternatively, O03 could decide that `acceptOwnership` removes the previous owner from publishers. Either way, write it down.

**Acceptance test.**
- T1-07′: after deployment `publisherCount = 1` and `isPublisher(A)`. Granting 15 distinct addresses brings the count to 16. The next grant reverts. Re-granting an existing address keeps 16. Removing an absent address keeps 16.
- T1-12 (new): A transfers to D and D accepts. Then `isPublisher(A) == true` (under P09 as written) and A can still call `setCurrent`. After D calls `setPublisher(A, false)`, A's `setCurrent` reverts.

---

### F12 — Medium — Undefined edge transitions

**Source.** SPEC:L122–124 (P11, P12), SPEC:L141 (O03).

**Explanation.** The following are unspecified: `transferOwnership(0)` (is it a cancellation?), `transferOwnership(owner)`, `acceptOwnership` when `pendingOwner == 0`, `setCurrent(currentVersion)`, `publish` of an already published or revoked version, `revoke` of a draft or of a revoked version, `setPublisher(owner, false)`, removing the last publisher, `setCurrent(0)`, and calls with `id > versionCount`.

**Proposed resolution.** Add a complete transition table to O03. My recommendation:
- `transferOwnership(0)` is allowed and cancels the nomination.
- `setCurrent(current)` reverts.
- Every disallowed transition reverts with a typed error.
- `setCurrent(0)` reverts. Note that the baseline then gives no on-chain way to take a site down if no other published version exists; that is out of scope here and is a change request if wanted.
- Removing the owner or the last publisher is allowed.

**Acceptance test.** Table-driven T1-13: for each (state, call, caller) row, assert the success or the exact revert, plus a full slot diff showing no change on revert.

---

### F13 — Medium — Negative vectors with several faults

**Source.** SPEC:L165; `vectors/negative-manifests.json`; verification in Section 2.

**Explanation.** Three vectors fail several rules at once, so they cannot show that a specific rule is enforced:
- `unknown-mime` sets the only file, which is also the entry, to MIME 11. A validator that checks only "entry is HTML" rejects it, so a missing MIME range check goes unnoticed.
- `zero-chunk-length` also breaks `Σlen = size` and the code length check, so a missing `len ≥ 1` check goes unnoticed.
- `noncanonical-rlp` (`0x8101`) is not a list. A validator that only checks shape rejects it without ever checking canonical encoding.

None of the fixtures carries a machine-readable rule ID, although §7 asks tests to "identify the exact rejection rule". The validation order (structural before fetch) is also unspecified.

**Proposed resolution.**
- `unknown-mime`: add a second, non-entry file `/z.txt` with MIME 11 and keep the entry valid.
- `zero-chunk-length`: keep `size` consistent by adding `[addr, 0]` alongside the real `[addr, 111]` chunk. Specify that structural checks run before any fetch, so the expected result is rejection with **zero** `eth_getCode` calls.
- `noncanonical-rlp`: inside the valid 79-byte manifest, re-encode `version` `0x01` as `0x8101` and fix the enclosing list lengths.
- Add `rule` and `stage` (`decode` / `structure` / `fetch` / `hash`) fields to every fixture.

**Acceptance test.** Run each fixture against the reference validator with a mutation harness that disables one rule at a time. Each fixture must pass only when its own rule is enabled, and the reported `rule` must equal the fixture's `rule`.

---

### F14 — Medium — Rejection cases with no fixtures

**Source.** SPEC:L167; FD:L907 (T16 list), FD:L4958 ("T1 and T16 with literal inputs").

**Explanation.** The baseline T16 cases "len mismatch", "another factory" and "code[0] ≠ 0" exist only as prose descriptions of mocked tests, with no literal bytes. Rules from B04/B05/P02 have no fixtures at all.

**Proposed resolution.** Add literal fixtures as `{manifest, codeProvider: {addr: code}, rule, stage}`:
- **Code provider:** empty code; `code[0] = 0x01`; code length = declared len + 1; one data byte flipped (the T11a analogue); a valid chunk from another factory (F06); a chunk referenced twice (positive, allowed).
- **Decoding:** integer with a leading zero (`size = 0x00 6f`); zero encoded as `0x00` instead of `0x80`; a short string in long form (`0xb8 0x0b …`); a short list in long form; a top-level list with 2 or 4 items; a file with 4 or 6 fields; a list where a string is expected and vice versa.
- **Ranges:** version 0 and version 2; `entryIndex = len(files)`; `entryIndex ≥ 2^32` if a width is chosen (F19); MIME 0; MIME 256 (out of u8); `size = 2^32`; `contentHash` of 31 and 33 bytes; `addr` of 19 and 21 bytes; empty `files`.
- **Paths:** see F03.

**Acceptance test.** Every fixture is rejected by both the publisher-side and the viewer-side validators with the same `rule`. The viewer side makes zero fetches for every fixture with `stage ∈ {decode, structure}`.

---

### F15 — Medium — Boundary fixtures are absent

**Source.** SPEC:L169; SPEC:L63 (B07), SPEC:L103 (B11).

**Explanation.** §7 requires boundary fixtures but the package ships none. Pairs needed:
- chunk length 24575/24576 (manifest side)
- file 1048576/1048577 bytes (43/44 chunks)
- manifest 65536/65537 bytes
- 256/257 files
- 4194304/4194305 site bytes (see F16)
- path 256/257 bytes
- versions 1024/1025
- publishers 16/17

**Proposed resolution.** Extend `generate_vectors.py` deterministically, using a seeded content pattern rather than literal hex for the large files, and commit hashes of every generated fixture.

**Acceptance test.** Each pair gives accept at the limit and reject one past it, at the layer that owns the rule (contract or client per O01). The generator produces identical hashes on two machines.

---

### F16 — Medium — "Bytes per site" is undefined

**Source.** SPEC:L63 (B07); FD:L905, FD:L1309, FD:L5319 (ChunkCache ≤ 4 MiB + 64 KiB per session).

**Explanation.** Because files may share chunks (T25 reuses `style.css`'s chunk), "4194304 bytes per site" could mean Σ`size`, Σ unique chunk bytes, or either of those plus the manifest. The ChunkCache bound of 4 MiB + 64 KiB suggests unique content plus manifest, but the text never says so.

**Proposed resolution.** Define the limit as Σ`size` over all files. It is checkable from the manifest alone before any fetch, and it also bounds the unique bytes. The manifest's ≤ 64 KiB is separate. State that duplicate chunk references are allowed.

**Acceptance test.** Two files of 2097152 bytes each sharing identical chunks: Σ`size` = 4194304, accepted. Add one 1-byte file: rejected before any fetch, even though unique bytes would still be ≤ 4 MiB.

---

### F17 — Medium — T1 tests are under-specified

**Source.** SPEC:L149–161.

**Explanation.**
- **T1-01:** if no version exists, `publish` and `setCurrent` by C revert with "missing version", not "unauthorized". The test passes for the wrong reason.
- **T1-02:** "check every state and slot" has no literal expected values.
- **T1-05:** does not check that the second `store` emits no CREATE2.
- **T1-06:** see F02.
- **T1-10:** does not say which fields may change (`status`, `publishedBlock`, `currentVersion`).
- **T1-11:** a publisher test with undefined crash points and an undefined checkpoint format.
- "Exact revert encodings await O03" leaves every negative T1 assertion weak.

**Proposed resolution.**
- **T1-01:** preconditions are a v1 draft (for `publish`) and a published v2 (for `setCurrent`). Assert the `Unauthorized` error once O03 defines it.
- **T1-02:** publish a literal expected-slot table, keyed as in F08.
- **T1-05:** assert from the call trace that there is no CREATE2 on repeat, and that `predict(data) == store(data)`.
- **T1-10:** full slot diff with the allowed fields listed.
- **T1-11:** define crash points K1 (after k of n chunks), K2 (after manifest chunks), K3 (after `createVersion`), K4 (after `publish`), and a JSON checkpoint schema without keys.

**Acceptance test.** These are the revised tests themselves. For each, a deliberately broken contract variant (auth check removed, `status` not written, CREATE2 repeated) must fail exactly the intended test.

---

### F18 — Medium — Parse order for omnibox, query, fragment and `%`

**Source.** SPEC:L87–99; FD:L967, FD:L1164 (`pathStr` = printable ASCII 0x21–0x7e, 1..2048, leading `/`), FD:L2730 (BR18c `/a%2fb`, `//x`).

**Explanation.**
- The omnibox form `…/path@v<n>` and `pathStr` both allow `@`, `?`, `#` and `%`. Is `@v2` in `/a.html?x=@v2` a version selector?
- Is `/x?y#z` cut at `#` first?
- Does `%` inside the query give 404? (The original order suggests yes; P08 says no.)
- Does `/docs` resolve when only `/docs/index.html` exists?
- Does `/x/` expansion that produces a path over 256 bytes give 404?
- BR18c vectors `/a%2fb` and `//x` are missing from the P08 table.

**Proposed resolution.** Normative order:
1. Omnibox only: strip a trailing `@v<digits>` matched against the whole input before `?` and `#` are considered. This is O02 input.
2. Cut at the first `#`.
3. Cut at the first `?`.
4. Apply B10 to the remaining path.

There is no implicit directory resolution (`/docs` gives 404 unless the file `/docs` exists). Over-length after expansion gives 404.

**Acceptance test.** Extend the P08 table with:

| Input | Expected |
|---|---|
| `/a#b?c` | `/a` |
| `/index.html?q=%20` | `/index.html` |
| `/a%2fb` | 404 |
| `//x` | 404 |
| `/a\b` | 404 |
| `/a;b` | 404 |
| `/docs` (only `/docs/index.html` exists) | 404 |
| 252-byte `/x…/` | 404 after expansion |
| omnibox `…/a.html@v2` | path `/a.html`, version 2 |
| omnibox `…/a.html?x=@v2` | path `/a.html`, current version |

Run the identical table through Navigator and through the viewer's initial load.

---

### F19 — Low — P02 encoding gaps

**Source.** SPEC:L49–57; FD:L892.

**Explanation.** P02 does not state:
- that the leading `1` is an integer that must equal 1;
- the range of `entryIndex` (the baseline gives no width);
- that `path`, `contentHash` and `addr20` are byte strings of exact length (1..256, 32 and 20).

`u32` for `len` is wider than needed, which is fine but should be acknowledged.

**Proposed resolution.** Add: version = 1 exactly; `entryIndex` is a u32 with `entryIndex < len(files)`; exact byte-string lengths; strings must not appear where lists are expected and vice versa.

**Acceptance test.** Covered by the F14 decoding and range fixtures.

---

### F20 — Low — The P01a mismatch branch is unreachable on a real chain

**Source.** SPEC:L37.

**Explanation.** The CREATE2 address commits to `keccak256(initcode)`. The initcode does not depend on its environment (it copies itself), so on a chain with honest state, code at `predict(data)` can only be `0x00 ‖ data`. A mismatch needs a hash collision or state injected outside the EVM (genesis alloc, `anvil_setCode`). The branch is harmless as defence in depth, but the spec should say it can only be tested through declared injection, as §7 does for other cases.

**Proposed resolution.** Add that note, and add a test that uses a declared state-injection fixture.

**Acceptance test.** (a) Inject `0x00‖data` at `predict(data)`: `store` returns that address with no CREATE2 in the trace. (b) Inject `0x00‖data'`: `store` reverts with `ChunkMismatch`. (c) Inject code of a different length: same revert.

---

### F21 — Low — Hardfork and EVM version are not pinned

**Source.** SPEC:L21–25 (B02).

**Explanation.** `0x5f` (`PUSH0`) requires Shanghai or later. The baseline targets Cancun (FD:L843), but M1-SPEC does not pin anvil's hardfork or solc's `evmVersion`. On a pre-Shanghai configuration the initcode hits an invalid opcode and every `store` fails.

**Proposed resolution.** Pin the anvil hardfork to `cancun` and set solc `evmVersion = cancun` in the experiment record.

**Acceptance test.** The test log records the hardfork and compiler settings. T1-06's accept cases pass under them.

---

### F22 — Low — Factory runtime bytecode is not pinned

**Source.** SPEC:L35; FD:L777, FD:L755 (GenesisBuilder compares `allocRoot`), FD:L851 (M4 check "ChunkFactory without SSTORE").

**Explanation.** The factory is a genesis system contract on PoCol, so its runtime code is part of `allocRoot`. D27 pins the chunk initcode but not the factory's own code.

**Proposed resolution.** Pin the compiler version, settings and source hash. Record the runtime code hash. Run the M4 "no SSTORE" check on the runtime bytecode.

**Acceptance test.** A reproducible build gives the recorded runtime hash. An opcode scan of the runtime finds no reachable `0x55` (SSTORE), and the scan understands PUSH data.

---

### F23 — Low — Factory address expansion is inferred

**Source.** SPEC:L41–43; FD:L777 (`0x…C0C005`, elided).

**Explanation.** The baseline writes the address with an ellipsis. `0x0000000000000000000000000000000000c0c005` is a plausible zero-fill, but it is an inference. `SYSTEM_ADDRESS = 0xff…fe` is elided the same way. Every chunk address vector depends on this value.

**Proposed resolution.** Have the baseline owner confirm the full 20-byte value, or record it as a P item.

**Acceptance test.** A documented confirmation exists. The generator asserts the constant and fails loudly if it is changed.

---

### F24 — Low — Gaps in the publisher workflow

**Source.** SPEC:L171–173 (§8); FD:L4606–4622 (T25).

**Explanation.** The workflow does not specify:
- the confirmation depth required before a chunk counts as deployed (a reorg can remove a chunk after the code check);
- nonce and replacement handling;
- the checkpoint schema;
- whether T25's "n = 44 new chunks for v2" counts the manifest chunk. Fixed 24575-byte splitting (P04) gives 1 + ⌈1032192/24575⌉ = 44 content chunks, so n matches P04 only if manifest chunks are excluded.

**Proposed resolution.** Add a confirmation depth (a parameter, with a default to be decided), a checkpoint schema, and a note that T25's n counts content chunks only.

**Acceptance test.** A simulated reorg removes a chunk after its code check. The publisher re-submits it before `createVersion`, and `createVersion` is never sent referencing a missing chunk. The T25 plan for v2 lists exactly 44 content chunks plus the manifest chunk(s), shown separately.

---

### F25 — Low — Retrieval limits omitted

**Source.** SPEC:L63 (B07); FD:L905.

**Explanation.** The baseline sets ≤ 8 parallel light connections to the local node, a 10 s timeout, and retry after `retryAfterMs` on −32021. M1-SPEC omits all three, even though P07 retries interact with them.

**Proposed resolution.** Copy them into B07 or B09.

**Acceptance test.** A mock provider records at most 8 concurrent `eth_getCode` requests. A load longer than 10 s gives an explicit timeout. −32021 responses are retried no earlier than `retryAfterMs`.

---

### F26 — Low — The per-unit gate needs a recorded decision

**Source.** SPEC:L8, SPEC:L175–181; README.txt; FD:L4955–5000 (the M1-spec contents list includes the extension and bridge annexes, e.g. "M1-spec contains this [BR17e] table literally").

**Explanation.** The baseline describes M1-spec as one document that includes the annexes. M1-SPEC splits the delivery and allows ChunkFactory, Website and the publisher to proceed once their own O items close. That is a reasonable reading, but the baseline does not state it. Under §9 it should be recorded as a decision rather than assumed.

**Proposed resolution.** Record an explicit decision, or a change request, that core units may enter implementation before the annexes are complete. Keep the existing statement that M1-spec is incomplete until the annexes land.

**Acceptance test.** A decision record exists and is referenced from §9. The M2 start gate checks that the annexes are complete.

---

## 4. Proposals and open decisions: recommended dispositions

**None of these is an approval.** "Accept with changes" means I recommend it to the gate in §9 once the linked findings are resolved.

| Item | Recommended disposition | Linked findings |
|---|---|---|
| P01 ABI `store`/`predict`, no storage | Accept with changes: add the explicit length check and typed errors; pin the bytecode | F02, F22 |
| P01a idempotent repeat | Accept with changes: note it is testable only through injection | F20 |
| P01b zero address is not success | Accept | F02 |
| P02 RLP integer rules | Accept with changes: add version literal, `entryIndex` width, exact byte lengths | F19, F14 |
| P03 reject empty files | Reject as a P item; it needs a change request if pursued | F01 |
| P04 deterministic splitting | Accept; also make it the canonical split for manifest chunks | F09, F24 |
| P05 validation list | Accept with changes: rule IDs, structural checks before fetch | F13, F14 |
| P06 factory re-derivation | Relabel the requirement B (T16); accept the mechanism with a constant factory address | F06, F23 |
| P07 snapshot reads | Accept with changes: pin by hash, bounded restart, K_eff handling, define `@v<n>` | F04, F07, F08 |
| P08 normalization order | Accept with changes: two-stage resolution, full parse order, BR18c vectors | F03, F05, F18 |
| P09 IDs, owner as first publisher | Needs decision on post-transfer rights; rewrite T1-07 | F11 |
| P10 method list | Accept with changes: bounded, canonical manifest chunk arrays | F09, F12 |
| P11 state transitions | Accept with changes: full transition table, draft-reuse rule | F10, F12 |
| P12 ownership rules | Accept with changes: define zero nomination and self-nomination | F12 |
| P13 storage layout | Accept with changes: literal slot-key fixtures, pinned `storageLayout` | F08 |
| O01 responsibility split | Open. Recommendation in F09 | F09, F16 |
| O02 normalization vs. `httpsUrl`/`site_navigate` | Open. Recommendation in F05 and F18 | F05, F18 |
| O03 errors, events, enumeration | Open. Needs typed errors for every T1 negative, events for every state change, and a decision on publisher enumeration (a mapping cannot be enumerated from state; events or a bounded 16-entry array) | F11, F12, F17 |

---

## 5. Not verified

- **EVM behaviour.** Nothing was executed on an EVM: CREATE2 at an occupied address, revert data, gas use (including the baseline's estimate of about 5.4M gas per chunk), PUSH0 on anvil's default hardfork, the solc `storageLayout`, and my gas estimate for on-chain manifest verification in F09.
- **Generator.** `generate_vectors.py` was not run, because the dependencies are missing and installing them was out of scope. Its outputs were recomputed field by field with my own Keccak/RLP code. The JSON files were not compared byte for byte against a fresh generator run. My Keccak is not one of M0's three independent libraries, and M0 K1–K3 remain undone.
- **Proposed slot keys.** The slot keys in F08 are my computation under P13 as written, not compiler output.
- **Baseline coverage.** The baseline is in Arabic and was read in my own translation; nuances such as "revoke prevents *default* display" could be misread. I read in full: the header, EVM (FD:L738–868), storage (FD:L869–963), parts of browser (FD:L964–1060, L1160–1360), T1/T16/T11a/T12/BR18/T25 in the validation plan, the M1 and M1-spec contents in the development plan (FD:L4660–5000), and decisions D10/D26/D27/D96. Other sections were searched by keyword only. Of the files under `reference/`, I checked only `storage.md` against FD; it differs only in heading level. `browser.md`, `validation.md`, `implementation.md`, `decisions.json` and the rest were not read in full. A rule that contradicts these findings could exist in text I did not read.
- **Node behaviour.** I did not verify whether pocold supports EIP-1898 block-hash parameters for internal calls, or whether `K_eff` applies to `eth_getCode` and `eth_getStorageAt` at historical blocks (F07).
- **R9.** I could not determine what R9's `B_code_max ≥ 24577` refers to (F02); it may count something other than runtime bytes.
- **Elided addresses.** I did not verify the full 20-byte value behind `0x…C0C005` (F23).
- **Browser and contract runs.** Nothing was run in Chrome or against a real contract, and no Claude or Codex reviewer session other than this one was involved.
