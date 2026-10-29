// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
import "RegistryPayment.sol";
// Real pending/bond component, not complete settlement/finalization acceptance.
// Missing finalize/settle selectors fail closed until the P6 implementation.
contract PendingEscrow is RewardEscrow {
    error BadSystemCall();error BadPending();
    function base(uint256 h) internal pure returns(uint256) {return uint256(keccak256(abi.encode(h,uint256(3))));}
    function get(uint256 k) internal view returns(uint256 v){assembly{v:=sload(k)}}
    function put(uint256 k,uint256 v) internal {assembly{sstore(k,v)}}
    function closeBlock(uint256 tips) external {
        if(msg.sender!=SystemAddress.SYSTEM||msg.data.length!=36)revert BadSystemCall();
        if(block.number==0||block.number>type(uint64).max)revert BadSystemCall();
        uint256 b=base(block.number);
        if(get(b)!=0||get(b+1)!=0||get(b+2)!=0)revert BadPending();
        uint256 amount=ICaps(SystemAddress.CAPS).get(15)+tips;
        uint256 meta=tx.origin==address(0)?1:(uint256(uint160(tx.origin))<<8);
        put(b,amount);put(b+1,meta);
        // M1 D141: do NOT write winner (word2). It remains zero until h+1.
    }
    function recordResult(uint256 h,address winner) external {
        if(msg.sender!=SystemAddress.SYSTEM||msg.data.length!=68||winner==address(0)||block.number<2||h!=block.number-1)revert BadSystemCall();
        uint256 b=base(h);uint256 meta=get(b+1);
        if(meta==0||meta>>168!=0||(meta&255)>1||((meta&255)==1&&meta!=1)||get(b+2)!=0)revert BadPending();
        put(b+2,uint256(uint160(winner)));
    }
}
