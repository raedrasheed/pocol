# Annex E1: epoch items, record names, values, largest-wins (D103, D134)

**Status: Partial.** This is an English restatement of the baseline, cited as `file:line` in `reference/`.

Scope: worker-generation recovery in the MV3 extension, that is, service-worker lifetimes. It is **not** the protocol chain's epoch, and **not** the local coordinator UI's recovery.

## E1.1 Fence item (browser.md:508)

- **Name:** `'pocol:epoch:' + hex16(E)`. It is shared by every generation that computes the same E. There is **no nonce**.
  - P-E1-1: `hex16` is exactly 16 lowercase hex digits, zero-padded.
  - Any other spelling is not an epoch item of this format and is never normalized. That covers uppercase, short, prefixed and nonce-suffixed names (the old D103 form).
- **Range:** `E ∈ [1, 2^64 − 1]` (browser.md:539). If E is exhausted, local storage is disabled.
- **Value:** `{bootMs}`, the wall-clock boot time of the writing generation. That `bootMs` is the boot time is fixed by BR22h h6: "G2 at 70000 confirms F(6) with bootMs = 70000".
- **Size:** ≤ `EPOCH_BYTES_MAX = 256`. P-E1-2: measured as compact JSON.

## E1.2 Records (browser.md:534–542)

- **Name:** `'site:' + netKey + ':' + addr + ':r:' + hex16(E) + ':' + hex8(seq)`.
  - `seq ∈ [0, 2^32 − 1]`.
  - A record is never overwritten. Every `set` takes a fresh `seq` from the single SeqAlloc of (key, generation), allocated before issue, including checkpoints and their retries.
  - **A `seq` is never reused, even after a rejected or timed-out promise** (browser.md:540).
- **Value:** `{fmt: 2, epoch, seq, tomb, b64}`. `b64` is base64 of `[be32(len k) ‖ k ‖ be32(len v) ‖ v]` in byte order of the keys (D104).
  - **Staged dependency:** the fmt-2 byte codec and RECORD_BYTES_MAX belong to row E4. This package uses an *abstract* value `{fmt: 2, epoch, seq, tomb, dict}` and makes no claim about the wire encoding (P-E1-3).
  - A record is valid only if `fmt = 2`, `epoch` and `seq` equal the name's, `tomb` is boolean, and the dictionary maps strings to strings.
  - P-E1-4: a tomb carries the empty dictionary. A tomb means the site was deleted by AdminDelete (D106/D108, rows E6/E7). For largest-wins it is an ordinary record whose state is `{}`.

## E1.3 Confirmed-state lookup (browser.md:541, 550–553)

1. The confirmed state is the dictionary of the **largest present valid** record, ordered by `(epoch, seq)`, with epoch first.
2. Recovery lists the key's names by prefix (`getKeys`), then reads the largest with `get`.
3. A corrupt record is skipped and counted (`recoveryCorrupt`), and the next lower one is read, up to `RECOVERY_GETS_MAX = 4` reads.
4. **Outcomes:**
   - corrupt records with no valid one within the four reads: the key is `failed`, with reason `corrupt`;
   - no record at all: the dictionary is `{}`.
5. After a record succeeds, the lower records are removed on a best-effort basis. A failed remove leaves the record counted on disk (D104).

Vectors: `vectors/e1-units.json`, covering names, ranges, strict parsing, nine lookup cases and epoch values.
