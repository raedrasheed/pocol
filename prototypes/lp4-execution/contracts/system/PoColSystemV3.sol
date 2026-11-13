// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

// PoCol system programs, v3 network (development/decisions/V3-DESIGN.md, V3-DECISIONS.md): a copy of
// PoColSystemEarnIn.sol (itself from PoColSystemV1.sol with a continuous markClose, marked "EARN-IN");
// every difference from PoColSystemEarnIn.sol is marked "V3-xx". Reachable only through its own
// sysCodeHash on a specVersion-2 genesis (CP schema v2). The programs do not check that: on a 52-field CP
// ChainCaps.get(52..63) reads 0 (P_MAX 0, T_PROB 0, R_react 0). The host registry (apply.rs SystemPrograms::selectable)
// keeps the set off a CP schema v1 genesis (tests/v3_contracts.rs). V3-08/V3-09 registry budgets: REGISTRY-LAYOUT.md,
// tests/v3_model.py. V3-16/V3-17 evidence penalties (RewardEscrow recordResult/invalidate/burnBond, MinerRegistry
// punish): REGISTRY-LAYOUT.md "v3 evidence penalties", tests/v3_model.py. V3-02 late share credit (MinerRegistry and
// ShareLedger recordLate): REGISTRY-LAYOUT.md "v3 late share credit", tests/v3_model.py. V3-19/V3-15/V3-20 issuance, carry
// and tip split (RewardEscrow closeBlock(tips, issued), settleOne, finalize): REGISTRY-LAYOUT.md "v3 issuance, carry and
// tip split", tests/v3_model.py. V3-10/V3-14 contestable seats and the probation-pool contest (MinerRegistry recordShares/
// recordLate epoch counters, registerContest, contestStatus): REGISTRY-LAYOUT.md "v3 contestable seats", tests/v3_model.py.
// Authorities: M1 execution.md and
// incentives.md (phases, pending, settle formulas, bond machine); LP4-PAY-01/02, LP4-REGISTRY-01/02,
// LP4-SYS-01, LP4-ISSUANCE-01; LP4-LEDGER-01 option 1 (development/LP4-LEDGER-APPROVAL.md).
// Addresses: C0C001 MinerRegistry, C0C002 RewardEscrow, C0C003 ShareLedger,
// C0C004 ChainCaps, C0C005 ChunkFactory (separate source, unchanged D27 code). None emits a LOG.

interface ICaps { function get(uint256 n) external view returns(uint256); }
interface IRegistryV1 {
    function bondRecord(address id) external view returns(uint256,address,uint256);
    function payInfos(address[] calldata ids) external view returns(address[] memory rewards,bool[] memory accruing);
    function accrue(address id,uint256 amount) external returns(uint256 excess);
}
interface IEscrowV1 { function releaseBond(address id) external returns(bool); function burnBond(uint256 amount,address reward) external; } // V3-17: burnBond pays the reporter's cut
interface ILedgerV1 {
    function record(uint64 j,address[] calldata seq) external;
    function recordLate(uint64 j,address[] calldata seq) external; // V3-02
    function totalOf(uint64 j) external view returns(uint256);
    function countOf(uint64 j,address a) external view returns(uint256);
    function ownersOf(uint64 j) external view returns(address[] memory);
    function entriesOf(uint64 j) external view returns(address[] memory,uint256[] memory,uint256);
}

library Sys {
    address internal constant REGISTRY=address(0xC0C001);
    address internal constant ESCROW=address(0xC0C002);
    address internal constant LEDGER=address(0xC0C003);
    address internal constant CAPS=address(0xC0C004);
    address internal constant PAY=address(0xC0C0FF);
    address internal constant SYSTEM=address(uint160(type(uint160).max-1));
    uint256 internal constant LOW64=type(uint64).max;
    function cfg(uint256 n) internal view returns(uint256){return ICaps(CAPS).get(n);}
    /// Native PAY: exactly 5000 gas, no recipient code execution, returns success.
    function pay(address recipient,uint256 amount) internal returns(bool ok) {
        bytes memory data=abi.encode(recipient,amount);
        (ok,)=PAY.call{gas:5000}(data);
    }
}

/// V3-34: complete CP (64 fields, schema v2 order) at slots 0..63, written only by genesis.
contract ChainCaps {
    function get(uint256 n) external view returns(uint256 v) {
        require(n<64); assembly { v:=sload(n) } // V3-34: CP schema v2 length (52 v1 fields + 12 appended; FN-2a deleted F0_*)
    }
}

/// LEDGER-01 option 1: slot0 total, slot1 count, slot2 owners (standard layout).
contract ShareLedger {
    error Unauthorized(); error BadCalldata(); error BadPhase(); error BadState();
    mapping(uint64=>uint256) internal total;
    mapping(uint64=>mapping(address=>uint256)) internal count;
    mapping(uint64=>address[]) internal owners;

    function totalOf(uint64 j) external view returns(uint256){return total[j];}
    function countOf(uint64 j,address a) external view returns(uint256){return count[j][a];}
    function ownersOf(uint64 j) external view returns(address[] memory){return owners[j];}
    /// One call for settlement: distinct owners, their counts and the total.
    function entriesOf(uint64 j) external view returns(address[] memory list,uint256[] memory counts,uint256 t){
        list=owners[j];counts=new uint256[](list.length);t=total[j];
        for(uint256 i;i<list.length;i++) counts[i]=count[j][list[i]];
    }

    /// CALLER=Registry only. Exactly selector||j||0x40||N||N addresses (100+32N bytes).
    function record(uint64 j,address[] calldata) external {
        if(msg.sender!=Sys.REGISTRY) revert Unauthorized();
        uint256 n; uint256 off;
        assembly { off:=calldataload(36) n:=calldataload(68) }
        if(off!=64 || n==0 || n>Sys.cfg(10)+1 || msg.data.length!=100+32*n) revert BadCalldata();
        uint256 height=block.number;
        if(height<2 || height>Sys.LOW64 || uint256(j)!=height-1) revert BadPhase();
        if(total[j]!=0) revert BadState();
        address[] storage list=owners[j];
        for(uint256 i;i<n;i++){
            uint256 raw; assembly { raw:=calldataload(add(100,mul(32,i))) }
            if(raw==0 || raw>>160!=0) revert BadCalldata();
            address a=address(uint160(raw));
            uint256 c=count[j][a];
            if(c==0) list.push(a);
            count[j][a]=c+1;
        }
        total[j]=n;
    }

    /// V3-02 late share credit, CALLER=Registry only, exactly selector||j||0x40||N||N addresses: one owner per late
    /// share of template j, which the host validated (kind-3 evidence of the carrying block, phase 0 after
    /// recordShares/recordResult of h-1 and before settle(h-D)). j is in [max(1,h-D), h-1] with its header shares
    /// recorded (total[j] != 0), and the no-dilution cap holds: total[j] + N <= kappa*m (C_header(j) + C_late(j)), so
    /// q_j = max(total, kappa*m) never changes and late credit fills only the uncovered part of the pool.
    function recordLate(uint64 j,address[] calldata) external {
        if(msg.sender!=Sys.REGISTRY) revert Unauthorized();
        uint256 n; uint256 off;
        assembly { off:=calldataload(36) n:=calldataload(68) }
        if(off!=64 || n==0 || n>Sys.cfg(10) || msg.data.length!=100+32*n) revert BadCalldata();
        uint256 height=block.number;
        if(height<2 || height>Sys.LOW64 || j==0 || uint256(j)>=height || uint256(j)+Sys.cfg(11)<height) revert BadPhase();
        uint256 t=total[j];
        if(t==0 || t+n>Sys.cfg(8)*Sys.cfg(9)) revert BadState();
        address[] storage list=owners[j];
        for(uint256 i;i<n;i++){
            uint256 raw; assembly { raw:=calldataload(add(100,mul(32,i))) }
            if(raw==0 || raw>>160!=0) revert BadCalldata();
            address a=address(uint160(raw));
            uint256 c=count[j][a];
            if(c==0) list.push(a);
            count[j][a]=c+1;
        }
        total[j]=t+n;
    }

    /// SYSTEM only. K=max(D,W_w); deletes exactly j=h-K-1 when h>K+1, else no-op.
    function pruneShares() external {
        if(msg.sender!=Sys.SYSTEM) revert Unauthorized();
        if(msg.data.length!=4) revert BadCalldata();
        uint256 height=block.number; if(height>Sys.LOW64) revert BadPhase();
        uint256 d=Sys.cfg(11); uint256 w=Sys.cfg(12); uint256 k=d>w?d:w;
        if(height<=k+1) return;
        uint64 j=uint64(height-k-1);
        uint256 t=total[j]; if(t==0) revert BadState();
        address[] storage list=owners[j]; uint256 n=list.length;
        if(n==0 || n>t) revert BadState();
        uint256 sum;
        for(uint256 i;i<n;i++){
            address a=list[i]; uint256 c=count[j][a];
            if(a==address(0) || c==0) revert BadState();
            sum+=c; delete count[j][a];
        }
        if(sum!=t) revert BadState();
        delete owners[j]; delete total[j];
    }
}

/// Pending pots, bond bridge, settlement and finalization (M1 incentives.md).
/// Layout: slot0 burnedTotal; slot1 final height F (set only by finalize); slots 4 and 5 the carry into pot t at slot
/// 4+(t&1) (V3-15, lagged one block: the base burn of the pots settled in block t-1; slot 2 is not used); pending[j] at
/// keccak(be256(j)||be256(3))+k,
/// k=0 amount, k=1 meta = proposer<<8 | kind (0 signed, 1 fallback) | INVALID<<168,
/// k=2 winner (zero until recordResult) | WIN_ACCRUING<<160 (V3-16).
contract RewardEscrow {
    error Unauthorized(); error BondUnavailable(); error PaymentFailed(); error BadCalldata();
    error BadSystemCall(); error BadPending();
    uint256 constant INVALID=uint256(1)<<168;
    // V3-16: recordResult commits in the winner word whether pot j's winner was ACCRUING (so its bond below B_reg)
    // when the result was recorded, at the start of block j+1 before any of its evidence (apply.rs runs the v3
    // phase 0 in that order). INVALID (whole-pot burn) is set only for type-2 evidence against that winner.
    uint256 constant WIN_ACCRUING=uint256(1)<<160;
    function base(uint256 j) internal pure returns(uint256) {return uint256(keccak256(abi.encode(j,uint256(3))));}
    function get(uint256 k) internal view returns(uint256 v){assembly{v:=sload(k)}}
    function put(uint256 k,uint256 v) internal {assembly{sstore(k,v)}}
    function sys(uint256 len) internal view {if(msg.sender!=Sys.SYSTEM||msg.data.length!=len)revert BadSystemCall();if(block.number==0||block.number>Sys.LOW64)revert BadSystemCall();}

    function burnedTotal() external view returns(uint256){return get(0);}
    /// V3-15: slot of the carry into pot t. Settlement in block h adds to the carry into pot h+1 and closeBlock(h)
    /// drains the carry into pot h, so the two never share a slot; at most these two carries are outstanding.
    function carrySlot(uint256 t) internal pure returns(uint256){return 4+(t&1);}
    /// V3-15: every outstanding carry (the Escrow invariant's term): between blocks h and h+1 only the carry into pot h+1.
    function carry() external view returns(uint256){return get(4)+get(5);}
    function pending(uint256 j) external view returns(uint256,uint256,uint256){uint256 b=base(j);return(get(b),get(b+1),get(b+2));}

    function releaseBond(address id) external returns(bool) {
        if(msg.sender!=Sys.REGISTRY) revert Unauthorized();
        if(msg.data.length!=36) revert BadCalldata();
        (uint256 w0,address reward,uint256 bond)=IRegistryV1(Sys.REGISTRY).bondRecord(id);
        uint256 st=(w0>>16)&255;
        if((st!=1 && st!=4) || ((w0>>8)&255)!=0 || (w0&255)==0) revert BondUnavailable();
        // EARN-IN F1-F3 SINK: an undeliverable bond (non-durable reward) is not a system fault.
        // Report failure instead of reverting, so the mandatory SYSTEM caller (sweep/markClose)
        // leaves the bond in Escrow and the record live & claimable rather than halting the chain.
        if(bond!=0 && !Sys.pay(reward,bond)) return false;
        return true;
    }
    /// Registry-only: an evidence-burned bond stays in Escrow balance as burned. V3-17: rho_bp/10^4 of it (CP 63) is
    /// paid to `reward`, the reward address of the carrying block's signed proposer (zero: a fallback block or no
    /// record, nothing paid); a failed PAY burns that part too.
    function burnBond(uint256 amount,address reward) external {
        if(msg.sender!=Sys.REGISTRY||msg.data.length!=68) revert Unauthorized();
        uint256 cut=reward==address(0)?0:amount*Sys.cfg(63)/10000;
        if(cut!=0 && !Sys.pay(reward,cut)) cut=0;
        put(0,get(0)+amount-cut);
    }

    /// Phase 2 (non-final): pending[h]={issued + (1 - gamma_tip)*tips + carry, kind, proposer}, winner=0 (D141).
    /// V3-19: the host passes issued = issuance(h) (lp4-execution issuance.rs), which it credited to Escrow.
    /// V3-20: gamma_tip_bp/10^4 of the tips (CP 56, floor) goes to the signed proposer (tx.origin) as the proposer part
    /// does (payTo: an ACCRUING record fills its bond first, a tomb or a failed PAY burns); a fallback block (origin zero)
    /// leaves all tips in the pot. V3-15 (lagged): the carry into pot h, the base burn of block h-1's settle(h-1-D), enters
    /// pot h and is zeroed. Block h's own settle(h-D) went to the carry into pot h+1, so pot h's carry is fixed before
    /// template h is built, and block h, the last carrier of late shares of h-D, has no claim on what leaving them out
    /// would carry.
    function closeBlock(uint256 tips,uint256 issued) external {
        sys(68);
        uint256 b=base(block.number);
        if(get(b)!=0||get(b+1)!=0||get(b+2)!=0) revert BadPending();
        uint256 share=tx.origin==address(0)?0:tips*Sys.cfg(56)/10000;
        uint256 k=carrySlot(block.number); uint256 c=get(k);
        if(c!=0) put(k,0);
        uint256 amount=issued+(tips-share)+c;
        uint256 meta=tx.origin==address(0)?1:(uint256(uint160(tx.origin))<<8);
        put(b,amount); put(b+1,meta);
        if(share!=0){uint256 burned=payTo(tx.origin,share); if(burned!=0) put(0,get(0)+burned);}
    }
    /// Phase 0 of h: pending[h-1].winner = owner(nonce of h-1).
    function recordResult(uint256 j,address winner) external {
        sys(68);
        if(winner==address(0)||block.number<2||j!=block.number-1) revert BadSystemCall();
        uint256 b=base(j); uint256 meta=get(b+1)&~INVALID;
        if(meta==0||meta>>168!=0||(meta&255)>1||((meta&255)==1&&meta!=1)||get(b+2)!=0) revert BadPending();
        put(b+2,uint256(uint160(winner))|(accruingNow(winner)?WIN_ACCRUING:0)); // V3-16
    }
    /// V3-16: a live ACCRUING record (its bond is below B_reg: a bond reaching B_reg promotes it to HELD).
    function accruingNow(address id) internal view returns(bool){
        (uint256 w,,)=IRegistryV1(Sys.REGISTRY).bondRecord(id);
        return (w&255)!=0 && ((w>>8)&255)==0 && ((w>>16)&255)==4;
    }
    /// Phase 0 type-2 evidence before settle(j), after recordResult(h-1) and before punish (V3-16): the pot settles
    /// normally, the tombed offender's winner part, proposer part and shares burning through payTo and distribute,
    /// unless the accused is pot j's winner and was ACCRUING when its result was recorded: then INVALID burns the
    /// whole pot at settle(j). Several pieces of evidence may name one pot.
    function invalidate(uint256 j,address accused) external {
        sys(68);
        uint256 b=base(j); uint256 meta=get(b+1); uint256 w=get(b+2);
        if(j>=block.number||meta==0||w==0) revert BadPending();
        if(address(uint160(w))==accused && w&WIN_ACCRUING!=0) put(b+1,meta|INVALID);
    }
    /// Phase 0 of h: settle(h-D).
    function settle(uint256 j) external {
        sys(36);
        uint256 d=Sys.cfg(11);
        if(block.number<=d || j!=block.number-d) revert BadSystemCall();
        settleOne(j);
    }
    /// Phase 2 of the final block F: no pending[F]; tips burned (finalBurn). Marks
    /// F in slot1 so that each remaining pot is settled by its own SYSTEM call
    /// settleFinal(j) (LP4-FINAL-01: keeps every call within the 30M per-call cap).
    /// V3-15: both outstanding carries burn here, the carry into pot F (block F-1's settle; no pot F exists) and the carry
    /// of F's own settle(F-D); the pots settleFinal settles after it burn their base burn directly (slot1 is set), so no
    /// carry is left at F.
    function finalize(uint256 tips) external {
        sys(36);
        uint256 h=block.number; uint256 b=base(h);
        if(get(b)!=0||get(b+1)!=0||get(b+2)!=0||get(1)!=0) revert BadPending();
        uint256 c=get(4); uint256 n=get(5);
        if(c!=0) put(4,0);
        if(n!=0) put(5,0);
        put(0,get(0)+tips+c+n); put(1,h);
    }
    function settleFinal(uint256 j) external {
        sys(36);
        uint256 h=block.number; if(get(1)!=h) revert BadSystemCall();
        uint256 d=Sys.cfg(11); uint256 start=h>d?h-d+1:1;
        if(j<start||j>=h) revert BadSystemCall();
        settleOne(j);
    }
    function settleOne(uint256 j) internal {
        uint256 b=base(j); uint256 amount=get(b); uint256 meta=get(b+1);
        address winner=address(uint160(get(b+2)));
        if(meta==0) revert BadPending();
        if(winner==address(0)) revert BadPending();
        put(b,0); put(b+1,0); put(b+2,0);
        uint256 burned;
        if(meta&INVALID!=0){ burned=amount; }
        else {
            uint256 wp=amount*Sys.cfg(13)/10000;
            uint256 pp=amount*Sys.cfg(14)/10000;
            burned=payTo(winner,wp);
            burned+=payTo((meta&255)==0?address(uint160(meta>>8)):winner,pp);
            burned+=distribute(j,amount-wp-pp);
        }
        put(0,get(0)+burned);
    }
    /// R split: p_i=floor(R*c_i/Q), A=floor(R*C/Q), Q=max(C,kappa*m);
    /// baseBurn=R-A, roundBurn=A-sum(p_i). Returns the burned value of R: roundBurn, payBurn and penaltyBurn. V3-15: the
    /// baseBurn goes to the carry into pot h+1 (lagged: not the pot this block closes), except at the final block (slot1
    /// set), where it burns.
    function distribute(uint256 j,uint256 r) internal returns(uint256 burned) {
        (address[] memory list,uint256[] memory counts,uint256 c)=ILedgerV1(Sys.LEDGER).entriesOf(uint64(j));
        if(c==0) revert BadPending();
        uint256 q=Sys.cfg(9)*Sys.cfg(8); if(c>q) q=c;
        (address[] memory rewards,bool[] memory accruing)=IRegistryV1(Sys.REGISTRY).payInfos(list);
        uint256 sum;
        for(uint256 i;i<list.length;i++){
            uint256 p=r*counts[i]/q; sum+=p;
            if(p==0) continue;
            if(rewards[i]==address(0)) {burned+=p; continue;}
            // EARN-IN P1: an ACCRUING record's payment fills its bond first; Escrow keeps that part.
            uint256 out=accruing[i]?IRegistryV1(Sys.REGISTRY).accrue(list[i],p):p;
            if(out!=0 && !Sys.pay(rewards[i],out)) burned+=out;
        }
        uint256 a=r*c/q;
        burned+=a-sum;
        if(r!=a){ if(get(1)!=0) burned+=r-a; else {uint256 k=carrySlot(block.number+1); put(k,get(k)+(r-a));} } // V3-15
    }
    /// Returns the burned amount: absent record (payBurn), tomb (penaltyBurn) or
    /// failed PAY (payBurn). Otherwise PAY to the record's reward address.
    function payTo(address id,uint256 amount) internal returns(uint256) {
        if(amount==0) return 0;
        (uint256 w0,address reward,)=IRegistryV1(Sys.REGISTRY).bondRecord(id);
        if(w0==0 || ((w0>>8)&255)!=0) return amount;
        // EARN-IN P1: winner/proposer payments to an ACCRUING record fill its bond first.
        uint256 out=((w0>>16)&255)==4?IRegistryV1(Sys.REGISTRY).accrue(id,amount):amount;
        if(out!=0 && !Sys.pay(reward,out)) return out;
        return 0;
    }
}

/// MinerRegistry v1: LP4 Registry user paths/sweep/closing cursor (REGISTRY-LAYOUT.md; the v3 set adds the budget
/// slot 9 and two clocks in spare record bits, V3-08/V3-09, the epoch counters in spare record bits and the epoch totals
/// slot 10 with registerContest, V3-10/V3-14) plus SYSTEM recordShares (LEDGER-01) and punish.
contract MinerRegistry {
    error BadCalldata(); error BadValue(); error Closing(); error RegOpsLimit();
    error BadState(); error Capacity(); error RewardNotDurable(); error PaymentFailed();
    error Unauthorized(); error BadPhase(); error HeightOverflow(); error NotContestable(); // V3-10
    uint256 constant LOW64=type(uint64).max;
    uint256 constant POSMASK=uint256(type(uint32).max);
    uint256 constant BEGIN_FLAG=uint256(1)<<88;
    uint256 constant CLOSING=uint256(1)<<89;
    uint256 constant CURSOR_MASK=uint256(type(uint32).max)<<90;
    uint256 constant CLOSE_INIT=uint256(1)<<122;
    uint256 constant MARK_MASK=uint256(type(uint64).max)<<123;
    uint256 constant COUNT_MASK=(uint256(1)<<24)-1;
    uint256 constant BEGIN_MASK=uint256(type(uint64).max)<<24;
    uint256 constant ACTPOS_MASK=uint256(type(uint32).max)<<56;
    uint256 constant RECPOS_MASK=uint256(type(uint32).max)<<24;
    uint256 constant STATE_MASK=uint256(255)<<16;
    uint256 constant LAST_MASK=uint256(type(uint64).max)<<88;
    // EARN-IN: ACCRUING bond state, registeredAt in w0 bits 152..215, accruing count in slot 8.
    uint256 constant ACCRUING=4;
    // V3-14: P_MAX and T_PROB are CP schema v2 fields 54 and 55 (was: constants 8 and 60480).
    uint256 constant CP_P_MAX=54;
    uint256 constant CP_T_PROB=55;
    uint256 constant ACC_COUNT=8;
    // V3-08 / V3-09 (REGISTRY-LAYOUT.md, v3 registry budgets). Slot 9 holds this block's maintenance and
    // reactivation budgets: maintenance used 0..7, reactivations used 8..15, the height they belong to 16..79
    // (a slot carrying another height reads as nothing used, so beginBlock writes nothing new). The per-record
    // clocks use spare record bits, so a record keeps four words and erase writes nothing new: lastMaintHeight
    // modulo 2^32 in w1 bits 224..255, lastReactHeight in w1 bits 160..223 (0 = never; user transactions run at
    // height >= 1). The modular clock can only refuse: a maintenance operation exactly k*2^32 blocks after the
    // identity's previous one (or at such a height, for one with none) reverts once.
    uint256 constant BUDGETS=9;
    uint256 constant REG_MAINT_MAX=8;
    uint256 constant CP_R_REACT=58;
    uint256 constant CP_C_REACT=59;
    // V3-PR-E4 moved lastMaintHeight from w0 bits 216..255 (mod 2^40) to w1 bits 224..255 (mod 2^32), so that the epoch
    // counters sit in w0 next to lastShare, in the word recordShares and recordLate already write.
    uint256 constant MAINT_MOD=(uint256(1)<<32)-1;
    uint256 constant LOW160=uint256(type(uint160).max);
    // V3-10 / V3-14 (REGISTRY-LAYOUT.md, v3 contestable seats). The epoch of a credited share is the epoch of its template j,
    // j / E_epoch (CP 60). A record's counters describe the epoch of its lastShare: countCur (w0 bits 216..235) the shares
    // credited in epoch lastShare / E_epoch, countPrev (236..255) those of the epoch before; each saturates at 2^20 - 1, and a
    // saturated count is never contestable. Slot 10 holds the totals of every credited share (header and late, owners with
    // a record or not): the latest epoch credited 0..63, its total 64..159, the previous epoch's total 160..255.
    uint256 constant EPOCHS=10;
    uint256 constant CP_E_EPOCH=60;
    uint256 constant CP_THETA=61;
    uint256 constant CP_K_SIGMA=62;
    uint256 constant CNT_SAT=(uint256(1)<<20)-1;
    uint256 constant CNT_MASK=((uint256(1)<<40)-1)<<216;
    uint256 constant LOW96=(uint256(1)<<96)-1;
    // V3-14, owner decision 2026-11-13 (option a; V3-OWNER-DECISIONS-2026-11-12.md): a pool-contest victim exits as by
    // deregister and gives up its probation seat at once. POOL_OUT, w0 bit 87 (the top bit of the ActList position 56..87,
    // which every exit clears and no position below M_max <= 65535 sets), marks it while it is exited. ACC_COUNT counts the
    // ACCRUING records that hold a pool seat, those without POOL_OUT: the contest lowers it once, and the victim's later
    // refund, punish or promotion to HELD does not lower it again; reactivate clears the bit and takes a pool seat again.
    uint256 constant POOL_OUT=uint256(1)<<87;
    function get(uint256 k) internal view returns(uint256 v){assembly{v:=sload(k)}}
    function put(uint256 k,uint256 v) internal {assembly{sstore(k,v)}}
    function cfg(uint256 n) internal view returns(uint256){return ICaps(Sys.CAPS).get(n);}
    function rec(address id) internal pure returns(uint256){return uint256(keccak256(abi.encode(id,uint256(7))));}
    function arr(uint256 slot) internal pure returns(uint256){return uint256(keccak256(abi.encode(slot)));}
    function h() internal view returns(uint256){if(block.number>LOW64)revert HeightOverflow();return block.number;}
    function sumHeight(uint256 a,uint256 b) internal pure returns(uint256){uint256 v=a+b;if(v>LOW64)revert HeightOverflow();return v;}
    function held(uint256 w) internal pure returns(bool){return (w&255)!=0 && ((w>>8)&255)==0 && ((w>>16)&255)==1;}
    function live(uint256 w) internal pure returns(bool){uint256 st=(w>>16)&255;return (w&255)!=0 && ((w>>8)&255)==0 && (st==1 || st==ACCRUING);}
    function accruingState(uint256 w) internal pure returns(bool){return ((w>>16)&255)==ACCRUING;}
    /// V3-14: an ACCRUING record that holds a pool seat (counted in ACC_COUNT): not a pool-contest victim (POOL_OUT).
    function pooled(uint256 w) internal pure returns(bool){return (w&(STATE_MASK|POOL_OUT))==(ACCRUING<<16);}
    function registeredAt(uint256 w) internal pure returns(uint256){return (w>>152)&LOW64;}
    function pastProbation(uint256 w,uint256 height) internal view returns(bool){return height>=registeredAt(w)+cfg(CP_T_PROB);} // V3-14
    /// HELD once the bond reaches B: leaves the accruing count (V3-14: a pool-contest victim left it at the contest).
    function promote(uint256 r,uint256 w,uint256 bond) internal {
        if(bond==cfg(16)){put(r,(w&~STATE_MASK)|(uint256(1)<<16));if(w&POOL_OUT==0)put(ACC_COUNT,get(ACC_COUNT)-1);}
    }
    function system() internal view {if(msg.sender!=Sys.SYSTEM)revert Unauthorized();if(msg.data.length!=4)revert BadCalldata();}
    function bondRecord(address id) external view returns(uint256,address,uint256){uint256 r=rec(id);return(get(r),address(uint160(get(r+1))),get(r+2));}
    /// Settlement payees: reward address of each live record; zero for an absent
    /// record (payBurn) or a tomb (penaltyBurn). Two SLOADs per id.
    function payInfos(address[] calldata ids) external view returns(address[] memory rewards,bool[] memory accruing){
        rewards=new address[](ids.length);accruing=new bool[](ids.length);
        for(uint256 i;i<ids.length;i++){uint256 r=rec(ids[i]);uint256 w=get(r);if(w!=0&&((w>>8)&255)==0){rewards[i]=address(uint160(get(r+1)));accruing[i]=accruingState(w);}}
    }
    function accruingCount() external view returns(uint256){return get(ACC_COUNT);}
    /// EARN-IN S5: Escrow-only. Fills an ACCRUING bond from a settled payment; returns the rest.
    function accrue(address id,uint256 amount) external returns(uint256 excess){
        if(msg.sender!=Sys.ESCROW||msg.data.length!=68) revert Unauthorized();
        uint256 r=rec(id);uint256 w=get(r);
        if(!live(w)||!accruingState(w)) return amount;
        uint256 bond=get(r+2);uint256 room=cfg(16)-bond;uint256 add=amount<room?amount:room;
        put(r+2,bond+add);put(4,get(4)+add);promote(r,w,bond+add);
        return amount-add;
    }
    /// V3-09: w1 is returned as the reward address alone (its bits 160..223 hold the C_react clock, recordClocks).
    function recordWords(address id) external view returns(uint256,uint256,uint256,uint256){uint256 r=rec(id);return(get(r),get(r+1)&LOW160,get(r+2),get(r+3));}
    function lengths() external view returns(uint256,uint256,uint256,uint256){return(get(0),get(1),get(2),get(4));}
    function counters() external view returns(uint256){return get(3);}
    /// V3-08/V3-09: lastMaintHeight modulo 2^32 (w1 bits 224..255), lastReactHeight (w1 bits 160..223); the budget slot.
    function recordClocks(address id) external view returns(uint256,uint256){uint256 x=get(rec(id)+1);return(x>>224,(x>>160)&LOW64);}
    function budgets() external view returns(uint256){return get(BUDGETS);}
    /// V3-08: setRewardAddress/topUp draw on REG_MAINT_MAX; V3-09: reactivate on R_react (CP 58), and no
    /// longer on R_max. Register, deregister, claimBond and removeInactive keep REG_OPS less the maintenance
    /// used this block; register also keeps R_max.
    function usedThisBlock() internal view returns(uint256){uint256 b=get(BUDGETS);return ((b>>16)&LOW64)==block.number?b&65535:0;}
    /// V3-10: a contest (`extra` 1) needs two REG_OPS units (a removal and a registration) and one R_max entry.
    function checkBudgets(uint256 c,bool registering,bool reactivating,bool maintaining,uint256 extra) internal view {
        uint256 used=usedThisBlock();
        if(maintaining?(used&255)>=REG_MAINT_MAX:(reactivating?((used>>8)&255)>=cfg(CP_R_REACT):((c&65535)+(used&255)+extra>=cfg(23) || (registering && ((c>>16)&255)>=cfg(17)))))revert RegOpsLimit();
    }
    function spendBudgets(uint256 c,bool registering,bool reactivating,bool maintaining) internal {
        if(maintaining||reactivating)put(BUDGETS,(usedThisBlock()+(maintaining?1:256))|(block.number<<16));
        else put(3,c+1+(registering?(uint256(1)<<16):0));
    }
    /// V3-08: at most one maintenance operation per identity per block (the clock in w1 bits 224..255).
    function maintain(uint256 r,uint256 height) internal {
        uint256 x=get(r+1);if((x>>224)==(height&MAINT_MOD))revert RegOpsLimit();
        put(r+1,(x&~(MAINT_MOD<<224))|((height&MAINT_MOD)<<224));
    }
    /// V3-09: an identity reactivates at most once per C_react blocks, counted from its previous reactivation.
    function reactClock(uint256 r,uint256 height) internal {
        uint256 x=get(r+1);uint256 last=(x>>160)&LOW64;
        if(last!=0 && height<last+cfg(CP_C_REACT))revert BadState();
        put(r+1,(x&~(LOW64<<160))|(height<<160));
    }
    /// V3-17: the reward address of the reporter's record (tomb or not), or zero (no reporter, or no record).
    function payee(address reporter) internal view returns(address){
        if(reporter==address(0))return address(0);uint256 r=rec(reporter);
        return get(r)==0?address(0):address(uint160(get(r+1)&LOW160));
    }
    function recordAt(uint256 i) external view returns(address){if(i>=get(0))revert BadState();return address(uint160(get(arr(5)+i)));}
    function activeAt(uint256 i) external view returns(address){if(i>=get(1))revert BadState();return address(uint160(get(arr(6)+i)));}

    fallback() external payable {
        if(msg.data.length<4)revert BadCalldata();bytes4 sel=msg.sig;
        if(sel==bytes4(keccak256("registerContest(address,address)"))){contest();return;} // V3-10 / V3-14
        bool registering=sel==bytes4(keccak256("register(address)"));
        bool reactivating=sel==bytes4(keccak256("reactivate()"));
        bool rewarding=sel==bytes4(keccak256("setRewardAddress(address)"));
        bool removing=sel==bytes4(keccak256("removeInactive(address)"));
        bool exiting=sel==bytes4(keccak256("deregister()"));
        bool claiming=sel==bytes4(keccak256("claimBond()"));
        bool topping=sel==bytes4(keccak256("topUp()"));
        if(!(registering||reactivating||rewarding||removing||exiting||claiming||topping))revert BadCalldata();
        address arg;
        if(registering||rewarding||removing){
            if(msg.data.length!=36)revert BadCalldata();uint256 raw;assembly{raw:=calldataload(4)}
            if(raw>>160!=0)revert BadCalldata();arg=address(uint160(raw));
        }else if(msg.data.length!=4)revert BadCalldata();
        // EARN-IN V1: register takes 0..B, topUp a positive value, every other path none.
        if(registering?msg.value>cfg(16):(topping?msg.value==0:msg.value!=0))revert BadValue();
        uint256 c=get(3);if(c&CLOSING!=0)revert Closing();
        bool maintaining=rewarding||topping; // V3-08
        checkBudgets(c,registering,reactivating,maintaining,0); // V3-08 / V3-09
        uint256 height=h();if(c&BEGIN_FLAG==0 || ((c>>24)&LOW64)!=height)revert BadPhase();
        address id=removing?arg:msg.sender;uint256 r=rec(id);uint256 w=get(r);
        if(registering){
            enroll(id,arg,r,w,height);
        }else{
            if(!live(w))revert BadState();
            if(maintaining)maintain(r,height); // V3-08
            if(topping){
                // EARN-IN U2: fill up to B; the rest returns to the sender (durable: it sent this transaction).
                if(!accruingState(w)||(w&255)!=1||pastProbation(w,height))revert BadState();
                uint256 bond=get(r+2);uint256 add=cfg(16)-bond;if(msg.value<add)add=msg.value;
                if(!Sys.pay(Sys.ESCROW,add))revert PaymentFailed();
                if(msg.value>add && !Sys.pay(msg.sender,msg.value-add))revert PaymentFailed();
                put(r+2,bond+add);put(4,get(4)+add);promote(r,w,bond+add);
            }
            else if(rewarding){if(!Sys.pay(arg,0))revert RewardNotDurable();put(r+1,(get(r+1)&~LOW160)|uint160(arg));} // V3-09: keeps both clocks
            else if(exiting||removing){
                if((w&255)!=1||get(1)<=cfg(19))revert BadState();uint256 clocks=get(r+3);
                if(removing){uint256 last=(w>>88)&LOW64;uint256 grace=clocks>>192;if(height<sumHeight(last>grace?last:grace,cfg(25)))revert BadState();}
                uint256 due=sumHeight(height,cfg(27));put(r,(removeActive(r,w)&~uint256(255))|2);
                put(r+3,(clocks&~((uint256(1)<<128)-1))|height|(due<<64));
            }else if(reactivating){
                // V3-09: after the W_act cooldown and strictly before dueHeight, where the refund sweep erases the record.
                uint256 clocks=get(r+3);if((w&255)!=2||height<sumHeight(clocks&LOW64,cfg(24))||height>=((clocks>>64)&LOW64))revert BadState();
                if(accruingState(w)&&pastProbation(w,height))revert BadState(); // EARN-IN U4
                reactClock(r,height); // V3-09
                uint256 active=get(1);if(active>=cfg(18))revert Capacity();
                if(accruingState(w)&&w&POOL_OUT!=0){uint256 a=get(ACC_COUNT);if(a>=cfg(CP_P_MAX))revert Capacity();put(ACC_COUNT,a+1);} // V3-14: a pool seat again
                put(arr(6)+active,uint160(id));put(1,active+1);put(r,((w&~(ACTPOS_MASK|uint256(255)))|1|(active<<56)));
                put(r+3,(clocks&(LOW64<<128))|(height<<192));
            }else{
                if((w&255)!=2||height<((get(r+3)>>64)&LOW64))revert BadState();if(!refund(id,r))revert PaymentFailed();erase(id,r,w);
            }
        }
        spendBudgets(c,registering,reactivating,maintaining); // V3-08 / V3-09
    }
    /// register(address) (and the registration of registerContest): a new record for `id` with reward `arg` and bond msg.value.
    function enroll(address id,address arg,uint256 r,uint256 w,uint256 height) internal {
        uint160 a=uint160(id);if(id==address(0)||id==Sys.SYSTEM||(a>=0xC0C001&&a<=0xC0C0FF)||w!=0)revert BadState();
        uint256 n=get(0);uint256 active=get(1);if(n>=cfg(20)||active>=cfg(18))revert Capacity();
        // EARN-IN U1: a self reward is durable once this transaction runs (R9), but ONLY when the
        // registrant is the externally-owned account that signed the transaction (msg.sender==tx.origin):
        // an EOA sender persists past this tx. A contract sender (msg.sender!=tx.origin) can be created
        // and destroyed within one transaction, so its self reward is NOT guaranteed durable and must
        // pass the PAY-02 probe like any other reward (F1-F3 source fix).
        if(!(arg==msg.sender && msg.sender==tx.origin) && !Sys.pay(arg,0))revert RewardNotDurable();
        bool probation=msg.value<cfg(16);
        if(probation && get(ACC_COUNT)>=cfg(CP_P_MAX))revert Capacity(); // V3-14
        if(msg.value!=0 && !Sys.pay(Sys.ESCROW,msg.value))revert PaymentFailed();
        put(r,1|((probation?ACCRUING:1)<<16)|(n<<24)|(active<<56)|(height<<152));put(r+1,uint160(arg));put(r+2,msg.value);put(r+3,height<<192);
        put(arr(5)+n,uint160(id));put(arr(6)+active,uint160(id));put(0,n+1);put(1,active+1);put(4,get(4)+msg.value);
        if(probation)put(ACC_COUNT,get(ACC_COUNT)+1);
    }
    /// V3-10 / V3-14 registerContest(address reward, address victim), payable 0..B_reg like register: the same user-path prefix
    /// (BadCalldata, BadValue, Closing, RegOpsLimit with two REG_OPS units and one R_max entry, BadPhase), the sender must have
    /// no record (BadState), the victim must pass contestCheck (codes 1 and 3: BadState; 2 and 4..7: NotContestable). A
    /// contest needs a full set (active == M_max, O-10=b "only when all seats are full"), the pool contest included. A
    /// probationer newcomer (msg.value < B_reg) facing a full pool (ACC_COUNT >= P_MAX) displaces an ACCRUING victim from
    /// the pool. Either way the victim exits as by deregister (refund by the sweep or claimBond at dueHeight = h + U, no
    /// burn; payable and punishable until then); a pool-contest victim also gives up its pool seat at once (POOL_OUT,
    /// ACC_COUNT down by one, owner decision 2026-11-13). The sender then registers exactly as register(reward) does.
    function contest() internal {
        if(msg.data.length!=68)revert BadCalldata();
        uint256 a;uint256 v;assembly{a:=calldataload(4) v:=calldataload(36)}
        if(a>>160!=0||v>>160!=0)revert BadCalldata();
        if(msg.value>cfg(16))revert BadValue();
        uint256 c=get(3);if(c&CLOSING!=0)revert Closing();
        checkBudgets(c,true,false,false,1);
        uint256 height=h();if(c&BEGIN_FLAG==0 || ((c>>24)&LOW64)!=height)revert BadPhase();
        uint256 r=rec(msg.sender);if(get(r)!=0)revert BadState();
        address victim=address(uint160(v));uint256 vr=rec(victim);uint256 vw=get(vr);
        bool probation=msg.value<cfg(16);
        (uint256 code,,)=contestCheck(vr,vw,probation,height);
        if(code==1||code==3)revert BadState();
        if(code!=0)revert NotContestable();
        // V3-10: the victim exits; refunded after U. V3-14 (owner decision 2026-11-13, option a): a pool-contest victim exits
        // the same way, keeping its accrued bond, its pending pot parts and its evidence exposure until dueHeight, and leaves
        // the pool now (POOL_OUT; ACC_COUNT falls here, not again at its refund), so the newcomer takes its probation seat.
        bool pool=probation&&get(ACC_COUNT)>=cfg(CP_P_MAX);
        exitAt(vr,vw,get(vr+3),height,pool?POOL_OUT:0);
        if(pool)put(ACC_COUNT,get(ACC_COUNT)-1);
        enroll(msg.sender,address(uint160(a)),r,0,height);
        put(3,c+2+(uint256(1)<<16));
    }
    /// V3-10 / V3-14 contest rule for victim record word vw (record slot vr) against a newcomer that is a probationer or not,
    /// at `height`. Codes, in order: 0 contestable; 1 the victim is not a live active record; 2 nothing to contest (the set
    /// is not full: active < M_max, for a pool contest too, so E's denominator is always M_max); 3 the pool is full and the
    /// victim is not ACCRUING; 4 no
    /// measured epoch yet (height < E_epoch) or the late window of its last template still open (height mod E_epoch < D - 1);
    /// 5 the victim is in grace (height < graceStart + Theta_inact); 6 it registered or reactivated after the first block of
    /// the measured epoch; 7 its count in the measured epoch is not below the threshold (or saturated). Also returns that
    /// count and the threshold (codes 7 and 0).
    function contestCheck(uint256 vr,uint256 vw,bool probation,uint256 height) internal view returns(uint256,uint256,uint256){
        if(!live(vw)||(vw&255)!=1)return(1,0,0);
        bool pool=probation&&get(ACC_COUNT)>=cfg(CP_P_MAX);
        if(get(1)<cfg(18))return(2,0,0);
        if(pool&&!accruingState(vw))return(3,0,0);
        uint256 e=cfg(CP_E_EPOCH);uint256 cur=height/e;
        if(cur==0||height%e+1<cfg(11))return(4,0,0);
        uint256 grace=get(vr+3)>>192;
        if(height<grace+cfg(25))return(5,0,0);
        if(grace>(cur-1)*e)return(6,0,0);
        (uint256 count,uint256 thr)=measured(vw,cur-1,e);
        if(count>=thr||count>=CNT_SAT)return(7,count,thr);
        return(0,count,thr);
    }
    /// V3-10: the victim's count and the threshold of measured epoch p: countCur when the record's epoch (lastShare / E) is
    /// p, countPrev when it is p + 1, else 0; Tprev likewise from slot 10; active = the active count now, which contestCheck
    /// has required to be M_max (so exits before a contest cannot raise E).
    function measured(uint256 vw,uint256 p,uint256 e) internal view returns(uint256 count,uint256 thr){
        uint256 el=((vw>>88)&LOW64)/e;count=el==p?(vw>>216)&CNT_SAT:(el==p+1?vw>>236:0);
        uint256 t=get(EPOCHS);uint256 g=t&LOW64;
        (thr,)=threshold(g==p?(t>>64)&LOW96:(g==p+1?t>>160:0),get(1));
    }
    /// V3-10: countPrev < E - k_sigma*sqrt(E), E = theta*Tprev/active, theta = theta_bp/10^4 (CP 61), k_sigma CP 62, decided
    /// exactly over the reals. With N = theta_bp*Tprev, Dn = 10^4*active and s = isqrt(k^2*N*Dn) (floor), the real threshold
    /// is (N - sqrt(k^2*N*Dn))/Dn and the integer one ceil((N - s)/Dn) (ceil(N - x) = N - floor(x)); an integer count is
    /// below the real threshold iff it is below the integer one. 0 when N <= s (nobody is below). Returns s as well.
    function threshold(uint256 tp,uint256 active) internal view returns(uint256 thr,uint256 s){
        uint256 n=cfg(CP_THETA)*tp;uint256 d=10000*active;uint256 k=cfg(CP_K_SIGMA);
        s=isqrt(k*k*n*d);
        if(n>s)thr=(n-s+d-1)/d;
    }
    /// floor(sqrt(x)): an initial 2^(floor(log2 x)/2) within a factor of two, seven Newton steps, then the floor
    /// (OpenZeppelin Math.sqrt's method).
    function isqrt(uint256 x) internal pure returns(uint256 y){
        if(x==0)return 0;
        uint256 l;uint256 t=x;
        if(t>>128!=0){t>>=128;l+=128;}if(t>>64!=0){t>>=64;l+=64;}if(t>>32!=0){t>>=32;l+=32;}if(t>>16!=0){t>>=16;l+=16;}
        if(t>>8!=0){t>>=8;l+=8;}if(t>>4!=0){t>>=4;l+=4;}if(t>>2!=0){t>>=2;l+=2;}if(t>>1!=0){l+=1;}
        y=uint256(1)<<(l>>1);
        y=(y+x/y)>>1;y=(y+x/y)>>1;y=(y+x/y)>>1;y=(y+x/y)>>1;y=(y+x/y)>>1;y=(y+x/y)>>1;y=(y+x/y)>>1;
        uint256 q=x/y;if(q<y)y=q;
    }
    /// V3-10 views: the contest rule for `victim` at this block (as registerContest would judge it now, for a probationer
    /// newcomer or not) with the measured count and threshold; the threshold and isqrt(k^2*N*Dn) for given Tprev and active.
    function contestStatus(address victim,bool probation) external view returns(uint256,uint256,uint256){uint256 vr=rec(victim);return contestCheck(vr,get(vr),probation,h());}
    function contestThreshold(uint256 tprev,uint256 active) external view returns(uint256,uint256){if(active==0)revert BadState();return threshold(tprev,active);}
    /// V3-10: slot 10 (epoch totals).
    function epochTotals() external view returns(uint256 epoch,uint256 cur,uint256 prev){uint256 t=get(EPOCHS);return(t&LOW64,(t>>64)&LOW96,t>>160);}
    /// V3-10: n shares of template j credited to record word w: lastShare = max(lastShare, j) and the epoch counters, rolled
    /// forward when j's epoch is past the record's; a credit to the record's previous epoch (a late share of j < lastShare)
    /// raises countPrev, one to an older epoch only lastShare's max (it can never be measured).
    function credit(uint256 w,uint256 j,uint256 n,uint256 e) internal pure returns(uint256){
        uint256 last=(w>>88)&LOW64;uint256 cur=(w>>216)&CNT_SAT;uint256 prev=w>>236;uint256 el=last/e;uint256 ej=j/e;
        if(ej==el)cur+=n;
        else if(ej>el){prev=ej==el+1?cur:0;cur=n;}
        else if(ej+1==el)prev+=n;
        if(cur>CNT_SAT)cur=CNT_SAT;if(prev>CNT_SAT)prev=CNT_SAT;if(j>last)last=j;
        return (w&~(LAST_MASK|CNT_MASK))|(last<<88)|(cur<<216)|(prev<<236);
    }
    /// V3-10: n shares of template j into the epoch totals (slot 10), rolled forward as a record's counters.
    function credited(uint256 j,uint256 n,uint256 e) internal {
        uint256 t=get(EPOCHS);uint256 g=t&LOW64;uint256 cur=(t>>64)&LOW96;uint256 prev=t>>160;uint256 ej=j/e;
        if(ej==g)cur+=n;
        else if(ej>g){prev=ej==g+1?cur:0;cur=n;g=ej;}
        else if(ej+1==g)prev+=n;
        else return;
        if(cur>LOW96)cur=LOW96;if(prev>LOW96)prev=LOW96;
        put(EPOCHS,g|(cur<<64)|(prev<<160));
    }
    function removeActive(uint256,uint256 w) internal returns(uint256) {
        uint256 n=get(1);uint256 pos=(w>>56)&POSMASK;uint256 b=arr(6);if(pos>=n)revert BadState();
        if(pos!=n-1){address moved=address(uint160(get(b+n-1)));put(b+pos,uint160(moved));uint256 m=rec(moved);put(m,(get(m)&~ACTPOS_MASK)|(pos<<56));}
        put(b+n-1,0);put(1,n-1);return w&~ACTPOS_MASK;
    }
    /// One sweep step (the v1 loop body, moved out for stack depth). Returns whether the record was erased.
    function sweepOne(address id,uint256 height) internal returns(bool){
        uint256 r=rec(id);uint256 w=get(r);uint256 clocks=get(r+3);
        if(live(w)&&(w&255)==2&&height>=((clocks>>64)&LOW64)){if(refund(id,r)){erase(id,r,w);return true;}return false;}
        // EARN-IN S1: the probation deadline exits the record like deregister.
        if(live(w)&&(w&255)==1&&accruingState(w)&&pastProbation(w,height)){probationExit(r,w,clocks,height);return false;}
        if(((w>>8)&255)!=0&&((w>>16)&255)!=1&&height>=sumHeight((clocks>>128)&LOW64,cfg(26)+cfg(11)+1)){erase(id,r,w);return true;}
        return false;
    }
    /// EARN-IN S1: exits like deregister, while more than M_min members stay active.
    function probationExit(uint256 r,uint256 w,uint256 clocks,uint256 height) internal {
        if(get(1)<=cfg(19))return;
        exitAt(r,w,clocks,height,0);
    }
    /// The exit of deregister: status 2, out of the active list, exitHeight = height, dueHeight = height + U; `out` is
    /// POOL_OUT for a pool-contest victim (V3-14), else 0.
    function exitAt(uint256 r,uint256 w,uint256 clocks,uint256 height,uint256 out) internal {
        uint256 due=sumHeight(height,cfg(27));put(r,(removeActive(r,w)&~uint256(255))|2|out);
        put(r+3,(clocks&~((uint256(1)<<128)-1))|height|(due<<64));
    }
    function refund(address id,uint256 r) internal returns(bool){
        // EARN-IN F1-F3 SINK: only draw down held_total/ACC_COUNT once the bond PAY succeeds.
        // On an undeliverable bond the record stays live & claimable and its bond stays in Escrow
        // accounted in held_total; the caller decides whether to erase/settle (it must not).
        if(!IEscrowV1(Sys.ESCROW).releaseBond(id))return false;
        put(4,get(4)-get(r+2));
        if(pooled(get(r)))put(ACC_COUNT,get(ACC_COUNT)-1); // EARN-IN S2; V3-14: once only for a pool-contest victim
        return true;
    }
    function erase(address,uint256 r,uint256 w) internal {
        uint256 n=get(0);uint256 pos=(w>>24)&POSMASK;uint256 b=arr(5);if(pos>=n)revert BadState();
        if(pos!=n-1){address moved=address(uint160(get(b+n-1)));put(b+pos,uint160(moved));uint256 m=rec(moved);put(m,(get(m)&~RECPOS_MASK)|(pos<<24));}
        put(b+n-1,0);put(0,n-1);put(r,0);put(r+1,0);put(r+2,0);put(r+3,0);
    }
    function beginBlock() external {
        system();uint256 height=h();uint256 c=get(3);if(height==0 || (c&BEGIN_FLAG!=0 && height<=((c>>24)&LOW64)) || ((c>>123)&LOW64)!=height)revert BadPhase();
        put(3,(c&~(COUNT_MASK|BEGIN_MASK))|BEGIN_FLAG|(height<<24));
        if(c&CLOSING!=0)return;
        uint256 n=get(0);uint256 cur=get(2);if(n==0){if(cur!=0)put(2,0);return;}
        uint256 scans=cfg(22);if(scans>64)scans=64;uint256 deletes=cfg(21);if(deletes>16)deletes=16;
        uint256 removed;
        for(uint256 i;i<scans && removed<deletes && n!=0;i++){
            if(cur>=n)cur=0;
            if(sweepOne(address(uint160(get(arr(5)+cur))),height)){removed++;n--;}else{cur++;}
        }
        put(2,n==0?0:cur%n);
    }
    function markClose() external {
        system();uint256 height=h();uint256 c=get(3);if(height==0||((c>>123)&LOW64)>=height)revert BadPhase();
        c=(c&~MARK_MASK)|(height<<123);
        if(c&CLOSING==0 && ((cfg(47)==0 && cfg(48)==0) || (height<cfg(47) && block.timestamp<cfg(48)))){put(3,c);return;}
        if(c&CLOSE_INIT==0)c=(c&~CURSOR_MASK)|CLOSING|CLOSE_INIT|(height<<187);
        uint256 cursor=(c>>90)&POSMASK;uint256 n=get(0);uint256 end=cursor+cfg(50);if(end>n)end=n;
        for(;cursor<end;cursor++){
            address id=address(uint160(get(arr(5)+cursor)));uint256 r=rec(id);uint256 w=get(r);
            if(live(w)){if(refund(id,r))put(r,(w&~STATE_MASK)|(uint256(3)<<16));}
        }
        put(3,(c&~CURSOR_MASK)|(cursor<<90));
    }
    /// LEDGER-01 clause 2/3: one SYSTEM message; nested CALL to Ledger.record;
    /// lastShare=j once per existing distinct record; absent all-zero records skipped. V3-10: each record's epoch counters
    /// gain its count of the call (credit) and the epoch totals all n shares.
    function recordShares(uint64 j,address[] calldata seq) external {
        if(msg.sender!=Sys.SYSTEM) revert Unauthorized();
        uint256 n; uint256 off;
        assembly { off:=calldataload(36) n:=calldataload(68) }
        if(off!=64 || n==0 || n>cfg(10)+1 || msg.data.length!=100+32*n) revert BadCalldata();
        uint256 height=h(); if(height<2 || uint256(j)!=height-1) revert BadPhase();
        for(uint256 i;i<n;i++){
            uint256 raw; assembly { raw:=calldataload(add(100,mul(32,i))) }
            if(raw==0 || raw>>160!=0) revert BadCalldata();
        }
        ILedgerV1(Sys.LEDGER).record(j,seq);
        (address[] memory owners,uint256[] memory counts,)=ILedgerV1(Sys.LEDGER).entriesOf(j);
        uint256 e=cfg(CP_E_EPOCH);
        for(uint256 i;i<owners.length;i++){
            uint256 r=rec(owners[i]); uint256 w=get(r);
            if(w==0){ if(get(r+1)!=0||get(r+2)!=0||get(r+3)!=0) revert BadState(); continue; }
            if(((w>>88)&LOW64)>uint256(j)) revert BadState();
            put(r,credit(w,j,counts[i],e)); // V3-10
        }
        credited(j,n,e); // V3-10
    }
    /// V3-02 (phase 0, SYSTEM, kind-3 evidence): one nested CALL to Ledger.recordLate, then lastShare=max(lastShare,j)
    /// for every existing record among the owners (absent all-zero records skipped, as recordShares). Unlike
    /// recordShares it never reverts on lastShare > j: a later header share of the owner already moved it past j.
    /// V3-10: each late share also counts in its owner's epoch counters (credit) and the epoch totals, so late credit
    /// enters the contest statistic like a header share.
    function recordLate(uint64 j,address[] calldata seq) external {
        if(msg.sender!=Sys.SYSTEM) revert Unauthorized();
        uint256 n; uint256 off;
        assembly { off:=calldataload(36) n:=calldataload(68) }
        if(off!=64 || n==0 || n>cfg(10) || msg.data.length!=100+32*n) revert BadCalldata();
        ILedgerV1(Sys.LEDGER).recordLate(j,seq);
        uint256 e=cfg(CP_E_EPOCH);
        for(uint256 i;i<n;i++){
            uint256 raw; assembly { raw:=calldataload(add(100,mul(32,i))) }
            uint256 r=rec(address(uint160(raw))); uint256 w=get(r);
            if(w==0){ if(get(r+1)!=0||get(r+2)!=0||get(r+3)!=0) revert BadState(); continue; }
            put(r,credit(w,j,1,e)); // V3-02 lastShare = max(lastShare, j); V3-10 counters
        }
        credited(j,n,e); // V3-10: late shares count in the epoch totals
    }
    /// Evidence effect (phase 0, SYSTEM): HELD -> BURNED+tomb (active exits);
    /// PAID -> tomb only. Tomb/absent records are a host error (block invalid).
    /// V3-17: `reporter` is the carrying block's signed proposer, which must be tx.origin (zero on a fallback block);
    /// Escrow pays rho_bp of the burned bond to its record's reward address (a self-report included) and burns the rest.
    function punish(address id,address reporter) external {
        if(msg.sender!=Sys.SYSTEM||msg.data.length!=68) revert Unauthorized();
        uint256 raw; assembly{raw:=calldataload(4)} if(raw>>160!=0) revert BadCalldata();
        if(reporter!=tx.origin) revert BadCalldata(); // V3-17: no reporter on a fallback block
        uint256 height=h(); uint256 r=rec(id); uint256 w=get(r);
        if(w==0 || ((w>>8)&255)!=0) revert BadState();
        uint256 st=(w>>16)&255; uint256 clocks=get(r+3);
        if(st==1||st==ACCRUING){
            if(pooled(w))put(ACC_COUNT,get(ACC_COUNT)-1); // EARN-IN S4; V3-14: a pool-contest victim left it at the contest
            if((w&255)==1){ w=(removeActive(r,w)&~uint256(255))|2; clocks=(clocks&~((uint256(1)<<128)-1))|height|(height<<64); }
            w=(w&~STATE_MASK)|(uint256(2)<<16)|(uint256(1)<<8);
            uint256 bond=get(r+2); put(4,get(4)-bond); IEscrowV1(Sys.ESCROW).burnBond(bond,payee(reporter)); // V3-17
        } else if(st==3){ w=w|(uint256(1)<<8); }
        else revert BadState();
        clocks=(clocks&~(LOW64<<128))|(height<<128);
        put(r,w); put(r+3,clocks);
    }
}
