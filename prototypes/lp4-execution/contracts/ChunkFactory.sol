// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;
// M1 storage.md D27: exact initcode, no storage, no events, idempotent CREATE2.
contract ChunkFactory {
    error BadChunk();
    function store(bytes calldata data) external returns(address chunk) {
        if(data.length==0||data.length>24575)revert BadChunk();
        bytes32 salt=keccak256(data);
        bytes memory init=abi.encodePacked(hex"61",bytes2(uint16(1+data.length)),hex"80600a5f395ff300",data);
        chunk=address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xff),address(this),salt,keccak256(init))))));
        if(chunk.code.length!=0){
            if(chunk.codehash!=keccak256(abi.encodePacked(bytes1(0),data)))revert BadChunk();
            return chunk;
        }
        address deployed;
        assembly{deployed:=create2(0,add(init,32),mload(init),salt)}
        if(deployed==address(0)||deployed!=chunk)revert BadChunk();
    }
}
