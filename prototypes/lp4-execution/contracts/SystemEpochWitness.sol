// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
// Diagnostic only: no system contract, activation or approved new runtime rule.
contract SystemEpochWitness {
    function readSlot() external view returns(uint256 value) {
        assembly { value := sload(0) }
    }
    function setTransient() external { assembly { tstore(0, 7) } }
    function readTransient() external view returns(uint256 value) {
        assembly { value := tload(0) }
    }
    function persistTransient() external {
        assembly { sstore(1, tload(0)) }
    }
    function clearSlot() external { assembly { sstore(0, 0) } }
    function restoreSlot() external { assembly { sstore(0, 11) } }
    function originAndCaller() external view returns(address, address) {
        return (tx.origin, msg.sender);
    }
}
