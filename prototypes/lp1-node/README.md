# lp1-node: LP1 local experimental Rust node + LP2 candidate journal (LP2 author revision 1, transport 0.37)

LP1 is a local, experimental M3-foundation slice built on the accepted M1 formats (baseline M1 spec
0.32). It is **not** an M1 amendment and makes **no** change to the baseline. It is not full M3, not
consensus and not an EVM. It has no P2P, no transactions, no contracts or publisher, no keys and no
signing. The M1 contract/publisher phase is **not** claimed as executed.

LP2 adds a durable, **experimental** journal of header **candidates**, with a CLI (see "LP2
candidate journal" below). Candidates are admitted through strict decoding and the scoped H-pre
checks. They are never H-full, executed, canonical, live or consensus. Status always reports
`candidate`, `awaiting: "H-full"`, `executedHeight: 0` and `consensus: false`. The fixture headers
appended in tests are not live block production.

The m1-draft-0.34 to 0.37 folders are only the broker's transport namespace. Root imports the code
onto the prototype branch, generates `Cargo.lock`, copies the fixtures, then builds, tests and
reviews. **The author compiled, ran and formatted nothing.** Every acceptance criterion below is
untested until root runs it.

Revision 2 (0.35) corrected:

- **LP1-I01:** a raw byte string with non-ASCII text in `src/json.rs` tests, which blocked compilation;
- **LP1-I02:** request ids now follow the accepted C19 binary64 value semantics.

Revision 3 (0.36) corrected:

- **LP1-I04:** the public RLP depth option can no longer request unsafe recursion. The hard ceiling is 16, and larger requests are refused.
- **LP1-I05:** the adversarial genesis corpus now classifies byte-identical no-op mutants separately from genuinely changed preimages.

LP2 revision 1 (0.37) adds:

- `src/store.rs`;
- the `store` CLI commands;
- `tests/store_journal.rs`, `tests/store_process.rs` and `tests/e2e_store.py`.

No LP1 module, fixture, RPC behaviour or LP1 test was changed.

## Layout

| Path | Content |
|---|---|
| `src/fixed.rs` | `U256` `[u64;4]` and `U512` `[u64;8]`: decimal and `2^N` parsing, shifts, bit length, long division, `work(t)` |
| `src/asert.rs` | Bounded ASERT: checked `i128` with floor `div_euclid` for dt, e, s and f; `u128` cubic; `U512` X and Y; early exit at s≥256 (MAX) and s≤-257 (1); clamp to 1..MAX; no heap |
| `src/rlp.rs` | Strict canonical RLP (same acceptance as `rlp_strict.py`), hard nesting ceiling 16 (see "RLP depth" below), exact raw spans, encoder |
| `src/header.rs` | V1 header item codec: 18-field UT, sig 0/65, nonce 8, at most 256 shares, winnerSig 65; minimal integers, widths, target ≥ 1, UT/ST/HDR size limits; typed re-encode |
| `src/hashes.rs` | SHA-256 and Keccak-256; TemplateID, powHash and shareHash preimages; sigMsg, winMsg, shareRoot and blockHash; secp256k1 recovery with explicit r/s range, low-S and v∈{0,1}. Verification only. |
| `src/genesis.rs` | GSV1 identity decode (structure, counts, minimal integers, CP widths, fixed lengths) for binding the profile to the literal 341-byte preimage |
| `src/fixtures.rs` | Runtime fixture loading (exact integers, `"2^240"`), profile binding, provenance SHA-256 check |
| `src/window.rs` | RP window port of `v1_ref_027.check_window` (see below for items and order), counters, TemplateID cache per snapshot |
| `src/chain.rs` | Immutable fixture chain, servable only after full validation from the genesis parent; explicit empty chain |
| `src/json.rs` | Strict request JSON (see "RPC" below) |
| `src/rpc.rs` | Read-only router: `eth_chainId`, `eth_blockNumber`, `pocol_getHeaders`; everything else is -32601 |
| `src/http.rs` | Bounded std-only loopback HTTP/1.1 transport |
| `src/store.rs` | **LP2:** experimental candidate-header journal: format, bounded validating scan, exclusive Windows writer, recovery |
| `src/verify.rs`, `src/main.rs` | Fixture checks and the `lp1-node` binary (including `store` commands) |
| `tests/*.rs` | Integration tests that read `fixtures/` at run time; `asert_alloc` (harness=false); seeded adversarial corpus; in-process socket tests; LP2 `store_journal` and `store_process` |
| `tests/e2e_http.py`, `tests/e2e_store.py` | Process-level end-to-end checks (Python standard library only) |

## Fixtures (root copies them byte-identically)

Root copies `coordination/lp1-inputs/*` into `fixtures/`:

- `profile.json`
- `chain.json`
- `window-cases.json`
- `asert-oracle.json`
- `hash-oracle.json`
- `PROVENANCE.json`
- `LICENSES.json`

The author did not rewrite or copy any of them. The binary's default fixture directory is
`<crate root>/fixtures`, fixed at compile time through `CARGO_MANIFEST_DIR`, so the binary runs from
any working directory. `--fixtures DIR` overrides it.

The 16 rows of the native C19 id oracle (`coordination/lp1-review-r1/c19-native-oracle.json`) are
embedded as test constants in `src/json.rs` (`C19_ORACLE`). They are not a fixture file.

The V1NET profile is **synthetic and non-bootable**. Its state, tx, receipts and evidence roots, its
allocation root, system code hash and M_0 entries are opaque fixture bytes.

The genesis binding checks:

- keccak256(preimage) equals the stated genesis hash;
- the GSV1 structure, counts, minimal integers and CP field widths;
- each CP value in `profile.json` equals the CP decoded from the literal preimage (`target_g` `"2^240"` is converted exactly, with no floating point);
- chainId;
- the fork schedule as `[version, startHeight]` pairs.

It does **not** check:

- CP lower or upper bounds beyond width;
- gsOrder or gsSys;
- ParamGate, GenesisBuilder or Draw;
- allocation or system code.

## Commands (run from the crate root; root generates the lock file)

```
cargo fmt -- --check
cargo build --offline
cargo test --offline
cargo test --offline --test asert_alloc
target/debug/lp1-node verify-fixtures
target/debug/lp1-node profile
target/debug/lp1-node window [--case RW-h20-phase0] [--no-cache]
target/debug/lp1-node hash
target/debug/lp1-node asert-batch [--in fixtures/asert-oracle.json] [--out asert-rust.jsonl]
target/debug/lp1-node serve [--bind 127.0.0.1] [--port 0] [--empty-chain] [--max-requests N] [--max-runtime-ms N] [--conn-timeout-ms 5000]
target/debug/lp1-node store init    --store NEW_DIR
target/debug/lp1-node store append  --store DIR (--range A:B | --hex HEADER_RLP_HEX)
target/debug/lp1-node store status  --store DIR
target/debug/lp1-node store recover --store DIR --dest NEW_DIR
python tests/e2e_http.py --bin target/debug/lp1-node.exe --fixtures fixtures
python tests/e2e_store.py --bin target/debug/lp1-node.exe --fixtures fixtures
```

Exit status: 0 means the command's checks passed, 1 means a check failed, and 2 means a usage,
fixture or bind error. `store` commands print exactly one JSON line and use these exit codes:

| Code | Meaning |
|---|---|
| 0 | success |
| 1 | append rejected or not linear; nothing was written |
| 2 | usage, I/O, busy, bound, metadata, profile mismatch, corrupt, or existing target |
| 3 | recovery required (`status` prints the verified-prefix status first) |

`--range A:B` takes headers A..=B from the fixture `chain.json`.

`asert-batch` writes one JSON line per oracle row: `{"i", "target"` (32-byte hex), `"early",
"maxShiftBits", "maxBits", "match"}`, then a summary line. Root can use it for an independent
differential over the 1027 rows. `maxShiftBits` is the native oracle metric max(bits X, bits Y).
`maxBits` is the `v1_ref_027` metric, which also covers |num|, |e| and poly.

`serve` prints exactly one ready record on stdout before accepting:

```
{"event":"ready","transport":"lp1-loopback-http-serial","addr":"127.0.0.1","port":<actual>,"head":20,"chainId":"0xbdb2a","maxRequests":N|null,"maxRuntimeMs":N|null,"connTimeoutMs":N,"m7":false}
```

It logs one JSON line per connection on stderr. When a bound is reached it prints
`{"event":"shutdown","served":N,"reason":"maxRequests"|"maxRuntime"}` and exits with status 0. The
default port 0 is ephemeral. The server never uses any existing coordinator port or process.

## RLP depth (LP1-I04)

The strict decoder recurses once per nested list, so the nesting limit is also the recursion limit.
`rlp::MAX_DEPTH = 16` is both the default and a **hard prototype ceiling**.

| Call | Result |
|---|---|
| `decode(b)` | Uses limit 16. |
| `decode_with_depth(b, d)` with `d` in 0..=16 | Admits at most `d` nested lists. Limit 0 means byte strings only. |
| `decode_with_depth(b, d)` with `d` > 16 | Refused with `ErrKind::LimitAboveCeiling` (`decode.depthLimit`) before the input is read. Never clamped silently. |
| Input nested deeper than the limit | `ErrKind::TooDeep` (`decode.depth`), detected before descending further. |

As a result, recursion never exceeds 17 frames for any input or request, and no larger thread stack
is needed. Unit tests cover:

- 16 levels accepted by default and 17 refused;
- a 2000-level well-formed input refused at the default limit and at an explicit 16;
- requests of 17, 2000, 2001 and `usize::MAX` refused on that input, and on trivial and empty input;
- for every d in 0..=16, d levels accepted and d+1 refused;
- framing errors still reported first at limit 0.

The only internal caller with a smaller limit is the genesis decoder (3). The accepted M1 depth rules
are unchanged.

## RP window (A5)

The window logic is ported from `m1-draft-0.27/tools/v1_ref_027.py`.

**Plan:**

- b = max(1, h-12);
- from = max(1, b-1);
- count = h - from + 1;
- n = h - b + 1.

At h = 0 the result is `viewNoBlocks`, and no headers are requested.

**Decode stage:** the outer list must hold exactly `count` items. Every item is decoded strictly
(rule 1), and every height must match its position (otherwise `viewIncomplete`). The reference header
gets netid and then viewFuture.

**Each window header** is checked in this order:

1. netid (`netChain`, `netGenesis`, `netVersion`)
2. viewFuture
3. item 2: height, parent hash or `viewGenesis`, H_END
4. item 3: template signature form and recovered address
5. item 4: stamp window and fallback stamp
6. item 5: ASERT
7. viewTargetCeil (ceil = min(MAX, target_g·16))
8. item 6: nonce < n_max
9. item 7: powHash ≤ target
10. item 8: winner signature recovers
11. item 9: shares are strictly ascending, < n_max and ≠ nonce, with shareHash ≤ min(MAX, target·m)

viewFuture runs before ASERT, and the ceil check runs before PoW. **Items 10–17 are not evaluated.**

**Counters** use the 0.27 transcript key set. The TemplateID cache is per snapshot. The
`--no-cache` run reports a higher SHA-256 cost and gives the same verdicts. Expected named values:

- RW-h20: 14 decodes, 13 ASERT, 14 TemplateIDs, 13 powHash, 26 ecrecover, 27 SHA-256, maxAsertBits 257.
- SHA-W: 3328 shareHash and 3355 SHA-256 in total.

## RPC (A6)

**Methods served:**

- `eth_chainId` returns `"0xbdb2a"`.
- `eth_blockNumber` returns `"0x14"`, or `"0x0"` with `--empty-chain`.
- `pocol_getHeaders` returns the real RLP list of the stored header encodings, in ascending order.

`params` may be absent or `[]` for the first two methods.

**F0–F4** follow network.md:172-181 in fixed priority:

| Check | Condition | Error |
|---|---|---|
| F0 | exactly two canonical u64 quantities ("0x" + lowercase hex, no leading zero, at most 16 digits) | -32602 `{path:"params"\|"params[i]"}` |
| F1 | from < 1 | -32018 `{reason:"fromZero"}` |
| F2 | count outside 1..512 | -32018 `{reason:"countRange"}` |
| F3 | from > head | -32018 `{reason:"fromAboveHead"}` |
| F4 | from+count-1 > head, computed in u128 | -32018 `{reason:"beyondHead"}` |

The -32018 message is `"params"`, as in `m1-draft-0.26/vectors/v1-window-cases.json`. F0–F2 do not
read state. The chain is immutable, so the head is a per-request snapshot by construction.
Quantities stay exact: they are hex strings parsed as u64, never floats.

**Every other method returns -32601 `{reason:"unsupported"}` and leaves state unchanged.** This
includes:

- wallet and account methods;
- signing methods;
- write and submission methods.

The router holds only a shared reference, and tests compare a state digest before and after. The RPC
serves the LP1 fixture chain only; it does not read the LP2 candidate journal.

**Request JSON rules:**

- The body must be valid UTF-8.
- The grammar is strict RFC 8259; numbers are kept as raw tokens.
- Duplicate keys are rejected at any depth, compared after unescaping.
- Containers may nest at most 16 deep.
- The HTTP body is at most 4096 bytes.
- Batches are refused.
- Members other than jsonrpc, id, method and params are refused.

**Request id (accepted C19 value semantics, LP1-I02):**

- `id` is required, and must be a JSON number token.
- Its value is the IEEE-754 binary64 value of the token, as `JSON.parse` / Python `json` compute it. Rust's correctly rounded `str::parse::<f64>` is applied only to tokens that already passed the strict grammar.
- The value must be finite, integral and in 0..2^32-1. It is echoed normalised to an integer.
- Booleans, strings and null are refused with -32600 `{reason:"id"}` and `id: null`.

| Id token | Outcome |
|---|---|
| `1`, `1.0`, `1e0` | id 1 |
| `-0`, `-0.0` | id 0 |
| `1e-400` | id 0 (underflow) |
| `4294967295.0000000001` | id 4294967295 (rounds down) |
| `4294967295.9999999999` | refused (rounds to 2^32) |
| `4294967296` | refused (out of range) |
| `-1` | refused (negative) |
| `2.5` | refused (fractional) |
| `1e400` | refused (overflows to infinity) |
| `true`, `"2"`, `null` | refused (not a number) |

These are exactly the 16 rows of the native C19 oracle. They are tested in `json::tests` and over
the router in `rpc::tests::c19_oracle_ids_over_rpc`.

**Prototype wire convention** (LP1 only):

- -32700 `parse` `{reason: utf8|json|duplicateKey|depth}` with `id: null`.
- -32600 `request` `{reason: batch|kind|id|jsonrpc|method|member}`.
- -32601 `method` `{reason:"unsupported"}`.

The node has no outbound network, no account methods and no wallet methods.

## HTTP transport (A7) and limitations

The transport is bounded and std-only. It binds only the literal `127.0.0.1`; `--bind` with any other
value exits with status 2 before loading anything. The default port is ephemeral, and the actual
listener address is re-checked as loopback.

Connections are handled serially: one request per connection, then `Connection: close`.

**Limits and rejections:**

| Condition | Response |
|---|---|
| Request head over 8192 bytes, or over 32 header lines | 431 |
| Body over 4096 bytes | 413 |
| Method other than POST | 405 |
| Target other than `/` | 404 |
| HTTP version other than 1.1 or 1.0 | 505 |
| Content-Length missing | 411 |
| Content-Length repeated or not plain decimal | 400 |
| Transfer-Encoding present | 501 |
| Content-Type present and not `application/json` | 415 |
| Bytes after the declared body | 400 |
| Per-connection deadline exceeded (`--conn-timeout-ms`) | 408 |

A client disconnect ends that connection only.

**Shutdown** happens through `--max-requests` or `--max-runtime-ms`. The node has no signal handling.

**Not implemented, and no M7 claim is made:**

- hyper and socket2;
- RpcGuard pacing, fairness and S1 reservation accounting;
- keep-alive and pipelining;
- concurrency;
- `pocol_getParams.rpcLimits`;
- resource budgets beyond the fixed limits above;
- signal-driven graceful shutdown.

## LP2 candidate journal (experimental local storage, not a protocol format)

### What is admitted

A candidate is accepted only if all of the following hold:

- it decodes strictly as a V1 header item;
- it passes `Window::link` against the store tip (or the genesis parent);
- it is the next height of this linear store.

`Window::link` covers NetID, items 2–9, height, parent, H_END, signatures, PoW and shares. It runs
with the ceiling `U256::MAX`: the RP `viewTargetCeil` is a client rule, not admission. No wall clock
is applied, neither at admission nor during replay. Future and timing rules are LP3/LP6 work.

**Linearity:**

- Replaying the exact current tip is idempotent; nothing is written.
- Any other input at a stored or skipped height is refused as `notLinear` (`unsupportedByLinearStore`). That is a limit of this store, not a block-invalidity verdict, and nothing is cached.

### Journal format, version 1

The store is one file, `<store>/candidates.lp2j`. All integers are big-endian.

**Metadata (128 bytes):**

| Bytes | Field |
|---|---|
| 0..8 | magic `"PCLP2J01"` |
| 8..12 | version (u32) = 1 |
| 12..16 | bodyLen (u32) = 72 |
| 16..48 | SHA-256 of the exact `profile.json` bytes |
| 48..80 | genesisHash |
| 80..88 | chainId (u64) |
| 88..120 | SHA-256 of bytes 0..88 |
| 120..128 | marker `"LP2META!"` |

**Record i** (120 bytes plus the header):

| Bytes | Field |
|---|---|
| 0..4 | magic `"LP2R"` |
| 4..12 | seq (u64) = height |
| 12..16 | len (u32), 1..3067 |
| 16..20 | prefix guard: first 4 bytes of SHA-256(prevChecksum ‖ bytes 0..16) |
| 20..52 | prevHash: previous blockHash, or genesisHash |
| 52..84 | blockHash |
| 84.. | header (exact strict RLP) |
| +32 | checksum: SHA-256(prevChecksum ‖ bytes 0..84+len) |
| +4 | marker `"CMT!"` |

prevChecksum starts from the metadata checksum. Records are therefore chained both by block hash and
by checksum, and every open re-validates every record from genesis. The prefix guard keeps a corrupted
length in the final record from being mistaken for a torn tail.

### Bounds (checked before allocation)

| Item | Limit |
|---|---|
| records | 1024 |
| journal file | 128 + 1024·3187 bytes |
| header | 3067 bytes |
| append batch | 256 headers |
| profile | 64 KiB |

### Open classification

| Condition | Result |
|---|---|
| Fewer than 128 bytes | `metadataIncomplete`. Never reported as an empty successful store. |
| Any wrong metadata field, checksum or marker | `metadataCorrupt` |
| Profile bytes, genesis or chainId differ from the store | `profileMismatch` |
| A complete record with any wrong field, header, hash, checksum or marker | `corrupt`. Never skipped. |
| A present partial field already inconsistent | `corrupt` |
| A final record that is a consistent strict prefix | torn tail: the verified prefix plus `recoveryRequired` |

### Writer and reader contracts

**Writer** (`store init`, `store append`):

- Windows only. The journal is opened with `OpenOptionsExt::share_mode(0)`, so no other handle in any process can open it while the writer lives.
- The OS releases the handle when the process exits or is killed. There are no lock files to go stale.
- On other platforms the writer returns `unsupportedPlatform`. No portable lock is claimed.

**Reader** (`store status`, and the source side of `store recover`):

- Opens read-only with share mode READ.
- Fails with `busy` while a writer is live, and keeps writers out while it reads.

**Append:**

- The whole batch is validated first, with no I/O.
- Each record is then written with `write_all` + `sync_all`. The in-memory tip advances only after both succeed.
- Atomicity is **per record, not per batch**.
- Any write or sync error poisons the writer: later appends fail with `poisoned`, and the store must be reopened.
- A record whose sync failed is not reported committed by that writer. If its complete bytes reached the file, a later open validates and lists it.

**Recovery** (`store recover`):

- Never modifies the source.
- Copies the verified prefix into a journal in a **new** destination directory, and refuses an existing one.
- Syncs the file, reopens it with full validation, and confirms the source SHA-256 is unchanged.
- Does not recover a corrupt (as opposed to torn) source.

### Limitations

- Directory entries are not fsynced; std has no directory sync on Windows. Only the journal file is synced.
- A crash after `create_dir` but before the metadata sync leaves a directory that reports `noStore` or `metadataIncomplete`. It is never reported as an initialized store, and init refuses to reuse it.
- This is a bounded experimental journal. It is not the final redb/RetentionStore design, and it may change without migration.

### Tests

**`tests/store_journal.rs` (in process):**

- init/never-overwrite;
- H1..20 appended in two batches across reopen, with block hashes equal to the LP1 fixture chain;
- idempotent tip, notLinear, gap and sibling;
- every invalid mutation leaves the journal SHA unchanged, including a mid-batch failure;
- exact profile-byte binding;
- every metadata truncation and byte flip;
- per-field record flips, oversized and zero lengths, reorder, duplicate, gap and trailing bytes;
- the file bound;
- every truncation offset of the final record;
- recovery to new and existing destinations;
- a 1500-case seeded corruption corpus that is never clean;
- write and sync failure injection with poisoning;
- the in-process exclusive writer.

**`tests/store_process.rs` (Windows, real processes):**

- a second writer and the CLI are refused while a helper process holds the store;
- the killed holder releases the handle with no lock file left behind;
- a helper that aborts mid-record leaves the committed prefix plus a torn tail, which the CLI recovers into a new store.

The helper is this test executable re-run with LP2_HELPER_* variables. The node binary has no fault
options.

**`tests/e2e_store.py` (stdlib):** CLI lifecycle across processes, rejection, mismatch,
torn/recover/existing destination, corrupt, metadata cases and the file bound.

## LP3 S1 GenesisSpec decoder and `genesis-decode` (experimental, unreviewed)

`src/genesis_spec.rs` decodes GenesisSpec v1 with the accepted reference order and details
(L0, structure, gsVersion, gsCount, gsInt, gsLen, gsRange, gsOrder, gsSys). The gsVersion detail is
the exact decimal value of the specVersion item at any length (`src/decimal.rs`). Identity only: no
ParamGate, nothing bootable.

```
target/debug/lp1-node genesis-decode --in FILE [--out FILE]
python tests/lp3_genesis_diff.py --bin target/debug/lp1-node --m1 ../../development/m1 [--max-version]
python tests/lp3_genesis_cli.py  --bin target/debug/lp1-node --m1 ../../development/m1 [--max-version]
cargo test --offline --test genesis_batch_alloc     # LP3_ALLOC_MAX_VERSION=1 adds the full-size gsVersion case
```

`genesis-decode` reads one hex input per line (optional `0x`, optional `\r`; an empty line is the
empty input) and writes one JSON row per line plus a summary row. Exit status: 0 every line was
decoded or rejected by the decoder and every accepted input re-encoded exactly; 1 some line was
refused; 2 I/O or buffer reservation error.

Tool limits (not GenesisSpec validity rules; a refused line gets no gs* code and no verdict):

| Limit | Value | Why |
|---|---|---|
| One input | `MAX_SPEC_BYTES` = 2,818,474 bytes | Largest encoding that can pass decode and ParamGate R12 (65535 members, every field at its widest). Every possibly valid spec fits. |
| One line | `MAX_LINE_BYTES` = 2·MAX_SPEC_BYTES + 4 | `0x`, the hex digits, `\r`, `\n`. A longer line is skipped in the reader's chunks and reported as `{"refused":"inputTooLarge","lineBytes":N}`; the next line is processed. |
| Buffers | 8,455,426 bytes, reserved once | Line and byte buffers, reused for every line; reservation failure is exit 2 with a message, not an abort. |
| Not hex | per line | `{"refused":"hex"}`, next line processed (round 1 stopped the whole command). |

The file is streamed, so memory does not depend on its size or line count. Measured peak heap
above the buffers (`tests/genesis_batch_alloc.rs`): refused 256 MiB line 15 B, 30000 lines 421 B,
largest valid spec 5.4 MB, deepest nesting 8.4 MB, 256 KiB gsVersion 1.9 MB, full-size gsVersion
19.1 MB. Time is linear except the gsVersion decimal conversion, O(n^1.59 log n): 23 s in a
release build for a full-size specVersion item on the author's Linux container, peak RSS 34.7 MiB
under a 64 MiB address-space limit (CPython 3.13 needs about 6 s for the same `str(int)`; Python
>= 3.11 refuses it by default above 4300 digits).

The Python harnesses write their generated inputs as exact bytes (binary mode), so Windows newline
translation cannot change them, and name `encoding='utf-8'` for every text read and write (the M1
vector files are UTF-8; Windows run 37972422364 failed on the cp1252 default). Windows checks: `.github/workflows/lp3-s1-windows-verify.yml`
(manual dispatch, Rust 1.58.1, GitHub-hosted `windows-2022`) runs `.github/scripts/
lp3-s1-windows-verify.ps1` and uploads logs and `results.json` (PASS / FAIL / BLOCKED BY PLATFORM /
NOT RUN per check, with the tested commit). It has not been run yet; see
`development/lp3/s1-genesisspec/author-r3-windows-ci/README.md`.

## Conformance notes and recorded questions

1. **JSON id semantics: resolved in revision 2, and no longer a question.** Revision 1 used exact decimal values, which departed from accepted C19 (LP1-I02). That blocked A6 conformance. Revision 2 implements the accepted binary64 value semantics described under "RPC" above. No M1 change.
2. **Error-envelope convention.** The -32700, -32600 and -32601 `data.reason` values, and the transport HTTP status codes, are a prototype convention. They are not part of M1, and the reviewer will assess them.
3. **Nesting bound.** The window's outer-reply decode applies the strict RLP nesting bound of 16, which is also the hard recursion ceiling. A header whose field is a list nested deeper than 16 is therefore `viewIncomplete` here. Python would decode it and report rule 1. Python itself fails with `viewIncomplete` at its recursion limit.
4. **Genesis wrapped single byte.** A wrapped single byte in the genesis preimage is an L0 framing error here, but gsInt in `netprofile_ref`. Both reject it.
5. **Load-time chain validation has no wall clock.** viewFuture is not applied at load; it is applied in every window check.
6. **Fork schedule validation.** The rule that start heights must be strictly ascending and the schedule non-empty is an LP1 loader rule.
7. **Notifications.** Requests without an id are refused with -32600 `{reason:"id"}` rather than left unanswered.
8. **LP2 admission uses `Window::link` codes.** A wrong genesis parent hash at height 1 is reported with the RP code `viewGenesis`. This is a reporting label only; admission does not use RP's viewFuture or its ceiling.

Items 2–8 are prototype choices that the review will assess. None of them is claimed as full M3 or
M7 behaviour.

## Adversarial corpus (LP1-I05)

The generator applies 1–4 random edits, which can cancel out, so a mutant may be byte-identical to
its base.

The genesis corpus has 5000 deterministic cases with seed `SEED ^ 0x9e`. It classifies each case:

- **No-op mutants** (identical bytes) must decode to exactly the base identity and hash.
- **Genuinely changed preimages** that decode must **not** carry the frozen genesis hash.

The no-op count is pinned at 2, matching the reviewer's independent replay in
`genesis-noop-independent-v2.json`. Every case is still run through the decoder (no-panic coverage).

Genesis hashing is unchanged.

## Licenses and versions

- Project license: Apache-2.0.
- Toolchain: rustc 1.58.1 (MSVC) and cargo 1.58.0, used offline with the cached registry.

Pinned dependencies, per `fixtures/LICENSES.json`:

| Crate | Version | License |
|---|---|---|
| sha2 | =0.9.9 | MIT OR Apache-2.0 |
| sha3 | =0.9.1 | MIT OR Apache-2.0 |
| libsecp256k1 | =0.5.0 | Apache-2.0 |
| serde_json | =1.0.79 | MIT OR Apache-2.0 |
| serde | 1.0.136 | MIT OR Apache-2.0 |

serde is pulled transitively; it is not a direct dependency. Root records the transitive set from the
generated `Cargo.lock`. LP2 adds no dependency.

The code uses no API newer than Rust 1.58. In particular it avoids:

- let-else;
- `abs_diff`;
- `std::array::from_fn`;
- `bool::then_some`;
- `std::thread::scope`;
- `std::hint::black_box`;
- const `thread_local!`.

LP2 uses `std::os::windows::fs::OpenOptionsExt::share_mode` (stable since Rust 1.10) and
`env!("CARGO_BIN_EXE_lp1-node")` in integration tests (stable since Rust 1.43).

Non-ASCII text appears only in normal UTF-8 string literals, never in byte-string literals.

## Acceptance criteria (all untested by the author)

LP1:

- **A0:**
  - `fixtures::verify_provenance` must report all 5 outputs equal to PROVENANCE SHA-256.
  - The fixture scan for private material must pass.
  - The baseline files must be unchanged; root diffs this.
- **A1:**
  - `cargo build --offline` and `cargo test --offline` must pass on 1.58.1 with the generated lock, as a full run with no abort.
  - `cargo fmt -- --check` must pass. Root applies canonical rustfmt.
  - Licenses must be recorded, with no target artifacts or binaries in Git.
- **A2:**
  - `rlp`/`header` unit tests must pass, including `depth_bound_default`, `depth_requests_above_ceiling_are_refused` and `small_and_zero_limits_are_consistent`.
  - `a2_chain_headers_reencode_exactly` must pass.
  - `adversarial` must pass in full: the seeded header corpus with every accepted mutant re-encoding identically, and the genesis corpus with no-op classification. No panic anywhere.
- **A3:**
  - `hashes` unit tests (K1–K3, shape faults) must pass.
  - `a3_signatures_and_hash_oracle` must pass: all 3677 digests recomputed, and every preimage derived from fixture headers.
  - `lp1-node hash` must pass.
- **A4:**
  - `asert` unit boundaries and mutations must pass.
  - `a4_asert_oracle_rows` must report 1027/1027.
  - `asert_alloc` must report 0 allocations, a working counter probe, and maxShiftBits 512 on the named case.
  - `lp1-node asert-batch` must pass, plus root's independent differential.
- **A5:**
  - All 22 window cases must match outcomes and every counter.
  - Named counts must match.
  - The `a5_mutations_are_rejected` suite must pass.
  - The uncached run must give the same outcomes.
- **A6:**
  - Chain load must reject mutations.
  - The RPC tests over head 20 and the empty chain must pass.
  - All 16 C19 oracle id rows must pass, both directly and over RPC.
  - The window check over the node's own RPC must equal the RW-h20 counters.
- **A7:**
  - `http_loopback` tests must pass.
  - `tests/e2e_http.py` must pass against the built binary from a temporary cwd, including bind refusals, limits, timeout, disconnect recovery, and bounded shutdown with exit 0.
- **A8:** Codex review and root's corrections. No completion is claimed until root passes A0–A8.

LP2 (`LP2-MILESTONE.md`):

- **P0:** all LP1 suites above pass unchanged.
- **P1:**
  - `p1_*` tests pass.
  - `e2e_store.py` passes its lifecycle checks.
- **P2:** `p2_*` tests pass, plus the rejection and idempotence checks in `e2e_store.py`.
- **P3:** `p3_*` tests and the corruption corpus pass.
- **P4:**
  - `p4_every_truncation_of_the_final_record` passes.
  - The recovery checks in `e2e_store.py` pass.
- **P5:** `store_process` passes on Windows.
- **P6:** both e2e scripts and all seeded corpora pass.
- **P7:** root's offline locked build, fmt and full test run, with exit codes; Codex's independent journal checks.
- **P8:** the Arabic dashboard (`LP2-PROGRESS-0.37-AR.md`).

No LP2 acceptance is claimed by the author.
