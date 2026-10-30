# LP8 independent review (Website, publisher/fetcher, journal replay) — resolution

Reviewer: an independent agent with read-only access. It recompiled `Website.sol` with solc 0.8.28 and got
bytecode byte-identical to `tests/data/website.json`. It checked the JS chunk addresses against
`m1/vectors/chunks.json`, ran the draft-0.2 manifest vectors through the extension decoder, and read
the Rust code. No authorization bypass in Website.sol. The replay TrieRebuild fix was found correct.

| # | Severity | Finding | Resolution |
|---|---|---|---|
| H1 | high | Body checks (TX_MAX, BODY_MAX, evidence bounds, fallback-body) ran before the body was bound to the header roots, and a root mismatch was a blockReject. A peer relaying a real header with a foreign body got the valid block marked invalid (and its descendants `ancestorInvalid`) until restart | `chain.rs`: the body is bound to `tx_root`/`evidence_root` first; a mismatch is an envelope rejection (only that copy, never marked invalid); body-content checks run only on the bound body. Tests: `a_valid_header_with_a_foreign_body_rejects_only_that_copy` (the real block is accepted afterwards). Item 12 is now tested with a header that commits to the non-empty body |
| M1 | medium | The publisher stored chunks before checking later files, the 4 MiB total, the entry and the manifest | `Publisher::publish` builds the manifest from predicted CREATE2 addresses and runs every check and `Manifest::decode` before the first transaction. e2e_site: a 5 MiB site is rejected with the nonce unchanged |
| M2 | medium | Empty files (size 0, no chunks; `pos-empty-file`) were rejected by both fetchers and by the publisher | The non-empty-chunks checks are removed (Σlen = size with len ≥ 1 still rejects `neg-empty-with-chunk` and `neg-nonempty-no-chunks`). Rust and JS run all 11 positive and 56 decode/structure/semantic negative vectors. e2e_site: an empty file round-trips |
| M3 | medium | The version index was read before `createVersion`; a concurrent publisher could get its draft published and made current under another publisher's name | After `createVersion` the publisher scans the new indexes for the draft with its exact manifest hash and length, then publishes and sets that one. e2e_site: two concurrent publishers each publish exactly their own manifest (the race occurred: versions 2 and 3) |
| L1 | low | Website.sol differs from draft-0.1 proposals P11/P12/P13 (revoke of drafts, zero addresses, slot packing) | Kept: the normative reference `storage.md` defines slots 0–5 and "revoke for any non-current version". Tools read through the ABI views, not raw slots. `transferOwnership(0)` cancels a pending transfer; `addPublisher(0)` is inert. Recorded as an explicit deviation from the draft proposals |
| L2 | low | `decode_current` could panic on hostile output | Bounds-checked ABI words and arrays (`abi_word`, `abi_addresses`); test `hostile_current_output_never_panics` |
| L3 | low | `fetch --out` could overwrite on case-insensitive filesystems or fail on file/dir prefixes | `write_out` requires an empty directory and refuses case-insensitive collisions and file/directory prefix conflicts |
