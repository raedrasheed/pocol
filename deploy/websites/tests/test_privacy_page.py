#!/usr/bin/env python3
"""pocol.net/privacy.html, the privacy policy of the PoCol Chrome extension and websites (2026-11-22; the
Chrome Web Store listing links it). The page is English with an Arabic rendering (privacy.js), like the
faucet page: the static HTML is privacy.js's English, every English entry has its Arabic one, and the script renders text
only. Every page's footer links it. The policy describes the repository's code, so each statement it rests on is
checked against that code here: when the code changes, this test fails until the policy follows."""
import html, json, re, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAIN = ROOT / 'deploy' / 'websites' / 'pocol.net'
EXT = ROOT / 'prototypes' / 'pocol-extension'

def translations():
    text = (MAIN / 'privacy.js').read_text(encoding='utf-8')
    start = text.index('const translations = ') + len('const translations = ')
    value, _ = json.JSONDecoder().raw_decode(text, start)
    return value

def visible(fragment):
    return html.unescape(re.sub(r'<[^>]+>', '', fragment))

class PrivacyPage(unittest.TestCase):
    def test_the_static_page_is_the_english_text(self):
        en = translations()['en']
        page = (MAIN / 'privacy.html').read_text(encoding='utf-8')
        self.assertEqual(visible(re.search(r'<title>(.*?)</title>', page).group(1)), en['title'])
        self.assertEqual(html.unescape(re.search(r'<meta name="description" content="([^"]*)">', page).group(1)), en['description'])
        for key in ('eyebrow', 'heading', 'updated', 'lead'):
            m = re.search(rf'<[a-z0-9]+ [^>]*id="privacy-{key}"[^>]*>(.*?)</', page)
            self.assertEqual(visible(m.group(1)), en[key], key)
        body = re.search(r'<div id="privacy-body">(.*?)</div></article>', page, re.S).group(1)
        sections = re.findall(r'<section class="privacy-section"><h2>(.*?)</h2><ul>(.*?)</ul></section>', body, re.S)
        self.assertEqual([visible(h) for h, _ in sections], [s['heading'] for s in en['sections']])
        for (_, items), section in zip(sections, en['sections']):
            self.assertEqual([visible(i) for i in re.findall(r'<li>(.*?)</li>', items, re.S)], section['items'])

    def test_every_english_entry_has_its_arabic_one(self):
        t = translations()
        self.assertEqual(set(t), {'en', 'ar'})
        en, ar = t['en'], t['ar']
        self.assertEqual(set(en), set(ar))
        self.assertEqual([len(s['items']) for s in en['sections']], [len(s['items']) for s in ar['sections']])
        texts = [ar[k] for k in ('title', 'description', 'eyebrow', 'heading', 'updated', 'lead')]
        texts += [s['heading'] for s in ar['sections']] + [i for s in ar['sections'] for i in s['items']]
        for text in texts:
            self.assertRegex(text, r'[؀-ۿ]', text)
        # The hosts and URLs the policy names are the same in both languages.
        urls = lambda d: sorted(re.findall(r'https://[A-Za-z0-9./_-]+[A-Za-z0-9/_-]', json.dumps(d, ensure_ascii=False)))
        self.assertEqual(urls(en), urls(ar))

    def test_the_page_loads_the_site_scripts_and_renders_text_only(self):
        page = (MAIN / 'privacy.html').read_text(encoding='utf-8')
        scripts = re.findall(r'<script src="([^"?]+)', page)
        self.assertEqual(scripts, ['locales.js', 'about-locales.js', 'language-picker.js', 'app.js', 'about-nav.js', 'privacy.js'])
        self.assertIn('<link rel="stylesheet" href="privacy.css?v=', page)
        script = (MAIN / 'privacy.js').read_text(encoding='utf-8')
        self.assertNotRegex(script, r'innerHTML|outerHTML|insertAdjacentHTML|document\.write|\bfetch\(|XMLHttpRequest')
        self.assertIn("window.addEventListener('pocol:language', render);", script)

    def test_every_page_footer_links_the_policy_in_both_languages(self):
        pages = sorted(MAIN.glob('*.html'))
        self.assertIn(MAIN / 'privacy.html', pages)
        for page in pages:
            footer = re.search(r'<footer.*?</footer>', page.read_text(encoding='utf-8'), re.S).group(0)
            self.assertRegex(footer, r'<a href="privacy\.html\?v=[0-9a-f]{12}" data-i18n="privacyLink">Privacy policy</a>', page.name)
        text = (MAIN / 'locales.js').read_text(encoding='utf-8')
        locales = json.loads(text[len('window.POCOL_LOCALES = '):].rstrip().rstrip(';'))
        self.assertEqual(locales['en']['privacyLink'], 'Privacy policy')
        self.assertEqual(locales['ar']['privacyLink'], 'سياسة الخصوصية')

class PolicyMatchesTheCode(unittest.TestCase):
    """The behaviour each statement of the policy describes."""
    def test_the_extension_contacts_only_the_named_hosts_and_websites(self):
        build = (EXT / 'build.mjs').read_text(encoding='utf-8')
        self.assertIn("host_permissions: ['http://127.0.0.1/*', 'https://explorer.pocol.net/*'],", build)
        self.assertEqual(re.findall(r"matches:\['([^']+)'\]", build), ['https://studio.pocol.net/*', 'https://ide.pocol.net/*'])
        network = (EXT / 'src' / 'lib' / 'studio-network.js').read_text(encoding='utf-8')
        self.assertIn('"endpoint": "https://explorer.pocol.net/studio/rpc"', network)
        policy = json.dumps(translations()['en'])
        for named in ('https://explorer.pocol.net/studio/rpc', '127.0.0.1', 'https://studio.pocol.net', 'https://ide.pocol.net'):
            self.assertIn(named, policy)

    def test_the_wallet_encryption_is_the_one_the_policy_names(self):
        wallet = (EXT / 'src' / 'lib' / 'wallet.js').read_text(encoding='utf-8')
        self.assertIn('export const PBKDF2_ITER = 600000,', wallet)
        self.assertIn("iterations: PBKDF2_ITER, hash: 'SHA-256' }, base, { name: 'AES-GCM', length: 256 }", wallet)
        keyfile = (EXT / 'src' / 'lib' / 'studio-keyfile.js').read_text(encoding='utf-8')
        self.assertIn('...(await encryptSeed(secret, password))', keyfile)
        self.assertIn('PBKDF2-SHA256 with 600,000 iterations and AES-256-GCM', json.dumps(translations()['en']))

    def test_the_wallet_statements_are_what_the_worker_does(self):
        # 2026-11-22 (EXT-02/04): import from a private key or a BIP-39 phrase (only the derived key is kept), a backup at
        # any time, the list of sent transfers kept on the device, receipts read by hash, and the first-run agreement.
        policy = json.dumps(translations()['en'])
        sw = (EXT / 'src' / 'sw.js').read_text(encoding='utf-8')
        account = (EXT / 'src' / 'lib' / 'account.js').read_text(encoding='utf-8')
        self.assertIn("else if (msg.kind === 'phrase') secret = acct.phraseToSecret(msg.phrase);", sw)
        self.assertIn('Only the derived key is kept; the phrase itself is never stored.', account)
        self.assertIn('Of a recovery phrase only the derived key of its first account is kept; the phrase itself is not stored.', policy)
        self.assertIn("case 'account.export': {", sw)
        self.assertIn('when you create your wallet or ask for a backup', policy)
        self.assertIn("HISTORY_KEY = 'wallet:history'", sw)
        self.assertIn("rpc('eth_getTransactionReceipt', [t.hash])", sw)
        self.assertIn('a list of the transfers you sent from it (hash, recipient, amount and status)', policy)
        self.assertIn('the hashes of the transfers you sent (to read their receipts)', policy)
        self.assertIn('async function requireDisclosure()', sw)
        self.assertIn('Before the toolbar wallet makes its first such request (network status, balance, Send or transfer status), it shows what it sends and asks you to agree.', policy)

    def test_the_disclosure_statement_covers_exactly_the_requests_it_gates(self):
        # The agreement gates the toolbar wallet's four network operations (tests/account_test.mjs drives each one);
        # Studio and IDE operations are not behind it, and the policy says what they send before their own start.
        policy = json.dumps(translations()['en'])
        sw = (EXT / 'src' / 'sw.js').read_text(encoding='utf-8')
        gated = []
        for m in re.finditer(r'\nasync function (\w+)\([^)]*\) \{\n(.*?)\n\}', sw, re.S):
            if 'await requireDisclosure();' in m.group(2): gated.append(m.group(1))
        self.assertEqual(sorted(gated), ['accountBalance', 'netStatus', 'prepareTransfer', 'refreshHistory'])
        for page in ('publisher-ui.js', 'contract-ui.js', 'viewer.js', 'lib/publisher.js', 'lib/contracts.js'):
            self.assertNotIn('disclosure', (EXT / 'src' / page).read_text(encoding='utf-8'), page)
        self.assertIn('A Studio publication or an IDE contract operation sends your address and its files only after you unlock the wallet in the extension\'s own confirmation window and start the operation there; until then that window reads only the network\'s identity and fees.', policy)
        # Those windows read only identity and fees before the user's start (publisher-ui refreshQuote, contract-ui Prepare).
        ui = (EXT / 'src' / 'publisher-ui.js').read_text(encoding='utf-8')
        quote = re.search(r'async function refreshQuote\(.*?\n\}', ui, re.S).group(0)
        self.assertEqual(sorted(set(re.findall(r"rpc\('(\w+)'", quote))), ['eth_chainId', 'pocol_status'])
        self.assertIn('const fees = await readNetworkFees(rpc)', quote)
        self.assertNotRegex(quote, r'addressOf|eth_getBalance|eth_getTransactionCount')
        self.assertIn("$('prepare').onclick = async () => {", (EXT / 'src' / 'contract-ui.js').read_text(encoding='utf-8'))

    def test_viewer_sites_get_no_account_on_mainnet_as_the_policy_says(self):
        policy = json.dumps(translations()['en'])
        sw = (EXT / 'src' / 'sw.js').read_text(encoding='utf-8')
        self.assertIn("export const seedWalletAllowed = (p) => !!p && p.trustLevel !== 'RN' && p.netKey !== PUBLIC_NET_KEY;", sw)
        self.assertIn("if (!seedWalletAllowed(sess.profile)) return m.method === 'eth_accounts' ? { result: [] } : E(4100, SEED_LOCAL_ONLY);", sw)
        self.assertIn('On PoCol Mainnet this version gives such sites no account and no signature: their requests are refused without a dialog.', policy)
        self.assertNotIn('When such a site asks for your account or a signature, the extension asks you first', policy)

    def test_the_limited_use_statement_is_in_the_policy(self):
        # Chrome Web Store User Data Policy, Limited Use: an affirmative statement on the extension's website.
        en, ar = translations()['en'], translations()['ar']
        section = next(s for s in en['sections'] if s['heading'] == 'Chrome Web Store User Data Policy')
        self.assertIn('complies with the Chrome Web Store User Data Policy, including the Limited Use requirements', section['items'][0])
        self.assertIn('Limited Use', ' '.join(i for s in ar['sections'] for i in s['items']))

    def test_the_servers_write_no_request_log_and_the_faucet_keeps_no_ip_on_disk(self):
        for rel in ('deploy/rewards/gateway.py', 'deploy/studio/pocol_studio_gateway.py', 'deploy/faucet/pocol_faucet.py'):
            self.assertRegex((ROOT / rel).read_text(encoding='utf-8'), r'def log_message\(self[^)]*\): pass', rel)
        for rel in ('deploy/explorer/explorer_site.py', 'deploy/faucet/faucet_route.py', 'deploy/studio/install_studio_gateway.py'):
            self.assertNotRegex((ROOT / rel).read_text(encoding='utf-8'), r'(?m)^\s*log(\s*\{|\s*$)', rel)
        faucet = (ROOT / 'deploy' / 'faucet' / 'pocol_faucet.py').read_text(encoding='utf-8')
        self.assertIn("self.ledger.claims[address] = {'day': day, 'nonce': nonce, 'state': 'sending'}", faucet)
        self.assertIn("self.ledger.claims[address].update({'state': 'sent', 'hash': tx})", faucet)
        self.assertIn('self.clients[(day, client)] = used + 1', faucet)
        self.assertIn('return str(ipaddress.IPv6Network((int(ip) >> 64 << 64, 64)))', faucet)

    def test_the_gateways_count_requests_per_ip_in_memory_as_the_policy_says(self):
        # The explorer gateway (EXP-02): a token bucket per client, the client an IP address or an IPv6 /64, at most
        # CLIENTS_MAX (4,096) remembered, in a dict only. The Studio relay: a bucket per IP address, at most 4,096.
        gateway = (ROOT / 'deploy' / 'rewards' / 'gateway.py').read_text(encoding='utf-8')
        self.assertIn('CLIENT_BURST, CLIENT_RATE, CLIENTS_MAX = 40.0, 10.0, 4096', gateway)
        self.assertIn("return str(ipaddress.ip_network(f'{address}/64', strict=False))", gateway)
        self.assertIn('self.clients[client] = (own, now)', gateway)
        self.assertIn('while len(self.clients) > CLIENTS_MAX: self.clients.popitem(last=False)', gateway)
        studio = (ROOT / 'deploy' / 'studio' / 'pocol_studio_gateway.py').read_text(encoding='utf-8')
        self.assertIn('while len(self.buckets) > 4096:', studio)
        self.assertIn('try: return str(ipaddress.ip_address(value))', studio)
        for text in (gateway, studio):
            self.assertNotRegex(text, r"open\([^)]*['\"][wa]")
        en, ar = (json.dumps(translations()[k], ensure_ascii=False) for k in ('en', 'ar'))
        self.assertIn('the explorer gateway (/rpc and the explorer\u2019s /api reads) counts requests per IP address (for IPv6, '
                      'per /64 network) and the Studio relay (/studio/rpc) counts requests per IP address, in memory only and for '
                      'at most the 4,096 most recently seen; neither writes IP addresses to disk.', en)
        self.assertIn('بوابة المستكشف', ar); self.assertIn('4096', ar)

    def test_the_explorer_still_loads_google_fonts_as_the_policy_says(self):
        css = ''.join(p.read_text(encoding='utf-8') for p in (ROOT / 'deploy/websites/explorer.pocol.net/assets').glob('*.css'))
        self.assertIn('fonts.googleapis.com', css)
        self.assertIn('fonts.googleapis.com', json.dumps(translations()['en']))

    def test_no_pocol_site_sets_a_cookie(self):
        for base in ('deploy/websites', 'prototypes/pocol-studio/dist', 'prototypes/pocol-extension/src'):
            for path in sorted((ROOT / base).rglob('*')):
                if path.suffix in ('.js', '.php', '.html'):
                    self.assertNotRegex(path.read_text(encoding='utf-8', errors='replace'), r'document\.cookie\s*=|setcookie\s*\(|session_start\s*\(', path)

if __name__ == '__main__':
    unittest.main()
