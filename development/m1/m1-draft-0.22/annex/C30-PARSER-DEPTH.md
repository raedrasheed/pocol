# Annex C30: non-recursive GenesisSpec framing parser and E05 supplement (M1 draft 0.22)

**Proposed for review. Not approved. Nothing here was executed by the author.**

## 1. Defect

- The 0.21 parser (`m1-draft-0.21/tools/netprofile_ref.py`) recursed once per nested list.
- Root's independent probe used 1500 canonically framed nested lists around `80`. That is 4291 bytes, well inside the 47104-byte RecvFit bound.
- On that probe `decode_genesis` raised an uncaught `RecursionError`, where it should have rejected the malformed top shape with `gsStructure` (top[0] is a list).
- `validate` uses the same path.

## 2. Repair (`tools/iterative_parse.py`)

`make_parse(GsError)` returns a parser with an explicit stack of open lists `[children, end]`.

**What it keeps from 0.21:**
- Node shapes: `('b', bytes, wrapped)` and `('l', nodes)`. A wrapped single byte (`81 xx`, xx < 0x80) is valid framing and is judged later as gsInt.
- Every L0 detail string, raised at the same point and in the same order:
  - item header errors: truncated, truncated length, leading zero in length, long form for short string or list;
  - `truncated string` / `truncated list` against the whole buffer;
  - `item crosses list end` when a completed child ends beyond its parent;
  - `N trailing bytes` after the root.
- The stage order of the decoder is unchanged.

**What it does not do:** there is no recursion-limit change, no depth limit, no input restriction, and no suppressed exception. Memory is linear in the input.

**Installation.** The runner loads the unchanged 0.21 runner, which brings in its unchanged model, and sets `NP.parse` to the repaired parser. `decode_genesis` resolves `parse` at call time, so the repair takes effect without editing any file.

## 3. Fixtures (`vectors/c30-depth.json`)

- All fixtures are built by a loop, never by recursion.
- Lengths are hand-derived. One wrap of L bytes adds 1 byte while L ≤ 55, 2 while L ≤ 255, and 3 while L ≤ 65535. So 1 byte wrapped 1500 times is 256 + 3·1345 = 4291 bytes, matching the root probe.
- Each deep case (depth 1500) has a shallow twin. Both must produce the same code and detail, so depth never changes precedence.

| Case | Deep / shallow length | Code (detail) |
|---|---|---|
| T-nest (the root probe) | 4291 / 3 | gsStructure (top[0]) |
| T-truncated (last byte dropped) | 4290 / 2 | L0 (truncated list) |
| T-crossing (`c2c28080` inside) | 4300 / 5 | L0 (item crosses list end) |
| T-trailing (+`00`) | 4292 / 4 | L0 (1 trailing bytes) |
| T-longForm (`f80180`) | 4297 / 4 | L0 (long form for short list) |
| T-leadingZero (`f9000180`) | 4300 / 5 | L0 (leading zero in length) |
| T-truncString (`8201` at the end) | 4294 / 3 | L0 (truncated string) |
| T-wrappedInt (`8105`) | 4294 / 4 | gsStructure: structure is judged before gsInt |
| G-cpLeaf (CP element 3 nested) | 4632 / 342 | gsStructure (CP item is a list) |
| G-m0Entry (rewardAddr nested) | 4674 / 342 | gsStructure (M_0List entry) |
| G-extraTop (7th top item nested) | 4632 / 343 | gsCount (top 7) |
| G-versionAndDeepCp (+ version 02) | 4632 / 342 | gsStructure, before gsVersion |
| G-cpLeafTruncated | 4631 / 341 | L0 (truncated list), before structure |

**Checks on these fixtures:**
- **Old parser on every deep input:** RecursionError. The two exceptions are T-truncated and G-cpLeafTruncated, where the 0.21 parser already rejects at the outermost header without recursing.
- **Old and repaired parsers must give identical results** on every shallow twin, on GSV1 and on all 32 old negatives.
- **Repaired parser:** each fixture is decoded twice, with identical results; GSV1 then still decodes and re-encodes byte-identically.
- **`validate` path:** deep T-nest, T-crossing and G-m0Entry profiles are rejected at decode with the trace `shape:ok, decode:fail`. A GSV1 profile then still validates.

## 4. Replay and harness note

- The whole 0.21 suite is replayed with the repaired parser: GSV1, the 32 negatives, every profile, RF1–RF4 and its edges, the transport scripts, and the 0.21 coverage and preservation guards. Entries carry the prefix `suite021.`.
- **Harness note.** The first 0.21 review run failed only `reuse.keccakFrom0.2`. That check compared absolute paths, and the run had imported the preserved review copy of the same files. This is a harness mismatch, not a model semantic failure.
  - The preserved failure result (`coordination/review-001/run-results-0.21-copy-path-failure.json`) is kept and checked.
  - The replay instead checks content hashes against the protected sources, including the E05-recorded Keccak source SHA-256, plus the origin role (`m1-draft-0.2/tools/<file>`).

## 5. E05 supplement (`vectors/v3-e05-supplement.json`)

- Three independent Keccak-256 libraries agree on K1–K3 and on the 341-byte GSV1: the protected Python source, @noble/hashes 2.4.0 and js-sha3 0.13.0.
- GSV1 input SHA-256: `2905b38d045661682f75beda865a447ed22e43e1e288b622a78a4e9833a4bf84`.
- **H_GSV1** = `0xde518e30a5e333ac3ddf127b6201f1a66f80dce2b56566524df4037bcdeb2277`.
- This resolves DG-V3-3. The GSV1 bytes and every other hash-bearing fixture are unchanged.
- No dependency files, private paths or transcripts are copied into this package.

## 6. Unchanged

- CP values, limits, the trust policy, and every 0.21 expectation.
- DG-V3-1, 2 and 4–9 and P-V3-1..7 remain open proposals or gaps.
- Owner decisions, the full gate, CONF_DEPTH, the CR-E4 proposals and RF-E6-1.
