// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

/// Only the mandatory storage write sets. NOT the full Registry or accepted
/// execution: no admission, PAY, phase, timing or eligibility implementation.
/// Distinct changed slots are a lower bound even before those additional paths.
contract RegistryWriteBudgetWitness {
    function rec(address id) internal pure returns (uint256) {
        return uint256(keccak256(abi.encode(id, uint256(7))));
    }
    function base(uint256 slot) internal pure returns (uint256) {
        return uint256(keccak256(abi.encode(slot)));
    }
    function registerWrites(address reward) external payable {
        uint256 r=rec(msg.sender);uint256 a=base(5);uint256 b=base(6);
        uint256 height=block.number;
        // Positive B_reg, durable reward and h>0 make all four words nonzero.
        // Append without overwriting the existing active member.
        assembly {
            sstore(r, or(0x101,or(shl(16,sload(0)),shl(48,sload(1))))) // witness packing
            sstore(add(r,1), reward)
            sstore(add(r,2), callvalue())
            sstore(add(r,3), shl(192,height)) // graceStart=h
            sstore(add(a,sload(0)), caller())
            sstore(add(b,sload(1)), caller())
            sstore(0, add(sload(0),1)) // RecLen
            sstore(1, add(sload(1),1)) // ActLen
            sstore(4, add(sload(4),callvalue())) // heldTotal
            sstore(3, add(sload(3),1)) // approved successful-user-call budget
        }
    }
    function rewardWrites(address reward) external {
        uint256 r=rec(msg.sender);
        assembly {
            sstore(add(r,1),reward)
            sstore(3,add(sload(3),1))
        }
    }
    function middleClaimWrites() external {
        uint256 r=rec(msg.sender);uint256 a=base(5);
        address moved;uint256 tail;assembly {tail:=add(a,sub(sload(0),1)) moved:=sload(tail)}
        uint256 m=rec(moved);
        // Fixture has claimant at0, an unaffected active member at1, and moved
        // exited record at2; all bonds positive. M_min=1 remains satisfied.
        assembly {
            sstore(a,moved)
            sstore(tail,0)
            sstore(m,0x201) // exited/HELD, recPos now0 (witness packing)
            sstore(0,sub(sload(0),1))
            sstore(4,sub(sload(4),sload(add(r,2))))
            sstore(r,0)
            sstore(add(r,1),0)
            sstore(add(r,2),0)
            sstore(add(r,3),0)
            sstore(3,add(sload(3),1))
        }
    }
}
