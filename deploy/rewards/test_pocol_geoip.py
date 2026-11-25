"""pocol_geoip.py (development/explorer/MINERS-MAP-HOLDERS.md §4, §4.1, §5.3): the DB-IP CSV build, lookups at range
edges, the public-unicast filter, keyed ids, the credential file, every answer of POST /api/geo-report.php and the
pocol-miners-geography-v2 answer with its invariants. Standard library only, no network; every database is closed
before its temporary directory is removed (Windows cannot delete a mapped file)."""
import contextlib
import gzip
import hashlib
import hmac
import io
import ipaddress
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import pocol_geoip as geo  # noqa: E402

SAMPLE = HERE / 'testdata' / 'geoip-sample.csv'
NET = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32}
SALT_HEX, KEY1_HEX, KEY2_HEX, KEY3_HEX = 'aa' * 32, 'b1' * 32, 'b2' * 32, 'b3' * 32
CREDENTIAL_RAW = json.dumps({'salt': SALT_HEX, 'reporters': {'founder-1': KEY1_HEX, 'founder-2': KEY2_HEX,
                                                            'founder-3': KEY3_HEX}}).encode()
KEYS = {'founder-1': bytes.fromhex(KEY1_HEX), 'founder-2': bytes.fromhex(KEY2_HEX), 'founder-3': bytes.fromhex(KEY3_HEX)}
SALT = bytes.fromhex(SALT_HEX)
HEAD_HASH = '0x' + 'cd' * 32
MINER_A, MINER_B, MINER_C, MINER_D, MINER_E = ('0x' + c * 40 for c in 'abcde')
NOT_A_RECORD = '0x' + '9' * 40
NOW = 1_795_300_000


class DatabaseCase(unittest.TestCase):
    """A database built from the synthetic sample, shared by the class and closed before its folder is removed."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.path = Path(cls.tmp.name) / 'geoip-country.bin'
        cls.counts = geo.build(SAMPLE, '2026-11', cls.path)
        cls.db = geo.GeoDatabase(cls.path)

    @classmethod
    def tearDownClass(cls):
        cls.db.close()
        cls.tmp.cleanup()


class BuildTests(DatabaseCase):
    def test_the_sample_builds_merged_sorted_records_with_its_header(self):
        n4, n6, digest = self.counts
        # IPv4: DE (two adjacent rows merged), FR, US, US, JP, BR, AR, US, GB, AU, NZ; the ZZ rows are left out.
        # IPv6: NL, IE, DE (two adjacent rows merged), ZA, CA; the ZZ row is left out.
        self.assertEqual((n4, n6), (11, 5))
        self.assertEqual(digest, hashlib.sha256(SAMPLE.read_bytes()).hexdigest())
        self.assertEqual((self.db.release, self.db.source_sha256, self.db.ipv4_ranges, self.db.ipv6_ranges),
                         ('2026-11', digest, 11, 5))
        self.assertTrue(self.db.verify())
        self.assertEqual(self.db.descriptor(), {'provider': 'DB-IP Lite', 'release': '2026-11', 'license': 'CC BY 4.0'})
        raw = self.path.read_bytes()
        self.assertEqual(raw[:8], b'POCOLGEO')
        self.assertEqual(len(raw), 96 + 11 * 10 + 5 * 34)

    def test_lookups_at_range_edges(self):
        table = {
            '4.255.255.255': None, '5.0.0.0': 'DE', '5.0.0.255': 'DE', '5.0.1.0': 'DE', '5.0.1.255': 'DE',
            '5.0.2.0': 'FR', '5.0.2.255': 'FR', '5.0.3.0': None, '5.0.3.255': None, '5.0.4.0': None,
            '9.9.8.255': None, '9.9.9.0': 'US', '9.9.9.255': 'US', '9.9.10.0': None, '11.0.0.0': 'US', '11.255.255.255': 'US',
            '23.0.0.0': 'JP', '23.0.0.255': 'JP', '45.0.0.127': 'BR', '45.0.0.128': 'AR', '45.0.1.0': None,
            '223.255.255.0': 'NZ', '223.255.255.255': 'NZ', '255.255.255.255': None, '0.0.0.0': None,
            '2a00:144f:ffff:ffff:ffff:ffff:ffff:ffff': None, '2a00:1450::': 'IE', '2a00:1450:ffff:ffff:ffff:ffff:ffff:ffff': 'IE',
            '2a00:1451::': None, '2a01::': 'DE', '2a01:0:0:1:ffff:ffff:ffff:ffff': 'DE', '2a01:0:0:2::': None,
            '2c0f:fb50::1': 'ZA', '2c0f:fb51::1': None, '3ffe::1': 'CA', '::': None, 'ffff::1': None,
            # IPv4-mapped IPv6 is searched as the IPv4 address it maps.
            '::ffff:5.0.2.1': 'FR', '::ffff:9.9.9.9': 'US',
        }
        for address, country in table.items():
            with self.subTest(address=address):
                self.assertEqual(self.db.lookup(address), country)
        self.assertEqual(self.db.lookup(ipaddress.ip_address('23.0.0.7')), 'JP')
        for bad in ('', 'x', '5.0.0.1:30300', '5.0.0.0/8', 'fe80::1%eth0', None, 5):
            self.assertIsNone(self.db.lookup(bad))

    def test_a_gzip_source_builds_the_same_records_and_names_the_file_it_was_given(self):
        with tempfile.TemporaryDirectory() as folder:
            source = Path(folder) / 'dbip-country-lite-2026-11.csv.gz'
            source.write_bytes(gzip.compress(SAMPLE.read_bytes()))
            out = Path(folder) / 'out.bin'
            n4, n6, digest = geo.build(source, '2026-11', out)
            self.assertEqual((n4, n6), (11, 5))
            self.assertEqual(digest, hashlib.sha256(source.read_bytes()).hexdigest())
            self.assertEqual(out.read_bytes()[96:], self.path.read_bytes()[96:])
            source.write_bytes(b'\x1f\x8b not gzip')
            with self.assertRaisesRegex(geo.GeoDatabaseError, 'gzip'):
                geo.build(source, '2026-11', out)

    def test_malformed_rows_are_refused_with_their_line(self):
        cases = {
            '1.0.0.0,1.0.0.255\n': 'expected start_ip,end_ip,country_code',
            '1.0.0.0,1.0.0.255,US,extra\n': 'expected',
            '1.0.0.x,1.0.0.255,US\n': 'not an IP address',
            '1.0.0.0/24,1.0.0.255,US\n': 'not an IP address',
            'fe80::1%eth0,fe80::2,US\n': 'not an IP address',
            '1.0.0.0,::1,US\n': 'mixes IPv4 and IPv6',
            '1.0.0.0,1.0.0.255,us\n': 'two upper-case letters',
            '1.0.0.0,1.0.0.255,USA\n': 'two upper-case letters',
            '1.0.0.0,1.0.0.255,\n': 'two upper-case letters',
            '1.0.0.9,1.0.0.1,US\n': 'start after end',
            '2.0.0.0,2.0.0.255,US\n1.0.0.0,1.0.0.255,US\n': 'not sorted',
            '1.0.0.0,1.0.0.255,US\n1.0.0.255,1.0.1.255,FR\n': 'overlaps',
            '1.0.0.0,1.0.0.255,US\n1.0.0.0,1.0.0.255,US\n': 'overlaps',
            '2a00::,2a00::ff,US\n2a00::80,2a00::1ff,DE\n': 'overlaps',
            '1.0.0.0,1.0.0.255,US\n' + 'x' * 300 + '\n': 'longer than',
        }
        for text, message in cases.items():
            with self.subTest(text=text[:40]), self.assertRaisesRegex(geo.GeoDatabaseError, message):
                geo.parse_csv(text)
        with self.assertRaisesRegex(geo.GeoDatabaseError, 'line 2'):
            geo.parse_csv('1.0.0.0,1.0.0.255,US\n1.0.0.7,1.0.0.9,US\n')
        # Families are ordered separately (DB-IP lists IPv4 then IPv6); 'ZZ' rows still take part in the order.
        v4, v6 = geo.parse_csv('1.0.0.0,1.0.0.255,ZZ\n::1,::1,ZZ\n2.0.0.0,2.0.0.255,US\n\n')
        self.assertEqual((v4, v6), ([(int(ipaddress.ip_address('2.0.0.0')), int(ipaddress.ip_address('2.0.0.255')), 'US')], []))
        with self.assertRaisesRegex(geo.GeoDatabaseError, 'overlaps'):
            geo.parse_csv('1.0.0.0,1.0.0.255,ZZ\n1.0.0.9,1.0.0.9,US\n')

    def test_the_release_and_a_failed_build_leave_nothing_behind(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / 'out.bin'
            for release in ('2026-13', '26-11', '2026-1', '', '2026-11x'):
                with self.subTest(release=release), self.assertRaisesRegex(geo.GeoDatabaseError, 'YYYY-MM'):
                    geo.build(SAMPLE, release, out)
            self.assertEqual(list(Path(folder).iterdir()), [])

    def test_a_damaged_or_foreign_file_does_not_open(self):
        raw = self.path.read_bytes()
        with tempfile.TemporaryDirectory() as folder:
            bad = Path(folder) / 'bad.bin'
            cases = {'flipped record': raw[:-3] + bytes([raw[-3] ^ 1]) + raw[-2:], 'truncated': raw[:-1],
                     'too short': raw[:40], 'magic': b'NOTPOCOL' + raw[8:], 'version': raw[:8] + b'\0\x02' + raw[10:],
                     'release': raw[:12] + b'2026-99\0' + raw[20:]}
            for name, content in cases.items():
                bad.write_bytes(content)
                with self.subTest(name), self.assertRaises(geo.GeoDatabaseError):
                    geo.GeoDatabase(bad)
            self.assertIsNone(geo.load_database(Path(folder) / 'absent.bin'))
            self.assertIsNone(geo.load_database(None))
            bad.write_bytes(raw)
            with geo.load_database(bad) as db:
                self.assertEqual(db.lookup('5.0.0.1'), 'DE')
            self.assertIsNone(db.lookup('5.0.0.1'), 'a closed database looks nothing up')

    def test_the_command_line(self):
        with tempfile.TemporaryDirectory() as folder:
            out = Path(folder) / 'cli.bin'
            printed = io.StringIO()
            with contextlib.redirect_stdout(printed):
                self.assertEqual(geo.main(['build', '--input', str(SAMPLE), '--release', '2026-11', '--output', str(out)]), 0)
                self.assertEqual(geo.main(['info', str(out)]), 0)
                self.assertEqual(geo.main(['lookup', str(out), '5.0.2.9']), 0)
                self.assertEqual(geo.main(['lookup', str(out), '192.0.2.1']), 0)
            lines = printed.getvalue().splitlines()
            self.assertTrue(lines[0].startswith('BUILT ') and '11 IPv4 and 5 IPv6' in lines[0])
            self.assertEqual(json.loads(lines[1])['release'], '2026-11')
            self.assertEqual(lines[2:], ['FR', 'not a public unicast address (never looked up)'])
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(geo.main(['info', str(Path(folder) / 'absent.bin')]), 1)
                self.assertEqual(geo.main(['build', '--input', str(SAMPLE), '--release', 'x', '--output', str(out)]), 1)


class AddressTests(unittest.TestCase):
    def test_the_public_unicast_filter(self):
        excluded = ['0.0.0.0', '0.1.2.3', '10.0.0.1', '100.64.0.1', '100.127.255.255', '127.0.0.1', '169.254.1.1',
                    '172.16.0.1', '172.31.255.255', '192.0.0.8', '192.0.2.1', '192.88.99.1', '192.168.1.1', '198.18.0.1',
                    '198.19.255.255', '198.51.100.7', '203.0.113.9', '224.0.0.1', '239.255.255.255', '240.0.0.1',
                    '255.255.255.255', '::', '::1', 'fe80::1', 'fec0::1', 'fc00::1', 'fd12:3456::1', 'ff02::1',
                    '64:ff9b::1.2.3.4', '100::1', '2001::1', '2001:0:4136:e378::1', '2001:db8::1', '2002:c000:0204::1',
                    '3fff::1', '::ffff:10.0.0.1', '::ffff:127.0.0.1', '::ffff:0:0', '::1.2.3.4', 'fe80::1%eth0',
                    '', 'localhost', '5.0.0.1:30300', '1.2.3', None]
        public = ['5.0.0.1', '9.9.9.9', '100.63.255.255', '100.128.0.0', '172.15.255.255', '172.32.0.0', '192.0.1.255',
                  '198.17.255.255', '198.20.0.0', '223.255.255.255', '2a00:1450::1', '2c0f:fb50::1', '2600::1',
                  '::ffff:5.0.0.1']
        for address in excluded:
            with self.subTest(excluded=address):
                self.assertIsNone(geo.public_address(address))
        for address in public:
            with self.subTest(public=address):
                self.assertIsNotNone(geo.public_address(address))
        self.assertEqual(geo.public_address('::ffff:5.0.0.1'), ipaddress.ip_address('5.0.0.1'))

    def test_keyed_ids_are_stable_salt_dependent_and_canonical(self):
        one = geo.keyed_id(SALT, '5.0.0.1')
        self.assertRegex(one, r'^[0-9a-f]{24}$')
        self.assertEqual(one, hmac.new(SALT, bytes([5, 0, 0, 1]), hashlib.sha256).hexdigest()[:24])
        self.assertEqual(one, geo.keyed_id(SALT, '5.0.0.1'))
        self.assertEqual(one, geo.keyed_id(SALT, '::ffff:5.0.0.1'), 'IPv4-mapped IPv6 is the IPv4 address')
        self.assertEqual(one, geo.keyed_id(SALT, ipaddress.ip_address('5.0.0.1')))
        self.assertNotEqual(one, geo.keyed_id(b'\xbb' * 32, '5.0.0.1'))
        self.assertNotEqual(one, geo.keyed_id(SALT, '5.0.0.2'))
        self.assertEqual(geo.keyed_id(SALT, '2A00:1450::1'), geo.keyed_id(SALT, '2a00:1450:0:0:0:0:0:1'))
        with self.assertRaises(ValueError):
            geo.keyed_id(SALT, 'not an ip')

    def test_pocold_peer_strings(self):
        cases = {'5.0.0.1:30300': '5.0.0.1', '[2a00:1450::1]:30300': '2a00:1450::1', '[::ffff:5.0.0.1]:41000': '5.0.0.1',
                 '[fe80::1%eth0]:30300': None, '[fe80::1%2]:30300': None, '5.0.0.1': None, 'host.example:30300': None,
                 '[2a00:1450::1]': None, '5.0.0.1:30300:1': None, '': None, 7: None, 'x' * 100: None}
        for text, address in cases.items():
            with self.subTest(text=text):
                found = geo.peer_address(text)
                self.assertEqual(str(found) if found is not None else None, address)


class ObservePeersTests(DatabaseCase):
    def test_peers_are_deduplicated_by_ip_and_only_public_ones_are_looked_up(self):
        peers = ['5.0.0.1:30300', '5.0.0.1:51234', '[::ffff:5.0.0.1]:40000', '10.0.0.7:30300', '[fe80::1%eth0]:30300',
                 '192.0.2.5:30300', '[2a00:1450::9]:30300', '45.0.1.1:30300', 'garbage']
        nodes, unlocated = geo.observe_peers(peers, SALT, self.db)
        self.assertEqual(nodes, {geo.keyed_id(SALT, '5.0.0.1'): 'DE', geo.keyed_id(SALT, '2a00:1450::9'): 'IE',
                                 geo.keyed_id(SALT, '45.0.1.1'): None})
        self.assertEqual(unlocated, 4, '10.0.0.7, the zoned link-local, the documentation address and the garbage')
        nodes, unlocated = geo.observe_peers(peers, SALT, None)
        self.assertEqual(set(nodes.values()), {None}, 'without a database nothing is located')
        self.assertEqual(geo.observe_peers('not a list', SALT, self.db), ({}, 0))


class CredentialTests(unittest.TestCase):
    def test_a_valid_credential(self):
        credentials = geo.parse_credentials(CREDENTIAL_RAW)
        self.assertEqual(credentials.salt, SALT)
        self.assertEqual(credentials.reporters, KEYS)
        self.assertNotIn(SALT_HEX, repr(credentials)); self.assertNotIn(KEY1_HEX, repr(credentials))

    def test_invalid_credentials_never_show_a_secret(self):
        good = json.loads(CREDENTIAL_RAW)
        bad = [b'', b'[]', b'{', b'x' * 70000, json.dumps({'salt': SALT_HEX}).encode(),
               json.dumps({**good, 'extra': 1}).encode(),
               json.dumps({**good, 'salt': SALT_HEX.upper()}).encode(), json.dumps({**good, 'salt': 'aa' * 31}).encode(),
               json.dumps({**good, 'salt': 'aa' * 65}).encode(), json.dumps({**good, 'reporters': {}}).encode(),
               json.dumps({**good, 'reporters': {'Founder-1': KEY1_HEX}}).encode(),
               json.dumps({**good, 'reporters': {'-x': KEY1_HEX}}).encode(),
               json.dumps({**good, 'reporters': {'founder-1': 'zz' * 32}}).encode(),
               json.dumps({**good, 'reporters': {'founder-1': 7}}).encode(),
               json.dumps({**good, 'reporters': {'founder-1': SALT_HEX}}).encode(),
               json.dumps({**good, 'reporters': {'a': KEY1_HEX, 'b': KEY1_HEX}}).encode(),
               json.dumps({**good, 'reporters': {f'r{i}': f'{i:02x}' * 32 for i in range(17)}}).encode(),
               b'{"salt": "' + SALT_HEX.encode() + b'", "salt": "' + SALT_HEX.encode() + b'", "reporters": {}}',
               '{"salt": NaN, "reporters": {}}'.encode()]
        for raw in bad:
            with self.subTest(raw=raw[:60]):
                with self.assertRaises(geo.CredentialError) as caught:
                    geo.parse_credentials(raw)
                for secret in (SALT_HEX, KEY1_HEX, SALT_HEX.upper(), 'zz' * 32):
                    self.assertNotIn(secret, str(caught.exception))

    def test_the_systemd_credential_directory(self):
        with tempfile.TemporaryDirectory() as folder:
            self.assertIsNone(geo.load_credentials(folder), 'no credential: the report route answers 503')
            self.assertIsNone(geo.load_credentials(''))
            (Path(folder) / 'geo-reporters').write_bytes(CREDENTIAL_RAW)
            self.assertEqual(geo.load_credentials(folder).salt, SALT)
            (Path(folder) / 'geo-reporters').write_bytes(b'{}')
            with self.assertRaises(geo.CredentialError):
                geo.load_credentials(folder)


def report_value(reporter='founder-1', at=NOW, height=1000, miner=MINER_A, peer_ips=(), unlocated=0, **changes):
    value = {'schema': 'pocol-geo-report-v1', 'reporter': reporter, 'chainId': NET['chainId'],
             'genesisHash': NET['genesisHash'], 'at': at, 'head': {'height': height, 'hash': HEAD_HASH}, 'miner': miner,
             'peers': [{'id': geo.keyed_id(SALT, ip), 'country': country} for ip, country in peer_ips],
             'unlocated': unlocated}
    value.update(changes)
    return value


def signed(value, reporter=None, key=None, raw=None):
    """(headers, body) of a report as the reporter sends it."""
    body = raw if raw is not None else json.dumps(value, separators=(',', ':')).encode()
    name = reporter or (value.get('reporter') if isinstance(value, dict) else 'founder-1')
    signature = hmac.new(key if key is not None else KEYS.get(name, b'\0'), body, hashlib.sha256).hexdigest()
    return [('Content-Type', 'application/json'), ('Content-Length', str(len(body))), ('X-PoCol-Reporter', name),
            ('X-PoCol-Signature', signature)], body


class VerifyReportTests(DatabaseCase):
    def setUp(self):
        self.credentials = geo.parse_credentials(CREDENTIAL_RAW)
        self.state = geo.ReportState(NET, self.db)

    def verify(self, headers, body, source='5.0.0.10', now=NOW):
        return geo.verify_report(headers, body, source, self.credentials, self.state, now)

    def test_an_accepted_report_keeps_the_source_country_and_never_the_ip(self):
        headers, body = signed(report_value(peer_ips=[('9.9.9.9', 'US'), ('45.0.0.1', 'ZZ')], unlocated=2, miner=MINER_A.upper().replace('0X', '0x')))
        status, report = self.verify(headers, body)
        self.assertEqual(status, 204)
        self.assertEqual(report, {'reporter': 'founder-1', 'at': NOW, 'receivedAt': NOW, 'head': (1000, HEAD_HASH),
                                  'miner': MINER_A, 'unlocated': 2,
                                  'peers': ((geo.keyed_id(SALT, '9.9.9.9'), 'US'), (geo.keyed_id(SALT, '45.0.0.1'), None)),
                                  'sourceId': geo.keyed_id(SALT, '5.0.0.10'), 'sourceCountry': 'DE'})
        self.assertNotIn('5.0.0.10', repr(report))
        # An IPv4-mapped source is the same node; a source that is not public is never looked up.
        status, report = self.verify(*signed(report_value(at=NOW + 1)), source='::ffff:5.0.0.10', now=NOW + 1)
        self.assertEqual((status, report['sourceId'], report['sourceCountry']), (204, geo.keyed_id(SALT, '5.0.0.10'), 'DE'))
        status, report = self.verify(*signed(report_value(at=NOW + 2)), source='127.0.0.1', now=NOW + 2)
        self.assertEqual((status, report['sourceId'], report['sourceCountry']), (204, None, None))
        state = geo.ReportState(NET, None)
        status, report = geo.verify_report(*signed(report_value()), '5.0.0.10', self.credentials, state, NOW)
        self.assertEqual((status, report['sourceCountry']), (204, None), 'no database: the source is not located')
        status, _ = self.verify(*signed(report_value(reporter='founder-2', miner=None)))
        self.assertEqual(status, 204)

    def test_503_without_credentials(self):
        self.assertEqual(geo.verify_report(*signed(report_value()), '5.0.0.10', None, self.state, NOW), (503, None))

    def test_413_and_415(self):
        headers, body = signed(report_value())
        self.assertEqual(self.verify(headers, body + b' ' * (16385 - len(body)))[0], 413)
        self.assertEqual(self.verify(headers, b' ' * 16384)[0], 400, 'exactly 16384 bytes is not too large')
        self.assertEqual(self.verify(headers, 'text')[0], 413)
        for content_type in (None, 'text/plain', 'application/json-seq', ['application/json', 'application/json']):
            changed = [h for h in headers if h[0] != 'Content-Type']
            if isinstance(content_type, list):
                changed += [('Content-Type', c) for c in content_type]
            elif content_type is not None:
                changed.append(('Content-Type', content_type))
            with self.subTest(content_type=content_type):
                self.assertEqual(self.verify(changed, body)[0], 415)
        charset = [h if h[0] != 'Content-Type' else ('content-type', 'Application/JSON; charset=utf-8') for h in headers]
        self.assertEqual(self.verify(charset, body)[0], 204)

    def test_400_on_headers(self):
        headers, body = signed(report_value())
        def without(name):
            return [h for h in headers if h[0] != name]
        cases = {'origin': headers + [('Origin', 'https://explorer.pocol.net')],
                 'transfer-encoding': headers + [('Transfer-Encoding', 'chunked')],
                 'length mismatch': without('Content-Length') + [('Content-Length', str(len(body) + 1))],
                 'two lengths': headers + [('Content-Length', str(len(body)))],
                 'no reporter': without('X-PoCol-Reporter'), 'no signature': without('X-PoCol-Signature'),
                 'bad name': without('X-PoCol-Reporter') + [('X-PoCol-Reporter', 'Founder 1')],
                 'two names': headers + [('X-PoCol-Reporter', 'founder-2')],
                 'short signature': without('X-PoCol-Signature') + [('X-PoCol-Signature', 'ab' * 31)],
                 'signature not hex': without('X-PoCol-Signature') + [('X-PoCol-Signature', 'zz' * 32)]}
        for name, changed in cases.items():
            with self.subTest(name):
                self.assertEqual(self.verify(changed, body), (400, None))
        self.assertEqual(self.verify(without('Content-Length'), body)[0], 204, 'Content-Length is optional (the gateway read it)')

    def test_401_unknown_reporter_or_bad_signature(self):
        value = report_value(reporter='founder-9')
        self.assertEqual(self.verify(*signed(value, key=KEYS['founder-1'])), (401, None))
        self.assertEqual(self.verify(*signed(report_value(), key=KEYS['founder-2'])), (401, None))
        headers, body = signed(report_value())
        self.assertEqual(self.verify(headers, body.replace(b'1000', b'1001')), (401, None), 'the signature covers the body')
        upper = [h if h[0] != 'X-PoCol-Signature' else (h[0], h[1].upper()) for h in headers]
        self.assertEqual(self.verify(upper, body)[0], 204)
        # A bad signature does not consume the replay state: the genuine report is still accepted afterwards.
        self.assertEqual(self.state.last_at, {'founder-1': NOW})

    def test_400_on_the_body(self):
        too_many = [{'id': f'{i:024x}', 'country': None} for i in range(513)]
        cases = {'not json': None, 'schema': dict(schema='pocol-geo-report-v2'), 'reporter differs': dict(reporter='founder-2'),
                 'chain': dict(chainId=1), 'chain bool': dict(chainId=True), 'chain string': dict(chainId=str(NET['chainId'])),
                 'genesis': dict(genesisHash='0x' + '00' * 32), 'genesis upper': dict(genesisHash=NET['genesisHash'].upper()),
                 'at float': dict(at=NOW + 0.5), 'at string': dict(at=str(NOW)), 'at negative': dict(at=-1),
                 'head missing hash': dict(head={'height': 1}), 'head extra': dict(head={'height': 1, 'hash': HEAD_HASH, 'x': 1}),
                 'head negative': dict(head={'height': -1, 'hash': HEAD_HASH}),
                 'head hash short': dict(head={'height': 1, 'hash': '0x' + 'cd' * 31}),
                 'head hash upper': dict(head={'height': 1, 'hash': HEAD_HASH.upper()}),
                 'miner short': dict(miner='0x' + 'ab' * 19), 'miner number': dict(miner=7), 'miner no prefix': dict(miner='ab' * 20),
                 'peers not a list': dict(peers={}), 'peer extra key': dict(peers=[{'id': 'a' * 24, 'country': None, 'ip': '5.0.0.1'}]),
                 'peer id short': dict(peers=[{'id': 'a' * 23, 'country': None}]),
                 'peer id upper': dict(peers=[{'id': 'A' * 24, 'country': None}]),
                 'peer ip as id': dict(peers=[{'id': '5.0.0.1', 'country': None}]),
                 'peer country lower': dict(peers=[{'id': 'a' * 24, 'country': 'us'}]),
                 'peer country long': dict(peers=[{'id': 'a' * 24, 'country': 'USA'}]),
                 'duplicate peer': dict(peers=[{'id': 'a' * 24, 'country': None}] * 2),
                 'unlocated negative': dict(unlocated=-1), 'unlocated bool': dict(unlocated=True), 'unlocated huge': dict(unlocated=5000),
                 'extra key': dict(ip='5.0.0.1'), 'missing key': 'drop-unlocated'}
        for name, change in cases.items():
            with self.subTest(name):
                if change is None:
                    headers, body = signed(None, reporter='founder-1', raw=b'{"schema":')
                elif change == 'drop-unlocated':
                    value = report_value(); del value['unlocated']; headers, body = signed(value)
                else:
                    headers, body = signed(report_value(**change), reporter='founder-1')
                self.assertEqual(self.verify(headers, body), (400, None))
        dup = b'{"schema":"pocol-geo-report-v1","schema":"pocol-geo-report-v1"}'
        self.assertEqual(self.verify(*signed(None, reporter='founder-1', raw=dup)), (400, None))
        self.assertEqual(self.state.last_at, {}, 'a malformed report consumes no replay state')
        # More than 512 peers cannot fit in 16384 bytes (about 390 do): the size limit answers first.
        headers, body = signed(report_value() | {'peers': too_many})
        self.assertEqual(self.verify(headers, body)[0], 413)
        self.assertIsNone(geo._report_shape(report_value() | {'peers': too_many}, 'founder-1', self.state))
        self.assertIsNotNone(geo._report_shape(report_value() | {'peers': too_many[:512]}, 'founder-1', self.state))

    def test_409_stale_and_replayed(self):
        self.assertEqual(self.verify(*signed(report_value(at=NOW - 121))), (409, None))
        self.assertEqual(self.verify(*signed(report_value(at=NOW + 121))), (409, None))
        self.assertEqual(self.verify(*signed(report_value(at=NOW - 120)))[0], 204)
        self.assertEqual(self.verify(*signed(report_value(at=NOW - 120)))[0], 409, 'the same report again is a replay')
        self.assertEqual(self.verify(*signed(report_value(at=NOW - 121)), now=NOW - 60)[0], 409, 'older than the last')
        self.assertEqual(self.verify(*signed(report_value(at=NOW + 120)))[0], 204)
        self.assertEqual(self.verify(*signed(report_value(at=NOW + 5)))[0], 409, 'not newer than the last accepted')
        self.assertEqual(self.verify(*signed(report_value(reporter='founder-2', at=NOW)))[0], 204, 'per reporter')


class FakeHolders:
    """The holders index as Geography reads it (registry(), winners(window), miner_rewards())."""

    def __init__(self, height=1000, records='default', window=None, rewards=None):
        self.height = height
        self.records = records if records != 'default' else [
            record(MINER_A), record(MINER_B), record(MINER_C, last_share=100), record(MINER_D, status=2),
            record(MINER_E, tomb=1)]
        self.window = window if window is not None else {'from': 901, 'to': 1000, 'blocks': 100,
                                                         'miners': {MINER_A: 50, MINER_B: 30, MINER_C: 15, MINER_D: 5}}
        self.rewards = rewards
        self.windows_asked = []

    def registry(self):
        if self.records is None:
            return None
        return {'height': self.height, 'blockHash': HEAD_HASH, 'thetaInact': 360, 'records': self.records}

    def winners(self, window):
        self.windows_asked.append(window)
        return self.window

    def miner_rewards(self):
        return self.rewards


def record(ident, status=1, tomb=0, bond_state=1, last_share=990, grace_start=0):
    return {'id': ident, 'status': status, 'tomb': tomb, 'bondState': bond_state, 'lastShare': last_share,
            'graceStart': grace_start}


class Clock:
    def __init__(self, now=NOW):
        self.now = now

    def __call__(self):
        return self.now


OBSERVER_PEERS = ['5.0.0.10:30300', '23.0.0.5:4444', '23.0.0.5:5555', '10.0.0.1:30300', '[2a00:1450::1]:30300',
                  '[fe80::1%eth0]:30300']


class GeographyTests(DatabaseCase):
    def make(self, database='db', holders=None, peers=OBSERVER_PEERS, credentials=True, status=None):
        self.clock = Clock()
        self.observer_calls = 0
        def observer():
            self.observer_calls += 1
            if isinstance(status, Exception):
                raise status
            return status if status is not None else {'chainId': NET['chainId'], 'genesisHash': NET['genesisHash'],
                                                      'peers': list(peers), 'miner': None}
        self.holders = holders or FakeHolders()
        return geo.Geography(self.db if database == 'db' else database, None, NET, self.holders, observer, self.clock,
                             credentials=geo.parse_credentials(CREDENTIAL_RAW) if credentials else None)

    def send(self, geography, reporter, source, **value):
        value.setdefault('at', int(self.clock.now))
        return geography.report(*signed(report_value(reporter=reporter, **value)), source)

    def scenario(self, **kwargs):
        geography = self.make(**kwargs)
        self.assertEqual(self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A, height=1000,
                                   peer_ips=[('9.9.9.9', 'US'), ('45.0.0.1', 'BR')], unlocated=1), 204)
        self.assertEqual(self.send(geography, 'founder-2', '9.9.9.9', miner=MINER_B, height=990,
                                   peer_ips=[('5.0.0.10', 'DE'), ('45.0.0.1', 'BR')]), 204)
        return geography

    def test_the_answer_shape_and_every_count(self):
        geography = self.scenario()
        self.holders.rewards = {MINER_A: 7 * 10 ** 18, MINER_B: 3, MINER_C: 1}
        answer = geography.answer()
        self.assertEqual(self.holders.windows_asked, [8640])
        self.assertEqual(list(answer), ['schema', 'chainId', 'genesisHash', 'generatedAt', 'height', 'blockHash', 'database',
                                        'miners', 'nodes', 'sources', 'blocksWindow', 'countries', 'unknown'])
        self.assertEqual(answer['schema'], 'pocol-miners-geography-v2')
        self.assertEqual((answer['chainId'], answer['genesisHash'], answer['generatedAt'], answer['height'], answer['blockHash']),
                         (NET['chainId'], NET['genesisHash'], NOW, 1000, HEAD_HASH))
        self.assertEqual(answer['database'], {'provider': 'DB-IP Lite', 'release': '2026-11', 'license': 'CC BY 4.0'})
        # A, B, C active (E is punished, D exited); C's last share is 900 blocks old: inactive.
        self.assertEqual(answer['miners'], {'registered': 5, 'active': 3, 'inactive': 1, 'exited': 1, 'punished': 1,
                                            'located': 2, 'unknown': 3, 'connected': 1, 'thetaInact': 360})
        # Nodes by keyed id: 5.0.0.10 (founder-1's own node, an observer peer and founder-2's peer), 23.0.0.5 (two
        # ports, one node), 2a00:1450::1, 9.9.9.9 (founder-2's node and founder-1's peer), 45.0.0.1 (two servers'
        # peer); not locatable: the observer's 10.0.0.1 and zoned link-local, and founder-1's one.
        self.assertEqual(answer['nodes'], {'observed': 8, 'located': 5, 'unlocated': 3})
        self.assertEqual(answer['sources'], [
            {'kind': 'observer', 'name': 'explorer', 'fresh': True, 'reportedAt': NOW},
            {'kind': 'operator', 'name': 'founder-1', 'fresh': True, 'reportedAt': NOW},
            {'kind': 'operator', 'name': 'founder-2', 'fresh': True, 'reportedAt': NOW},
            {'kind': 'operator', 'name': 'founder-3', 'fresh': False, 'reportedAt': None}])
        self.assertEqual(answer['blocksWindow'], {'from': 901, 'to': 1000, 'blocks': 100})
        self.assertEqual(answer['countries'], [
            {'code': 'DE', 'miners': {'located': 1, 'active': 1, 'inactive': 0, 'connected': 1}, 'nodes': 1, 'blocks': 50,
             'blocksSharePercentE10': '500000000000', 'rewardsWei': str(7 * 10 ** 18)},
            {'code': 'US', 'miners': {'located': 1, 'active': 1, 'inactive': 0, 'connected': 0}, 'nodes': 1, 'blocks': 30,
             'blocksSharePercentE10': '300000000000', 'rewardsWei': '3'},
            {'code': 'BR', 'miners': {'located': 0, 'active': 0, 'inactive': 0, 'connected': 0}, 'nodes': 1, 'blocks': 0,
             'blocksSharePercentE10': '0', 'rewardsWei': '0'},
            {'code': 'IE', 'miners': {'located': 0, 'active': 0, 'inactive': 0, 'connected': 0}, 'nodes': 1, 'blocks': 0,
             'blocksSharePercentE10': '0', 'rewardsWei': '0'},
            {'code': 'JP', 'miners': {'located': 0, 'active': 0, 'inactive': 0, 'connected': 0}, 'nodes': 1, 'blocks': 0,
             'blocksSharePercentE10': '0', 'rewardsWei': '0'}])
        self.assertEqual(answer['unknown'], {'miners': 3, 'blocks': 20})
        self.assertInvariants(answer)

    def assertInvariants(self, answer):
        miners, nodes = answer['miners'], answer['nodes']
        self.assertEqual(miners['located'] + miners['unknown'], miners['registered'])
        self.assertEqual(answer['unknown']['miners'], miners['unknown'])
        self.assertEqual(sum(c['miners']['located'] for c in answer['countries']), miners['located'])
        self.assertLessEqual(sum(c['miners']['connected'] for c in answer['countries']), miners['connected'])
        self.assertEqual(sum(c['nodes'] for c in answer['countries']), nodes['located'])
        self.assertEqual(nodes['located'] + nodes['unlocated'], nodes['observed'])
        self.assertEqual(sum(c['blocks'] for c in answer['countries']) + answer['unknown']['blocks'], answer['blocksWindow']['blocks'])
        for country in answer['countries']:
            self.assertRegex(country['code'], r'^[A-Z]{2}$')
            self.assertNotEqual(country['code'], 'ZZ')
            self.assertLessEqual(country['miners']['inactive'], country['miners']['active'])
            self.assertLessEqual(country['miners']['active'], country['miners']['located'])
            total = answer['blocksWindow']['blocks']
            self.assertEqual(country['blocksSharePercentE10'], str(country['blocks'] * 10 ** 12 // total if total else 0))
        self.assertNoLeak(answer)

    def assertNoLeak(self, answer):
        text = json.dumps(answer)
        self.assertIsNone(re.search(r'(?<![0-9.])\d{1,3}(\.\d{1,3}){3}(?![0-9.])', text), 'no IPv4 address')
        self.assertIsNone(re.search(r'[0-9a-fA-F]{0,4}::?[0-9a-fA-F]{1,4}:', text), 'no IPv6 address')
        self.assertIsNone(re.search(r'(?<![0-9a-f])[0-9a-f]{24}(?![0-9a-f])', text), 'no keyed id')
        self.assertIsNone(re.search(r'0x[0-9a-fA-F]{40}(?![0-9a-fA-F])', text), 'no miner or reward address')
        for secret in (SALT_HEX, KEY1_HEX, KEY2_HEX):
            self.assertNotIn(secret, text)
        allowed = {NET['genesisHash'], HEAD_HASH}
        self.assertTrue(set(re.findall(r'0x[0-9a-f]+', text)) <= allowed)

    def test_peers_are_never_counted_as_miners(self):
        geography = self.make(peers=OBSERVER_PEERS + ['9.9.9.9:30300', '45.0.0.1:30300'])
        answer = geography.answer()
        self.assertEqual(answer['miners']['located'], 0)
        self.assertEqual(answer['miners']['unknown'], 5)
        self.assertEqual(answer['nodes'], {'observed': 7, 'located': 5, 'unlocated': 2})
        self.assertTrue(all(c['miners']['located'] == 0 for c in answer['countries']))
        self.assertEqual(answer['unknown'], {'miners': 5, 'blocks': 100})
        self.assertInvariants(answer)

    def test_one_miner_reported_by_two_servers_counts_once(self):
        geography = self.make()
        self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A)
        self.send(geography, 'founder-3', '5.0.0.11', miner=MINER_A.upper().replace('0X', '0x'))
        answer = geography.answer()
        self.assertEqual((answer['miners']['located'], answer['miners']['connected']), (1, 1))
        de = next(c for c in answer['countries'] if c['code'] == 'DE')
        self.assertEqual((de['miners']['located'], de['blocks'], de['nodes']), (1, 50, 2))
        self.assertInvariants(answer)

    def test_a_miner_two_servers_place_in_different_countries_stays_unknown(self):
        geography = self.make()
        self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A)
        self.send(geography, 'founder-3', '45.0.0.1', miner=MINER_A)
        answer = geography.answer()
        self.assertEqual((answer['miners']['located'], answer['miners']['unknown'], answer['miners']['connected']), (0, 5, 1))
        self.assertEqual(answer['unknown']['blocks'], 100)
        self.assertEqual({c['code']: c['nodes'] for c in answer['countries']}, {'DE': 1, 'BR': 1, 'IE': 1, 'JP': 1})
        self.assertInvariants(answer)
        # A report from an address the database does not locate does not contradict a located one.
        geography = self.make()
        self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A)
        self.send(geography, 'founder-3', '45.0.1.1', miner=MINER_A)
        self.assertEqual(geography.answer()['miners']['located'], 1)

    def test_a_report_naming_no_registry_record_places_no_miner_but_counts_its_node(self):
        geography = self.make(peers=[])
        self.send(geography, 'founder-1', '5.0.0.10', miner=NOT_A_RECORD, peer_ips=[('23.0.0.5', 'JP')])
        answer = geography.answer()
        self.assertEqual((answer['miners']['located'], answer['miners']['connected']), (0, 0))
        self.assertEqual(answer['nodes'], {'observed': 2, 'located': 2, 'unlocated': 0})
        self.assertInvariants(answer)

    def test_stale_reports_and_observer_readings_are_dropped(self):
        geography = self.scenario()
        self.clock.now += 180
        self.assertEqual(geography.answer()['miners']['located'], 2, 'fresh for 180 s')
        self.clock.now += 1
        answer = geography.answer()
        self.assertEqual(answer['miners']['located'], 0)
        self.assertEqual([s['fresh'] for s in answer['sources']], [True, False, False, False])
        self.assertEqual(answer['sources'][1]['reportedAt'], NOW)
        self.assertEqual(answer['nodes'], {'observed': 5, 'located': 3, 'unlocated': 2}, 'only the observer, re-read now')
        self.assertInvariants(answer)

    def test_the_observer_is_read_at_most_every_10_seconds_and_a_failure_keeps_the_last_reading_while_fresh(self):
        geography = self.make()
        geography.answer(); geography.answer()
        self.assertEqual(self.observer_calls, 1)
        self.clock.now += 10
        geography.answer()
        self.assertEqual(self.observer_calls, 2)
        geography.observer_status_fn = lambda: (_ for _ in ()).throw(OSError('node down'))
        self.clock.now += 100
        answer = geography.answer()
        self.assertEqual((answer['nodes']['located'], answer['sources'][0]['fresh']), (3, True))
        self.clock.now += 81
        answer = geography.answer()
        self.assertEqual(answer['nodes'], {'observed': 0, 'located': 0, 'unlocated': 0})
        self.assertEqual(answer['sources'][0], {'kind': 'observer', 'name': 'explorer', 'fresh': False, 'reportedAt': NOW + 10})
        # A node of another network is not read as the observer.
        geography = self.make(status={'chainId': 1, 'genesisHash': NET['genesisHash'], 'peers': OBSERVER_PEERS})
        self.assertEqual(geography.answer()['sources'][0], {'kind': 'observer', 'name': 'explorer', 'fresh': False, 'reportedAt': None})
        geography = self.make(status=OSError('down'))
        self.assertEqual(geography.answer()['nodes']['observed'], 0)

    def test_connected_means_a_fresh_report_with_a_head_within_6_blocks_of_the_index_tip(self):
        for height, connected in ((1000, 1), (994, 1), (1006, 1), (993, 0), (1007, 0)):
            with self.subTest(height=height):
                geography = self.make()
                self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A, height=height)
                answer = geography.answer()
                self.assertEqual(answer['miners']['connected'], connected)
                self.assertEqual(answer['countries'][0]['miners']['connected'], connected)
        # The tip is the newer of the registry snapshot and the newest indexed block.
        holders = FakeHolders(height=998)
        geography = self.make(holders=holders)
        self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A, height=993)
        self.assertEqual(geography.answer()['miners']['connected'], 0)

    def test_blocks_and_shares_are_exact_integers(self):
        window = {'from': 1, 'to': 1000, 'blocks': 8640, 'miners': {MINER_A: 2881, MINER_B: 2879, MINER_C: 1}}
        geography = self.make(holders=FakeHolders(window=window))
        self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A)
        self.send(geography, 'founder-2', '9.9.9.9', miner=MINER_B)
        answer = geography.answer()
        shares = {c['code']: (c['blocks'], c['blocksSharePercentE10']) for c in answer['countries'] if c['blocks']}
        self.assertEqual(shares, {'DE': (2881, str(2881 * 10 ** 12 // 8640)), 'US': (2879, str(2879 * 10 ** 12 // 8640))})
        self.assertEqual(shares['DE'][1], '333449074074')
        self.assertEqual(answer['unknown']['blocks'], 8640 - 2881 - 2879)
        self.assertIsNone(answer['countries'][0]['rewardsWei'], 'no exact rewards: null')
        self.assertInvariants(answer)

    def test_without_a_database_nothing_is_located_and_the_answer_is_still_valid(self):
        geography = self.scenario(database=None)
        answer = geography.answer()
        self.assertIsNone(answer['database'])
        self.assertEqual(answer['countries'], [])
        self.assertEqual(answer['miners']['located'], 0)
        self.assertEqual(answer['miners']['connected'], 1, 'connected does not need a location')
        self.assertEqual(answer['nodes'], {'observed': 8, 'located': 0, 'unlocated': 8})
        self.assertEqual(answer['unknown'], {'miners': 5, 'blocks': 100})
        self.assertInvariants(answer)

    def test_without_credentials_only_the_observer_is_shown(self):
        geography = self.make(credentials=False)
        self.assertFalse(geography.reports_enabled)
        self.assertEqual(self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A), 503)
        answer = geography.answer()
        self.assertEqual(answer['sources'], [{'kind': 'observer', 'name': 'explorer', 'fresh': True, 'reportedAt': NOW}])
        self.assertEqual(answer['miners']['located'], 0)
        self.assertInvariants(answer)
        self.assertEqual(len(geography.salt), 32)

    def test_an_answer_needs_the_holders_index(self):
        self.assertIsNone(self.make(holders=FakeHolders(records=None)).answer())
        holders = FakeHolders(); holders.window = None
        self.assertIsNone(self.make(holders=holders).answer())
        self.assertIsNone(geo.Geography(self.db, None, NET, None, None, Clock()).answer())
        bad = [FakeHolders(records=[{'id': 'x'}]), FakeHolders(records=[record(MINER_A), record(MINER_A.upper().replace('0X', '0x'))]),
               FakeHolders(records=[record(MINER_A, status='1')]),
               FakeHolders(window={'from': 1, 'to': 2, 'blocks': 1, 'miners': {MINER_A: 2}}),
               FakeHolders(window={'from': 1, 'to': 2, 'blocks': 1, 'miners': {'5.0.0.1': 1}}),
               FakeHolders(rewards={MINER_A: -1}), FakeHolders(rewards={MINER_A: '1'})]
        for holders in bad:
            with self.subTest(holders=holders.records[:1] if holders.records else None), self.assertRaises(ValueError):
                self.make(holders=holders).answer()

    def test_a_newer_report_replaces_the_reporters_previous_one(self):
        geography = self.make(peers=[])
        self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A)
        self.clock.now += 60
        self.send(geography, 'founder-1', '5.0.0.10', miner=None)
        answer = geography.answer()
        self.assertEqual((answer['miners']['located'], answer['nodes']['observed']), (0, 1))
        self.assertEqual(self.send(geography, 'founder-1', '5.0.0.10', miner=MINER_A, at=NOW), 409)

    def test_the_salt_is_the_credential_salt(self):
        credentials = geo.parse_credentials(CREDENTIAL_RAW)
        self.assertEqual(geo.Geography(None, SALT, NET, FakeHolders(), None, credentials=credentials).salt, SALT)
        with self.assertRaises(ValueError):
            geo.Geography(None, b'\x01' * 32, NET, FakeHolders(), None, credentials=credentials)
        with self.assertRaises(ValueError):
            geo.Geography(None, b'short', NET, FakeHolders(), None)
        with self.assertRaises(ValueError):
            geo.Geography(None, None, {'chainId': 0, 'genesisHash': NET['genesisHash']}, FakeHolders(), None)


if __name__ == '__main__':
    unittest.main()
