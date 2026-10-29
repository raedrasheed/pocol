// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
// Compiled execution controls only; not one of the five protocol programs.
contract SystemBoundaryWitness {
    function leaf(uint256 required, bool abort) external returns(uint256 initial) {
        initial=gasleft();require(initial>=required);
        assembly { sstore(0, 9) tstore(0, 7) }
        require(!abort);
    }
    function forward(uint256 requested,uint256 required,bool abortChild,bool abortOuter)
        external returns(bool ok,uint256 initial,uint256 child,uint256 stored,uint256 transientValue)
    {
        initial=gasleft();bytes memory result;
        (ok,result)=address(this).call{gas:requested}(abi.encodeCall(this.leaf,(required,abortChild)));
        if(ok)child=abi.decode(result,(uint256));
        assembly { stored:=sload(0) transientValue:=tload(0) }
        require(!abortOuter);
    }
    function captureCoinbaseBalance() external returns(uint256 amount) {
        amount=block.coinbase.balance;assembly { sstore(1, amount) }
    }
}
