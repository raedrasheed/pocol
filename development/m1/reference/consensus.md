# Consensus rules

**0. Pinned configuration (D72, D77, D79)**

cfg = {chainId, genesisHash, ForkSchedule, CP, M_0}.

GenesisSpec = RLP([specVersion, chainId, CP, allocRoot, M_0List, sysCodeHash]):
- specVersion: u8 = 1; otherwise gsVersion.
- chainId: u64 ∈ [1, 2^64−1].
- allocRoot and sysCodeHash: exactly 32 bytes.
- genesisHash = keccak256(GenesisSpec), excluding ForkSchedule.

Decoding:
- Strict RLP, minimal-byte integers, zero = 0x80.
- Errors: gsInt (leading zero or integer ≤ 0x7f not encoded as one byte), gsCount, gsRange (out of range or unknown enum), and gsLen.
- Evaluation order: structure, gsVersion, gsCount, gsInt, gsLen, gsRange, gsOrder, then gsSys.

CP (version 1, 52 ordered elements):
- 1–8: g_ts u64, target_g u256 [1, 2^256−1], T_blk u32 ≥ 1, tau u32 ≥ 1, nonceMode u8 (0 bounded, 1 unbounded), c u8 ≥ 1, D_att u32, dFbWait u32.
- 9–15: m u16 ≥ 1, kappa u8 ≥ 1, S_max u16 ≤ 256, D u16, W_w u16, alpha_bp u16 ≤ 10^4, gamma_bp u16 ≤ 10^4−alpha_bp.
- 16–24: subsidy u256, B_reg u256, R_max u8, M_max u16 ≥ 1, M_min u16 ≥ 1, REG_RECORDS_MAX u32, SWEEP_MAX u16, SWEEP_SCAN u16, REG_OPS u16.
- 25–30: W_act u32, Theta_inact u32, E_win u32, U u32, E_max u8, EVID_BYTES_MAX u32.
- 31–40: GAS_LIMIT u64, baseFee0 u256, TX_MAX u32, BODY_MAX u32, B_code_max u32, CODE_CAP u64, TXBYTES_CAP u64, TX_CAP u64, SLOT_CAP u64, ACCT_CAP u64.
- 41–45: SYS_SLOT_MAX u32, SYS_KEYS_BLOCK_MAX u32, SYS_KEYS_USER_MAX u32, SYS_ACCT_BLOCK_MAX u32, G_SYS u64.
- 46–52: H_END u64, T_END u64, H_CLOSE u64, T_CLOSE u64, Z_close u32, RET u32 ≥ 1, P2 u32.

ParamGate, not decoding, checks inter-field relationships. R_max=0 is valid and disables register and reactivate.

Local parameters outside CP enter neither genesisHash nor validity: Δ, φ, Φ, t_exec, Δ_rel, Γ, A_fb, K_hist, R_keep, k_*, VIEW_SLACK_S, and RpcGuard/BridgeAuth parameters.

M_0List = [[id 20 B, rewardAddr 20 B]…]:
- M_min ≤ |M_0List| ≤ M_max, and |M_0List| ≥ 1.
- id strictly increases bytewise; duplicates give gsOrder.
- id ∉ {0x00×20, SYSTEM_ADDRESS, 0x…C0C001–0x…C0C0FF}; otherwise gsSys.

Deriving genesis state:
1. alloc.
2. System code.
3. For each MinerRegistry entry: active and HELD, bond = B_reg, lastShare = 0, graceStart = 0. ActList follows M_0List order; RecLen = ActLen = |M_0|.
4. Escrow += |M_0|·B_reg; heldTotal = |M_0|·B_reg.

allocRoot = the resulting state's MPT root; a mismatch violates R24(A).

sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80 for PAY])).

ForkSchedule = ordered [(v, H_v)], starting with (1,0). activeVersion(h) = the last v with H_v ≤ h. cfg is never read from a header, peer or RPC.

**1. Encoding**

Strict RLP: no leading zeros, shortest prefixes, exact element counts and no trailing bytes.

Structures:
- UT (18 elements, ≤ 613 B): [tag 'PoCol-tpl-v1', chainId u64, genesisHash, parentHash, h u64, a u32, protocolVersion u16, ts u64, target ∈ [1, 2^256−1], stateRoot, txRoot, receiptsRoot, logsBloom 256, gasLimit, gasUsed, baseFee, evidenceRoot, proposer 20 B or empty].
- SignedTemplate = [UT, sig 65 or empty] ≤ 682 B.
- Header = [SignedTemplate, nonce 8, shareList ≤ 256, winnerSig 65] ≤ 3067 B.
- Body = [txs, evidence] ≤ BODY_MAX.

Derivations:
- TemplateID = SHA256(RLP(UT)).
- SigMsg = keccak256(0x19‖'PoCol template'‖0x0a‖TemplateID).
- powHash = SHA256(TemplateID‖be64(nonce)).
- seal = keccak256(TemplateID‖be64(nonce)); seal_genesis = keccak256(genesisHash).
- shareRoot = keccak256(RLP(shareList)).
- blockHash = keccak256(TemplateID‖be64(nonce)‖shareRoot); block-0 hash = genesisHash.
- WinMsg = keccak256(0x19‖'PoCol winner'‖0x0a‖TemplateID‖be64(nonce)‖shareRoot).

Additional rules:
- txRoot and evidenceRoot are MPTs keyed by RLP(i).
- Signature: s ≤ n/2, v ∈ {0,1}, RFC6979.
- shareList is strictly increasing and excludes the winning nonce.
- Virtual genesis header = {h=0, ts=CP.g_ts, seal=seal_genesis, blockHash=genesisHash}.

**1b. Template independence from the mining result (D141)**
- Every UT field, including stateRoot, receiptsRoot, logsBloom and gasUsed, depends only on parent state, parent header, cfg, non-root UT fields and the body.
- apply for block h reads none of these from block h: nonce, its owner, shareList, winnerSig or sig.
- Block h's result (winner and shares) is recorded in h+1 through recordShares(h) and recordResult(h), using ParentResult(h) (D142), not local state.
  - ParentResult(p) = {blockHash(p), winner, shares}. shares is [(n, owner(n))] for each n in shareList(p), in its ascending order; winner = owner(p.nonce). The winning nonce is an implicit share counted for winner.
  - shareList is committed by shareRoot inside blockHash, so every envelope copy of p yields the same list.
  - Owners come from p's assignment: the seed from the grandparent's seal and A_p* from grandparent state. ParentResult therefore depends only on committed content.
  - Two parents with the same template/nonce but different shareRoot values have different blockHash values and independent ParentResult values. Their children's stateRoot values therefore differ if the shares differ; they are not mixed.
- Lemma: for fixed UT, every nonce satisfying items 6–9 and 13 in any A_h* owner's range yields the same roots. Thus two blocks with the same template share TemplateID and stateRoot and differ only in blockHash.
- Final block F has no child, so its result is not recorded, consistently with its lack of a direct reward.

**2. Membership**
- M_h = MinerRegistry records in parent state [PDF:87]; Act_h = ActList ≤ M_max.
- A_h = {m ∈ Act_h, tomb=0 : lastShare ≥ h−W_act or graceStart ≥ h−W_act}.
- A_h* = A_h, or Act_h excluding tombstones if A_h is empty.
- deregister and removeInactive fail if they would make |Act| < M_min.
- register and reactivate ≤ R_max per block, effective from h+1.
- Registry operations are rejected during closing, so RecLen does not increase after closeH.

**3. Timing**
- L(a) = p.ts+a·D_att+1 and U(a) = p.ts+(a+1)·D_att, computed in u128. A block is invalid if U(a) ≥ 2^63.
- Validity: L(a) ≤ ts ≤ U(a).
- Fallback timestamp: L(0)+Δ_fb_wait at a=0; L(a) at a ≥ 1.
- ts > C+Φ means a queue bounded to ≤ 256.
- The proposer builds with max(L(0), C); transition to a+1 occurs when C > U(a)+Γ.
- T_j is in milliseconds; arrival exactly at T_j locks.

**4. State machine per (parentHash, h, a)**
- (A) a=0: WAIT until T_j. A valid signed template from Draw(h,0) arriving at time ≤ T_j locks, then PROP; otherwise sticky FB. An invalid template counts as absent and the peer is penalized.
- (B) Empty Act: stop.
- (C) a ≥ 1: FB when C ≥ L(a).
- A second template from the same proposer: type-1 evidence, then FB.
- A template preceding its parent is stored after form and 1b checks (≤ 4 per parent, ≤ 64 total), and removed on parentInvalid.

**4b. Build_fb(p, a)**
- chainId and genesisHash from cfg; protocolVersion = activeVersion(h).
- proposer and sig are empty; ts follows §3.
- target = ASERT(p), gasLimit = GAS_LIMIT, baseFee = nextBaseFee(p).
- Body = 0xc2c0c0; roots come from full apply.

**5. Seed and partition**
- seed = SHA256('PoColNonce'‖seal_p‖be64(h)‖be32(a)).
- ticket = SHA256(seed‖id20), sorted ascending over A_h* [PDF:91, modified].
- start(i) = floor(i·nMax/n); end(i) = floor((i+1)·nMax/n)−1 [PDF:92].
- work(t) = floor(2^256/(t+1)).
- nMax = min(2^64, c·work(target)) in nonceMode=0; 2^64 in nonceMode=1.

**6. Draw**
- w(m) = Σcount[j][m] over j ∈ [h−1−W_w, h−2], for A_h* members.
- P+ = {w > 0}; if empty, select uniformly.
- x_k = SHA256('PoColProposer'‖seal_p‖be64(h)‖be32(0)‖be32(k)), with rejection sampling to remove bias.
- Select the first member in address order whose cumulative sum exceeds x_k mod W.

**7. Fixed-width ASERT (D74, D83)**

floor_div rounds toward −∞.
1. Δt = (p.ts−g_ts) − T_blk·p.h, in i128, with |Δt| < 2^97.
2. e = floor_div(Δt·65536, τ).
3. s = floor_div(e, 65536); f = e−65536·s ∈ [0, 65535].
4. s ≥ 256 gives 2^256−1; s ≤ −257 gives 1; computation ends.
5. F = 65536 + floor_div(195766423245049·f + 971821376·f² + 5127·f³ + 2^47, 2^48), in u128, with F ∈ [65536, 131071].
6. X = target_g·F < 2^273; k = s−16 ∈ [−272, 239]. Y = X·2^k if k ≥ 0; otherwise Y = floor(X/2^(−k)).
7. target = min(max(Y, 1), 2^256−1).

Equivalence lemma: X ∈ [2^16, 2^273), so k ≥ 240 clamps to 2^256−1, while k ≤ −273 yields 0 then 1. Hence D83 = D74. At block h=1 the target is target_g.

**8. Closing and finalization (D57, D85)**
- closing is sticky, beginning when h ≥ H_CLOSE or ts ≥ T_CLOSE. closeH is the first closing block; closeH ≤ H_CLOSE.
- The cursor processes ≤ RET records per block starting at closeH. cursorDone is true in state closeH+⌈RecLen_closeH/RET⌉−1 at the latest.
- gate2: h ≥ H_END−P2 or ts ≥ T_END−P2·T_blk.
- final = closing_p and cursorDone_p and h ≥ closeH_p+D and (h ≥ H_END or ts ≥ T_END). The final block has no valid child.
- Item 10a: h ≤ H_END.

Theorem E1 (under R13 and R20):
- At h = H_END, p.h ≥ closeH+⌈REG_RECORDS_MAX/RET⌉+P2+D.
- Thus cursorDone_p is true and H_END is final; no block lies above it, and the non-genesis block count is ≤ H_END.
- E1 is an upper bound, not an arrival guarantee.

**H-pre** (stateless, with parent header or virtual genesis header):
1. Encoding and sizes; each transaction ≤ TX_MAX.
1b. NetID in order: chainId, genesisHash, activeVersion. Errors: netChain, netGenesis, netVersion.
2. h = p.h+1, parentHash = blockHash(p), h ≤ H_END (10a).
3. Form: either (proposer≠∅ and sig≠∅ and a=0 and ecrecover = proposer), or (proposer=∅ and sig=∅).
4. Time window and fallback timestamp.
5. target = ASERT(p).
6. nonce < nMax.
7. powHash ≤ target.
8. winnerSig is recoverable.
9. Every share < nMax and distinct from nonce; SHA256(TemplateID‖be64(n)) ≤ T_share.

**H-full** (with parent state):
10. final_p = false.
11. gasLimit and baseFee.
12. Exclusive branching: signed requires proposer = Draw(h,0). Fallback requires byte-identical RLP(UT) = Build_fb(p,a).UT and Body = 0xc2c0c0.
13. nonce owner ∈ A_h*; winnerSig is from that owner.
14. Share owners ∈ A_h*.
15. Body matches roots and limits.
16. txBytes and txCount are within caps.
17. apply matches stateRoot, receiptsRoot, logsBloom and gasUsed; system assertions hold.

Evaluation order (D75):
- The node stops at the first violation (firstReject); diagnostics evaluate every evaluable item.
- Without parent header: items 2–9 are notEvaluable. Without parent state: items 10–17 are notEvaluable.
- Additional messages: unknownParent, ancestorInvalid, parentInvalid, templateForm.
- Envelope classification (D129): blockHash commits to neither sig nor winnerSig; each violation belongs to one of two classes:
  - envReject: a violation depending only on sig or winnerSig bytes. Includes:
    - Failure to decode the header, or malformed encoding of the two signature fields (item 1).
    - Agreement between sig presence and proposer, and signature recovery (item 3, excluding committed condition a=0).
    - winnerSig recovery (item 8).
    - The winnerSig signer being the nonce owner (signature portion of item 13).

    Effect: reject header copy hdrId = keccak256(RLP(Header)), not the block. Penalize the sender; do not record a block in pocol_getRejects or trigger parentInvalid/ancestorInvalid. The block remains admissible through another copy.
  - blockReject: every other violation. It depends on committed content and rejects blockHash and its descendants.
- A block is valid if its committed content is valid and a valid envelope copy exists.
- The node stores the first envelope-valid copy and serves it through gossip, synchronization, pocol_getHeaders and GetEnvelope (D131).
- envWait (D131): a block whose known copies all received envReject remains undecided at blockHash level. Alternative-envelope fetching continues until a valid copy arrives or the block is removed under sync-queue limits or D118. Removal is local retention, not invalidity. Further copies of an accepted blockHash are dropped without checking.
- This is an implementation specification; it changes neither validity outcomes nor chain selection.
- Envelope-check stages (D130):
  - envPre: stateless; decoding both signature fields and recovering sig/winnerSig (signature portions of items 1, 3 and 8).
  - envOwn: parent state, no execution; checks that the winnerSig signer owns nonce in A_h* (signature portion of item 13). Cost is assignment computation from parent state, cached per parent.
  - Before parent state is available, the copy is envPending, with neither verdict nor rejection.
  - An envOwn verdict is final for each hdrId because the parent is committed in blockHash. Retaining settled verdicts is bounded by local policy D136 (network). Eviction allows rechecking hdrId with the same deterministic result and does not change block validity.
- Execution (apply) occurs once per blockHash after its first envOk copy; its verdict is cached in BlockCache.
- envReject does not invoke RejectPath.cancel, set HfullDone or record a block in pocol_getRejects. These three are restricted to blockReject or import.

**9. Chain selection (D133, amends D15)**

From the thesis [PDF:74 §4.2]: only the cumulative-work rule. The tie-break is unavailable in the provided excerpts; the following is our addition.

Definitions:
- Candidates: headers of blocks imported (passed H-full) by the node.
- W(t) = Σwork from genesis to t.
- importSeq(b): a local monotonic counter assigned to each blockHash at its first successful import. It remains unchanged while the block is stored. A block removed by D118 and imported again receives a new number.
- key(b) = (signed(b) ? 0 : 1, importSeq(b), blockHash(b)); lexicographically smaller is better.

The ordering t1 ≻ t2 holds in either case:
- W(t1) > W(t2).
- Or W is equal and key(c1) < key(c2), where c1 and c2 are children of the lowest common ancestor on the paths to t1 and t2.

Total-order lemma:
- work(t) ≥ 1 for every target, so a candidate's path cannot prefix another's with equal work. Thus distinct children c1 ≠ c2 always exist when W is equal.
- Comparison is lexicographic over paths from genesis; the key totally orders siblings, so ≻ is a transitive total order.
- Head = maximum under ≻, independent of examination order. No sequential pairwise comparison.

Transition lemma:
- W and key are fixed for each stored block.
- The set grows only through import; D118 does not remove canonical blocks.
- Hence head changes only on import, at most once per import.

Effect:
- Signed precedes fallback at the first divergence (D15).
- Same-type sibling ties use local import precedence and cannot be ground.
- The rule changes no block's validity.

**10. Recovery**
- Displaced transactions return to the pool after revalidation.
- Rollback uses inverse diffs for all state, including pending[j] (D78).
- Trie window: K_eff ≤ K_hist; requests outside it return −32017.
- Roll back up to R_keep, then TrieRebuild.
- Side blocks ≤ 2 per height (D118):
  - Retain the canonical block if present and one noncanonical block ranked by its branch's highest cumulative work, then first seen.
  - Compare another arrival with the stored noncanonical block, retain the higher-ranked one and delete the loser with its stored descendants.
  - A branch that later becomes heavier is fetched through synchronization via Status.
  - This is a local-retention rule, not validity.

**11. Progress (conditional)**
An honest miner starts within X^W; in bounded mode p0 ≥ 1−exp(−c·ρ_hon).

**12. Finality**
Probabilistic: safe = head−6, finalized = head−60 [PDF:76-77], without a mathematical guarantee.

**13. Consensus independence from RPC and the extension (D87–D97)**
- Validation/mining run in a separate prioritized runtime with one redb writer.
- RPC reads use snapshots released at computation completion.
- The following enter no validity item:
  - RpcGuard, SendPacer and PacerTrace.
  - The pocol_getLogs anchor and sendDone/receiveDone events.
  - BridgeAuth, bridge-message envelope, rate bucket, pending counter, SiteStorage quota and node origin blocking.
- pocol_getLogs reads the canonical chain without changing it.
