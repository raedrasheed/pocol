#!/usr/bin/env python3
"""deploy/websites/tools/site_identity.py: the snapshots carry exactly the declared identity at their anchors; a switch
to a test identity and back is byte-identical; a moved, duplicated or stray anchor value fails and writes nothing;
anchored edits rename the hashed bundle and regenerate SHA256SUMS.txt; --check refuses chatgpt.site origins, stale
download links, missing index.html assets, misnamed bundles, stale checksums and content versions, retired identities
and any config.php (which is never read); no config.php is tracked or committable, and the explorer's api/.htaccess
keeps its deny rule (php -S ignores .htaccess, so nothing else checks it).

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

# Every tree the tool reads (the document roots, the PHP test, the declared identity, the retired list and the
# extension version).
COPIED = ['deploy/websites', si.STUDIO]
COPIED_FILES = [si.RETIRED, si.EXTENSION_PACKAGE]

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
        for site, has_sums in si.SITES.items():
            if not has_sums:
                continue
            for line in (root / site / si.SUMS).read_text(encoding='utf-8').splitlines():
                digest, name = line.split('  ', 1)
                if (root / site / name).exists():
                    self.assertEqual(hashlib.sha256((root / site / name).read_bytes()).hexdigest(), digest, name)
                else:
                    self.assertRegex(name, r'^downloads/[^/]+\.zip$')
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

    def test_the_explorer_api_denies_config_and_lib(self):
        text = (ROOT / si.HTACCESS).read_text(encoding='utf-8')
        self.assertRegex(text, si.HTACCESS_RULE)
        self.assertIn('<FilesMatch "^(config|lib)\\.php$">', text)
        # The detector finds the rule only when it denies: a weakened rule is missed.
        self.assertIsNone(si.HTACCESS_RULE.search(text.replace('Require all denied', 'Require all granted')))
        self.assertIsNone(si.HTACCESS_RULE.search(text.replace('(config|lib)', '(lib)')))

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
        base = 'https://github.com/palcertcom/pocol/'
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
                self.assertEqual(self.run_tool(root, '--to', str(root / 'test.json')), 2)
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
        # An edit that breaks an identity anchor is caught by the next check.
        (root / 'edits.json').write_text(json.dumps([{'file': 'explorer-bundle', 'old': f'Ae={DECLARED["chainId"]}', 'new': f'Ae ={DECLARED["chainId"]}', 'count': 1}]), encoding='utf-8')
        self.assertEqual(self.run_tool(root, '--edits', str(root / 'edits.json')), 0, self.last_errors)
        self.assertEqual(self.run_tool(root, '--check'), 2)

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
        def no_deny_rule(root):
            p = root / si.HTACCESS; p.write_text(p.read_text(encoding='utf-8').replace('Require all denied', 'Require all granted'))
        def config_php(root):
            (root / si.EXPLORER / 'api' / 'config.php').write_text('<?php // test file that the tool must never read\n')
        cases = {chatgpt: 'chatgpt.site', old_download: '0.3.9', missing_asset: 'does not exist',
                 misnamed_bundle: 'index-extension041-000000000000.js: would be removed', stale_sums: 'SHA256SUMS.txt', unlisted: 'SHA256SUMS.txt',
                 stale_version: '.html: does not carry', no_deny_rule: 'deny rule', config_php: 'config.php must never'}
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

    def test_a_retired_identity_left_after_a_switch_fails(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root, '--to', str(root / 'test.json')), 0, self.last_errors)
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

if __name__ == '__main__':
    unittest.main()
