#!/usr/bin/env python3
"""Network identity fixtures for tests/v3_decoder_test.mjs, made by deploy/mainnet/set_network_identity.py itself, so
the packager checks hold for the tree as it is before the identity switch (the placeholder, decision D1) and after it:

    python3 -B tests/identity_fixture.py placeholder ROOT FILE   # FILE (a built lib/studio-network.js) rewritten in place
                                                                 # as --placeholder writes the committed placeholder into it
    python3 -B tests/identity_fixture.py switch ROOT OUT         # OUT: a copy of ROOT's identity files, switched from the
                                                                 # tree's network to a spec-2 test network; prints its JSON

ROOT is a repository tree (the repository, or an OUT of `switch`): its node file (ANCHOR) names the tree's network and
its retired list the retired ones. The placeholder invitation is always the repository's committed
deploy/mainnet/placeholder-network.pocol.json. Nothing outside the temporary directories and OUT is written.
"""
import contextlib, json, shutil, sys, tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(REPO / 'deploy' / 'mainnet'))
import set_network_identity as ni  # noqa: E402

PLACEHOLDER = REPO / 'deploy' / 'mainnet' / 'placeholder-network.pocol.json'
NETWORK_FILE = 'prototypes/pocol-extension/src/lib/studio-network.js'
# Chain IDs of no network for the simulated switch: the first that is not the tree's own, the placeholder's or a retired
# one is used.
TEST_CHAINS = (424_247, 424_251)

def run(argv):
    """set_network_identity.main(argv) with its report on stderr; SystemExit on a refusal."""
    with contextlib.redirect_stdout(sys.stderr):
        rc = ni.main(argv)
    if rc:
        raise SystemExit(f'set_network_identity.py {" ".join(argv)}: exit {rc}')

def copy(root, out, rels):
    for rel in rels:
        (out / rel).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / rel, out / rel)

def placeholder(root, file):
    with tempfile.TemporaryDirectory(prefix='pocol-ph-') as tmp:
        tmp = Path(tmp)
        copy(root, tmp, [ni.ANCHOR, ni.RETIRED])
        (tmp / NETWORK_FILE).parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(file, tmp / NETWORK_FILE)
        run(['--placeholder', str(PLACEHOLDER), '--root', str(tmp)])
        shutil.copy2(tmp / NETWORK_FILE, file)

def with_chain(spec, chain):
    """The GenesisSpec with its chainId item replaced."""
    _, s, e = ni.rlp_item(spec, 0)
    _, _, version_end = ni.rlp_item(spec, s)
    _, _, chain_end = ni.rlp_item(spec, version_end)
    raw = chain.to_bytes((chain.bit_length() + 7) // 8, 'big')
    payload = spec[s:version_end] + bytes([0x80 + len(raw)]) + raw + spec[chain_end:e]
    n = len(payload)
    size = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return (bytes([0xc0 + n]) if n <= 55 else bytes([0xf7 + len(size)]) + size) + payload

def invitation(spec):
    return json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': [],
                       'files': {'genesis.spec.hex': '0x' + spec.hex()}})

def switch(root, out):
    out.mkdir(parents=True, exist_ok=True)
    rels = set(ni.FILES) | {ni.ANCHOR, ni.NODE_SPEC, ni.RETIRED, ni.INSTALLER} | set(ni.INSTALLER_SOURCES.values()) \
        | {page for page, _ in ni.VERSIONED} | {rel for _, rel in ni.VERSIONED}
    copy(root, out, sorted(rels))
    cur_chain, _, cur_spec = ni.current_identity(out)
    corpus = json.loads((REPO / 'prototypes/pocol-extension/tests/data/decoder-corpus-v3.json').read_text(encoding='utf-8'))
    base = bytes.fromhex(next(i['hex'] for i in corpus['items'] if i['label'] == 'v3/base'))
    ph_chain, _, _, _ = ni.invitation_identity(PLACEHOLDER, (2,))
    retired = {c for c, _ in ni.read_retired((out / ni.RETIRED).read_bytes())}
    chain = next((c for c in TEST_CHAINS if c not in retired | {cur_chain, ph_chain}), None)
    if chain is None:
        raise SystemExit('no test chain ID for the simulated switch: each is the tree\'s, the placeholder\'s or retired')
    spec = with_chain(base, chain)
    genesis = ni.keccak256(spec).hex()
    with tempfile.TemporaryDirectory(prefix='pocol-inv-') as tmp:
        src, dst = Path(tmp) / 'from.json', Path(tmp) / 'to.json'
        src.write_text(invitation(cur_spec), encoding='utf-8')
        dst.write_text(invitation(spec), encoding='utf-8')
        run(['--from', str(src), '--to', str(dst), '--root', str(out)])
        run(['--from', str(src), '--to', str(dst), '--check', '--root', str(out)])
    print(json.dumps({'chainId': chain, 'genesisHash': '0x' + genesis, 'genesisPre': '0x' + spec.hex(),
                      'networkFile': NETWORK_FILE}))

if __name__ == '__main__':
    args = sys.argv[1:]
    if len(args) == 3 and args[0] == 'placeholder':
        placeholder(Path(args[1]), Path(args[2]))
    elif len(args) == 3 and args[0] == 'switch':
        switch(Path(args[1]), Path(args[2]))
    else:
        raise SystemExit(__doc__)
