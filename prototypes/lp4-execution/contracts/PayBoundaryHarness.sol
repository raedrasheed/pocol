// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

// Execution test harnesses only: NOT accepted PoCol system contract implementations.
// Native contract: development/LP4-PAY-APPROVAL.md, LP4-PAY-01.
contract PayBoundaryHarness {
    address constant PAY = address(0xC0C0ff);
    function sendRaw(bytes calldata data, uint256 gasAmount, uint256 value, uint8 mode)
        external returns (bool ok, uint256 spent, bytes memory result)
    {
        uint256 beforeGas = gasleft();
        if (mode == 0) (ok, result) = PAY.call{gas: gasAmount, value: value}(data);
        else if (mode == 1) (ok, result) = PAY.staticcall{gas: gasAmount}(data);
        else if (mode == 2) (ok, result) = PAY.delegatecall{gas: gasAmount}(data);
        else {
            address pay = PAY;
            bytes memory input = data;
            assembly { ok := callcode(gasAmount, pay, value, add(input, 32), mload(input), 0, 0) }
        }
        spent = beforeGas - gasleft();
    }
    function twoCalls(bytes calldata data,uint256 gasAmount) external returns(bool first,bool second) {
        (first,) = PAY.call{gas:gasAmount}(data);
        (second,) = PAY.call{gas:gasAmount}(data);
    }
    function staticVia(address target, bytes calldata data) external returns(bool ok, bytes memory result) {
        (ok,result) = target.staticcall(data);
    }
    function sendThenRevert(address recipient, uint256 amount) external {
        (bool ok,) = PAY.call(abi.encode(recipient, amount));
        require(ok, "PAY failed");
        revert("outer rollback");
    }
    function delegateForeign(address foreign, bytes calldata data) external returns(bool ok) {
        (ok,) = foreign.delegatecall(data);
    }
}
contract RecipientTrap {
    fallback() external payable { revert("callback forbidden"); }
}
