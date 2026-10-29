// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

// Synthetic compiled LP4-CODE-01 witness, never a system contract or deployment.
contract CodeCapsWitness {
    function make(bytes memory init, uint256 salt, bool use2, bool undo)
        external returns (address child)
    {
        assembly {
            switch use2
            case 0 { child := create(0, add(init, 32), mload(init)) }
            default { child := create2(0, add(init, 32), mload(init), salt) }
        }
        if (undo) revert();
    }

    function twice(bytes memory init) external returns (address a, address b) {
        assembly {
            a := create(0, add(init, 32), mload(init))
            b := create(0, add(init, 32), mload(init))
        }
    }

    function destroyThenMake(address old, bytes memory init)
        external returns (address child)
    {
        (bool ok,) = old.call("");
        require(ok);
        assembly { child := create(0, add(init, 32), mload(init)) }
    }

    function makeDestroyMake(bytes memory first, bytes memory second)
        external returns (address a, address b)
    {
        assembly { a := create(0, add(first, 32), mload(first)) }
        (bool ok,) = a.call("");
        require(ok);
        assembly { b := create(0, add(second, 32), mload(second)) }
    }
}
