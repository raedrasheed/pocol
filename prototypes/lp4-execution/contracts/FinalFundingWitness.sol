// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
import "PendingEscrow.sol";
// Synthetic F-routing/balance observer ONLY; does not settle remaining pots.
// It must NEVER be counted as real protocol finalization/SV6 acceptance.
contract FinalFundingWitness is PendingEscrow {
    function finalize(uint256 tips) external {
        if(msg.sender!=SystemAddress.SYSTEM||msg.data.length!=36)revert BadSystemCall();
        uint256 amount=address(this).balance;
        assembly { sstore(11,amount) sstore(12,tips) }
    }
}
