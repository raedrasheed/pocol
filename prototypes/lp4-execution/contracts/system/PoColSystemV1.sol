// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

// PoCol system programs v1 (local devnet). Authorities:
//  - M1 execution.md / incentives.md (phases, pending, settle formulas, bond machine)
//  - LP4-PAY-01/02, LP4-REGISTRY-01/02, LP4-SYS-01, LP4-ISSUANCE-01
//  - LP4-LEDGER-01 option 1 (development/LP4-LEDGER-APPROVAL.md)
// Addresses: C0C001 MinerRegistry, C0C002 RewardEscrow, C0C003 ShareLedger,
// C0C004 ChainCaps, C0C005 ChunkFactory (separate source, unchanged D27 code).
// None of these programs emits a LOG.

interface ICaps { function get(uint256 n) external view returns(uint256); }
interface IRegistryV1 {
    function bondRecord(address id) external view returns(uint256,address,uint256);
    function payInfos(address[] calldata ids) external view returns(address[] memory rewards);
}
interface IEscrowV1 { function releaseBond(address id) external; function burnBond(uint256 amount) external; }
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

/// Complete CP (52 fields, schema order) at slots 0..51, written only by genesis.
contract ChainCaps {
    function get(uint256 n) external view returns(uint256 v) {
        require(n<52); assembly { v:=sload(n) }
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

    function releaseBond(address id) external {
        if(msg.sender!=Sys.REGISTRY) revert Unauthorized();
        if(msg.data.length!=36) revert BadCalldata();
        (uint256 w0,address reward,uint256 bond)=IRegistryV1(Sys.REGISTRY).bondRecord(id);
        if(((w0>>16)&255)!=1 || ((w0>>8)&255)!=0 || (w0&255)==0) revert BondUnavailable();
        if(!Sys.pay(reward,bond)) revert PaymentFailed();
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
        address[] memory rewards=IRegistryV1(Sys.REGISTRY).payInfos(list);
        uint256 sum;
        for(uint256 i;i<list.length;i++){
            uint256 p=r*counts[i]/q; sum+=p;
            if(p==0) continue;
            if(rewards[i]==address(0) || !Sys.pay(rewards[i],p)) burned+=p;
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
        if(!Sys.pay(reward,amount)) return amount;
        return 0;
    }
}

/// MinerRegistry v1: LP4 Registry user paths/sweep/closing cursor (unchanged
/// layout, REGISTRY-LAYOUT.md) plus SYSTEM recordShares (LEDGER-01) and punish.
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
    function get(uint256 k) internal view returns(uint256 v){assembly{v:=sload(k)}}
    function put(uint256 k,uint256 v) internal {assembly{sstore(k,v)}}
    function cfg(uint256 n) internal view returns(uint256){return ICaps(Sys.CAPS).get(n);}
    function rec(address id) internal pure returns(uint256){return uint256(keccak256(abi.encode(id,uint256(7))));}
    function arr(uint256 slot) internal pure returns(uint256){return uint256(keccak256(abi.encode(slot)));}
    function h() internal view returns(uint256){if(block.number>LOW64)revert HeightOverflow();return block.number;}
    function sumHeight(uint256 a,uint256 b) internal pure returns(uint256){uint256 v=a+b;if(v>LOW64)revert HeightOverflow();return v;}
    function held(uint256 w) internal pure returns(bool){return (w&255)!=0 && ((w>>8)&255)==0 && ((w>>16)&255)==1;}
    function system() internal view {if(msg.sender!=Sys.SYSTEM)revert Unauthorized();if(msg.data.length!=4)revert BadCalldata();}
    function bondRecord(address id) external view returns(uint256,address,uint256){uint256 r=rec(id);return(get(r),address(uint160(get(r+1))),get(r+2));}
    /// Settlement payees: reward address of each live record; zero for an absent
    /// record (payBurn) or a tomb (penaltyBurn). Two SLOADs per id.
    function payInfos(address[] calldata ids) external view returns(address[] memory rewards){
        rewards=new address[](ids.length);
        for(uint256 i;i<ids.length;i++){uint256 r=rec(ids[i]);uint256 w=get(r);if(w!=0&&((w>>8)&255)==0)rewards[i]=address(uint160(get(r+1)));}
    }
    function recordWords(address id) external view returns(uint256,uint256,uint256,uint256){uint256 r=rec(id);return(get(r),get(r+1),get(r+2),get(r+3));}
    function lengths() external view returns(uint256,uint256,uint256,uint256){return(get(0),get(1),get(2),get(4));}
    function counters() external view returns(uint256){return get(3);}
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
        if(!(registering||reactivating||rewarding||removing||exiting||claiming))revert BadCalldata();
        address arg;
        if(registering||rewarding||removing){
            if(msg.data.length!=36)revert BadCalldata();uint256 raw;assembly{raw:=calldataload(4)}
            if(raw>>160!=0)revert BadCalldata();arg=address(uint160(raw));
        }else if(msg.data.length!=4)revert BadCalldata();
        if(msg.value!=(registering?cfg(16):0))revert BadValue();
        uint256 c=get(3);if(c&CLOSING!=0)revert Closing();
        if((c&65535)>=cfg(23) || ((registering||reactivating) && ((c>>16)&255)>=cfg(17)))revert RegOpsLimit();
        uint256 height=h();if(c&BEGIN_FLAG==0 || ((c>>24)&LOW64)!=height)revert BadPhase();
        address id=removing?arg:msg.sender;uint256 r=rec(id);uint256 w=get(r);
        if(registering){
            uint160 a=uint160(id);if(id==address(0)||id==Sys.SYSTEM||(a>=0xC0C001&&a<=0xC0C0FF)||w!=0)revert BadState();
            uint256 n=get(0);uint256 active=get(1);if(n>=cfg(20)||active>=cfg(18))revert Capacity();
            if(!Sys.pay(arg,0))revert RewardNotDurable();
            if(!Sys.pay(Sys.ESCROW,msg.value))revert PaymentFailed();
            put(r,1|(uint256(1)<<16)|(n<<24)|(active<<56));put(r+1,uint160(arg));put(r+2,msg.value);put(r+3,height<<192);
            put(arr(5)+n,uint160(id));put(arr(6)+active,uint160(id));put(0,n+1);put(1,active+1);put(4,get(4)+msg.value);
        }else{
            if(!held(w))revert BadState();
            if(rewarding){if(!Sys.pay(arg,0))revert RewardNotDurable();put(r+1,uint160(arg));}
            else if(exiting||removing){
                if((w&255)!=1||get(1)<=cfg(19))revert BadState();uint256 clocks=get(r+3);
                if(removing){uint256 last=(w>>88)&LOW64;uint256 grace=clocks>>192;if(height<sumHeight(last>grace?last:grace,cfg(25)))revert BadState();}
                uint256 due=sumHeight(height,cfg(27));put(r,(removeActive(r,w)&~uint256(255))|2);
                put(r+3,(clocks&~((uint256(1)<<128)-1))|height|(due<<64));
            }else if(reactivating){
                uint256 clocks=get(r+3);if((w&255)!=2||height<sumHeight(clocks&LOW64,cfg(24)))revert BadState();uint256 active=get(1);if(active>=cfg(18))revert Capacity();
                put(arr(6)+active,uint160(id));put(1,active+1);put(r,((w&~(ACTPOS_MASK|uint256(255)))|1|(active<<56)));
                put(r+3,(clocks&(LOW64<<128))|(height<<192));
            }else{
                if((w&255)!=2||height<((get(r+3)>>64)&LOW64))revert BadState();refund(id,r);erase(id,r,w);
            }
        }
        put(3,c+1+((registering||reactivating)?(uint256(1)<<16):0));
    }
    function removeActive(uint256,uint256 w) internal returns(uint256) {
        uint256 n=get(1);uint256 pos=(w>>56)&POSMASK;uint256 b=arr(6);if(pos>=n)revert BadState();
        if(pos!=n-1){address moved=address(uint160(get(b+n-1)));put(b+pos,uint160(moved));uint256 m=rec(moved);put(m,(get(m)&~ACTPOS_MASK)|(pos<<56));}
        put(b+n-1,0);put(1,n-1);return w&~ACTPOS_MASK;
    }
    function refund(address id,uint256 r) internal {IEscrowV1(Sys.ESCROW).releaseBond(id);put(4,get(4)-get(r+2));}
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
            if(cur>=n)cur=0;address id=address(uint160(get(arr(5)+cur)));uint256 r=rec(id);uint256 w=get(r);uint256 clocks=get(r+3);bool deletion;
            if(held(w)&&(w&255)==2&&height>=((clocks>>64)&LOW64)){refund(id,r);deletion=true;}
            else if(((w>>8)&255)!=0&&((w>>16)&255)!=1&&height>=sumHeight((clocks>>128)&LOW64,cfg(26)+cfg(11)+1)){deletion=true;}
            if(deletion){erase(id,r,w);removed++;n--;}else{cur++;}
        }
        put(2,n==0?0:cur%n);
    }
    function markClose() external {
        system();uint256 height=h();uint256 c=get(3);if(height==0||((c>>123)&LOW64)>=height)revert BadPhase();
        c=(c&~MARK_MASK)|(height<<123);
        if(c&CLOSING==0 && height<cfg(47) && block.timestamp<cfg(48)){put(3,c);return;}
        if(c&CLOSE_INIT==0)c=(c&~CURSOR_MASK)|CLOSING|CLOSE_INIT|(height<<187);
        uint256 cursor=(c>>90)&POSMASK;uint256 n=get(0);uint256 end=cursor+cfg(50);if(end>n)end=n;
        for(;cursor<end;cursor++){
            address id=address(uint160(get(arr(5)+cursor)));uint256 r=rec(id);uint256 w=get(r);
            if(held(w)){refund(id,r);put(r,(w&~STATE_MASK)|(uint256(3)<<16));}
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
        if(st==1){
            if((w&255)==1){ w=(removeActive(r,w)&~uint256(255))|2; clocks=(clocks&~((uint256(1)<<128)-1))|height|(height<<64); }
            w=(w&~STATE_MASK)|(uint256(2)<<16)|(uint256(1)<<8);
            uint256 bond=get(r+2); put(4,get(4)-bond); IEscrowV1(Sys.ESCROW).burnBond(bond);
        } else if(st==3){ w=w|(uint256(1)<<8); }
        else revert BadState();
        clocks=(clocks&~(LOW64<<128))|(height<<128);
        put(r,w); put(r+3,clocks);
    }
}
