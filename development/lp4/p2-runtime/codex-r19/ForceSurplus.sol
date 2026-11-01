// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
// Test-only forced-transfer witness, not system code or an authorized withdrawal.
contract ForceSurplus {
    function force(address payable recipient) external { selfdestruct(recipient); }
}
