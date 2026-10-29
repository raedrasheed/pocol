# Annex V1: HeaderNetCheck, the RP header window (normative, draft 0.26)

**For root review. Not approved. The author ran nothing.** This annex restates browser.md:22–72 in English, with the consensus rules it invokes (consensus.md:48–71, 92–98, 113–118, 126–137, 151–161), and adds the conventions marked **P-V1-n**.

- **Literal fixtures:** `../vectors/v1-*.json`.
- **Reference checker:** `../tools/v1_ref.py`.
- **Generated literal bytes:** `results/v1-transcripts-0.26.json` and `results/hash-freeze-v1-0.26.json`, both written by `tools/run_checks_026.py`.

It is a specification of the client check. It is not the M3-TS model and makes no browser claim. Items 10–17 (execution and membership) are not evaluable in RP and are excluded, as the baseline says.

## 1. Inputs

| Input | Definition |
|---|---|
| `cfg` | `{chainId, genesisHash, forkSchedule, CP}` of the RP profile. The vectors use **V1NET**: the 341-byte GSV1 with chainId 777002 (P-V1-8) |
| `C` | Browser clock in seconds |
| RPC | `eth_blockNumber` and `pocol_getHeaders(from, count)` |

**P-V1-1 (wire form).**
- Requests use JSON quantities with no leading zeros (network.md:174).
- The `pocol_getHeaders` result is `'0x' + hex(RLP list of the count headers)`. Each element is the Header list `[[UT, sig], nonce8, shareList, winnerSig65]` (consensus.md:53–55).

## 2. Constants (browser.md:29–33)

| Constant | Value |
|---|---|
| `VIEW_SLACK_S` | 4 |
| `ceilTarget` | `min(2^256 − 1, target_g · 2^4)` |
| `minWorkRP` | `floor(2^256 / (ceilTarget + 1))` |
| `Φ_view` | 10 s |
| `work(t)` | `floor(2^256 / (t + 1))` |

For V1NET (`target_g = 2^240`): `ceilTarget = 2^244` and `minWorkRP = 4095`.

## 3. Counts (D82)

- `b = max(1, h − 12)` and `n = h − b + 1 ≤ 13`.
- The reference is `b − 1` when `b > 1`. It is never counted, never linked to an earlier header, and never PoW-checked.
- The request is `from = max(1, b − 1)` and `count = h − from + 1 ≤ 14`.

| h | b | from | count | n | Notes |
|---|---|---|---|---|---|
| 1 | 1 | 1 | 1 | 1 | anchored on genesis |
| 3 | 1 | 1 | 3 | 3 | anchored on genesis |
| 13 | 1 | 1 | 13 | 13 | anchored on genesis |
| 14 | 2 | 1 | 14 | 13 | reference 1 |
| 20 | 8 | 7 | 14 | 13 | reference 7, headers 8..20 |
| 2^64 − 1 | 2^64 − 13 | 2^64 − 14 | 14 | 13 | no overflow |

## 4. Steps (browser.md:35–56), in order; the first violation stops the check

**0.** `h = eth_blockNumber`.
- If `h = 0`: send no `pocol_getHeaders` and create no frame. The result is **viewNoBlocks** with the literal message «الشبكة لم تنتج الكتلة 1 بعد؛ لا رأس للتحقق في RP». `eth_requestAccounts` and `eth_sendTransaction` get 4901. Re-check after 30 s.
- An unparsable reply gives viewIncomplete (P-V1-9).

**1.** Send `pocol_getHeaders(from, count)`.
- −32018 → viewIncomplete.
- −32021 → wait `retryAfterMs` and resend, at most 3 times, then viewIncomplete. The wait is exactly `retryAfterMs` (P-V1-10).
- Any other error → viewIncomplete (P-V1-9).

**2.** The result must be an RLP list of exactly `count` elements with heights `from..h` in order. Any other count, or a height out of place, → viewIncomplete.
- Each element is parsed against item 1 (one header decode per element, ≤ 14).
- An element that fails item 1 → rule `1` at its position (P-V1-2).

**3.** If `b > 1`, check the reference in this order:
- item 1;
- item 1b: chainId → `netChain`, genesisHash → `netGenesis`, `protocolVersion = activeVersion(h)` → `netVersion`;
- viewFuture: `ref.ts ≤ C + Φ_view`.

The reference gets no work check and no window check.

**4.** For each x in `W = [b, h]`, ascending, with `p` = the reference, or the genesis virtual header `{h = 0, ts = g_ts, blockHash = genesisHash}` when `b = 1`:
1. item 1b, then viewFuture;
2. item 2: `h = p.h + 1`; `parentHash = blockHash(p)`; for `x = 1`, `parentHash(1) = cfg.genesisHash` or **viewGenesis**; `h ≤ H_END`;
3. item 3: either proposer, sig and `a = 0` are all present and `ecrecover(SigMsg, sig) = proposer`, or proposer and sig are both empty;
4. item 4: `L(a) = p.ts + a·D_att + 1 ≤ ts ≤ U(a) = p.ts + (a + 1)·D_att` and `U(a) < 2^63`; a fallback block's `ts` must equal the fallback stamp (`L(0) + Δ_fb_wait` at `a = 0`, `L(a)` at `a ≥ 1`);
5. item 5: `target = ASERT(p)` (section 5);
6. **viewTargetCeil**: `target ≤ ceilTarget`, before any PoW evaluation;
7. item 6: `nonce < nMax`, where `nMax = 2^64` in nonceMode 1 and `min(2^64, c·work(target))` in nonceMode 0;
8. item 7: `powHash = SHA256(TemplateID ‖ be64(nonce)) ≤ target`;
9. item 8: `winnerSig` is recoverable over `WinMsg`;
10. item 9: each share is `< nMax`, `≠ nonce`, strictly ascending, and satisfies `SHA256(TemplateID ‖ be64(n)) ≤ T_share = min(2^256 − 1, target · m)` (validation.md:926, D143).

**5.** Success message: «RP: مرتبط بالعمل والهوية لـ`<count phrase>`[ حتى genesis]، بعمل محتسب لا يقل عن `<minWorkRP>` لكل رأس (work(ceilTarget))، دون تحقق تنفيذ أو عضوية».
- When `minWorkRP = 1`, append «، حد العمل غير فعال لهذه الشبكة».
- No ratio and no time estimate.
- **P-V1-4:** the count phrase is «رأس واحد» (1), «رأسين» (2), «n رؤوس» (3–10) or «n رأسًا» (11–13), following the source examples «10 رؤوس» and «13 رأسًا». Digits are ASCII.

**6.** On any violation: a rule label; no frame; 4901 for pending account and sign requests; a client log −32019 `{rule}` (network.md:362); no reliance on any root.

**7.** Items 10–17 are not evaluated (a consistent lie is possible), as the baseline states.

## 5. Fixed-width ASERT (consensus.md:126–137)

1. `Δt = (p.ts − g_ts) − T_blk·p.h`, with `|Δt| < 2^97`.
2. `e = floor(Δt·65536 / τ)`; `s = floor(e / 65536)`; `f = e − 65536·s`.
3. `s ≥ 256` gives `2^256 − 1`, and `s ≤ −257` gives 1. Both stop before any power of two is built.
4. Otherwise:
   - `F = 65536 + floor((195766423245049·f + 971821376·f² + 5127·f³ + 2^47) / 2^48)`;
   - `X = target_g·F`;
   - `Y = X·2^(s−16)` (a right shift when negative);
   - `target = min(max(Y, 1), 2^256 − 1)`.

Every intermediate is below 2^512. The reference checker records the maximum bit length per run.

The hand-derived rows (`v1-units.json` A1–A7) include:
- `Δt = 2400` → `2^244`;
- `Δt = 300` → `92674·2^224`;
- the NX-V3e timestamp → `2^256 − 1`, with no exponent-sized value.

## 6. Cost bounds (browser.md:58) and instrumentation

The checker counts header decodes, ASERT calls, PoW SHA-256 calls, TemplateID SHA-256 calls and ecrecover calls, with an ordered event log.

- **Required:** decodes ≤ 14, ASERT ≤ 13, PoW SHA-256 ≤ 13.
- **P-V1-3:** «≤ 13 SHA-256» is read as the PoW evaluations. Each checked header also needs `TemplateID = SHA256(RLP(UT))` for items 2, 3 and 8, so all SHA-256 calls together reach 14 + 13 = 27 when n = 13, and a literal reading could never be met. TemplateIDs are bounded separately (≤ 14). Root decision requested.

## 7. Vectors (`../vectors/v1-window-cases.json`)

| Case | Content | Expected |
|---|---|---|
| RW-h3 | h = 3 | n = 3, anchored on genesis, 4095 |
| RW-h13 | h = 13 | n = 13, anchored on genesis |
| RW-h14 | h = 14 | n = 13, reference 1 |
| RW-h20 | h = 20 | n = 13, reference 7 |
| NX-V1 | missing header | viewIncomplete |
| NX-V2 | wrong genesis parent | viewGenesis |
| NX-V3 | missing reference | viewIncomplete |
| NX-V3b | reference chainId 777001 | netChain |
| V1-refGenesis | reference genesisHash | netGenesis |
| V1-refVersion | reference version | netVersion |
| NX-V3c | reference fails PoW | accepted; the reference is not checked |
| NX-V3d | 15 headers | viewIncomplete |
| V1-outOfOrder | headers out of order | viewIncomplete |
| V1-wrongParent9 | wrong parent at 9 | rule 2 |
| NX-V3e | reference `ts = g_ts + 2^62` | viewFuture, with zero ASERT and zero PoW |
| NX-V3f | `C = g_ts + 10^6`; reference `C − 200`; headers every 10 s with ASERT target `2^256 − 1` | viewTargetCeil at 8, with zero PoW, no frame, 4901 |
| NX-V3g | honest 10 s spacing 4τ behind; targets exactly `2^244`; `powHash ≤ 2^244` | «13 رأسًا» and 4095 |
| NX-V4 | h = 0, then a re-check at 30 s at h = 1 | viewNoBlocks, then «رأس واحد حتى genesis» |
| V1-err32018 | −32018 | viewIncomplete |
| NX-V6 | −32021 four times | 3 retries, then viewIncomplete |

**Clock.** The fixture clock is the head's `ts + 5` s (validation.md:1052; P-V1-6). NX-V3f uses `C = g_ts + 10^6` from its source line.

**Field values.** Every header field value is literal in `v1-chains.json`. The runner then:
- encodes the headers;
- signs with TK-1 fixture key 1 (RFC 6979; the key is read from the published fixture and never written; signature layout r ‖ s ‖ v, P-V1-5);
- searches the smallest PoW nonce;
- exports every encoded header, TemplateID, powHash, blockHash, SigMsg, WinMsg and signer public key, with its preimage.

**Synthetic labels.** These are opaque labelled values, not computed hashes:
- the state root `0x55…`;
- the parent of the G/F references, `0x77…`;
- the variant genesis `0x99…`;
- the wrong parent `0x88…`.

## 8. Deferred experiments (Phase A; definitions complete)

`../supplements/s26-v1-experiments.json`:
- **S26-NXV3E-PING:** the worker answers pings within ≤ 100 ms while rejecting NX-V3e.
- **S26-NX-D:** all cases through MaliciousRpc and the real extension.
- **S26-NXV4-LIVE.**

A production M3-TS model is not required for the M1 spec.
