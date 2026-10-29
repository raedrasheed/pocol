// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
// Diagnostic only: two UNAPPROVED physical retention policies for one owner.
// Not ShareLedger, not installed in any profile or production phase schedule.
contract LedgerRetentionCountermodel {
    mapping(uint64=>uint256) public count;
    function prune(bool retainExtra) external {
        require(msg.sender==address(uint160(type(uint160).max-1)));
        // Sample h=5, D=2,W_w=3. Both preserve settlement j=3 and next weights2..4.
        uint256 lag=retainExtra?5:4;
        if(block.number<=lag)return;
        delete count[uint64(block.number-lag)];
    }
}
