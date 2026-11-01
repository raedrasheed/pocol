# D-3 / D-5 — the chain's end at H_END, proofs of non-exhaustion, and a plan against a silent split

> **Status: analysis for the owner's decision. Nothing here is approved or implemented.** No source code,
> spec text or existing document was changed. Every change named below is a PROPOSAL.
>
> Sources:
> - spec: `development/m1/reference/` (cited as `consensus.md:N` etc.);
> - code: the committed HEAD `e708be2`. `node.rs` and `rpc.rs` are cited from `git show e708be2:…`,
>   because other work was editing them in the working tree during this analysis. The other files are
>   unchanged there. HEAD moved to `d1604d0` (D-7) before this file was finished. There chain.rs and net.rs
>   are unchanged; node.rs and rpc.rs line numbers differ (e.g. no production at `node.rs:617`), but the
>   findings E-1 (no finality check in the pool) and E-2 (no `pocol_getClosing` handler) still hold;
> - the caps study: [CAPS-SUSTAINABILITY.md](CAPS-SUSTAINABILITY.md) ("the study"), its model
>   `model/caps_model.py` and vectors `vectors/caps-sustainability.json`;
> - the decision text: [DECISION-PACKAGE.md](DECISION-PACKAGE.md) §3 (D-3 at :233, D-5 at :264, at `e708be2`).
>
> Tags: **SPEC** (the reference says it), **CODE** (the code does it, by reading), **BOTH**, **PROOF**
> (follows from stated facts by exact arithmetic), **ASSUMPTION**, **INFERENCE** (my reasoning, not
> tested) and **UNVERIFIED**. Nothing was run on a chain. The only things run were the scratch script of §2
> and `caps_model.py` (into the scratch directory).

## 0. Summary

1. **The end is a height bound, with an earlier time trigger.** No valid block has h > H_END (item 10a), and
   no valid block has a final parent (item 10). The final block F is the first block whose parent is
   closing with its cursor done, at least D blocks after closeH, and with h ≥ H_END or ts ≥ T_END. On the
   network profile, height ends the chain first unless blocks average more than 80 s (T_blk is 60 s).
2. **After F the chain is frozen.** Every balance, including the rewards F pays out and the refunded bonds,
   stays in the final state forever. The spec keeps the data for 30 days after ts_F and then takes a signed
   snapshot. It defines no way to move balances elsewhere.
3. **The code matches the spec on validity at the end.** It differs in three interface points: the pool
   accepts transactions after F (the spec says −32016 {final: true}); `pocol_getClosing` is not served; and
   Hello carries no protocolVersion.
4. **Proofs.** Under O2, all three caps are UNREACHABLE before H_END on the network profile. TXBYTES_CAP and
   LOG_CAP have a proof that rests only on checks the code makes. TX_CAP's proof rests on one lemma: every
   included transaction adds at least 21,000 to gasUsed. That lemma holds for Cancun's refund rules (checked
   by enumeration). Without it the bound is 475 transactions per block, and 5·10^7 would not be enough
   (61,560,000 needed). The margins are thin, 1.1–1.5%.
5. **With the current values the network caps are REACHABLE.** Under N1 alone (LOG_CAP 2^32), only LOG_CAP
   becomes unreachable. On the devnet, LOG_CAP is already unreachable (2^30 ≥ 20,000 × 32,768), and TX_CAP and
   TXBYTES_CAP fill after 53 and 86 full blocks.
6. **A silent split from LOG_CAP needs a chain that crosses the smaller cap.** If H_END × LOG_BLOCK_MAX is
   at most both values, the two values give identical blocks (proof). So 2^30 → 2^32 cannot affect
   any existing devnet (H_END 20,000 ≤ 32,768). The risk is real only for long profiles, and for every
   per-block constant outside the CP (LOG_BLOCK_MAX, LOG_COUNT_MAX, the dirty limits, …), which bind in every block.
7. **Recommendation for D-5 (a refinement of the package's (b)).**
   - Make R27 a boot gate.
   - Add protocolVersion and a digest of the consensus constants to Hello, and refuse peers on a mismatch.
   - Ship both in the same release as N1, and no later than the first binary that can boot the network
     profile.
   - Require a new protocolVersion, with a ForkSchedule entry, for any future change of a constant.
8. **Two gaps in the D-3 text to settle first:**
   - The scope of R28/R29. As the study words them, they refuse the devnet and caps/64 caps that the
     same recommendation keeps.
   - The 21,000-gas lemma behind R28 is not stated anywhere.

---

## 1. Network behaviour at and after H_END

### 1.1 Parameters

| | network (governance.md:5, :16) | devnet (devnet.rs:36–38, not normative) |
|---|---|---|
| T_blk | 60 s | 4 s |
| H_END / H_CLOSE / Z_close | 129,600 / 129,440 / 160 | 20,000 / 19,000 / 160 |
| T_END / T_CLOSE | g_ts + 10,368,000 / T_END − 9,600 | g_ts + 10^7 / g_ts + 9,990,000 |
| RET, P2, D | 128, 2, 16 (governance.md:8) | 128, 2, 4 |
| H_END at nominal pace | day 90 | 22.2 h |

R13 (governance.md:90) holds on the network: 129,440 + 160 = 129,600 (governance.md:119).

### 1.2 Phases

| # | statement | tag | source |
|---|---|---|---|
| 1 | **Closing** starts at the first block with h ≥ H_CLOSE or ts ≥ T_CLOSE, and is sticky. closeH is that block's height, and closeH ≤ H_CLOSE. | BOTH | consensus.md:140; `finality_core.rs:10–12`; `RegistryPayment.sol:159–160` (cfg(47) = H_CLOSE, cfg(48) = T_CLOSE, 0-based CP slots) |
| 2 | During closing the cursor refunds at most RET held bonds per block (HELD → PAID). cursorDone holds by closeH + ⌈RecLen/RET⌉ − 1. | BOTH | consensus.md:141, incentives.md:49; `RegistryPayment.sol:161–166`. Not checked: that the deployed bytecode equals this Solidity source (UNVERIFIED) |
| 3 | During closing, registry operations are refused, so RecLen cannot grow. User transactions keep running, but they cannot write system state: SysWriteClosing is an exceptional halt. claimBond works only outside closing. | BOTH | consensus.md:90, execution.md:87, incentives.md:48; `user_phase.rs:203` |
| 4 | **gate2** (h ≥ H_END − P2 or ts ≥ T_END − P2·T_blk) is defined and listed in ctx. I found no rule that uses it, and the code defines it with no caller outside a unit test. Its only visible role is the P2 slack in R20 and E1. | SPEC def. / CODE unused | consensus.md:142, execution.md:7; `lp4-execution/src/finality_core.rs:21–25` (callers: `lp1-node/tests/finality_core.rs` only) |
| 5 | **final** = closing_p ∧ cursorDone_p ∧ h ≥ closeH_p + D ∧ (h ≥ H_END ∨ ts ≥ T_END). It is computed from the parent state and this block's (h, ts). | BOTH | consensus.md:143; `finality_core.rs:26–28`, `apply.rs:248–252`, `:266` |
| 6 | **E1** (under R13, R20): at h = H_END the predicate holds, so block H_END is final and there are at most H_END non-genesis blocks. On the network: p.h = 129,599 ≥ 129,440 + 128 + 2 + 16 = 129,586. E1 is an upper bound, not a guarantee that H_END is reached. | SPEC (PROOF in spec) | consensus.md:146–149, governance.md:99 |
| 7 | **The final block F** runs phase 0 and the user transactions (they count toward every cap). Phase 2 then calls `finalize(tips)` instead of `closeBlock`. F mints no subsidy and burns its tips. It settles every remaining pot j ∈ [F−D+1, F−1], and pending[F] is never created. recordResult(F−1) runs first, so F−1 is settled with its winner. | BOTH | execution.md:71, :128; incentives.md:76, :90, :96; `apply.rs:310–320` (settleFinal loop), `:348` (no pending after F) |
| 8 | F's own winner and shares are never recorded, so F's miners get nothing. | SPEC | consensus.md:82 |
| 9 | **No valid child of F.** Item 10 requires final_p = false. Item 10a requires h ≤ H_END, in H-pre item 2. | BOTH | consensus.md:143–144, :154, :164; `chain.rs:276` ("10a H_END"), `:306` ("10 final parent"); `apply.rs:258`, `system.rs:72` |
| 10 | The reason a child of F gets: at h = H_END + 1 it is "10a H_END", because item 2 is checked before item 10. When F < H_END (time ended the chain), it is "10 final parent". The spec test CL-END expects 701 to fail at item 2 (10a). | BOTH | `chain.rs:274–276`, `:306`; validation.md:978 |
| 11 | At F all bonds are paid or deleted, heldTotal = 0 and Σpending = 0 (the CL-END expectation). | SPEC (test) / CODE (`apply.rs:348` enforces Σpending = 0 at F) | validation.md:975–976 |

### 1.3 Can time end the chain before height?

- **BOTH.** ts ≥ T_END is one of the two triggers in the final predicate (row 5). ts ≥ T_CLOSE starts closing
  early (row 1).
- **PROOF (script §E).** The network has T_END − g_ts = 10,368,000 s and H_END·T_blk = 7,776,000 s. Time ends
  the chain first only if the average block interval exceeds 10,368,000 / 129,600 = 80 s, that is, if the
  chain runs more than 30 days behind its schedule g_ts + 60·h. On the devnet the threshold is 500 s (125 ×
  T_blk).
- **INFERENCE.** ASERT (consensus.md:126–137) steers each height toward g_ts + T_blk·h, so falling 30 days behind
  needs a sustained collapse of hash power. Block timestamps follow honest clocks: a signed block's ts is
  max(L(0), C), a fallback's ts is fixed (consensus.md:95–97), and blocks more than Φ = 10 s in the future are
  queued (consensus.md:96; `chain.rs:277`). So T_END works as a wall-clock bound with a few seconds of slack.
- **SPEC.** When time triggers first, F can still wait for the cursor: final needs cursorDone_p and
  h ≥ closeH + D. The network closes at T_CLOSE, 9,600 s (160 nominal blocks) before T_END. The cursor needs at
  most ⌈16,384/128⌉ = 128 blocks, so at normal block rates F comes at about T_END. On a slow chain, F is the
  first block after cursorDone with ts ≥ T_END. The spec gives no time bound like E1 (E1 bounds only height).

### 1.4 What each participant observes

| who | at and after F | tag | source |
|---|---|---|---|
| validating node | Imports and serves blocks up to F. Rejects every child of F (row 9). Chain selection still runs by work, so a heavier branch that forks below F and ends in its own F′ ≤ H_END could still replace F. After F no honest work is added on top of F. | BOTH for validity; INFERENCE for the reorg point | consensus.md:198–225; `chain.rs:276`, `:306` |
| miner / proposer | pocold stops production on a final head: "chain is final; no production". Mining F is paid only through the pots F settles (F−D+1 … F−1); F's own reward is zero. | CODE (node.rs); SPEC for rewards | `node.rs:552` (HEAD); incentives.md:90 |
| pool (eth_sendRawTransaction) | Spec: −32016 {final: true}. Code: no finality check. A transaction is executed on the pending state, pooled and relayed, and never included; its receipt stays null forever. | **SPEC ≠ CODE** | network.md:152; `node.rs:444–470` (HEAD `submit_tx_by`) |
| RPC: heads | eth_blockNumber stays at F. `pocol_status.head.final` and the `pocol.final` field of eth_getBlockByNumber are true for F. "safe" = head − 6 and "finalized" = head − 60 forever, so the probabilistic "finalized" label never reaches F. | CODE; SPEC for the labels | `node.rs:851`, `rpc.rs:1065`, `rpc.rs:887` (HEAD); consensus.md:241–242 |
| RPC: closing | `pocol_getClosing` → {closing, closeH, cursor, recLen, hEnd, finalAt} is specified, but pocold has no handler: −32601. The name appears only in the request-class table. The same holds for pocol_getCaps, pocol_getPot and pocol_getSettlement. | **SPEC ≠ CODE** | network.md:333, :337–338, :345; `rpc.rs:524`, `:761` (HEAD) |
| wallets (MetaMask, embedded) | Spec: a send after F fails with −32016 {final: true}. pocold: the send returns a hash that never gets a receipt. eth_call and eth_estimateGas still simulate on the final state. | SPEC; CODE by inspection, not run | network.md:152; execution.md:101–102 |
| peers | The handshake is unchanged at F. Sync keeps serving up to F. | CODE | `node.rs:754` (HEAD) |
| operators | "closing and end, then a signed snapshot". Retention until ts_F + 30 days, then a signed snapshot, with N_rep ≥ 3. | SPEC | governance.md:196, storage.md:84–85 |

### 1.5 Pending transactions and unspent value

- **Pending transactions.** Nothing can include them after F (row 9). The spec rejects only new submissions
  (network.md:152). The code keeps every pooled transaction: the pool has no expiry and no eviction (study
  §3a). CODE; INFERENCE that they stay until restart.
- **Rewards.** settle(j) pays at block j + D, and F settles every remaining pot (row 7). Escrow ends with no
  pending entry and heldTotal = 0 (row 11). The paid amounts are ordinary account balances.
  - INFERENCE: after F no transaction can ever move them on this chain, so every balance, reward and refunded
    bond is frozen in the final state.
  - Not found in the spec (UNVERIFIED that none exists): any rule that carries balances to another chain or
    redeems them. The snapshot (governance.md:196) is the only post-end artefact.
- **Burned at F.** F's tips are burned (finalBurn = blockTips_F). F mints no subsidy, so total issuance is at most
  subsidy·(H_END − 1) (incentives.md:90, :96).

### 1.6 Where code and spec differ (end of chain)

| id | difference | effect | candidate tag |
|---|---|---|---|
| E-1 | The pool accepts and relays transactions after F (network.md:152 says −32016 {final: true}) | users get hashes that never get receipts; pool and relay work for nothing | CONFORMANCE |
| E-2 | `pocol_getClosing` (and pocol_getCaps, pocol_getPot, pocol_getSettlement) are not served (−32601) | no RPC view of closing, cursor or finalAt; the extension cannot show the end | CONFORMANCE; the fields of getCaps are N6 (D-4) |
| E-3 | Hello has no protocolVersion (network.md:7's Status has it; `net.rs:65`) | a version mismatch is caught only at the first block (item 1b), not at handshake | CONFORMANCE (study §6 lists it as open) |
| E-4 | gate2 has no consumer in spec or code | none found; the owner may want it defined or removed | SPEC question |

---

## 2. Non-exhaustion: exact arithmetic

### 2.1 What counts as a proof

Every cap is cumulative per branch, and item 16 and the LOG_CAP halt read the parent's counter (study §1;
`chain.rs:337`, `:346`, `:353–356`; `user_phase.rs:197`). A branch has at most H_END non-genesis blocks
(E1, item 10a). So **if CAP ≥ H_END × (largest per-block increment the protocol allows), no block on any branch
can push the counter past CAP**:
- no "16 caps" block can occur;
- no LogCap halt can occur;
- the cap is UNREACHABLE.

This is a proof, given the per-block bound. If CAP < H_END × max, a branch of full blocks reaches the cap, so it
is REACHABLE. "Not exhausted" then holds only under an assumed load (the organic average), which is not a
proof.

### 2.2 The per-block maxima (lemmas)

| lemma | bound per block | why | status |
|---|---|---|---|
| L1 (TX count) | 380 = ⌊8,000,000/21,000⌋ | (a) A transaction is admitted only if its gas limit fits the gas left (`user_phase.rs:258`). `used` grows by the transaction's gasUsed (`:283`), or by its whole gas limit on a halt (`:277`). (b) **Every included transaction adds ≥ 21,000 to gasUsed.** Intrinsic gas is ≥ 21,000, and under Cancun (execution.md:106) a refund never exceeds the gas paid for the slot accesses and writes that earned it. 4,800 = 2,900 + 1,900 is EIP-3529's choice. | (a) CODE. (b) ASSUMPTION about Cancun/revm. The script checks every SSTORE sequence of up to 5 writes over 3 values, for both original values and three warm-up modes: 2,178 cases, min(paid − refund) = 0 (§2.6 B1). Not a formal proof for all programs |
| L1′ (no SSTORE lemma) | 475 = 1 + ⌊(8,000,000 − 21,000)/16,800⌋ | only the EIP-3529 cap refund ≤ gasUsed/5, so gasUsed ≥ 16,800 | PROOF from the refund cap alone |
| L2 (raw tx bytes) | 196,592; trivially ≤ BODY_MAX = 196,608 | validate checks the encoded body against BODY_MAX (`chain.rs:319`), and apply checks the raw sum (`apply.rs:263`). Each tx is an RLP string item (`types.rs:17`). An exhaustive search over item-header classes (script B3) gives 196,592 with 2 items | CODE + PROOF; the BODY_MAX bound needs no encoding argument |
| L3 (charged log bytes) | 32,768 = LOG_BLOCK_MAX | a transaction whose logs would pass LOG_BLOCK_MAX halts and commits no logs (`user_phase.rs:196`, `apply_halt`); the block's counter is the committed charge (`:293`); LOG-ACCT-01 (c) charge 36 + 32·topics + data (`user_phase.rs:36`). Gas alone would allow about 997,328 bytes, so LOG_BLOCK_MAX binds | CODE + PROOF |

### 2.3 Results (network H_END 129,600; devnet 20,000; caps/64 and caps-test 4,000)

| profile, option | TX_CAP | TXBYTES_CAP | LOG_CAP |
|---|---|---|---|
| network, current (10^7, 2^33, 2^30) | REACHABLE: 26,316 full blocks (18.3 d), organic 77.16/block | REACHABLE: 43,695 blocks (30.3 d), organic 66,280 B/block | REACHABLE: 32,768 blocks (22.8 d), organic 8,285 B/block |
| network, N1 only (LOG_CAP 2^32) | REACHABLE (as above) | REACHABLE (as above) | **UNREACHABLE** (proof), margin 48,234,496 B (1.12%) |
| network, **O2** (5·10^7, 24 GiB, 2^32) | **UNREACHABLE** under L1, margin 752,000 (1.50%). **Not proven under L1′** (needs 61,560,000) | **UNREACHABLE** (proof), margin 291,480,576 B (1.13%); also with the BODY_MAX bound (needs 25,480,396,800 ≤ 25,769,803,776) | **UNREACHABLE** (proof), margin 1.12% |
| network, R27–R29 minima (49,248,000; 25,478,323,200; 4,246,732,800) | UNREACHABLE under L1, margin 0 | UNREACHABLE, margin 0; not under the BODY_MAX bound | UNREACHABLE, margin 0 |
| devnet, current (20,000; 2^24; 2^30) | REACHABLE: 53 blocks (212 s) | REACHABLE: 86 blocks (344 s) | **UNREACHABLE** (proof), margin 418,381,824 B (39%) |
| devnet, N1 (LOG_CAP 2^32) | REACHABLE | REACHABLE | UNREACHABLE, margin 85% |
| caps/64 (20,000; 2^24; 2^30) | REACHABLE: 53 blocks | REACHABLE: 86 blocks | UNREACHABLE (margin 88%) |
| caps-test (LOG_CAP 2^24) | REACHABLE | REACHABLE | REACHABLE: 512 blocks, as T18b L6 intends (validation.md:2065) |

**Proof versus assumption, in one place:**
- Proofs (given E1 and item 10a): every UNREACHABLE cell for TXBYTES_CAP and LOG_CAP; every REACHABLE cell
  (a full-block branch exists: study §2, model `txBytesBlock` and `logFill`).
- Proof conditional on L1(b): the UNREACHABLE TX_CAP cells. Under O2 the margin is 752,000 transactions
  over the whole life. It survives only if no included transaction can ever be charged less than 21,000 gas.
- Not proofs: the organic averages (77 tx, 66 KB, 8.3 KB per block). These are the sustained loads that
  would exhaust each current network cap exactly at H_END. Whether real load stays below them is an
  assumption about usage.

### 2.4 Adversarial cost at the idle base fee (reachable caps only)

The base fee never falls below 7 wei once it is at or above 7, at any load at or below the target (script D).
From baseFee0 = 10^9 it takes 145 free empty blocks to reach 7 wei. Priority fees are taken as 0 (the attacker
can be the proposer; study §2). These figures are the minimum base fee burned.

| profile, cap | minimum burned | the study's strategy |
|---|---|---|
| network TX_CAP 10^7 | 7·21,000·10^7 = 1.47·10^12 wei (1.47·10^-6 coins) | 1,470,011,760,000 wei (52,632 blocks of 190) |
| network TXBYTES_CAP 2^33 | 249,957,449,728 wei (one 131,072 B transaction per block, 65,536 blocks = 45.5 d) | 253,111,851,720 wei (2-tx blocks, 43,695 blocks) |
| network LOG_CAP 2^30 | 66,140,340,224 wei (32,768 blocks, one LOG0 each) | same |
| devnet TX_CAP / TXBYTES_CAP | 2.94·10^9 / 488,198,144 wei | 2,960,580,000 / 498,171,856 wei |
| caps-test LOG_CAP 2^24 | 1,033,442,816 wei | same |

Under O2 no network cap is reachable, so there is no attack cost to give.

### 2.5 Discrepancies with the study and the decision text

The script reproduces every compared number of the committed vectors (0 mismatches, §2.6 G). Rerunning
`caps_model.py` into the scratch directory gives a file byte-identical to `vectors/caps-sustainability.json`.
These are the differences:

| # | item | finding | effect |
|---|---|---|---|
| X1 | TXBYTES attack cost (study §2 table, network) | 2.53·10^11 wei is the cost of the 2-tx-block strategy. One TX_MAX transaction per block costs 1.25% less: 2.4996·10^11 wei | none on the conclusion |
| X2 | TX_CAP attack cost | the study's 52,632 × 190 blocks carry 80 more transactions than 10^7 (+11,760,000 wei) | none |
| X3 | **Scope of R27–R29.** Study §5 O2 says their "only exception" is the caps-test build. The same study (§5 item 4) and DECISION-PACKAGE.md:241 keep the devnet caps. | The devnet's TX_CAP 20,000 < 7,600,000 and TXBYTES_CAP 2^24 < 3,931,840,000 fail R28/R29. So do caps/64 (T18b loads L2, L3 reach the caps on purpose, validation.md:2062, :2070–2071). As worded, the gates would refuse to boot both | **the owner must fix the scope**: e.g. R28/R29 apply to profileKind = network only, while R27 applies to every profile but caps-test |
| X4 | R28 = H_END·⌊GAS_LIMIT/21000⌋ ≤ TX_CAP | rests on L1(b), which the study does not state. The O2 margin is 1.5% | state L1(b) in the gate's text, with a test vector; or size TX_CAP from L1′ (≥ 61,560,000, e.g. 6.2·10^7) if the owner wants a lemma-free proof |
| X5 | R29 uses TXB_BLOCK = 196,592 | correct, but it depends on the body encoding. BODY_MAX alone also proves 24 GiB (margin 289,406,976 B) | optional simplification: R29′ H_END·BODY_MAX ≤ TXBYTES_CAP |
| X6 | "TX_MAX" label at `apply.rs:261` | one block's count is checked against TX_CAP under the label TX_MAX (study §1 already notes it) | none on the bounds |

### 2.6 Script and its exact output

The script is `caps_proof.py` in
`evidence/d3d5-hend/` (copied there from the agent's scratch directory).
That directory is outside the repository.
- It uses the standard library only.
- It runs with `python3 -I caps_proof.py …/proposals/vectors/caps-sustainability.json`.
- Its output is deterministic: sha256 of the output `b6dc8dcf…7c993e`; of the script `8b3fa6b0…606d38`.
- It does not import `caps_model.py`. It reads the committed vectors only for section G.

```
B1 SSTORE lemma: sequences checked = 2178 ; min(paid - refund) per slot = 0 at original 1 warm-up access list values (0,)
   => refund <= gas paid for that slot, so gasUsed(tx) >= intrinsic >= 21000 (cases above; slots are additive)
B2 tx per block: floor(GAS_LIMIT/21000) = 380 ; without the SSTORE lemma (refund cap 1/5 only): 475
B3 raw tx bytes per block: max = 196592 with 2 transactions (item headers [3, 4] ); trivial bound BODY_MAX = 196608
B4 charged log bytes per block <= LOG_BLOCK_MAX = 32768 (user_phase.rs:196 halts above it); gas alone would allow up to 997328 ; one LOG0 filling it costs 288349 gas

C  verdict: UNREACHABLE (proof) iff CAP >= H_END x per-block max; else REACHABLE (blocks to fill = ceil(CAP/max)).
-- network | current (governance.md:13; execution.md:82) | H_END 129600
   TX_CAP      cap        10000000  H_END*max        49248000  REACHABLE    filled after 26316 full blocks (1578960 s = 18.3 d); organic avg 77.160/block = 20.31% of max | robust (475/block): NOT PROVEN (needs 61560000)
   TXBYTES_CAP cap      8589934592  H_END*max     25478323200  REACHABLE    filled after 43695 full blocks (2621700 s = 30.3 d); organic avg 66280.360/block = 33.71% of max | with BODY_MAX bound: NOT PROVEN (needs 25480396800)
   LOG_CAP     cap      1073741824  H_END*max      4246732800  REACHABLE    filled after 32768 full blocks (1966080 s = 22.8 d); organic avg 8285.045/block = 25.28% of max
-- network | N1 only (LOG_CAP 2^32, TX caps unchanged) | H_END 129600
   TX_CAP      cap        10000000  H_END*max        49248000  REACHABLE    filled after 26316 full blocks (1578960 s = 18.3 d); organic avg 77.160/block = 20.31% of max | robust (475/block): NOT PROVEN (needs 61560000)
   TXBYTES_CAP cap      8589934592  H_END*max     25478323200  REACHABLE    filled after 43695 full blocks (2621700 s = 30.3 d); organic avg 66280.360/block = 33.71% of max | with BODY_MAX bound: NOT PROVEN (needs 25480396800)
   LOG_CAP     cap      4294967296  H_END*max      4246732800  UNREACHABLE  margin 48234496 (1.12% of cap)
-- network | O2 = N1+N2+N3 (2^32, 5e7, 24 GiB) | H_END 129600
   TX_CAP      cap        50000000  H_END*max        49248000  UNREACHABLE  margin 752000 (1.50% of cap) | robust (475/block): NOT PROVEN (needs 61560000)
   TXBYTES_CAP cap     25769803776  H_END*max     25478323200  UNREACHABLE  margin 291480576 (1.13% of cap) | with BODY_MAX bound: UNREACHABLE
   LOG_CAP     cap      4294967296  H_END*max      4246732800  UNREACHABLE  margin 48234496 (1.12% of cap)
-- network | O2 exact minima (R27-R29 with equality) | H_END 129600
   TX_CAP      cap        49248000  H_END*max        49248000  UNREACHABLE  margin 0 (0.00% of cap) | robust (475/block): NOT PROVEN (needs 61560000)
   TXBYTES_CAP cap     25478323200  H_END*max     25478323200  UNREACHABLE  margin 0 (0.00% of cap) | with BODY_MAX bound: NOT PROVEN (needs 25480396800)
   LOG_CAP     cap      4246732800  H_END*max      4246732800  UNREACHABLE  margin 0 (0.00% of cap)
-- devnet | current (devnet.rs:39-40) | H_END 20000
   TX_CAP      cap           20000  H_END*max         7600000  REACHABLE    filled after 53 full blocks (212 s = 0.0 d); organic avg 1.000/block = 0.26% of max | robust (475/block): NOT PROVEN (needs 9500000)
   TXBYTES_CAP cap        16777216  H_END*max      3931840000  REACHABLE    filled after 86 full blocks (344 s = 0.0 d); organic avg 838.861/block = 0.43% of max | with BODY_MAX bound: NOT PROVEN (needs 3932160000)
   LOG_CAP     cap      1073741824  H_END*max       655360000  UNREACHABLE  margin 418381824 (38.96% of cap)
-- devnet | N1 (LOG_CAP 2^32) | H_END 20000
   TX_CAP      cap           20000  H_END*max         7600000  REACHABLE    filled after 53 full blocks (212 s = 0.0 d); organic avg 1.000/block = 0.26% of max | robust (475/block): NOT PROVEN (needs 9500000)
   TXBYTES_CAP cap        16777216  H_END*max      3931840000  REACHABLE    filled after 86 full blocks (344 s = 0.0 d); organic avg 838.861/block = 0.43% of max | with BODY_MAX bound: NOT PROVEN (needs 3932160000)
   LOG_CAP     cap      4294967296  H_END*max       655360000  UNREACHABLE  margin 3639607296 (84.74% of cap)
-- devnet | R28/R29 minima (O2 applied to devnet) | H_END 20000
   TX_CAP      cap         7600000  H_END*max         7600000  UNREACHABLE  margin 0 (0.00% of cap) | robust (475/block): NOT PROVEN (needs 9500000)
   TXBYTES_CAP cap      3931840000  H_END*max      3931840000  UNREACHABLE  margin 0 (0.00% of cap) | with BODY_MAX bound: NOT PROVEN (needs 3932160000)
   LOG_CAP     cap      4294967296  H_END*max       655360000  UNREACHABLE  margin 3639607296 (84.74% of cap)
-- caps/64 | current (validation.md:2062) | H_END 4000
   TX_CAP      cap           20000  H_END*max         1520000  REACHABLE    filled after 53 full blocks (530 s = 0.0 d); organic avg 5.000/block = 1.32% of max | robust (475/block): NOT PROVEN (needs 1900000)
   TXBYTES_CAP cap        16777216  H_END*max       786368000  REACHABLE    filled after 86 full blocks (860 s = 0.0 d); organic avg 4194.304/block = 2.13% of max | with BODY_MAX bound: NOT PROVEN (needs 786432000)
   LOG_CAP     cap      1073741824  H_END*max       131072000  UNREACHABLE  margin 942669824 (87.79% of cap)
-- caps-test | caps-test build (validation.md:2065) | H_END 4000
   TX_CAP      cap           20000  H_END*max         1520000  REACHABLE    filled after 53 full blocks (530 s = 0.0 d); organic avg 5.000/block = 1.32% of max | robust (475/block): NOT PROVEN (needs 1900000)
   TXBYTES_CAP cap        16777216  H_END*max       786368000  REACHABLE    filled after 86 full blocks (860 s = 0.0 d); organic avg 4194.304/block = 2.13% of max | with BODY_MAX bound: NOT PROVEN (needs 786432000)
   LOG_CAP     cap        16777216  H_END*max       131072000  REACHABLE    filled after 512 full blocks (5120 s = 0.1 d); organic avg 4194.304/block = 12.80% of max

D  empty blocks from baseFee0 1e9 to the idle fee: 145 ; fee never falls below 7 from >= 7 at u <= target: True
   network   TX_CAP  min burn 7*21000*TX_CAP = 1470000000000 wei; 190-tx blocks from 7 wei: 1470011760000 wei; from 1 gwei: 12768360473453760000 wei (12.768 coins)
   network   TXBYTES 2-tx blocks (827528 gas) from 7 wei: 253111851720 wei; cheaper 1-tx blocks of TX_MAX raw (544864 gas, 65536 blocks): 249957449728 wei
   network   LOG_CAP one LOG0 per block (288349 gas), 32768 blocks from 7 wei: 66140340224 wei
   devnet    TX_CAP  min burn 7*21000*TX_CAP = 2940000000 wei; 190-tx blocks from 7 wei: 2960580000 wei; from 1 gwei: 416075721122460000 wei (0.416 coins)
   devnet    TXBYTES 2-tx blocks (827528 gas) from 7 wei: 498171856 wei; cheaper 1-tx blocks of TX_MAX raw (544864 gas, 128 blocks): 488198144 wei
   devnet    LOG_CAP unreachable (needs 32768 blocks > H_END 20000): no attack cost
   caps/64   TX_CAP  min burn 7*21000*TX_CAP = 2940000000 wei; 190-tx blocks from 7 wei: 2960580000 wei; from 1 gwei: 416075721122460000 wei (0.416 coins)
   caps/64   TXBYTES 2-tx blocks (827528 gas) from 7 wei: 498171856 wei; cheaper 1-tx blocks of TX_MAX raw (544864 gas, 128 blocks): 488198144 wei
   caps/64   LOG_CAP unreachable (needs 32768 blocks > H_END 4000): no attack cost
   caps-test TX_CAP  min burn 7*21000*TX_CAP = 2940000000 wei; 190-tx blocks from 7 wei: 2960580000 wei; from 1 gwei: 416075721122460000 wei (0.416 coins)
   caps-test TXBYTES 2-tx blocks (827528 gas) from 7 wei: 498171856 wei; cheaper 1-tx blocks of TX_MAX raw (544864 gas, 128 blocks): 488198144 wei
   caps-test LOG_CAP one LOG0 per block (288349 gas), 512 blocks from 7 wei: 1033442816 wei

E  network   H_END*T_blk = 7776000 s (90.0 d); T_END-g_ts = 10368000 s (120.0 d); time ends first only if the mean interval > 80.00 s (1.33x T_blk)
E  devnet    H_END*T_blk = 80000 s (0.9 d); T_END-g_ts = 10000000 s (115.7 d); time ends first only if the mean interval > 500.00 s (125.00x T_blk)
E  caps/64   H_END*T_blk = 40000 s (0.5 d); T_END-g_ts = 10000000 s (115.7 d); time ends first only if the mean interval > 2500.00 s (250.00x T_blk)
E  caps-test H_END*T_blk = 40000 s (0.5 d); T_END-g_ts = 10000000 s (115.7 d); time ends first only if the mean interval > 2500.00 s (250.00x T_blk)

F  LOG_CAP values A and B give identical execution on every branch iff H_END*LOG_BLOCK_MAX <= min(A, B)
   largest H_END on which 2^30 never binds: 32768 ; 2^32: 131072 ; 2^24: 512
   network   H_END 129600: 2^30 vs 2^32 neutral: False
   devnet    H_END  20000: 2^30 vs 2^32 neutral: True
   caps/64   H_END   4000: 2^30 vs 2^32 neutral: True
   caps-test H_END   4000: 2^30 vs 2^32 neutral: True

G  cross-check with vectors/caps-sustainability.json
   network TX_CAP perBlockMax                                 mine 380                    model 380                    =
   network TX_CAP value                                       mine 10000000               model 10000000               =
   network TX_CAP sizeToCoverH_END                            mine 49248000               model 49248000               =
   network TX_CAP reachableBeforeH_END                        mine True                   model True                   =
   network TX_CAP minBlocks                                   mine 26316                  model 26316                  =
   network TXBYTES_CAP perBlockMax                            mine 196592                 model 196592                 =
   network TXBYTES_CAP value                                  mine 8589934592             model 8589934592             =
   network TXBYTES_CAP sizeToCoverH_END                       mine 25478323200            model 25478323200            =
   network TXBYTES_CAP reachableBeforeH_END                   mine True                   model True                   =
   network TXBYTES_CAP minBlocks                              mine 43695                  model 43695                  =
   network LOG_CAP perBlockMax                                mine 32768                  model 32768                  =
   network LOG_CAP value                                      mine 1073741824             model 1073741824             =
   network LOG_CAP sizeToCoverH_END                           mine 4246732800             model 4246732800             =
   network LOG_CAP reachableBeforeH_END                       mine True                   model True                   =
   network LOG_CAP minBlocks                                  mine 32768                  model 32768                  =
   devnet TX_CAP perBlockMax                                  mine 380                    model 380                    =
   devnet TX_CAP value                                        mine 20000                  model 20000                  =
   devnet TX_CAP sizeToCoverH_END                             mine 7600000                model 7600000                =
   devnet TX_CAP reachableBeforeH_END                         mine True                   model True                   =
   devnet TX_CAP minBlocks                                    mine 53                     model 53                     =
   devnet TXBYTES_CAP perBlockMax                             mine 196592                 model 196592                 =
   devnet TXBYTES_CAP value                                   mine 16777216               model 16777216               =
   devnet TXBYTES_CAP sizeToCoverH_END                        mine 3931840000             model 3931840000             =
   devnet TXBYTES_CAP reachableBeforeH_END                    mine True                   model True                   =
   devnet TXBYTES_CAP minBlocks                               mine 86                     model 86                     =
   devnet LOG_CAP perBlockMax                                 mine 32768                  model 32768                  =
   devnet LOG_CAP value                                       mine 1073741824             model 1073741824             =
   devnet LOG_CAP sizeToCoverH_END                            mine 655360000              model 655360000              =
   devnet LOG_CAP reachableBeforeH_END                        mine False                  model False                  =
   caps/64 TX_CAP perBlockMax                                 mine 380                    model 380                    =
   caps/64 TX_CAP value                                       mine 20000                  model 20000                  =
   caps/64 TX_CAP sizeToCoverH_END                            mine 1520000                model 1520000                =
   caps/64 TX_CAP reachableBeforeH_END                        mine True                   model True                   =
   caps/64 TX_CAP minBlocks                                   mine 53                     model 53                     =
   caps/64 TXBYTES_CAP perBlockMax                            mine 196592                 model 196592                 =
   caps/64 TXBYTES_CAP value                                  mine 16777216               model 16777216               =
   caps/64 TXBYTES_CAP sizeToCoverH_END                       mine 786368000              model 786368000              =
   caps/64 TXBYTES_CAP reachableBeforeH_END                   mine True                   model True                   =
   caps/64 TXBYTES_CAP minBlocks                              mine 86                     model 86                     =
   caps/64 LOG_CAP perBlockMax                                mine 32768                  model 32768                  =
   caps/64 LOG_CAP value                                      mine 1073741824             model 1073741824             =
   caps/64 LOG_CAP sizeToCoverH_END                           mine 131072000              model 131072000              =
   caps/64 LOG_CAP reachableBeforeH_END                       mine False                  model False                  =
   caps-test TX_CAP perBlockMax                               mine 380                    model 380                    =
   caps-test TX_CAP value                                     mine 20000                  model 20000                  =
   caps-test TX_CAP sizeToCoverH_END                          mine 1520000                model 1520000                =
   caps-test TX_CAP reachableBeforeH_END                      mine True                   model True                   =
   caps-test TX_CAP minBlocks                                 mine 53                     model 53                     =
   caps-test TXBYTES_CAP perBlockMax                          mine 196592                 model 196592                 =
   caps-test TXBYTES_CAP value                                mine 16777216               model 16777216               =
   caps-test TXBYTES_CAP sizeToCoverH_END                     mine 786368000              model 786368000              =
   caps-test TXBYTES_CAP reachableBeforeH_END                 mine True                   model True                   =
   caps-test TXBYTES_CAP minBlocks                            mine 86                     model 86                     =
   caps-test LOG_CAP perBlockMax                              mine 32768                  model 32768                  =
   caps-test LOG_CAP value                                    mine 16777216               model 16777216               =
   caps-test LOG_CAP sizeToCoverH_END                         mine 131072000              model 131072000              =
   caps-test LOG_CAP reachableBeforeH_END                     mine True                   model True                   =
   caps-test LOG_CAP minBlocks                                mine 512                    model 512                    =
   network emptyBlocksFromBaseFee0ToIdle                      mine 145                    model 145                    =
   network logFill gas                                        mine 288349                 model 288349                 =
   network txBytesBlock rawLengths sum                        mine 196592                 model 196592                 =
   devnet emptyBlocksFromBaseFee0ToIdle                       mine 145                    model 145                    =
   devnet logFill gas                                         mine 288349                 model 288349                 =
   devnet txBytesBlock rawLengths sum                         mine 196592                 model 196592                 =
   caps/64 emptyBlocksFromBaseFee0ToIdle                      mine 145                    model 145                    =
   caps/64 logFill gas                                        mine 288349                 model 288349                 =
   caps/64 txBytesBlock rawLengths sum                        mine 196592                 model 196592                 =
   caps-test emptyBlocksFromBaseFee0ToIdle                    mine 145                    model 145                    =
   caps-test logFill gas                                      mine 288349                 model 288349                 =
   caps-test txBytesBlock rawLengths sum                      mine 196592                 model 196592                 =
   network TX_CAP attack from 7 wei (190-tx blocks)           mine 1470011760000          model 1470011760000          =
   network TX_CAP attack from 1 gwei                          mine 12768360473453760000   model 12768360473453760000   =
   network TXBYTES attack from 7 wei (2-tx blocks)            mine 253111851720           model 253111851720           =
   network LOG_CAP attack from 7 wei                          mine 66140340224            model 66140340224            =
   network TX_CAP fullBlocksUntilOneBlockCostsASubsidy        mine 212                    model 212                    =
G  mismatches: 0
```

The script's section F says "iff". The "if" direction is the proof of §4.1. The "only if"
direction holds because a branch of full-log blocks exists (L3 and the model's `logFill`).

---

## 3. What happens when a cap is reachable and reached (brief; study §3)

| cap | what stops | what continues | source |
|---|---|---|---|
| TX_CAP / TXBYTES_CAP | every user transaction on that branch: transfers, calls, creations, and the registry's register, deregister and claimBond. A block that carries one is "16 caps"; the builder skips it | blocks, system phases, settlement and rewards (the pot is the subsidy alone, with no tips), closing and F. Balances cannot be spent on that branch | study §3a; consensus.md:170; execution.md:83; `chain.rs:337` |
| LOG_CAP | every transaction whose successful frames log: it halts, pays its whole gas limit and keeps nonce + 1. ERC-20/721 transfers are frozen | non-logging transfers and calls, registry operations, system phases | study §3b; execution.md:82; `user_phase.rs:197` |
| interface | the pool keeps accepting transactions that can never be included (N5); eth_call and eth_estimateGas report success for a transaction that will halt (N7) | — | study §3a, §3b |

On the network profile under O2 none of this can happen before H_END (§2.3). On the devnet it happens after 53
or 86 full blocks, and the approved recommendation accepts that as the declared freeze (study §5 item 4).

---

## 4. Release and compatibility plan against a silent split (PROPOSALS, not approved)

### 4.1 When a constant change splits silently

- **Fact (BOTH).** genesisHash = keccak256(GenesisSpec), with specVersion, chainId, CP, allocRoot, M_0List and
  sysCodeHash (consensus.md:7, :11). LOG_CAP is a protocol constant outside the CP (execution.md:82;
  `user_phase.rs:24`). So is every other per-block limit in `user_phase.rs:16–31`: LOG_BLOCK_MAX,
  LOG_COUNT_MAX (546), DIRTY_BLOCK_MAX, ACCT_DIRTY_BLOCK_MAX and the 16 user SYS accounts. LAG = 2048 is
  another one (governance.md:67).
- **Fact (CODE).** pocold's Hello checks chainId and genesisHash only (`node.rs:754`, HEAD; `net.rs:65`).
  ForkSchedule supports v1 only (`p1_profile.rs:229`). ForkSchedule is local configuration, never read from a
  peer (consensus.md:46).
- **Claim (PROOF).** Two nodes that differ only in LOG_CAP values A and B compute identical blocks on every
  branch if H_END·LOG_BLOCK_MAX ≤ min(A, B).
  - Why: by L3 the counter after h blocks is at most h·LOG_BLOCK_MAX ≤ min(A, B), so neither node's LogCap
    check (`user_phase.rs:197`) ever fires.
  - Otherwise a branch of full-log blocks makes their receipts and stateRoots differ at item 17, a silent
    split (study §4).
- **Consequences:**
  - 2^30 → 2^32 is **consensus-neutral on every existing devnet and on caps/64**: H_END ≤ 32,768 = 2^30/2^15
    (script F). Existing devnet data stays valid, and genesisHash does not change.
  - On the network profile it is not neutral (129,600 > 32,768). No network chain exists, and no binary boots
    the network profile today: the A13 gate refuses it (DECISION-PACKAGE.md §1, model 150.83 GiB against 6 GiB),
    and the devnet builder refuses chainId 777001 (`devnet.rs:102`). That protection ends when S1–S5 land.
  - `pocold init-devnet --h-end` accepts any H_END (`main.rs:42`). A devnet with H_END > 32,768 is exposed
    today.
  - The per-block constants (LOG_BLOCK_MAX, LOG_COUNT_MAX, the dirty limits, …) bind in **every** block. A
    mismatch in any of them splits silently at its first binding block, on every profile. R27 cannot cover
    them.

### 4.2 Options

| id | proposal | genesisHash | wire | validity | RPC | effect on existing nodes | catches | misses |
|---|---|---|---|---|---|---|---|---|
| P0 | nothing (D-5 (a)) | — | — | — | — | — | — | every out-of-CP constant mismatch |
| PA | **R27 as a boot gate on every profile but caps-test** (part of N1): H_END·LOG_BLOCK_MAX ≤ LOG_CAP, refused with −32018 [R27] | no | no | no (a boot rule) | no | none on current profiles (all pass, script C) | makes LOG_CAP never binding, so R27-compliant nodes cannot split over it (proof §4.1) | the per-block constants; an old binary without R27 |
| PB | **protocolVersion in Hello**, as network.md:7 already specifies for Status; refuse an unsupported version | no | yes (Hello arity 7 → 8; old Hellos are refused by arity, `net.rs:99–101`, test `:287–289`) | no | no | old and new pocold stop peering: a loud refusal, which is fine for single-operator devnets | ForkSchedule-level version mismatches at handshake, before the first block | same-version binaries built with different constants |
| PC | **consensus-constants digest** constsDigest(v) = keccak256(RLP([v, LOG_CAP, LOG_BLOCK_MAX, LOG_COUNT_MAX, DIRTY_BLOCK_MAX, ACCT_DIRTY_BLOCK_MAX, USER_SYS_ACCOUNTS, LAG, …])) for activeVersion(head), in Hello/Status and in pocol_getParams; refuse on a mismatch ("netConsts") | no | yes (one more Hello field) | no | pocol_getParams gains a field | as PB | **every** constant mismatch, including a rebuild with a changed constant or a test feature on a release network | constants left out of the list, so the list must be normative and vector-tested |
| PD | **versioned constants**: any change of an out-of-CP constant needs a new protocolVersion v, a ForkSchedule entry (v, H_v), a signed announcement ≥ 1,440 blocks before H_v (governance.md:183), and a new constsDigest(v) | no | header protocolVersion already exists (consensus.md:53) | from H_v, un-upgraded nodes reject blocks by netVersion (item 1b, `chain.rs:269`) | activeVersion is already in pocol_getParams (network.md:336) | an announced, named split, not a silent one (governance.md:184) | in-place upgrades of a live chain | needs v2 support in code (`p1_profile.rs:229` refuses v ≠ 1) |
| PE | LOG_CAP into CP (D-5 (c)): CP v2, 52 → 53 fields | **changes** for every network | GenesisSpec schema | gsCount for old decoders | genesis tooling, refdec.js, rp.js (accepts v1 only, study §4) | every devnet re-genesis; `store::open` refuses old data (journal bound to genesisHash, study §4) | LOG_CAP only | the other constants; in-place upgrades |
| PF | bind the constant table to specVersion (specVersion 2 = "v1 rules with table 2") | changes for new chains only | GenesisSpec specVersion (consensus.md:8) | old decoders: gsVersion | genesis tooling | old chains keep specVersion 1 and their table | all constants, at genesis | in-place upgrades; "a specVersion change is a new network" (governance.md:185) |

### 4.3 Recommendation

**PA + PB + PC, released together with N1, plus PD as the rule for every later change.** This refines
DECISION-PACKAGE.md:272's (b), which is PB + PD, in two ways:
- (b) alone cannot detect two binaries with the same version and different constants, because ForkSchedule is
  local configuration (consensus.md:46). PC detects that.
- PA turns LOG_CAP into a non-binding constant on every bootable profile, so even a missed LOG_CAP mismatch
  cannot split a compliant network.

PE and PF are heavier and still miss in-place upgrades. Keep them only if the owner wants the constants bound
into genesisHash.

**Release sequence (proposal):**
1. The owner approves D-3, with the scope of R27–R29 fixed (X3) and L1(b) stated (X4), and D-5 as above.
2. Spec edits, all in one change:
   - execution.md:82: LOG_CAP = 2^32;
   - governance.md: R27 (all profiles but caps-test), R28/R29 (network) and the gate checks;
   - network.md:7: Hello/Status = [chainId, genesisHash, protocolVersion, constsDigest, headHash, height,
     totalWork], with refusal reasons netVersion and netConsts;
   - consensus.md: the normative list of out-of-CP constants per version, with the constsDigest(1) vector.
3. One binary release with N1 + R27–R29 + the new Hello. Old binaries cannot peer with it (Hello arity).
4. Devnet migration needs no re-genesis: genesisHash does not change (LOG_CAP is outside the CP), and replay is
   identical (§4.1, H_END ≤ 32,768). Every node of a devnet must upgrade together; the arity refusal forces it.
   A data directory whose profile fails R27 is refused at boot (−32018) and must be re-created.
5. The network genesis is created only after step 3. Its genesisHash is new anyway, because of the TX caps
   (study §4), so networks.json is updated then. **Constraint:** N1, R27 and PC must ship no later than the
   first binary able to boot the network profile, that is, before or with the end of C-1 S5.
6. Later changes follow PD.

### 4.4 Acceptance tests (proposal)

| id | test | pass condition |
|---|---|---|
| AT1 | **mixed-version two-node test**: a release-N node and a release-N+1 node on one devnet genesis | the Hello is refused (by arity, or by netVersion/netConsts) and both log the reason; neither imports a block of the other; each head descends only from its own blocks; **no fork is ever adopted** |
| AT2 | same protocolVersion, different constants (e.g. a test feature that changes LOG_COUNT_MAX, on simnet) | refused with netConsts at handshake; mutation control: with the digest check removed, the test detects the split at item 17 instead |
| AT3 | R27 boundary | H_END·LOG_BLOCK_MAX = LOG_CAP boots; LOG_CAP one smaller gives −32018 [R27]; caps-test is accepted only on simnet (study §6) |
| AT4 | neutrality replay | a devnet journal with full-log blocks (H_END 20,000), replayed with LOG_CAP 2^30 and 2^32, gives identical stateRoot, receipts and gasUsed at every height |
| AT5 | ForkSchedule v2 (sv-fix-v2, governance.md:175: [(1,0),(2,50)]) | an un-upgraded node rejects block 50 with netVersion and stays on 49; an upgraded node continues |
| AT6 | constsDigest vector | an independent Python model and the Rust constant agree for v1; changing any listed constant changes the digest (mutation per constant) |
| AT7 | non-exhaustion at H_END under O2 (D-3 acceptance test 3) | a gate unit test at the R27–R29 boundaries, plus a counter simulation of H_END full blocks staying ≤ each cap; the caps-test build still reaches LOG_CAP (exhaustedAt 513, study §6) |
| AT8 | end-of-chain conformance (E-1, E-2) | after F: eth_sendRawTransaction → −32016 {final: true}; pocol_getClosing returns finalAt = F; a child of F is refused with 10a or 10 |

---

## 5. Summary table

| decision | what the owner approves | consensus | genesisHash | compatibility | resources | recommendation |
|---|---|---|---|---|---|---|
| D-3 O1 | keep the caps | none | none | none | 19.65 GiB logical (storage.md:73) | no: network caps reachable (§2.3) |
| **D-3 O2** | N1–N3: LOG_CAP 2^32, TX_CAP 5·10^7, TXBYTES_CAP 24 GiB, gates R27–R29 | values only; no new mechanism | network: changes (TX caps are CP); devnet: unchanged | no network chain exists; devnets keep their caps | about 43 GiB logical; memory needs C-1 S1 (study §5, §5b) | **yes, after fixing X3 (gate scope) and X4 (state L1(b), or raise TX_CAP to ≥ 61,560,000 for a lemma-free proof)** |
| D-3 O3 | linear allowance | new validity/halt rule | only if BURST is CP | new vectors | unchanged | only if 19.65 GiB is a hard limit |
| D-5 (a) | nothing | — | — | silent split possible for any out-of-CP constant | — | no |
| D-5 (b) | version in ForkSchedule and Hello (PB + PD) | from H_v | no | loud split at H_v; handshake refusal | none | yes, extended as below |
| **D-5 (b+)** | PA + PB + PC with N1; PD afterwards | none now (PA is a boot gate) | no | old pocold refused at handshake; devnet data kept (neutral, proof) | none | **recommended** |
| D-5 (c) | LOG_CAP into CP | schema v2 | changes everywhere | every vector, refdec.js, rp.js; devnet re-genesis | none | no (heavy; misses other constants) |

---

## 6. What this analysis could not establish

- **No chain was run to H_END.** The behaviour of §1 comes from the spec and from reading the code. The pool
  after F (E-1) and the wallets' view are by inspection. CL-END (validation.md:970–980) is the spec's test for
  the end; I did not run it.
- **markClose.** I read the Solidity source (`RegistryPayment.sol:156–167`). I did not check that the bytecode in
  `tests/data/system-v1.json` equals it.
- **L1(b)** is checked by enumeration (up to 5 SSTOREs per slot, values {0, 1, 2}). That revm charges exactly
  Cancun's gas is an ASSUMPTION. Execution.md:106 makes Cancun conformance a test requirement.
- **The reorg after F** (§1.4) is an inference from the work rule. I did not test it.
- **Live-state caps** (SLOT_CAP, ACCT_CAP, CODE_CAP) are outside the study and outside this analysis. A rough
  ESTIMATE, not checked against the A03 meter (D-A03-CAPS.md): 380 transfers to new addresses per block
  would reach ACCT_CAP = 10^6 in about 2,632 blocks. Whether O2's principle ("no cumulative cap reachable
  before H_END") should cover them is an open question.
- **Balances after the end.** I found no rule for them; the signed snapshot is the only artefact named.
