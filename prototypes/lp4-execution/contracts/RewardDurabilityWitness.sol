// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
// Test-only witness, not a PoCol Registry implementation or a new payment rule.
contract EphemeralReward {
    function erase() external { selfdestruct(payable(msg.sender)); }
}
contract RewardDurabilityWitness {
    function probeThenErase() external returns(bool accepted,address reward) {
        EphemeralReward child = new EphemeralReward();
        reward = address(child);
        (accepted,) = address(0xC0C0ff).call{gas:5000}(abi.encode(reward,uint256(0)));
        child.erase();
    }
}
