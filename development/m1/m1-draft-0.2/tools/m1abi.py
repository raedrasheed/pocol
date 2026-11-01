"""Proposed ABI identifiers and storage-slot keys for M1 draft 0.2.

Every value here is computed with Keccak-256 from a PROPOSED signature or
from the PROPOSED layout P13. None of it is compiler output; compiler ABI
and storageLayout must be compared against these values when an
implementation exists (experiment E-04).
"""

from keccak import keccak256

FACTORY_FUNCTIONS = ['store(bytes)', 'predict(bytes)']
FACTORY_ERRORS = ['ChunkLength(uint256)', 'ChunkMismatch(address)', 'ChunkCreateFailed()']

WEBSITE_FUNCTIONS = [
    'createVersion(bytes32,uint32,address[],uint32[])', 'publish(uint32)', 'setCurrent(uint32)',
    'revoke(uint32)', 'setPublisher(address,bool)', 'transferOwnership(address)',
    'acceptOwnership()', 'owner()', 'pendingOwner()', 'versionCount()', 'currentVersion()',
    'getVersion(uint32)', 'isPublisher(address)', 'publisherCount()',
]
WEBSITE_ERRORS = [
    'NotOwner(address)', 'NotPublisher(address)', 'NotPendingOwner(address)', 'ZeroAddress()',
    'InvalidCandidate(address)', 'UnknownVersion(uint32)', 'WrongStatus(uint32,uint8)',
    'CurrentVersion(uint32)', 'AlreadyCurrent(uint32)', 'VersionLimit()', 'PublisherLimit()',
    'ManifestLength(uint32)', 'ChunkArrays(uint256,uint256)', 'ManifestSplit(uint256)',
    # Conditional on U07 (on-chain manifest verification):
    'ManifestChunkInvalid(uint256)', 'ManifestHashMismatch()',
]
WEBSITE_EVENTS = [
    'VersionCreated(uint32,address,bytes32,uint32)', 'VersionPublished(uint32,uint64)',
    'CurrentVersionChanged(uint32,uint32)', 'VersionRevoked(uint32)',
    'PublisherChanged(address,bool)', 'OwnershipNominated(address,address)',
    'OwnershipTransferred(address,address)',
]


def selector(sig):
    return '0x' + keccak256(sig.encode())[:4].hex()


def topic(sig):
    return '0x' + keccak256(sig.encode()).hex()


def be256(n):
    return n.to_bytes(32, 'big')


def pad_addr(a):
    return bytes(12) + a


# --- P13 layout -----------------------------------------------------------
SLOT_OWNER, SLOT_PENDING, SLOT_COUNTS, SLOT_VERSIONS, SLOT_PUBLISHERS, SLOT_PUBCOUNT = range(6)


def version_base(vid):
    return int.from_bytes(keccak256(be256(vid) + be256(SLOT_VERSIONS)), 'big')


def version_slots(vid, nchunks):
    base = version_base(vid)
    data = int.from_bytes(keccak256(be256(base + 2)), 'big')
    return {
        'manifestHash': base,
        'packed(manifestLen|status|publishedBlock)': base + 1,
        'chunks.length': base + 2,
        'chunks[i]': [data + i for i in range(nchunks)],
    }


def publisher_slot(addr):
    return int.from_bytes(keccak256(pad_addr(addr) + be256(SLOT_PUBLISHERS)), 'big')


def pack_counts(version_count, current):
    """Slot 2: versionCount at byte offset 0, currentVersion at offset 4 (low-order first)."""
    return version_count | (current << 32)


def pack_version_word(manifest_len, status, published_block):
    return manifest_len | (status << 32) | (published_block << 40)


def pack_chunk(addr, ln):
    return int.from_bytes(addr, 'big') | (ln << 160)


def revert_data(sig, *words):
    return selector(sig) + ''.join(be256(w).hex() for w in words)


def h32(n):
    return '0x' + be256(n).hex()
