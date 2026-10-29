# N26: normative representation clauses (CR-E4-01, CR-E4-02, CID-1)

**Status.** Root accepted these as qualified technical decisions in `REVIEWER-DECISIONS-0.25.json` (REVIEW-0.25). They are not owner decisions. This file states them as normative M1 text for implementation. No earlier helper or fixture is changed:
- The 0.13 codec (`fmt2_codec.py`, with its `epochNotJsSafe` cap) stays as preserved history.
- These clauses supersede it for implementations.
- Fixtures: `../vectors/n26-representation-cases.json`.

## N26-1: epoch in the fmt-2 value (CR-E4-01, alternative A)

1. **Value form.** The record value is `{fmt: 2, epoch, seq, tomb, b64}` (browser.md:534–535).
   - `epoch` is a JSON **string** of exactly 16 lowercase hex digits, equal to the hex16 of the record name.
   - `seq` is a JSON integer in [0, 2^32 − 1].
2. **Decoder rules.** A decoder accepts `epoch` only if all of these hold:
   - it matches `^[0-9a-f]{16}$`;
   - its value is in [1, 2^64 − 1];
   - it equals the name's epoch.

   Otherwise the record is corrupt (reason `epoch`). A JSON number in `epoch` is not a valid fmt-2 value.
3. **Unchanged.** The epoch domain stays [1, 2^64 − 1] and is not narrowed. Names, ranges, `RECORD_BYTES_MAX = 1442048` and all limits are unchanged.
   - The maximum model record is 1442009 bytes (root-verified in 0.25).
   - P-E4-4's netKey bound becomes ≤ 105 characters.
4. **Representation in TypeScript.** Implementations hold epochs as BigInt or as the hex string. They never convert them through a JS Number.

## N26-2: storage strings (CR-E4-02, existing authority)

1. `site_storageSet` params are `[str(256), str(61440) or null]` (browser.md:225). `str(n)` excludes lone surrogates (browser.md:200).
   - B3 rejects any key or value that contains one, with −32602 `{path: params[i]}`.
   - The rejection happens before StoreQueue accounting and before persistence.
2. Every string that reaches the quota accounting and the fmt-2 codec is therefore a sequence of Unicode scalar values.
   - TextEncoder bytes = UTF-8 bytes.
   - Valid surrogate pairs are accepted and preserved.
3. **Defensive rules.** P-Q1-1 (lone surrogate counted as 3 bytes) is defensive and unreachable through the bridge. The codec still rejects lone surrogates (`unpairedSurrogate`) and never normalizes.
4. **No baseline change.** WTF-8 persistence is not adopted.

## N26-3: profile chainId (CID-1)

1. `NetworkProfiles.validate` reads the profile `chainId` from the networks.json text **exactly**. The token must be a JSON integer literal without sign, fraction or exponent (`^[1-9][0-9]{0,19}$`), in [1, 2^64 − 1].
2. It is compared with the GenesisSpec chainId (step 3) and RLP-encoded into netKey (browser.md:18) as an exact integer.
3. A reader that passes the value through an IEEE-754 double is non-conforming:
   - 9007199254740993 would read as 9007199254740992;
   - 18446744073709551615 would read as 2^64.
4. The networks.json format and the u64 domain (consensus.md:9) are unchanged.
