// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.8.28;

// M1 storage.md "عقد Website": versioned site manifests stored as ChunkFactory
// chunks (D27). Slots: 0 owner, 1 pendingOwner, 2 versionCount|currentVersion<<128
// (currentVersion = index+1, 0 = none), 3 versions mapping, 4 publishers mapping,
// 5 publisherCount. Version status: 0 draft, 1 published, 2 revoked.
// Limits: manifest <= 64 KiB, versions <= 1024, publishers <= 16.
contract Website {
    error NotOwner(); error NotPublisher(); error BadVersion(); error BadState(); error Limit(); error BadChunk(); error NotPending();

    struct Version { bytes32 manifestHash; uint256 meta; address[] chunks; } // meta: manifestLen u32 | status u8 <<32 | publishedBlock u64 <<40

    address internal owner;          // slot 0
    address internal pendingOwner;   // slot 1
    uint256 internal counts;         // slot 2
    mapping(uint256=>Version) internal versions; // slot 3
    mapping(address=>bool) internal publishers;  // slot 4
    uint256 internal publisherCount; // slot 5

    address constant FACTORY=address(0xC0C005);
    uint256 constant MAX_MANIFEST=65536; uint256 constant MAX_VERSIONS=1024; uint256 constant MAX_PUBLISHERS=16;

    constructor(){ owner=msg.sender; publishers[msg.sender]=true; publisherCount=1; }

    modifier onlyOwner(){ if(msg.sender!=owner) revert NotOwner(); _; }
    modifier onlyPublisher(){ if(!publishers[msg.sender]) revert NotPublisher(); _; }

    function versionCount() public view returns(uint256){ return counts&type(uint128).max; }
    function currentVersion() public view returns(uint256){ return counts>>128; }

    /// Draft version from manifest chunks (each a ChunkFactory runtime: code[0]==0x00).
    function createVersion(bytes32 manifestHash,uint32 manifestLen,address[] calldata chunks) external onlyPublisher returns(uint256 v){
        if(manifestLen==0||manifestLen>MAX_MANIFEST||chunks.length==0||chunks.length>3) revert Limit();
        uint256 total;
        for(uint256 i;i<chunks.length;i++){
            address c=chunks[i]; uint256 len=c.code.length;
            if(len<2) revert BadChunk();
            bytes memory first=new bytes(1); assembly{ extcodecopy(c,add(first,32),0,1) }
            if(first[0]!=0x00) revert BadChunk();
            total+=len-1;
        }
        if(total!=manifestLen) revert BadChunk();
        v=versionCount(); if(v>=MAX_VERSIONS) revert Limit();
        Version storage x=versions[v]; x.manifestHash=manifestHash; x.meta=uint256(manifestLen); x.chunks=chunks;
        counts=counts+1;
    }
    function publish(uint256 v) external onlyPublisher {
        if(v>=versionCount()) revert BadVersion();
        Version storage x=versions[v]; if((x.meta>>32)&255!=0) revert BadState();
        x.meta=(x.meta&uint256(type(uint32).max))|(uint256(1)<<32)|(block.number<<40);
    }
    function setCurrent(uint256 v) external onlyPublisher {
        if(v>=versionCount()) revert BadVersion();
        if((versions[v].meta>>32)&255!=1) revert BadState();
        counts=(counts&type(uint128).max)|((v+1)<<128);
    }
    function revoke(uint256 v) external onlyOwner {
        if(v>=versionCount()) revert BadVersion();
        if(currentVersion()==v+1) revert BadState();
        Version storage x=versions[v]; if((x.meta>>32)&255==2) revert BadState();
        x.meta=(x.meta&~(uint256(255)<<32))|(uint256(2)<<32);
    }
    function addPublisher(address p) external onlyOwner { if(publishers[p]) revert BadState(); if(publisherCount>=MAX_PUBLISHERS) revert Limit(); publishers[p]=true; publisherCount+=1; }
    function removePublisher(address p) external onlyOwner { if(!publishers[p]) revert BadState(); publishers[p]=false; publisherCount-=1; }
    function transferOwnership(address p) external onlyOwner { pendingOwner=p; }
    function acceptOwnership() external { if(msg.sender!=pendingOwner) revert NotPending(); owner=msg.sender; pendingOwner=address(0); }

    function ownerOf() external view returns(address,address){ return (owner,pendingOwner); }
    function isPublisher(address p) external view returns(bool){ return publishers[p]; }
    function version(uint256 v) external view returns(bytes32 manifestHash,uint32 manifestLen,uint8 status,uint64 publishedBlock,address[] memory chunks){
        if(v>=versionCount()) revert BadVersion();
        Version storage x=versions[v]; uint256 m=x.meta;
        return (x.manifestHash,uint32(m),uint8(m>>32),uint64(m>>40),x.chunks);
    }
    /// The current published version (reverts when none is current).
    function current() external view returns(uint256 v,bytes32 manifestHash,uint32 manifestLen,uint64 publishedBlock,address[] memory chunks){
        uint256 c=currentVersion(); if(c==0) revert BadVersion(); v=c-1;
        Version storage x=versions[v]; uint256 m=x.meta;
        return (v,x.manifestHash,uint32(m),uint64(m>>40),x.chunks);
    }
}
