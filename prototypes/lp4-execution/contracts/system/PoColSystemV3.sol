// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

// PoCol system programs, v3 network (development/decisions/V3-DESIGN.md, V3-DECISIONS.md): a copy of
// PoColSystemEarnIn.sol (itself from PoColSystemV1.sol with a continuous markClose, marked "EARN-IN");
// every difference from PoColSystemEarnIn.sol is marked "V3-xx". Reachable only through its own
// sysCodeHash on a specVersion-2 genesis (CP schema v2). The programs do not check that: on a 52-field CP
// ChainCaps.get(52..66) reads 0 (P_MAX 0, T_PROB 0, R_react 0). The host registry (apply.rs SystemPrograms::selectable)
// keeps the set off a CP schema v1 genesis (tests/v3_contracts.rs). V3-08/V3-09 registry budgets: REGISTRY-LAYOUT.md,
// tests/v3_model.py. Authorities: M1 execution.md and
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
interface IEscrowV1 { function releaseBond(address id) external returns(bool); function burnBond(uint256 amount) external; }
interface ILedgerV1 {
    function record(uint64 j,address[] calldata seq) external;
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

/// V3-34: complete CP (67 fields, schema v2 order) at slots 0..66, written only by genesis.
contract ChainCaps {
    function get(uint256 n) external view returns(uint256 v) {
        require(n<67); assembly { v:=sload(n) } // V3-34: CP schema v2 length (52 v1 fields + 15 appended)
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
/// Layout: slot0 burnedTotal; slot1 final height F (set only by finalize); pending[j] at keccak(be256(j)||be256(3))+k,
/// k=0 amount, k=1 meta = proposer<<8 | kind (0 signed, 1 fallback) | INVALID<<168,
/// k=2 winner (zero until recordResult).
contract RewardEscrow {
    error Unauthorized(); error BondUnavailable(); error PaymentFailed(); error BadCalldata();
    error BadSystemCall(); error BadPending();
    uint256 constant INVALID=uint256(1)<<168;
    function base(uint256 j) internal pure returns(uint256) {return uint256(keccak256(abi.encode(j,uint256(3))));}
    function get(uint256 k) internal view returns(uint256 v){assembly{v:=sload(k)}}
    function put(uint256 k,uint256 v) internal {assembly{sstore(k,v)}}
    function sys(uint256 len) internal view {if(msg.sender!=Sys.SYSTEM||msg.data.length!=len)revert BadSystemCall();if(block.number==0||block.number>Sys.LOW64)revert BadSystemCall();}

    function burnedTotal() external view returns(uint256){return get(0);}
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
    /// Registry-only: an evidence-burned bond stays in Escrow balance as burned.
    function burnBond(uint256 amount) external {
        if(msg.sender!=Sys.REGISTRY||msg.data.length!=36) revert Unauthorized();
        put(0,get(0)+amount);
    }

    /// Phase 2 (non-final): pending[h]={subsidy+tips, kind, proposer}, winner=0 (D141).
    function closeBlock(uint256 tips) external {
        sys(36);
        uint256 b=base(block.number);
        if(get(b)!=0||get(b+1)!=0||get(b+2)!=0) revert BadPending();
        uint256 amount=Sys.cfg(15)+tips;
        uint256 meta=tx.origin==address(0)?1:(uint256(uint160(tx.origin))<<8);
        put(b,amount); put(b+1,meta);
    }
    /// Phase 0 of h: pending[h-1].winner = owner(nonce of h-1).
    function recordResult(uint256 j,address winner) external {
        sys(68);
        if(winner==address(0)||block.number<2||j!=block.number-1) revert BadSystemCall();
        uint256 b=base(j); uint256 meta=get(b+1)&~INVALID;
        if(meta==0||meta>>168!=0||(meta&255)>1||((meta&255)==1&&meta!=1)||get(b+2)!=0) revert BadPending();
        put(b+2,uint256(uint160(winner)));
    }
    /// Phase 0 type-2 evidence: pot_j becomes invalid before settle(j).
    function invalidate(uint256 j) external {
        sys(36);
        uint256 b=base(j); uint256 meta=get(b+1);
        if(j>=block.number||meta==0||meta&INVALID!=0) revert BadPending();
        put(b+1,meta|INVALID);
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
    function finalize(uint256 tips) external {
        sys(36);
        uint256 h=block.number; uint256 b=base(h);
        if(get(b)!=0||get(b+1)!=0||get(b+2)!=0||get(1)!=0) revert BadPending();
        put(0,get(0)+tips); put(1,h);
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
    /// baseBurn=R-A, roundBurn=A-sum(p_i). Returns all burned value of R.
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
        burned+=(r-a)+(a-sum);
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
/// slot 9 and two clocks in spare record bits, V3-08/V3-09) plus SYSTEM recordShares (LEDGER-01) and punish.
contract MinerRegistry {
    error BadCalldata(); error BadValue(); error Closing(); error RegOpsLimit();
    error BadState(); error Capacity(); error RewardNotDurable(); error PaymentFailed();
    error Unauthorized(); error BadPhase(); error HeightOverflow();
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
    // modulo 2^40 in w0 bits 216..255, lastReactHeight in w1 bits 160..223 (0 = never; user transactions run at
    // height >= 1). The modular clock can only refuse: a maintenance operation exactly k*2^40 blocks after the
    // identity's previous one (or at such a height, for one with none) reverts once.
    uint256 constant BUDGETS=9;
    uint256 constant REG_MAINT_MAX=8;
    uint256 constant CP_R_REACT=61;
    uint256 constant CP_C_REACT=62;
    uint256 constant MAINT_MOD=(uint256(1)<<40)-1;
    uint256 constant LOW160=uint256(type(uint160).max);
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
    function registeredAt(uint256 w) internal pure returns(uint256){return (w>>152)&LOW64;}
    function pastProbation(uint256 w,uint256 height) internal view returns(bool){return height>=registeredAt(w)+cfg(CP_T_PROB);} // V3-14
    /// HELD once the bond reaches B: leaves the accruing count.
    function promote(uint256 r,uint256 w,uint256 bond) internal {
        if(bond==cfg(16)){put(r,(w&~STATE_MASK)|(uint256(1)<<16));put(ACC_COUNT,get(ACC_COUNT)-1);}
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
    /// V3-08/V3-09: lastMaintHeight modulo 2^40 (w0 bits 216..255), lastReactHeight (w1 bits 160..223); the budget slot.
    function recordClocks(address id) external view returns(uint256,uint256){uint256 r=rec(id);return(get(r)>>216,(get(r+1)>>160)&LOW64);}
    function budgets() external view returns(uint256){return get(BUDGETS);}
    /// V3-08: setRewardAddress/topUp draw on REG_MAINT_MAX; V3-09: reactivate on R_react (CP 61), and no
    /// longer on R_max. Register, deregister, claimBond and removeInactive keep REG_OPS less the maintenance
    /// used this block; register also keeps R_max.
    function usedThisBlock() internal view returns(uint256){uint256 b=get(BUDGETS);return ((b>>16)&LOW64)==block.number?b&65535:0;}
    function checkBudgets(uint256 c,bool registering,bool reactivating,bool maintaining) internal view {
        uint256 used=usedThisBlock();
        if(maintaining?(used&255)>=REG_MAINT_MAX:(reactivating?((used>>8)&255)>=cfg(CP_R_REACT):((c&65535)+(used&255)>=cfg(23) || (registering && ((c>>16)&255)>=cfg(17)))))revert RegOpsLimit();
    }
    function spendBudgets(uint256 c,bool registering,bool reactivating,bool maintaining) internal {
        if(maintaining||reactivating)put(BUDGETS,(usedThisBlock()+(maintaining?1:256))|(block.number<<16));
        else put(3,c+1+(registering?(uint256(1)<<16):0));
    }
    /// V3-08: at most one maintenance operation per identity per block. Returns w0 with the clock set.
    function maintain(uint256 r,uint256 w,uint256 height) internal returns(uint256){
        if((w>>216)==(height&MAINT_MOD))revert RegOpsLimit();
        w=(w&((uint256(1)<<216)-1))|((height&MAINT_MOD)<<216);put(r,w);return w;
    }
    /// V3-09: an identity reactivates at most once per C_react blocks, counted from its previous reactivation.
    function reactClock(uint256 r,uint256 height) internal {
        uint256 x=get(r+1);uint256 last=(x>>160)&LOW64;
        if(last!=0 && height<last+cfg(CP_C_REACT))revert BadState();
        put(r+1,(x&~(LOW64<<160))|(height<<160));
    }
    function recordAt(uint256 i) external view returns(address){if(i>=get(0))revert BadState();return address(uint160(get(arr(5)+i)));}
    function activeAt(uint256 i) external view returns(address){if(i>=get(1))revert BadState();return address(uint160(get(arr(6)+i)));}

    fallback() external payable {
        if(msg.data.length<4)revert BadCalldata();bytes4 sel=msg.sig;
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
        checkBudgets(c,registering,reactivating,maintaining); // V3-08 / V3-09
        uint256 height=h();if(c&BEGIN_FLAG==0 || ((c>>24)&LOW64)!=height)revert BadPhase();
        address id=removing?arg:msg.sender;uint256 r=rec(id);uint256 w=get(r);
        if(registering){
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
        }else{
            if(!live(w))revert BadState();
            if(maintaining)w=maintain(r,w,height); // V3-08
            if(topping){
                // EARN-IN U2: fill up to B; the rest returns to the sender (durable: it sent this transaction).
                if(!accruingState(w)||(w&255)!=1||pastProbation(w,height))revert BadState();
                uint256 bond=get(r+2);uint256 add=cfg(16)-bond;if(msg.value<add)add=msg.value;
                if(!Sys.pay(Sys.ESCROW,add))revert PaymentFailed();
                if(msg.value>add && !Sys.pay(msg.sender,msg.value-add))revert PaymentFailed();
                put(r+2,bond+add);put(4,get(4)+add);promote(r,w,bond+add);
            }
            else if(rewarding){if(!Sys.pay(arg,0))revert RewardNotDurable();put(r+1,(get(r+1)&~LOW160)|uint160(arg));} // V3-09: keeps the C_react clock
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
                put(arr(6)+active,uint160(id));put(1,active+1);put(r,((w&~(ACTPOS_MASK|uint256(255)))|1|(active<<56)));
                put(r+3,(clocks&(LOW64<<128))|(height<<192));
            }else{
                if((w&255)!=2||height<((get(r+3)>>64)&LOW64))revert BadState();if(!refund(id,r))revert PaymentFailed();erase(id,r,w);
            }
        }
        spendBudgets(c,registering,reactivating,maintaining); // V3-08 / V3-09
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
        uint256 due=sumHeight(height,cfg(27));put(r,(removeActive(r,w)&~uint256(255))|2);
        put(r+3,(clocks&~((uint256(1)<<128)-1))|height|(due<<64));
    }
    function refund(address id,uint256 r) internal returns(bool){
        // EARN-IN F1-F3 SINK: only draw down held_total/ACC_COUNT once the bond PAY succeeds.
        // On an undeliverable bond the record stays live & claimable and its bond stays in Escrow
        // accounted in held_total; the caller decides whether to erase/settle (it must not).
        if(!IEscrowV1(Sys.ESCROW).releaseBond(id))return false;
        put(4,get(4)-get(r+2));
        if(accruingState(get(r)))put(ACC_COUNT,get(ACC_COUNT)-1); // EARN-IN S2
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
    /// lastShare=j once per existing distinct record; absent all-zero records skipped.
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
        address[] memory owners=ILedgerV1(Sys.LEDGER).ownersOf(j);
        for(uint256 i;i<owners.length;i++){
            uint256 r=rec(owners[i]); uint256 w=get(r);
            if(w==0){ if(get(r+1)!=0||get(r+2)!=0||get(r+3)!=0) revert BadState(); continue; }
            if(((w>>88)&LOW64)>uint256(j)) revert BadState();
            put(r,(w&~LAST_MASK)|(uint256(j)<<88));
        }
    }
    /// Evidence effect (phase 0, SYSTEM): HELD -> BURNED+tomb (active exits);
    /// PAID -> tomb only. Tomb/absent records are a host error (block invalid).
    function punish(address id) external {
        if(msg.sender!=Sys.SYSTEM||msg.data.length!=36) revert Unauthorized();
        uint256 raw; assembly{raw:=calldataload(4)} if(raw>>160!=0) revert BadCalldata();
        uint256 height=h(); uint256 r=rec(id); uint256 w=get(r);
        if(w==0 || ((w>>8)&255)!=0) revert BadState();
        uint256 st=(w>>16)&255; uint256 clocks=get(r+3);
        if(st==1||st==ACCRUING){
            if(st==ACCRUING)put(ACC_COUNT,get(ACC_COUNT)-1); // EARN-IN S4
            if((w&255)==1){ w=(removeActive(r,w)&~uint256(255))|2; clocks=(clocks&~((uint256(1)<<128)-1))|height|(height<<64); }
            w=(w&~STATE_MASK)|(uint256(2)<<16)|(uint256(1)<<8);
            uint256 bond=get(r+2); put(4,get(4)-bond); IEscrowV1(Sys.ESCROW).burnBond(bond);
        } else if(st==3){ w=w|(uint256(1)<<8); }
        else revert BadState();
        clocks=(clocks&~(LOW64<<128))|(height<<128);
        put(r,w); put(r+3,clocks);
    }
}
