// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

// Canonical user ABI for the strict fallback dispatcher; payable lets the
// implementation return the mandated BadValue custom error itself.
interface IRegistryUser {
    function register(address reward) external payable;
    function deregister() external payable;
    function removeInactive(address id) external payable;
    function reactivate() external payable;
    function setRewardAddress(address reward) external payable;
    function claimBond() external payable;
}
interface ICaps { function get(uint256 n) external view returns(uint256); }
interface IRegistry { function bondRecord(address id) external view returns(uint256,address,uint256); }
interface IEscrow { function releaseBond(address id) external; }

/// Source-built CP reader. Genesis writes the complete validated CP before use.
/// There is no runtime setter or user-supplied authorization/resource flag.
contract ChainCaps {
    function get(uint256 n) external view returns(uint256 v) {
        require(n<52); assembly { v:=sload(n) }
    }
}

library SystemAddress {
    address internal constant REGISTRY=address(0xC0C001);
    address internal constant ESCROW=address(0xC0C002);
    address internal constant CAPS=address(0xC0C004);
    address internal constant PAY=address(0xC0C0FF);
    address internal constant SYSTEM=address(uint160(type(uint160).max-1));
    function pay(address recipient,uint256 amount) internal returns(bool ok) {
        bytes memory data=abi.encode(recipient,amount);
        (ok,)=PAY.call{gas:5000}(data);
    }
}

/// Bond bridge only. Settlement/pending/finalization remains a separate mandatory
/// implementation obligation; this contract does not claim those gates passed.
contract RewardEscrow {
    error Unauthorized(); error BondUnavailable(); error PaymentFailed(); error BadCalldata();
    function releaseBond(address id) external {
        if(msg.sender!=SystemAddress.REGISTRY) revert Unauthorized();
        if(msg.data.length!=36) revert BadCalldata();
        (uint256 w0,address reward,uint256 bond)=IRegistry(SystemAddress.REGISTRY).bondRecord(id);
        if(((w0>>16)&255)!=1 || ((w0>>8)&255)!=0 || (w0&255)==0) revert BondUnavailable();
        if(!SystemAddress.pay(reward,bond)) revert PaymentFailed();
    }
}

/// LP4-PAY-01/02, LP4-REGISTRY-01/02; see separate approval records.
/// Explicit canonical layout documented in REGISTRY-LAYOUT.md. No compiler state
/// variables, hidden admin state, payable receive or runtime config setter.
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
    function get(uint256 k) internal view returns(uint256 v){assembly{v:=sload(k)}}
    function put(uint256 k,uint256 v) internal {assembly{sstore(k,v)}}
    function cfg(uint256 n) internal view returns(uint256){return ICaps(SystemAddress.CAPS).get(n);}
    function rec(address id) internal pure returns(uint256){return uint256(keccak256(abi.encode(id,uint256(7))));}
    function arr(uint256 slot) internal pure returns(uint256){return uint256(keccak256(abi.encode(slot)));}
    function h() internal view returns(uint256){if(block.number>LOW64)revert HeightOverflow();return block.number;}
    function sumHeight(uint256 a,uint256 b) internal pure returns(uint256){uint256 v=a+b;if(v>LOW64)revert HeightOverflow();return v;}
    function held(uint256 w) internal pure returns(bool){return (w&255)!=0 && ((w>>8)&255)==0 && ((w>>16)&255)==1;}
    function system() internal view {if(msg.sender!=SystemAddress.SYSTEM)revert Unauthorized();if(msg.data.length!=4)revert BadCalldata();}
    function bondRecord(address id) external view returns(uint256,address,uint256){uint256 r=rec(id);return(get(r),address(uint160(get(r+1))),get(r+2));}
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
            uint160 a=uint160(id);if(id==address(0)||id==SystemAddress.SYSTEM||(a>=0xC0C001&&a<=0xC0C0FF)||w!=0)revert BadState();
            uint256 n=get(0);uint256 active=get(1);if(n>=cfg(20)||active>=cfg(18))revert Capacity();
            if(!SystemAddress.pay(arg,0))revert RewardNotDurable();
            if(!SystemAddress.pay(SystemAddress.ESCROW,msg.value))revert PaymentFailed();
            put(r,1|(uint256(1)<<16)|(n<<24)|(active<<56));put(r+1,uint160(arg));put(r+2,msg.value);put(r+3,height<<192);
            put(arr(5)+n,uint160(id));put(arr(6)+active,uint160(id));put(0,n+1);put(1,active+1);put(4,get(4)+msg.value);
        }else{
            if(!held(w))revert BadState();
            if(rewarding){if(!SystemAddress.pay(arg,0))revert RewardNotDurable();put(r+1,uint160(arg));}
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
    function refund(address id,uint256 r) internal {IEscrow(SystemAddress.ESCROW).releaseBond(id);put(4,get(4)-get(r+2));}
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
}
