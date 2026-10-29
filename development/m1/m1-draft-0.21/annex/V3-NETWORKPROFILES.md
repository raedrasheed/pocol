# Annex V3: NetworkProfiles.validate (D58, D77, D79, D100), M1 draft 0.21

**Proposed for review. Not approved. Nothing here was executed by the author.**

- **Scope:** implementation.md:360 (GSV1, N1, N6, N11); validation.md:64–79 (GSV1, L4n) and 780–784 (RF1–RF4).
- **N1, N6 and N11 are the GenesisSpec negatives of L4n:**
  - N1: element 52 removed → gsCount;
  - N6: a 19-byte rewardAddr → gsLen;
  - N11: a declared genesisHash mismatch → profile rejected, R24(a).

  They are not the T23 NetSim cases with similar names.
- **Reference:** `tools/netprofile_ref.py`. It reuses the accepted `m1-draft-0.2` Keccak-256 and canonical RLP encoder read-only, with a FakeTransport. There is no network and no node.

## 1. Profile and acceptance order

- **Profile:** `{profileId, name, chainId, genesisPre, genesisHash, forkSchedule, endpoint, trustLevel ∈ {DEV, LN, RP}}`.
- **`validate(profile, trust)`** runs the steps below in order and stops at the first failure. Each step appends `[step, status, …]` to the trace.

| # | Step | Failure |
|---|---|---|
| 0 | **Shape** (P-V3-1): exact keys; non-empty strings; `chainId` a JSON integer (not bool or string); `genesisPre` `0x` + lowercase even-length hex; `genesisHash` `0x` + 64 lowercase hex; `forkSchedule` a list of integer pairs; `trustLevel` exactly DEV, LN or RP. **No normalization:** no case folding, trimming or URL rewriting. | `profileShape {field}` |
| 1 | Decode `genesisPre` (§2) | `L0` or `gs*` |
| 2 | `keccak256(genesisPre) = genesisHash` | `genesisHash`, R24(a) |
| 3 | `profile.chainId = GenesisSpec.chainId` (DG-V3-7) | `chainId` |
| 4 | `networks.json` signed, or a confirmation showing the **full** hash. The identity line is `<name> · chainId <n> · genesisHash <0x…64>` (P-V3-4). | `unconfirmed` |
| 5 | RecvFit (§3) | `−32019 {rule: recvFit, violations[…]}` |

- **On success** the result includes `netKey = keccak256(RLP(['PoCol-net-v1', chainId, genesisHash]))`.
- **Node start.** A validated profile is configuration only. Starting a node needs node-side R24(a), where allocRoot and sysCodeHash are recomputed from alloc (threat.md:43). In this model `may_start_node` is therefore never true.
  - GSV1, which is unsigned test data and not bootable, is additionally on the known-non-bootable list. It always yields `knownNonBootable`, whether its profile was signed or confirmed (P-V3-5).

## 2. GenesisSpec decode (consensus.md:7–34)

`GenesisSpec = RLP([specVersion, chainId, CP(52), allocRoot, M_0List, sysCodeHash])`.

- **Framing** errors are **L0** (validation.md:77 N9): truncation, trailing bytes, a non-canonical length form or a length with a leading zero. A single byte wrapped as `81 xx` with xx < 0x80 is a well-formed frame; it is judged as an integer (gsInt).
- **Evaluation order**, checked globally: each stage runs over the whole structure before the next stage starts.
  1. **structure:** the top level is a list; positions 2 and 4 are lists; all other positions, CP items and entry items are byte strings; entries are lists. Reported as `gsStructure` (P-V3-2).
  2. **gsVersion:** specVersion = 1.
  3. **gsCount:** 6 top items, 52 CP items, |M_0List| ≥ 1, every entry has 2 items.
  4. **gsInt:** specVersion, chainId and every CP item are minimal big-endian: no leading zero byte, no wrapped single byte. Zero is `80`.
  5. **gsLen:** allocRoot and sysCodeHash are 32 bytes; id and rewardAddr are 20 bytes.
  6. **gsRange:** chainId ∈ [1, 2^64−1]; each CP item fits its width (u8…u256) and its bounds: target_g ≥ 1, T_blk ≥ 1, tau ≥ 1, nonceMode ∈ {0,1}, c ≥ 1, m ≥ 1, kappa ≥ 1, S_max ≤ 256, alpha_bp ≤ 10^4, gamma_bp ≤ 10^4 − alpha_bp, M_max ≥ 1, M_min ≥ 1, RET ≥ 1.
  7. **gsOrder:** ids strictly ascending by bytes; a duplicate is gsOrder.
  8. **gsSys:** no id is 0x00×20, SYSTEM_ADDRESS (0xff…fe), or 0x00…C0C001–0x00…C0C0FF.
- **Relations.** `M_min ≤ |M_0List| ≤ M_max` is left to ParamGate R12, which is not part of validate (DG-V3-5).
- **GSV1** (341 bytes) is held in two independent encodings:
  - the literal byte segments of validation.md:65–72;
  - an item tree whose list headers are recomputed.

  They must be equal. The lengths are CP 179, M_0List 88, payload 338 and total 341.
- **Decode then re-encode** with the accepted 0.2 encoder must return the same bytes.
- **Decoded values** are hand-written:
  - chainId 777902 (sv-fix);
  - CP = sv-fix: g_ts 1700000000, target_g 2^240, … BODY_MAX 196608, … P2 2;
  - roots 0x22…/0x33…;
  - M_0 = (a1…, b1…), (a2…, b2…).
- **H_GSV1** is recorded from one library only. Three-library agreement is outstanding (DG-V3-3).

**Negatives (32).**
- All of L4n N1–N10. The edits are applied to the item tree, so list headers stay consistent.
- Widths, leading zeros, zero as a byte, chainId 0, the gamma relation and its equality edge, target_g = 2^256, and a 31-byte root.
- The system address, the zero id, C0C0FF, and C0C000/C0C100 (both accepted).
- The outer header (L0) and the CP-not-a-list structure; the empty M_0List (gsCount).
- Five precedence pairs, which show the global order: version before count, int before len, len before range, order before sys, count before int.

## 3. RecvFit (browser.md:16, 300–306)

WorstLegit for each exact method that depends on the profile:

| Method | WorstLegit | RECV_LIMIT | Accept edge | Reject edge |
|---|---|---|---|---|
| eth_getCode | 2·B_code_max + 1024 | 69632 | B = 34304 → 69632 (equal) | 34305 → 69634 |
| pocol_getParams | 2·\|genesisPre\| + 4096 | 98304 | \|gp\| = 47104 → 98304 (equal) | 47105 → 98306 |
| eth_getBlockByHash/Number | 2048 + ⌊BODY_MAX/85⌋·70 | 167936 | BODY_MAX = 201449 → 167878 | 201450 → 167948 |
| eth_getTransactionByHash | 2·TX_MAX + 2048 | 266240 | TX_MAX = 132096 → 266240 (equal) | 132097 → 266242 |

- **eth_getBlockBy\*:** equality is unreachable because the worst case moves in steps of 70. 165888/70 = 2369.83, so the largest admissible floor is 2369, and 2369·85 + 84 = 201449.
- **Every edge keeps the encoded width** of the changed field, so |genesisPre| stays at 341. Only the tested value moves.
- **All exceeding rows are reported**, in row order (RF-multi).
- **Length formula** (hand-derived):
  - one M_0 entry `ea 94 ‹20› 94 ‹20›` is 43 bytes;
  - with the GSV1 CP (179) and 3-byte headers, `|genesisPre| = 256 + 43n`.

| Case | Profile | Length | Result |
|---|---|---|---|
| RF1 | network CP (governance.md:4–16), chainId 777001, 4 placeholder HELD ids | 439 | accept (worst 66560 / 4974 / 163958 / 264192) |
| RF2 | end/full (governance.md:159–161): 1024 ids = keys 3–6 + 1020 keccak('PoColEnd'‖be32 i) ids | 44290 | accept (getParams 92676) |
| RF3 | GSV1 CP with M_max 1100, 1100 ids | 47556 | recvFit (99208 > 98304) |
| RF3 n = 1089 / 1090 | | 47083 / 47126 | accept / recvFit (98348) |
| RF3 edge | n = 1089, subsidy 2^184 (+21 bytes) / 2^192 (+22) | 47104 / 47105 | accept at the limit exactly / recvFit (98306) |
| RF4 | BODY_MAX 262144 | 341 | recvFit (217928 > 167936) |

## 4. Trust-pool fetch (FakeTransport)

- `networks.json` is fetched through the trust pool (TRUST_POOL = 2 MiB, browser.md:318).
- The response cap and deadline are proposals, P-V3-6: 1048576 bytes and 10000 ms, both inclusive.
- The reservation is the recvLimit, taken from the pool (P-V3-7).
- Nothing is accepted on a transport error. A body that is not a JSON list is rejected as a whole.
- **Literal scripts:**
  - cap equal → passes transport (the body is then not JSON); cap + 1 → recvLimit;
  - deadline equal → accepted; deadline + 1 → transport;
  - pool remaining equal → accepted; one byte short → busy;
  - a mixed list (GSV1, N11, RF4) → [true, false, false].

## 5. Deferred experiments (specification only)

| Experiment | Inputs | Pass criteria |
|---|---|---|
| TS/Chrome options page | `vectors/v3-gsv1.json`, `vectors/v3-profiles.json` | the same decisions, error codes, traces, lengths and RecvFit tables |
| Rust GenesisSpec decoder | the GSV1 bytes and the 32 negatives | the same code per negative; byte-exact re-encode |
| M0 three-library hash | the GSV1 bytes | three Keccak libraries agree on H_GSV1 after K1–K3; afterwards H_GSV1 is frozen |
| Node start | the GSV1 profile | never boots (unsigned, non-bootable, R24(a) on alloc) |

## 6. Gaps and proposals (recorded, not approved)

- **DG-V3-1:** the network genesis bytes are not literal (g_ts, target_g, HELD ids, roots).
- **DG-V3-2:** end/full roots need node-side alloc; the w..z addresses come from TK-1 and await an independent oracle.
- **DG-V3-3:** H_GSV1 is from one library only.
- **DG-V3-4:** the structure error name is not given.
- **DG-V3-5:** it is unclear which field relations belong to decode and which to ParamGate.
- **DG-V3-6:** the fetch cap, deadline and pool reservation are not specified.
- **DG-V3-7:** forkSchedule and the target of the chainId comparison are not specified.
- **DG-V3-8:** no string or URL rules are given for profile fields.
- **DG-V3-9:** implementation.md:292 says validate imports ASERT and the M3-TS model, but no acceptance step uses them. This model does not evaluate ASERT, and the runner records that rather than faking it.
- **Proposals:** P-V3-1 to P-V3-7.

Owner decisions, the full gate, CONF_DEPTH, the representation proposals and RF-E6-1 are unchanged.
