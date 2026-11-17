#!/usr/bin/env python3
"""deploy/websites/tools/site_identity.py: the snapshots carry exactly the declared identity at their anchors; a switch
to a test identity and back is byte-identical; a moved, duplicated or stray anchor value fails and writes nothing;
anchored edits rename the hashed bundle and regenerate SHA256SUMS.txt, and an edit that breaks an anchor or adds an
identity copy writes nothing; a switch that would leave an old gateway host named, and a loopback RPC URL in the
repository's own snapshots, are refused; --check refuses chatgpt.site origins, stale
download links, extension version labels that are not package.json's (capability messages "X or later" exempt),
native package names that are not assemble_release.py's (the linked retired names exempt while linked), missing
index.html assets, misnamed bundles, stale checksums and content versions, retired identities
(one on the declared chain ID, a postponed launch, by its genesis hash only), any config.php (which is never read) and
server code or a hidden file in the explorer snapshot (Sites W8: the gateway host serves it as static files); no
config.php is tracked or committable; --download writes the kept-out SHA256SUMS.txt lines of release files and drops
the ones they supersede; the W14 text edits reproduce the served files.

    python3 -B -m unittest discover -s deploy/websites/tests -p 'test_*.py' -v
"""
import hashlib, json, re, shutil, subprocess, sys, tempfile, unittest
from pathlib import Path
from unittest import mock

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'tools'))
import site_identity as si  # noqa: E402

ni = si.ni
ROOT = si.ROOT

def served_bundle(root=ROOT):
    """The explorer bundle index.html loads (`assets/index-<hash>.js`), read from the page rather than pinned here, so a
    rename (an identity switch, anchored edits) needs no test change."""
    page = (root / si.EXPLORER / 'index.html').read_text(encoding='utf-8')
    found = re.findall(r'<script\b[^>]*\bsrc="/(assets/index-[A-Za-z0-9_-]+\.js)"', page)
    assert len(found) == 1, found
    return found[0]
DECLARED = json.loads((ROOT / si.DECLARED).read_text(encoding='utf-8'))
# A spec-2 (v3) GenesisSpec for the test identity: the decoder corpus's v3 genesis (the devnet spec naming the v3
# program set) under a test chain ID.
_corpus = json.loads((ROOT / 'prototypes/pocol-extension/tests/data/decoder-corpus-v3.json').read_text(encoding='utf-8'))
_V3_BASE = bytes.fromhex(next(i['hex'] for i in _corpus['items'] if i['label'] == 'v3/base'))
TEST_CHAIN = 424_243
TEST_COMMIT = 'c0ffee' + '0' * 34
TEST_RPC = 'http://127.0.0.1:18545/studio/rpc'
TEST_SEEDS = ['192.0.2.10:30311', '192.0.2.11:30311']

def rlp_list(payload):
    n = len(payload)
    if n <= 55:
        return bytes([0xc0 + n]) + payload
    size = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([0xf7 + len(size)]) + size + payload

def with_chain(spec, chain):
    """The GenesisSpec with its chainId item replaced."""
    _, s, e = ni.rlp_item(spec, 0)
    _, _, version_end = ni.rlp_item(spec, s)
    _, _, chain_end = ni.rlp_item(spec, version_end)
    raw = chain.to_bytes((chain.bit_length() + 7) // 8, 'big')
    return rlp_list(spec[s:version_end] + bytes([0x80 + len(raw)]) + raw + spec[chain_end:e])

TEST_SPEC = with_chain(_V3_BASE, TEST_CHAIN)
TEST_GENESIS = '0x' + ni.keccak256(TEST_SPEC).hex()

def invitation(spec, seeds):
    _, chain = ni.spec_identity(spec)
    info = {'chainId': chain, 'genesisHash': '0x' + ni.keccak256(spec).hex()}
    return json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': seeds,
                       'files': {'genesis.spec.hex': '0x' + spec.hex(), 'devnet.json': json.dumps(info)}})

# Every tree the tool reads (the document roots, the PHP test, the declared identity, the retired list, the extension
# version and the native release version).
COPIED = ['deploy/websites', si.STUDIO]
COPIED_FILES = [si.RETIRED, si.EXTENSION_PACKAGE, si.NATIVE_PACKAGE]
EXT_VERSION = json.loads((ROOT / si.EXTENSION_PACKAGE).read_text(encoding='utf-8'))['version']
NATIVE_VERSION = si.release.native_version(ROOT)

def snapshot(root):
    out = {}
    for top in COPIED:
        for path in sorted((root / top).rglob('*')):
            if path.is_file() and '__pycache__' not in path.parts:
                out[path.relative_to(root).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    return out

class SiteIdentity(unittest.TestCase):
    def copy(self):
        tmp = Path(tempfile.mkdtemp(prefix='pocol-sites-'))
        self.addCleanup(shutil.rmtree, tmp)
        for top in COPIED:
            shutil.copytree(ROOT / top, tmp / top, ignore=shutil.ignore_patterns('__pycache__', 'config.php'))
        for rel in COPIED_FILES:
            (tmp / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ROOT / rel, tmp / rel)
        (tmp / 'old.json').write_text(invitation(ni.read_spec(DECLARED['genesisSpec']), DECLARED['seeds']), encoding='utf-8')
        (tmp / 'test.json').write_text(invitation(TEST_SPEC, TEST_SEEDS), encoding='utf-8')
        return tmp

    def run_tool(self, root, *args):
        with mock.patch('sys.stderr') as err:
            rc = si.main([*args, '--root', str(root)])
        self.last_errors = ''.join(str(c.args[0]) for c in err.write.call_args_list if c.args)
        return rc

    def bundle(self, root, key):
        site, prefix = si.BUNDLES[key]
        found = sorted((root / site / 'assets').glob(f'{prefix}-*.js'))
        self.assertEqual(len(found), 1, found)
        return found[0]

    def assert_consistent(self, root):
        """The derived files, checked independently of the tool."""
        for key, (site, prefix) in si.BUNDLES.items():
            path = self.bundle(root, key)
            self.assertEqual(path.name, f'{prefix}-{hashlib.sha256(path.read_bytes()).hexdigest()[:12]}.js')
            self.assertIn(f'"/assets/{path.name}"', (root / site / 'index.html').read_text(encoding='utf-8'))
            if key in si.INCLUDE_LISTS:
                listed = (root / si.INCLUDE_LISTS[key]).read_text(encoding='utf-8').split('\n')
                self.assertEqual([line for line in listed if line.startswith(f'assets/{prefix}-') and line.endswith('.js')],
                                 [f'assets/{path.name}'], si.INCLUDE_LISTS[key])
        for site, has_sums in si.SITES.items():
            if not has_sums:
                continue
            for line in (root / site / si.SUMS).read_text(encoding='utf-8').splitlines():
                digest, name = line.split('  ', 1)
                if (root / site / name).exists():
                    self.assertEqual(hashlib.sha256((root / site / name).read_bytes()).hexdigest(), digest, name)
                else:
                    self.assertRegex(name, r'^downloads/[^/]+\.(?:zip|tar\.gz|apk)(?:\.sig|\.sha256)?$')
            listed = {line.split('  ', 1)[1] for line in (root / site / si.SUMS).read_text(encoding='utf-8').splitlines()}
            present = {p.relative_to(root / site).as_posix() for p in (root / site).rglob('*') if p.is_file()} - {si.SUMS}
            self.assertEqual(present - listed, set(), site)

    # ---------------- the tree ----------------
    def test_the_tree_carries_the_declared_identity(self):
        self.assertEqual(self.run_tool(ROOT, '--check'), 0, self.last_errors)
        self.assert_consistent(ROOT)
        spec = ni.read_spec(DECLARED['genesisSpec'])
        self.assertEqual(DECLARED['genesisHash'], '0x' + ni.keccak256(spec).hex())
        self.assertEqual(DECLARED['chainId'], ni.spec_identity(spec)[1])

    def test_the_superseded_ide_bundle_is_gone(self):
        ide = ROOT / si.IDE
        self.assertFalse((ide / 'assets/index-CJbrNXr_.js').exists())
        self.assertNotIn('index-CJbrNXr_', (ide / si.SUMS).read_text(encoding='utf-8'))
        self.assertEqual([p.name for p in (ide / 'assets').glob('index-*.js')], [self.bundle(ROOT, 'ide-bundle').name])

    def test_no_config_php_is_tracked_or_committable(self):
        listed = subprocess.run(['git', '-C', str(ROOT), 'ls-files', '-z'], capture_output=True, check=True).stdout
        tracked = [rel for rel in listed.decode('utf-8').split('\0') if rel and Path(rel).name.lower() == 'config.php']
        self.assertEqual(tracked, [], 'a config.php holds host credentials and is never committed')
        for rel in ('deploy/websites/explorer.pocol.net/api/config.php', 'deploy/websites/ide.pocol.net/api/config.php',
                    'prototypes/pocol-studio/dist/api/config.php'):
            ignored = subprocess.run(['git', '-C', str(ROOT), 'check-ignore', '-q', '--no-index', rel])
            self.assertEqual(ignored.returncode, 0, f'.gitignore must ignore {rel}')

    def test_the_explorer_snapshot_is_static(self):
        """Sites W8: the PHP relays, their .htaccess files and the config example are retired; the gateway answers the
        /api paths (deploy/rewards/explorer_api.py) and Caddy sends the headers (deploy/explorer/explorer_site.py)."""
        names = [p.relative_to(ROOT / si.EXPLORER).as_posix() for p in (ROOT / si.EXPLORER).rglob('*')]
        self.assertFalse([n for n in names if si.SERVER_CODE.search(n)], 'server code or a hidden file in the explorer snapshot')
        self.assertFalse((ROOT / si.EXPLORER / 'api').exists())
        self.assertFalse((ROOT / 'deploy/websites/explorer-config.example.php').exists())
        sums = (ROOT / si.EXPLORER / si.SUMS).read_text(encoding='utf-8')
        self.assertNotIn('api/', sums); self.assertNotIn('.php', sums); self.assertNotIn('.htaccess', sums)
        for name in ('api/rpc.php', 'API/health.php', 'api', 'api/x.txt', '.htaccess', 'assets/.hidden', 'x.php', 'x.PHP7',
                     'x.phtml', 'x.phar'):
            self.assertRegex(name, si.SERVER_CODE)
        for name in ('index.html', served_bundle(), 'site.webmanifest', 'apis.html', 'php.png', 'assets/api.css'):
            self.assertNotRegex(name, si.SERVER_CODE)

    # ---------------- switch ----------------
    def test_a_switch_to_a_test_identity_and_back_is_byte_identical(self):
        root = self.copy()
        before = snapshot(root)
        self.assertEqual(self.run_tool(root, '--to', str(root / 'test.json'), '--check'), 2, 'check fails before the switch')
        self.assertEqual(snapshot(root), before)
        self.assertEqual(self.run_tool(root, '--to', str(root / 'test.json'), '--rpc-url', TEST_RPC,
                                       '--gateway-host', '192.0.2.10'), 0, self.last_errors)
        self.assert_consistent(root)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        declared = json.loads((root / si.DECLARED).read_text(encoding='utf-8'))
        self.assertEqual((declared['chainId'], declared['genesisHash'], declared['seeds'], declared['rpcUrl']),
                         (TEST_CHAIN, TEST_GENESIS, TEST_SEEDS, TEST_RPC))
        explorer = self.bundle(root, 'explorer-bundle').read_text(encoding='utf-8')
        self.assertEqual(len(re.findall(rf'(?<![0-9]){TEST_CHAIN}(?![0-9])', explorer)), 8)
        self.assertEqual(explorer.count(TEST_GENESIS), 7)
        self.assertIn('children:[`192.0.2.10`,`192.0.2.11`].map(', explorer)
        self.assertIn('[e,`:30311`]', explorer)
        self.assertIn('` installer on 192.0.2.10. It reads', explorer)
        ide = self.bundle(root, 'ide-bundle').read_text(encoding='utf-8')
        self.assertIn(f'chainId:{TEST_CHAIN},chainIdHex:`{hex(TEST_CHAIN)}`,genesisHash:`{TEST_GENESIS}`,rpcUrl:`{TEST_RPC}`', ide)
        readme = (root / si.IDE / 'DEPLOY-README-AR.txt').read_text(encoding='utf-8')
        for line in (f'Chain ID: {TEST_CHAIN}', f'Chain ID (hex): {hex(TEST_CHAIN)}', f'RPC: {TEST_RPC}', f'Genesis: {TEST_GENESIS}'):
            self.assertIn('\n' + line + '\n', readme)
        for rel in ('network.js', 'reward.js'):
            text = (root / si.MAIN / rel).read_text(encoding='utf-8')
            self.assertIn(f"const CHAIN_ID = {TEST_CHAIN};\n  const GENESIS = '{TEST_GENESIS}';", text)
            version = hashlib.sha256((root / si.MAIN / rel).read_bytes()).hexdigest()[:12]
            pages = [p for p in (root / si.MAIN).glob('*.html') if f'{rel}?v=' in p.read_text(encoding='utf-8')]
            self.assertTrue(pages, rel)
            for page in pages:
                self.assertEqual(set(re.findall(rf'{re.escape(rel)}\?v=([0-9a-f]{{12}})', page.read_text(encoding='utf-8'))), {version}, page)
        self.assertIn(f"'chainId'=>{TEST_CHAIN},", (root / 'deploy/websites/tests/api_security.php').read_text(encoding='utf-8'))
        # No copy of the old identity is left in the snapshots (the Studio's app.js is set_network_identity.py's).
        old = si.identity_regex(DECLARED['chainId'], DECLARED['genesisHash'])
        for rel in snapshot(root):
            path = root / rel
            if path.suffix in si.BINARY or rel in ni.FILES or rel == si.DECLARED:
                continue
            try:
                text = path.read_text(encoding='utf-8')
            except UnicodeDecodeError:
                continue
            self.assertIsNone(old.search(text), rel)
        # Back to the old (retired) identity: refused unless on purpose, then byte-identical to the start.
        self.assertEqual(self.run_tool(root, '--to', str(root / 'old.json')), 2)
        self.assertIn('retired network', self.last_errors)
        self.assertEqual(self.run_tool(root, '--to', str(root / 'old.json'), '--allow-revert', '--rpc-url', DECLARED['rpcUrl'],
                                       '--gateway-host', DECLARED['gatewayHost']), 0, self.last_errors)
        self.assertEqual(snapshot(root), before)
        self.assertEqual(list(root.rglob('*.site-identity.tmp')), [])

    def test_commit_points_every_source_link_at_it(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root, '--commit', TEST_COMMIT), 0, self.last_errors)
        explorer = self.bundle(root, 'explorer-bundle').read_text(encoding='utf-8')
        app = (root / si.STUDIO / 'app.js').read_text(encoding='utf-8')
        base = si.LINK_PREFIX
        self.assertEqual(base, 'https://github.com/raedrasheed/pocol/')
        for url in (f'tree/{TEST_COMMIT}', f'blob/{TEST_COMMIT}/prototypes/pocold/src/rpc.rs', f'tree/{TEST_COMMIT}/prototypes/pocold'):
            self.assertIn(f'href:`{base}{url}`', explorer)
        for url in (f'blob/{TEST_COMMIT}/prototypes/pocold/src/site.rs', f'tree/{TEST_COMMIT}/prototypes/pocol-extension'):
            self.assertIn(f'href="{base}{url}"', app)
        self.assertNotIn('pull/11', explorer)
        # The Studio page loads app.js by its content version, which follows the edit.
        index = (root / si.STUDIO / 'index.html').read_text(encoding='utf-8')
        self.assertIn(f'app.js?v={hashlib.sha256((root / si.STUDIO / "app.js").read_bytes()).hexdigest()[:12]}', index)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        for bad in ('c0ffee', 'C' * 40, 'g' * 40):
            self.assertEqual(self.run_tool(root, '--commit', bad), 2, bad)

    def test_bad_inputs_are_refused_and_nothing_is_written(self):
        root = self.copy()
        before = snapshot(root)
        cases = {
            'spec-1 seeds of two ports': invitation(TEST_SPEC, ['192.0.2.10:30311', '192.0.2.11:30312']),
            'no seeds': invitation(TEST_SPEC, []),
            'a seed that is not host:port': invitation(TEST_SPEC, ['192.0.2.10']),
            'a seed with a quote': invitation(TEST_SPEC, ['a`b.example:30300']),
            'a reserved chain': invitation(with_chain(_V3_BASE, 777001), TEST_SEEDS),
        }
        for name, text in cases.items():
            (root / 'bad.json').write_text(text, encoding='utf-8')
            self.assertEqual(self.run_tool(root, '--to', str(root / 'bad.json')), 2, name)
        for args in (['--rpc-url', 'https://gateway.pocol.net/studio/rpc`'], ['--rpc-url', 'http://example.net/rpc'],
                     ['--gateway-host', '999.1.1.1'], ['--to', str(root / 'missing.json')]):
            self.assertEqual(self.run_tool(root, *args), 2, args)
        self.assertEqual(snapshot(root), before)

    def test_a_switch_names_the_new_gateway_host(self):
        root = self.copy()
        before = snapshot(root)
        # The test network's seeds do not include the explorer's gateway host: without --gateway-host it is refused.
        self.assertNotIn(DECLARED['gatewayHost'], [s.split(':')[0] for s in TEST_SEEDS])
        self.assertEqual(self.run_tool(root, '--to', str(root / 'test.json'), '--rpc-url', TEST_RPC), 2)
        self.assertIn('pass --gateway-host', self.last_errors)
        self.assertEqual(snapshot(root), before)
        # A network whose seeds include the gateway host keeps it without --gateway-host. The declared host
        # (explorer.pocol.net) is a name the pages use beside the seeds, so this copy first names a seed IP as its host.
        port = DECLARED['seeds'][0].rsplit(':', 1)[1]
        host = '192.0.2.77'
        self.assertEqual(self.run_tool(root, '--gateway-host', host), 0, self.last_errors)
        same = [f'{host}:{port}'] + DECLARED['seeds']
        (root / 'same-seeds.json').write_text(invitation(TEST_SPEC, same), encoding='utf-8')
        self.assertEqual(self.run_tool(root, '--to', str(root / 'same-seeds.json'), '--rpc-url', TEST_RPC), 0, self.last_errors)
        declared = json.loads((root / si.DECLARED).read_text(encoding='utf-8'))
        self.assertEqual((declared['chainId'], declared['gatewayHost']), (TEST_CHAIN, host))
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)

    def test_the_repository_names_an_https_rpc_url(self):
        # A loopback RPC URL is only for a test copy: on the repository it is refused, --check included.
        self.assertEqual(self.run_tool(ROOT, '--rpc-url', TEST_RPC, '--check'), 2)
        self.assertIn('https RPC endpoint', self.last_errors)
        root = self.copy()
        self.assertEqual(self.run_tool(root, '--rpc-url', TEST_RPC), 0, self.last_errors)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        # The same tree, taken as the repository: its declared loopback rpcUrl fails --check, and a run writes nothing.
        before = snapshot(root)
        with mock.patch.object(si, 'ROOT', root.resolve()):
            self.assertEqual(self.run_tool(root, '--check'), 2)
            self.assertIn('https RPC endpoint', self.last_errors)
            self.assertEqual(self.run_tool(root, '--commit', TEST_COMMIT), 2)
            self.assertEqual(snapshot(root), before)
            self.assertEqual(self.run_tool(root, '--rpc-url', DECLARED['rpcUrl']), 0, self.last_errors)
            self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)

    # ---------------- anchors ----------------
    def test_a_moved_duplicated_or_stray_anchor_value_fails_and_writes_nothing(self):
        chain, genesis = str(DECLARED['chainId']), DECLARED['genesisHash']
        cases = [
            ('explorer-bundle', f'Ae={chain},', f'Ae= {chain},', 'holds 0 copies of the anchor'),
            ('explorer-bundle', f'children:[l?l.params.chainId:{chain},', f'children:[l?l.params.chainId :{chain},', 'holds 0 copies of the anchor'),
            ('explorer-bundle', None, f';var stray=`{genesis}`;', 'outside the anchors'),
            ('explorer-bundle', None, f';var stray={chain};', 'outside the anchors'),
            ('explorer-bundle', None, ';var stray=`108.165.128.90`;', 'outside the anchors'),
            ('ide-bundle', 'var w9=Object.freeze({', 'var w9 =Object.freeze({', 'holds 0 copies of the anchor'),
            ('ide-bundle', None, f';var w9=Object.freeze({{name:`{DECLARED["name"]}`,chainId:{chain},chainIdHex:`{hex(DECLARED["chainId"])}`,'
                                 f'genesisHash:`{genesis}`,rpcUrl:`{DECLARED["rpcUrl"]}`,explorerUrl:1}});', 'holds 2 copies of the anchor'),
            (f'{si.MAIN}/network.js', "const GENESIS = '", "const GENESIS  = '", 'holds 0 copies'),
            (f'{si.IDE}/DEPLOY-README-AR.txt', 'Chain ID (hex): ', 'Chain ID hex: ', 'holds 0 copies'),
        ]
        for target, old, new, message in cases:
            with self.subTest(target=target, new=new):
                root = self.copy()
                path = self.bundle(root, target) if target in si.BUNDLES else root / target
                text = path.read_text(encoding='utf-8')
                if old is None:
                    text += new
                else:
                    self.assertIn(old, text)
                    text = text.replace(old, new, 1)
                path.write_text(text, encoding='utf-8')
                before = snapshot(root)
                self.assertEqual(self.run_tool(root, '--check'), 2)
                self.assertIn(message, self.last_errors)
                self.assertEqual(self.run_tool(root, '--to', str(root / 'test.json'), '--gateway-host', '192.0.2.10'), 2)
                self.assertIn(message, self.last_errors)
                self.assertEqual(self.run_tool(root), 2)
                self.assertEqual(snapshot(root), before, 'nothing is written when an anchor fails')

    def test_anchored_edits_rename_the_bundle_and_regenerate_the_checksums(self):
        root = self.copy()
        old_name = self.bundle(root, 'explorer-bundle').name
        edits = [{'file': 'explorer-bundle', 'old': 'Looking up network data…', 'new': 'Reading network data…', 'count': 1}]
        (root / 'edits.json').write_text(json.dumps(edits, ensure_ascii=False), encoding='utf-8')
        self.assertEqual(self.run_tool(root, '--edits', str(root / 'edits.json')), 0, self.last_errors)
        path = self.bundle(root, 'explorer-bundle')
        self.assertNotEqual(path.name, old_name)
        self.assertFalse((root / si.EXPLORER / 'assets' / old_name).exists())
        # The S1 include list follows the rename (it named the old bundle, and the publish would have missed the new one).
        listed = (root / si.INCLUDE_LISTS['explorer-bundle']).read_text(encoding='utf-8')
        self.assertIn(f'\nassets/{path.name}\n', listed)
        self.assertNotIn(old_name, listed)
        self.assertIn('Reading network data…', path.read_text(encoding='utf-8'))
        self.assert_consistent(root)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        before = snapshot(root)
        for bad in ([{**edits[0], 'old': 'Reading network data…', 'count': 2}],
                    [{**edits[0], 'old': 'not in the bundle', 'count': 1}],
                    [{**edits[0], 'file': 'deploy/websites/explorer.pocol.net/api/config.php'}],
                    [{**edits[0], 'file': 'deploy/mainnet/retired-networks.json'}],
                    [{**edits[0], 'file': f'{si.EXPLORER}/{si.SUMS}'}],
                    [{'file': 'explorer-bundle', 'old': 'x', 'new': 'y'}]):
            (root / 'edits.json').write_text(json.dumps(bad, ensure_ascii=False), encoding='utf-8')
            self.assertEqual(self.run_tool(root, '--edits', str(root / 'edits.json')), 2, bad)
        self.assertEqual(snapshot(root), before)
        # An edit that breaks an identity anchor, or adds a copy of an identity value inside an anchored file, is
        # refused by the run itself and writes nothing (the tree stays one that --check accepts).
        chain, genesis = DECLARED['chainId'], DECLARED['genesisHash']
        for edit, message in (
                ({'file': 'explorer-bundle', 'old': f'Ae={chain}', 'new': f'Ae ={chain}', 'count': 1}, 'holds 0 copies of the anchor'),
                ({'file': 'explorer-bundle', 'old': 'Reading network data…', 'new': f'Reading network data of {chain}…', 'count': 1},
                 'outside the anchors'),
                ({'file': 'ide-bundle', 'old': 'var w9=Object.freeze({', 'new': f'var stray=`{genesis}`;var w9=Object.freeze({{', 'count': 1},
                 'outside the anchors'),
                ({'file': f'{si.IDE}/DEPLOY-README-AR.txt', 'old': f'\nChain ID: {chain}\n', 'new': f'\nChain ID:  {chain}\n', 'count': 1},
                 'holds 0 copies of the anchor')):
            with self.subTest(edit=edit['new']):
                (root / 'edits.json').write_text(json.dumps([edit], ensure_ascii=False), encoding='utf-8')
                self.assertEqual(self.run_tool(root, '--edits', str(root / 'edits.json')), 2, edit)
                self.assertIn('after the edits: ', self.last_errors)
                self.assertIn(message, self.last_errors)
                self.assertEqual(snapshot(root), before, 'nothing is written when an edit breaks an anchor')
                self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)

    def test_the_w7_edits_reproduce_the_served_explorer_bundle(self):
        # Sites W7 is anchored edits only (tools/explorer-w7-edits.json): undone in reverse on a copy, the tool's own
        # --edits run gives back the served bundle byte for byte, with the same name, index.html, SHA256SUMS.txt and S1
        # include list, so the committed bundle is exactly the patcher's output of the committed edits.
        root = self.copy()
        edits = json.loads((ROOT / 'deploy/websites/tools/explorer-w7-edits.json').read_text(encoding='utf-8'))
        served = self.bundle(ROOT, 'explorer-bundle')
        text = served.read_text(encoding='utf-8')
        for e in reversed(edits):
            self.assertEqual(text.count(e['new']), 1, e['new'][:80])
            text = text.replace(e['new'], e['old'])
        self.assertNotIn('PocolW7', text)
        base = text.encode('utf-8')
        base_name = f'index-{hashlib.sha256(base).hexdigest()[:12]}.js'
        (root / si.EXPLORER / 'assets' / served.name).unlink()
        (root / si.EXPLORER / 'assets' / base_name).write_bytes(base)
        for rel in (f'{si.EXPLORER}/index.html', si.INCLUDE_LISTS['explorer-bundle']):
            p = root / rel
            p.write_text(p.read_text(encoding='utf-8').replace(served.name, base_name), encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0, self.last_errors)  # the checksums of the reconstructed base
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        self.assertEqual(self.run_tool(root, '--edits', str(ROOT / 'deploy/websites/tools/explorer-w7-edits.json')), 0, self.last_errors)
        self.assertEqual(self.bundle(root, 'explorer-bundle').name, served.name)
        for rel in (f'{si.EXPLORER}/assets/{served.name}', f'{si.EXPLORER}/index.html', f'{si.EXPLORER}/{si.SUMS}',
                    si.INCLUDE_LISTS['explorer-bundle']):
            self.assertEqual((root / rel).read_bytes(), (ROOT / rel).read_bytes(), rel)

    def test_a_rename_the_include_list_cannot_follow_writes_nothing(self):
        # The S1 include list must name the old bundle on one line of its own; otherwise a rename is refused, so the
        # list is never left naming a bundle that no longer exists.
        edits = [{'file': 'explorer-bundle', 'old': 'Looking up network data…', 'new': 'Reading network data…', 'count': 1}]
        for name, change in (('missing', lambda text, line: text.replace(line + '\n', '')),
                             ('twice', lambda text, line: text + line + '\n'),
                             ('not on a line of its own', lambda text, line: text.replace(line, './' + line))):
            with self.subTest(name):
                root = self.copy()
                (root / 'edits.json').write_text(json.dumps(edits, ensure_ascii=False), encoding='utf-8')
                listed = root / si.INCLUDE_LISTS['explorer-bundle']
                listed.write_text(change(listed.read_text(encoding='utf-8'), served_bundle(root)), encoding='utf-8')
                before = snapshot(root)
                self.assertEqual(self.run_tool(root, '--edits', str(root / 'edits.json')), 2)
                self.assertIn(f'must name {served_bundle(root)} on one line of its own', self.last_errors)
                self.assertEqual(snapshot(root), before)

    # ---------------- checks ----------------
    def test_check_refuses_what_must_not_be_served(self):
        def chatgpt(root):
            p = root / si.MAIN / 'style.css'; p.write_text(p.read_text(encoding='utf-8') + '/* https://x.chatgpt.site */')
        def old_download(root):
            p = root / si.IDE / 'DEPLOY-README-AR.txt'; p.write_text(p.read_text(encoding='utf-8') + 'PoCol-Studio-IDE-Viewer-0.3.9.zip\n')
        def missing_asset(root):
            p = root / si.EXPLORER / 'index.html'; p.write_text(p.read_text(encoding='utf-8').replace('index-oGnBWUS-.css', 'index-missing.css'))
        def misnamed_bundle(root):
            b = self.bundle(root, 'ide-bundle'); b.rename(b.with_name('index-extension041-000000000000.js'))
            p = root / si.IDE / 'index.html'; p.write_text(p.read_text(encoding='utf-8').replace(b.name, 'index-extension041-000000000000.js'))
        def stale_sums(root):
            p = root / si.EXPLORER / 'pocol-logo.png'; p.write_bytes(p.read_bytes() + b'\0')
        def unlisted(root):
            (root / si.IDE / 'extra.txt').write_text('PoCol\n')
        def stale_version(root):
            p = root / si.MAIN / 'reward.js'; p.write_text(p.read_text(encoding='utf-8') + '\n')
        def relay_restored(root):
            (root / si.EXPLORER / 'api').mkdir(); (root / si.EXPLORER / 'api' / 'rpc.php').write_text('<?php\n')
        def htaccess_restored(root):
            (root / si.EXPLORER / '.htaccess').write_text('Options -Indexes\n')
        def php_page(root):
            (root / si.EXPLORER / 'status.php').write_text('<?php\n')
        def stale_include_list(root):
            p = root / si.INCLUDE_LISTS['explorer-bundle']
            p.write_text(p.read_text(encoding='utf-8').replace(served_bundle(root), 'assets/index-000000000000.js'), encoding='utf-8')
        def include_list_without_bundle(root):
            p = root / si.INCLUDE_LISTS['explorer-bundle']
            p.write_text(p.read_text(encoding='utf-8').replace(served_bundle(root) + '\n', ''), encoding='utf-8')
        def config_php(root):
            (root / si.EXPLORER / 'api').mkdir(exist_ok=True)
            (root / si.EXPLORER / 'api' / 'config.php').write_text('<?php // test file that the tool must never read\n')
        def append(rel, text):
            def make(root):
                p = root / rel; p.write_text(p.read_text(encoding='utf-8') + text, encoding='utf-8')
            return make
        stale_label = append(f'{si.MAIN}/chrome-extension.html', '<p>Download PoCol extension 0.3.9</p>')
        stale_label.__name__ = 'stale_label'
        stale_studio_label = append(f'{si.MAIN}/style.css', '/* PoCol Studio &amp; Viewer 0.3.9 */')
        stale_studio_label.__name__ = 'stale_studio_label'
        stale_arabic_label = append(f'{si.IDE}/DEPLOY-README-AR.txt', 'المسار الأساسي هو إضافة PoCol للإصدار 0.3.9.\n')
        stale_arabic_label.__name__ = 'stale_arabic_label'
        native_name = append(f'{si.MAIN}/wallet-downloads.js', f'// PoCol-Mainnet-{NATIVE_VERSION}-Windows.zip\n')
        native_name.__name__ = 'native_name'
        native_version = append(f'{si.MAIN}/wallet-downloads.js', '// PoCol-Mainnet-0.4.9-windows-x86_64.zip\n')
        native_version.__name__ = 'native_version'
        cases = {chatgpt: 'chatgpt.site', old_download: '0.3.9', missing_asset: 'does not exist',
                 misnamed_bundle: 'index-extension041-000000000000.js: would be removed', stale_sums: 'SHA256SUMS.txt', unlisted: 'SHA256SUMS.txt',
                 stale_version: '.html: does not carry', relay_restored: 'api/rpc.php: server code', htaccess_restored: '.htaccess: server code',
                 php_page: 'status.php: server code', config_php: 'config.php must never',
                 stale_include_list: "names ['assets/index-000000000000.js'], not the bundle",
                 include_list_without_bundle: 'names no bundle, not the bundle',
                 stale_label: "chrome-extension.html: labels the extension 'PoCol extension 0.3.9'",
                 stale_studio_label: "style.css: labels the extension 'Studio &amp; Viewer 0.3.9'",
                 stale_arabic_label: "DEPLOY-README-AR.txt: labels the extension 'إضافة PoCol للإصدار 0.3.9', 'للإصدار 0.3.9'",
                 native_name: f'wallet-downloads.js: names the native package PoCol-Mainnet-{NATIVE_VERSION}-Windows.zip',
                 native_version: 'wallet-downloads.js: names the native package PoCol-Mainnet-0.4.9-windows-x86_64.zip'}
        for make, message in cases.items():
            with self.subTest(make.__name__):
                root = self.copy()
                make(root)
                read = Path.read_bytes
                def guarded(path, *a, **k):
                    if path.name.lower() == 'config.php':
                        raise AssertionError(f'config.php was read: {path}')
                    return read(path, *a, **k)
                with mock.patch.object(Path, 'read_bytes', guarded):
                    self.assertEqual(self.run_tool(root, '--check'), 2, make.__name__)
                self.assertIn(message, self.last_errors, make.__name__)

    # ---------------- Sites W14: version labels, native names, kept-out download lines ----------------
    def test_version_labels_name_the_package_and_capability_messages_are_exempt(self):
        # The IDE's download button names the package it links (it said 0.4.0 next to the 0.4.1 href).
        ide = self.bundle(ROOT, 'ide-bundle').read_text(encoding='utf-8')
        self.assertIn(f'href:`/downloads/PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip`,download:!0,children:[(0,D.jsx)(Wr,'
                      f'{{name:`download`}}),`Download PoCol extension {EXT_VERSION}`]', ide)
        # Its capability messages name the oldest version with contract signing: matched by the label patterns, and
        # exempt only because they say "or later".
        for message in ('Update the PoCol extension to version 0.4.0 or later', 'Update your PoCol extension to 0.4.0 or later',
                        'Install or update PoCol Studio & Viewer to version 0.4.0 or later'):
            self.assertIn(message, ide)
            found = [m for rx in si.LABELS for m in rx.finditer(message)]
            self.assertEqual(len(found), 1, message)
            self.assertIsNotNone(si.OR_LATER.match(message, found[0].end()), message)
        for text in ('Version 0.3.9 · Chrome 130+', 'يدعم الإصدار 0.3.9 كليهما', 'تربط إضافة PoCol Studio & Viewer 0.3.9 النشر',
                     'Get PoCol extension 0.3.9', 'إضافة PoCol 0.3.9'):
            self.assertTrue(any(rx.search(text) for rx in si.LABELS), text)
        for text in ('إصدار المشروع: 0.1.0', 'Solidity 0.8.38', 'version:`19.3.1`', 'الإصدار 3', 'Chrome 130 أو أحدث'):
            self.assertFalse(any(rx.search(text) for rx in si.LABELS), text)
        root = self.copy()
        p = root / si.MAIN / 'chrome-extension.html'
        original = p.read_bytes()
        for line in (f'<p>Update the PoCol extension to version 0.3.9 or later.</p>', f'<p>حدّث إضافة PoCol إلى الإصدار 0.3.9 أو أحدث.</p>',
                     f'<p>Version {EXT_VERSION} · ZIP</p>'):
            with self.subTest(line=line):
                p.write_text(original.decode('utf-8') + line, encoding='utf-8')
                self.assertEqual(self.run_tool(root), 0, self.last_errors)  # the checksums of the change
                self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        # A stale label is refused by the run itself, which writes nothing.
        p.write_text(original.decode('utf-8') + '<p>Version 0.3.9 · ZIP</p>', encoding='utf-8')
        before = snapshot(root)
        self.assertEqual(self.run_tool(root), 2)
        self.assertIn("chrome-extension.html: labels the extension 'Version 0.3.9', not", self.last_errors)
        self.assertEqual(snapshot(root), before)

    def test_native_names_are_the_release_names_and_the_legacy_exemption_leaves_with_its_last_link(self):
        names = set(si.release.native_names(ROOT))
        self.assertEqual(names, {f'PoCol-Mainnet-{NATIVE_VERSION}-linux-x86_64.tar.gz',
                                 f'PoCol-Mainnet-{NATIVE_VERSION}-windows-x86_64.zip', f'PoCol-Mainnet-{NATIVE_VERSION}-arm64.apk'})
        # The exempt names are the retired packages the pages still link, and each is linked today.
        main = '\n'.join(p.read_text(encoding='utf-8') for p in (ROOT / si.MAIN).iterdir()
                         if p.suffix in ('.html', '.js'))
        for name in si.LEGACY_NATIVE:
            self.assertIn(name, main)
            self.assertNotIn(name, names)
        root = self.copy()
        p = root / si.MAIN / 'wallet-downloads.js'
        original = p.read_bytes()
        for name in sorted(names) + [n + '.sig' for n in sorted(names)]:
            with self.subTest(name=name):
                p.write_text(original.decode('utf-8') + f'// {name}\n', encoding='utf-8')
                self.assertEqual(self.run_tool(root), 0, self.last_errors)
                self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        p.write_bytes(original)
        self.assertEqual(self.run_tool(root), 0, self.last_errors)
        # An exempt name that no file names any more fails, so the exemption is removed with the last link.
        with mock.patch.object(si, 'LEGACY_NATIVE', si.LEGACY_NATIVE | {'PoCol-Mainnet-0.3.0-Windows.zip'}):
            self.assertEqual(self.run_tool(root, '--check'), 2)
        self.assertIn('LEGACY_NATIVE: no website file names PoCol-Mainnet-0.3.0-Windows.zip any more', self.last_errors)
        # The native version follows deploy/standalone/package.py.
        pkg = root / si.NATIVE_PACKAGE
        pkg.write_text(pkg.read_text(encoding='utf-8').replace(f"\nversion='{NATIVE_VERSION}'\n", "\nversion='9.9.9'\n"), encoding='utf-8')
        p.write_text(original.decode('utf-8') + f'// PoCol-Mainnet-9.9.9-arm64.apk\n', encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0, self.last_errors)
        p.write_text(original.decode('utf-8') + f'// PoCol-Mainnet-{NATIVE_VERSION}-arm64.apk\n', encoding='utf-8')
        self.assertEqual(self.run_tool(root), 2)
        self.assertIn(f'names the native package PoCol-Mainnet-{NATIVE_VERSION}-arm64.apk', self.last_errors)

    def test_download_writes_the_kept_out_lines_and_drops_the_superseded_ones(self):
        root = self.copy()
        sums = root / si.MAIN / si.SUMS
        lines = lambda: dict(reversed(line.split('  ', 1)) for line in sums.read_text(encoding='utf-8').splitlines())
        old_native, chrome, ext = 'a' * 64, lines()['downloads/PoCol-Studio-Chrome.zip'], lines()[f'downloads/PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip']
        # A kept-out line of the retired native package (it was listed before the 2026-11-13 clean-up): a run keeps it.
        sums.write_text(sums.read_text(encoding='utf-8') + f'{old_native}  downloads/PoCol-Mainnet-0.4.0-Windows.zip\n',
                        encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0, self.last_errors)
        self.assertEqual(lines()['downloads/PoCol-Mainnet-0.4.0-Windows.zip'], old_native, 'kept-out lines are kept')
        # And of an older extension package with an unversioned copy of it: kept too, so --check names the stale line.
        sums.write_text(sums.read_text(encoding='utf-8') + f'{"b" * 64}  downloads/PoCol-Studio-IDE-Viewer-0.3.9.zip\n'
                        f'{"b" * 64}  downloads/PoCol-Legacy-Chrome.zip\n', encoding='utf-8')
        self.assertEqual(self.run_tool(root, '--check'), 2, 'the stale 0.3.9 extension line')
        self.assertIn('SHA256SUMS.txt: links the extension package 0.3.9', self.last_errors)
        win = f'{si.MAIN}/downloads/PoCol-Mainnet-{NATIVE_VERSION}-windows-x86_64.zip'
        given = {win: '1' * 64, win + '.sig': '2' * 64, win + '.sha256': '3' * 64,
                 f'{si.MAIN}/downloads/PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip.sig': '4' * 64}
        before = snapshot(root)
        args = [a for path, digest in given.items() for a in ('--download', f'{path}={digest}')]
        self.assertEqual(self.run_tool(root, *args, '--check'), 2, 'check: the run would change SHA256SUMS.txt')
        self.assertEqual(snapshot(root), before)
        # The extension's own .sha256 is tracked, so a newer family member would have to remove it first: here the
        # 0.4.1 lines stay (same version) and the 0.3.9 line and its copy go (older version of the same family).
        self.assertEqual(self.run_tool(root, *args), 0, self.last_errors)
        got = lines()
        for path, digest in given.items():
            self.assertEqual(got[path[len(si.MAIN) + 1:]], digest, path)
        for gone in ('downloads/PoCol-Mainnet-0.4.0-Windows.zip', 'downloads/PoCol-Studio-IDE-Viewer-0.3.9.zip',
                     'downloads/PoCol-Legacy-Chrome.zip'):
            self.assertNotIn(gone, got)
        self.assertEqual((got['downloads/PoCol-Studio-Chrome.zip'], got[f'downloads/PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip']),
                         (chrome, ext), 'the current extension package and its copy stay')
        self.assertEqual(list(got), sorted(got))
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        self.assert_consistent(root)
        # The lines persist: a later run without --download keeps them, and --download is idempotent.
        self.assertEqual(self.run_tool(root), 0, self.last_errors)
        self.assertEqual(lines(), got)
        self.assertEqual(self.run_tool(root, *args, '--check'), 0, self.last_errors)
        # A newer extension supersedes the tracked 0.4.1 .sha256, which must be removed first (nothing is written).
        before = snapshot(root)
        newer = f'{si.MAIN}/downloads/PoCol-Studio-IDE-Viewer-9.9.9.zip'
        self.assertEqual(self.run_tool(root, '--download', f'{newer}={"5" * 64}'), 2)
        self.assertIn(f'downloads/PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip.sha256: superseded by --download {newer}', self.last_errors)
        self.assertEqual(snapshot(root), before)
        # Refused before anything is read: a file of the snapshot, a site without SHA256SUMS.txt, a path outside
        # downloads/, an unknown kind of file, a bad digest, a repeated path.
        tracked = f'{si.MAIN}/downloads/PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip.sha256'
        for value, message in ((f'{tracked}={"6" * 64}', 'is in the snapshot'),
                               (f'{si.STUDIO}/downloads/PoCol-Studio-IDE-Viewer-{EXT_VERSION}.zip={"6" * 64}', 'PATH=SHA256 with PATH'),
                               (f'{si.MAIN}/PoCol-Mainnet-{NATIVE_VERSION}-arm64.apk={"6" * 64}', 'PATH=SHA256 with PATH'),
                               (f'{si.MAIN}/downloads/../index.html={"6" * 64}', 'PATH=SHA256 with PATH'),
                               (f'{si.MAIN}/downloads/notes.txt={"6" * 64}', 'PATH=SHA256 with PATH'),
                               (f'{win}={"6" * 63}', '64 lower-case hex digits'),
                               (f'{win}={"A" * 64}', '64 lower-case hex digits'),
                               (win, 'PATH=SHA256 with PATH')):
            with self.subTest(value=value):
                self.assertEqual(self.run_tool(root, '--download', value), 2)
                self.assertIn(message, self.last_errors)
                self.assertEqual(snapshot(root), before)
        self.assertEqual(self.run_tool(root, '--download', f'{win}={"1" * 64}', '--download', f'{win}={"7" * 64}'), 2)
        self.assertIn('given twice', self.last_errors)
        self.assertEqual(snapshot(root), before)

    def test_the_w14_edits_reproduce_the_served_files(self):
        # Sites W14's site texts are anchored edits (tools/w14-text-edits.json): undone in reverse on a copy, the tool's
        # own --edits run gives back every served file byte for byte, with the IDE bundle's name and the derived files.
        root = self.copy()
        edits = json.loads((ROOT / 'deploy/websites/tools/w14-text-edits.json').read_text(encoding='utf-8'))
        texts = {}
        for e in reversed(edits):
            rel = si.anchor_file(si.Tree(ROOT), e['file'])
            texts.setdefault(rel, (ROOT / rel).read_text(encoding='utf-8'))
            self.assertEqual(texts[rel].count(e['new']), e['count'], e['new'][:80])
            texts[rel] = texts[rel].replace(e['new'], e['old'])
        for rel, text in texts.items():
            (root / rel).write_text(text, encoding='utf-8')
        # The texts before W14 fail the new checks (the IDE's 0.4.0 label, DEPLOY-README-AR.txt's), so a plain run
        # refuses them; --edits rebuilds every derived file (bundle name, content versions, checksums) from the texts.
        self.assertEqual(self.run_tool(root, '--check'), 2)
        self.assertIn("labels the extension 'PoCol extension 0.4.0'", self.last_errors)
        self.assertIn("DEPLOY-README-AR.txt: labels the extension 'إضافة PoCol للإصدار 0.4.0'", self.last_errors)
        self.assertEqual(self.run_tool(root, '--edits', str(ROOT / 'deploy/websites/tools/w14-text-edits.json')), 0, self.last_errors)
        self.assertEqual(snapshot(root), snapshot(ROOT))

    def test_a_retired_identity_left_after_a_switch_fails(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root, '--to', str(root / 'test.json'), '--gateway-host', '192.0.2.10'), 0, self.last_errors)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        # The old network is retired (deploy/mainnet/retired-networks.json): any copy of it now fails.
        self.assertIn((DECLARED['chainId'], DECLARED['genesisHash'][2:]),
                      ni.read_retired((root / si.RETIRED).read_bytes()))
        for leftover in (DECLARED['genesisHash'], str(DECLARED['chainId']), hex(DECLARED['chainId']).upper().replace('X', 'x')):
            with self.subTest(leftover=leftover):
                p = root / si.MAIN / 'about.css'
                original = p.read_bytes()
                p.write_text(original.decode('utf-8') + f'/* {leftover} */', encoding='utf-8')
                self.assertEqual(self.run_tool(root, '--check'), 2)
                self.assertIn('retired network', self.last_errors)
                p.write_bytes(original)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)

    def test_a_redo_on_the_declared_chain_id_is_checked_by_its_genesis_hash(self):
        # A postponed launch replaces the genesis and keeps the chain ID: set_network_identity.py retires the old
        # (chainId, genesisHash) pair, and the switched snapshots, which pin that chain ID at their anchors, pass; a
        # copy of the old genesis hash still fails.
        root = self.copy()
        chain, old_genesis = DECLARED['chainId'], DECLARED['genesisHash'][2:]
        (root / si.RETIRED).write_bytes(ni.with_retired((root / si.RETIRED).read_bytes(), chain, old_genesis))
        spec = with_chain(_V3_BASE, chain)
        genesis = '0x' + ni.keccak256(spec).hex()
        self.assertNotEqual(genesis, DECLARED['genesisHash'])
        (root / 'redo.json').write_text(invitation(spec, DECLARED['seeds']), encoding='utf-8')
        # The explorer's gateway host (explorer.pocol.net, owner decision 9) is not a seed, so the switch names it, as the
        # launch command does (Initialize-PoColNetwork.ps1).
        self.assertEqual(self.run_tool(root, '--to', str(root / 'redo.json'), '--gateway-host', DECLARED['gatewayHost']), 0,
                         self.last_errors)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        self.assert_consistent(root)
        declared = json.loads((root / si.DECLARED).read_text(encoding='utf-8'))
        self.assertEqual((declared['chainId'], declared['genesisHash']), (chain, genesis))
        explorer = self.bundle(root, 'explorer-bundle').read_text(encoding='utf-8')
        self.assertEqual(len(re.findall(rf'(?<![0-9]){chain}(?![0-9])', explorer)), 8)
        self.assertEqual(explorer.count(genesis), 7)
        gone = si.identity_regex(chain, old_genesis, keep_chain=chain)
        for rel in snapshot(root):
            path = root / rel
            if path.suffix in si.BINARY or rel in ni.FILES:
                continue
            try:
                text = path.read_text(encoding='utf-8')
            except UnicodeDecodeError:
                continue
            self.assertIsNone(gone.search(text), rel)
        for leftover in (DECLARED['genesisHash'], old_genesis.upper()):
            with self.subTest(leftover=leftover):
                p = root / si.MAIN / 'about.css'
                original = p.read_bytes()
                p.write_text(original.decode('utf-8') + f'/* {leftover} */', encoding='utf-8')
                self.assertEqual(self.run_tool(root, '--check'), 2)
                self.assertIn('retired network', self.last_errors)
                p.write_bytes(original)
        self.assertEqual(self.run_tool(root, '--check'), 0, self.last_errors)
        # The detector still finds the chain ID of a retired identity on another chain.
        self.assertIsNotNone(si.identity_regex(chain, old_genesis, keep_chain=TEST_CHAIN).search(f'id {chain};'))
        self.assertIsNone(si.identity_regex(chain, old_genesis, keep_chain=chain).search(f'id {chain};'))

if __name__ == '__main__':
    unittest.main()
