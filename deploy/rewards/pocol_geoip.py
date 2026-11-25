#!/usr/bin/env python3
"""Country geography of the PoCol network for the explorer gateway (development/explorer/MINERS-MAP-HOLDERS.md §4,
§4.1, §5.3). Python 3.10+, standard library only, no network access.

- `build`: converts a DB-IP "IP to Country Lite" CSV (start_ip,end_ip,country_code per row, IPv4 and IPv6, optionally
  gzip) into a compact sorted binary file (GeoDatabase) whose header carries the release month and the CSV's SHA-256.
- GeoDatabase: lookups by binary search on a read-only memory map (O(log n), the file is not loaded).
- public_address / keyed_id: only public unicast addresses are ever looked up; every IP is replaced by
  HMAC-SHA256(network salt, IP bytes)[:24 hex] before anything else keeps it.
- verify_report: the checks of POST /api/geo-report.php (operator node reports, signed by the reporter's key).
- Geography: the public answer `pocol-miners-geography-v2`. Reports, peer IPs and keyed ids live in memory only;
  nothing is written to disk and no IP, keyed id or miner address is ever in the answer.

What a country means (§4, §7): a node's country is the GeoIP country of its public IP. A miner has a country only when
a fresh signed report of an operator's node names it as that node's own miner (`pocol_status.miner`): the miner is
then placed in the country of the report's source address. `pocol_status.miner` is the address of the node's miner
key (prototypes/pocold/src/node.rs status_json), which is the registry record ID: the node finds its nonce range with
chain::assignment_range(ctx, key.address()), ranges being keyed by record ID (chain.rs assignment_range), and a
block's `miner` is that record ID too (apply.rs ParentResult.winner). It is never the record's reward address. No
protocol message binds a peer IP to a miner, so peers are network nodes, never miners.

    python3 pocol_geoip.py build --input dbip-country-lite-2026-11.csv.gz --release 2026-11 --output geoip-country.bin
    python3 pocol_geoip.py info geoip-country.bin
    python3 pocol_geoip.py lookup geoip-country.bin 192.0.2.1
"""
import argparse
import csv
import gzip
import hashlib
import hmac
import io
import ipaddress
import json
import mmap
import os
from pathlib import Path
import re
import struct
import sys
import tempfile
import threading
import time

# ---------------------------------------------------------------------------------------------------------------
# Binary database format, version 1 (all integers big-endian):
#   header (96 bytes): magic b'POCOLGEO', u16 version, u16 flags (0), 8 bytes release 'YYYY-MM' + NUL, 32 bytes
#   SHA-256 of the source CSV file (as given, compressed or not), 32 bytes SHA-256 of the records that follow, u32
#   IPv4 record count, u32 IPv6 record count, 4 zero bytes;
#   IPv4 records: u32 first, u32 last, 2 ASCII bytes country (10 bytes each);
#   IPv6 records: 16 bytes first, 16 bytes last, 2 ASCII bytes country (34 bytes each).
# Records of one family are sorted, never overlap, and adjacent ranges of one country are merged. Addresses in no
# record (DB-IP's 'ZZ' rows, gaps) have no country.
MAGIC = b'POCOLGEO'
FORMAT_VERSION = 1
HEADER = struct.Struct('>8sHH8s32s32sII4s')
V4_RECORD = struct.Struct('>II2s')
V6_RECORD = struct.Struct('>16s16s2s')
assert HEADER.size == 96 and V4_RECORD.size == 10 and V6_RECORD.size == 34
RELEASE = re.compile(r'20[0-9]{2}-(0[1-9]|1[0-2])')
COUNTRY = re.compile(r'[A-Z]{2}')
UNKNOWN_COUNTRY = 'ZZ'           # DB-IP's code for an unknown country: not located
MAX_ROWS = 8_000_000             # per build: DB-IP Lite has well under a million rows
MAX_LINE = 256
PROVIDER, LICENSE = 'DB-IP Lite', 'CC BY 4.0'

# ---------------------------------------------------------------------------------------------------------------
# Report and answer constants (§4, §4.1, §5.3).
REPORT_SCHEMA = 'pocol-geo-report-v1'
ANSWER_SCHEMA = 'pocol-miners-geography-v2'
MAX_REPORT_BYTES = 16384
MAX_PEERS = 512
AT_SKEW_SECONDS = 120            # a report's `at` must be within ±120 s of the gateway's clock
FRESH_SECONDS = 180              # older reports (and observer readings) are dropped
CONNECTED_BLOCKS = 6             # a reporter's node head within 6 blocks of the index tip
BLOCKS_WINDOW = 8640             # about 24 h at 10 s blocks
MAX_OBSERVED_PEERS = 4096        # bound on the observer node's peer list read
OBSERVE_SECONDS = 10             # the observer node's pocol_status is read at most this often
REPORTER_NAME = re.compile(r'[a-z0-9](?:[a-z0-9-]{0,30}[a-z0-9])?')
SIGNATURE = re.compile(r'[0-9a-fA-F]{64}')
KEYED_ID = re.compile(r'[0-9a-f]{24}')
ADDRESS = re.compile(r'0x[0-9a-fA-F]{40}')
HASH = re.compile(r'0x[0-9a-f]{64}')
SECRET_HEX = re.compile(r'(?:[0-9a-f]{2}){32,64}')     # 32 to 64 bytes, lower-case hex
CREDENTIAL_NAME = 'geo-reporters'
MAX_CREDENTIAL_BYTES = 65536
MAX_REPORTERS = 16
ID_HEX = 24


class GeoDatabaseError(ValueError):
    pass


class CredentialError(ValueError):
    """An invalid credential file. Its message never carries a secret value."""


# ---------------------------------------------------------------------------------------------------------------
# Addresses

_V4_EXCLUDED = tuple(ipaddress.ip_network(n) for n in (
    '0.0.0.0/8',          # "this network"
    '10.0.0.0/8',         # private
    '100.64.0.0/10',      # shared address space (CGNAT)
    '127.0.0.0/8',        # loopback
    '169.254.0.0/16',     # link-local
    '172.16.0.0/12',      # private
    '192.0.0.0/24',       # IETF protocol assignments
    '192.0.2.0/24',       # documentation (TEST-NET-1)
    '192.88.99.0/24',     # 6to4 relay anycast (deprecated)
    '192.168.0.0/16',     # private
    '198.18.0.0/15',      # benchmarking
    '198.51.100.0/24',    # documentation (TEST-NET-2)
    '203.0.113.0/24',     # documentation (TEST-NET-3)
    '224.0.0.0/4',        # multicast
    '240.0.0.0/4',        # reserved, and 255.255.255.255 broadcast
))
_V6_GLOBAL_UNICAST = ipaddress.ip_network('2000::/3')
_V6_EXCLUDED = tuple(ipaddress.ip_network(n) for n in (
    '2001::/23',          # IETF protocol assignments, Teredo 2001::/32 included (a tunnel relay's address)
    '2001:db8::/32',      # documentation
    '2002::/16',          # 6to4 (a tunnel address; the embedded IPv4 is not looked up through it)
    '3fff::/20',          # documentation (RFC 9637)
))


def parse_ip(value):
    """An ipaddress object for a plain IP string (no zone, no port), with an IPv4-mapped IPv6 address returned as the
    IPv4 address it maps; None for anything else."""
    if isinstance(value, (ipaddress.IPv4Address, ipaddress.IPv6Address)):
        address = value
    elif isinstance(value, str) and 2 <= len(value) <= 45 and '%' not in value:
        try:
            address = ipaddress.ip_address(value)
        except ValueError:
            return None
    else:
        return None
    if address.version == 6:
        if address.scope_id is not None:
            return None
        if address.ipv4_mapped is not None:
            return address.ipv4_mapped
    return address


def public_address(value):
    """The address when it is public unicast (and so may be looked up), else None. IPv4: none of the IANA special
    ranges (private, CGNAT, loopback, link-local, documentation, benchmarking, multicast, reserved). IPv6: inside
    global unicast 2000::/3 and outside the IETF assignments (Teredo), documentation and 6to4 ranges; so loopback,
    unspecified, ULA, link-local, site-local, multicast, NAT64 and discard prefixes are all excluded. Explicit tables,
    not ipaddress.is_global, whose answer differs between Python versions."""
    address = parse_ip(value)
    if address is None:
        return None
    if address.version == 4:
        return None if any(address in n for n in _V4_EXCLUDED) else address
    if address not in _V6_GLOBAL_UNICAST or any(address in n for n in _V6_EXCLUDED):
        return None
    return address


def canonical_bytes(address):
    """The bytes a keyed id is computed over: 4 for IPv4 (an IPv4-mapped IPv6 address included), 16 for IPv6."""
    address = parse_ip(address)
    if address is None:
        raise ValueError('not an IP address')
    return address.packed


def keyed_id(salt, address):
    """id = HMAC-SHA256(salt, canonical IP bytes), first 24 hex digits. Stable for one salt, unlinkable without it."""
    return hmac.new(salt, canonical_bytes(address), hashlib.sha256).hexdigest()[:ID_HEX]


_PEER_V4 = re.compile(r'([0-9.]{7,15}):([0-9]{1,5})')
_PEER_V6 = re.compile(r'\[([0-9A-Fa-f:.]{2,45})(?:%[^\]]{1,32})?\]:([0-9]{1,5})')


def peer_address(text):
    """The IP of a pocold peer string (`pocol_status.peers`: Rust SocketAddr text, 'ip:port' or '[ipv6]:port', a zone
    possible on link-local IPv6), or None. A zoned address is link-local and returns None (never public)."""
    if not isinstance(text, str) or len(text) > 80:
        return None
    found = _PEER_V4.fullmatch(text) or _PEER_V6.fullmatch(text)
    if found is None or '%' in text:
        return None
    return parse_ip(found.group(1))


def observe_peers(peers, salt, database, limit=MAX_OBSERVED_PEERS):
    """({keyed id: country or None}, unlocated) for a pocold peer list. Peers are deduplicated by IP (an inbound
    peer's port is ephemeral, so two connections from one IP are one node). A peer whose address is not public
    unicast or cannot be read is never looked up and gets no id: it only counts in `unlocated`. A public address the
    database has no country for keeps its id with country None. Without a database nothing is located."""
    seen, nodes, unlocated = set(), {}, 0
    for text in list(peers)[:limit] if isinstance(peers, list) else []:
        address = peer_address(text)
        if address is None:
            unlocated += 1
            continue
        if address in seen:
            continue
        seen.add(address)
        if public_address(address) is None:
            unlocated += 1
            continue
        nodes[keyed_id(salt, address)] = database.lookup(address) if database is not None else None
    return nodes, unlocated


# ---------------------------------------------------------------------------------------------------------------
# Building the database

def _read_source(path):
    raw = Path(path).read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    if raw[:2] == b'\x1f\x8b':
        try:
            raw = gzip.decompress(raw)
        except (OSError, EOFError) as error:
            raise GeoDatabaseError(f'{path}: not a valid gzip file ({error})') from None
    try:
        text = raw.decode('utf-8')
    except UnicodeDecodeError:
        raise GeoDatabaseError(f'{path}: not UTF-8 text') from None
    return text.lstrip('﻿'), digest


def parse_csv(text):
    """(ipv4 ranges, ipv6 ranges) of a DB-IP country CSV: lists of (first int, last int, country). Every row is
    start_ip,end_ip,country_code with both ends of one family and first <= last; rows of one family must be sorted
    and must not overlap (ValueError naming the line otherwise). 'ZZ' rows (unknown) are left out; adjacent ranges of
    one country are merged. Blank lines and lines starting with '#' (test fixtures) are skipped."""
    families = {4: [], 6: []}
    last = {4: None, 6: None}
    rows = 0
    for number, line in enumerate(io.StringIO(text), 1):
        stripped = line.strip()
        if not stripped or stripped.startswith('#'):
            continue
        if len(stripped) > MAX_LINE:
            raise GeoDatabaseError(f'line {number}: longer than {MAX_LINE} characters')
        rows += 1
        if rows > MAX_ROWS:
            raise GeoDatabaseError(f'more than {MAX_ROWS} rows')
        fields = next(csv.reader([stripped]))
        if len(fields) != 3:
            raise GeoDatabaseError(f'line {number}: expected start_ip,end_ip,country_code')
        first_text, last_text, country = (f.strip() for f in fields)
        first, end = parse_ip_strict(first_text), parse_ip_strict(last_text)
        if first is None or end is None:
            raise GeoDatabaseError(f'line {number}: not an IP address')
        if first.version != end.version:
            raise GeoDatabaseError(f'line {number}: the range mixes IPv4 and IPv6')
        if COUNTRY.fullmatch(country) is None:
            raise GeoDatabaseError(f'line {number}: country code {country[:8]!r} is not two upper-case letters')
        family, low, high = first.version, int(first), int(end)
        if low > high:
            raise GeoDatabaseError(f'line {number}: start after end')
        previous = last[family]
        if previous is not None:
            if low < previous[0]:
                raise GeoDatabaseError(f'line {number}: rows are not sorted (IPv{family})')
            if low <= previous[1]:
                raise GeoDatabaseError(f'line {number}: the range overlaps the previous one (IPv{family})')
        last[family] = (low, high)
        if country == UNKNOWN_COUNTRY:
            continue
        ranges = families[family]
        if ranges and ranges[-1][2] == country and ranges[-1][1] + 1 == low:
            ranges[-1] = (ranges[-1][0], high, country)
        else:
            ranges.append((low, high, country))
    return families[4], families[6]


def parse_ip_strict(text):
    """An address as DB-IP writes it: plain IPv4 or IPv6 (no zone, no prefix). IPv4-mapped IPv6 is kept as IPv6 here:
    lookups map it to IPv4 before searching, so such rows are never consulted."""
    if not 2 <= len(text) <= 45 or '%' in text or '/' in text:
        return None
    try:
        return ipaddress.ip_address(text)
    except ValueError:
        return None


def encode_database(v4, v6, release, source_sha256):
    if RELEASE.fullmatch(release or '') is None:
        raise GeoDatabaseError('--release must be YYYY-MM')
    if not re.fullmatch(r'[0-9a-f]{64}', source_sha256):
        raise GeoDatabaseError('source SHA-256 must be 64 lower-case hex digits')
    body = bytearray()
    for low, high, country in v4:
        body += V4_RECORD.pack(low, high, country.encode('ascii'))
    for low, high, country in v6:
        body += V6_RECORD.pack(low.to_bytes(16, 'big'), high.to_bytes(16, 'big'), country.encode('ascii'))
    header = HEADER.pack(MAGIC, FORMAT_VERSION, 0, release.encode('ascii').ljust(8, b'\0'), bytes.fromhex(source_sha256),
                         hashlib.sha256(body).digest(), len(v4), len(v6), b'\0' * 4)
    return header + bytes(body)


def build(source, release, output):
    """Convert a DB-IP CSV (or .csv.gz) file into the binary database at `output` (written atomically). Returns
    (ipv4 records, ipv6 records, source SHA-256)."""
    text, digest = _read_source(source)
    v4, v6 = parse_csv(text)
    raw = encode_database(v4, v6, release, digest)
    output = Path(output)
    fd, name = tempfile.mkstemp(prefix='.geoip-', dir=str(output.parent))
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        os.chmod(name, 0o644)
        os.replace(name, output)
    finally:
        Path(name).unlink(missing_ok=True)
    return len(v4), len(v6), digest


# ---------------------------------------------------------------------------------------------------------------
# Reading the database

class GeoDatabase:
    """A built database, memory-mapped read-only. Opening checks the header, the exact file size and the records'
    SHA-256; verify() also checks every record's order and code. Close it (or use `with`) before the file is
    replaced or deleted (Windows cannot delete a mapped file)."""

    def __init__(self, path):
        self.path = str(path)
        self._file = open(self.path, 'rb')
        self._map = None
        try:
            size = os.fstat(self._file.fileno()).st_size
            if size < HEADER.size:
                raise GeoDatabaseError(f'{self.path}: not a PoCol GeoIP database (too short)')
            self._map = mmap.mmap(self._file.fileno(), 0, access=mmap.ACCESS_READ)
            magic, version, flags, release, source, records, n4, n6, reserved = HEADER.unpack_from(self._map, 0)
            if magic != MAGIC:
                raise GeoDatabaseError(f'{self.path}: not a PoCol GeoIP database')
            if version != FORMAT_VERSION or flags != 0 or reserved != b'\0' * 4:
                raise GeoDatabaseError(f'{self.path}: unsupported database format version {version}')
            release = release.rstrip(b'\0').decode('ascii', 'replace')
            if RELEASE.fullmatch(release) is None:
                raise GeoDatabaseError(f'{self.path}: invalid release in the header')
            if size != HEADER.size + n4 * V4_RECORD.size + n6 * V6_RECORD.size:
                raise GeoDatabaseError(f'{self.path}: the file size does not match its record counts')
            with memoryview(self._map) as view:
                if hashlib.sha256(view[HEADER.size:]).digest() != records:
                    raise GeoDatabaseError(f'{self.path}: the records do not match the header digest (damaged file)')
            self.release, self.source_sha256, self.ipv4_ranges, self.ipv6_ranges = release, source.hex(), n4, n6
            self._v6_base = HEADER.size + n4 * V4_RECORD.size
        except BaseException:
            self.close()
            raise

    def close(self):
        if self._map is not None:
            self._map.close()
            self._map = None
        if self._file is not None:
            self._file.close()
            self._file = None

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

    def descriptor(self):
        return {'provider': PROVIDER, 'release': self.release, 'license': LICENSE}

    def lookup(self, value):
        """The country code of an address (IPv4-mapped IPv6 searched as IPv4), or None. It does not apply the public
        filter: callers look up only public_address() results."""
        address = parse_ip(value)
        if address is None or self._map is None:
            return None
        if address.version == 4:
            key, base, count, record = int(address), HEADER.size, self.ipv4_ranges, V4_RECORD
            start_of = lambda i: struct.unpack_from('>I', self._map, base + i * 10)[0]
        else:
            key, base, count, record = address.packed, self._v6_base, self.ipv6_ranges, V6_RECORD
            start_of = lambda i: self._map[base + i * 34: base + i * 34 + 16]
        low, high = 0, count
        while low < high:                # the last record whose first address is <= key
            middle = (low + high) // 2
            if start_of(middle) <= key:
                low = middle + 1
            else:
                high = middle
        if low == 0:
            return None
        _, last, country = record.unpack_from(self._map, base + (low - 1) * record.size)
        return country.decode('ascii') if key <= last else None

    def verify(self):
        """Every record in order (first <= last, after the previous record), codes two upper-case letters, not 'ZZ'."""
        for base, count, record in ((HEADER.size, self.ipv4_ranges, V4_RECORD), (self._v6_base, self.ipv6_ranges, V6_RECORD)):
            previous = None
            for i in range(count):
                first, last, country = record.unpack_from(self._map, base + i * record.size)
                text = country.decode('ascii', 'replace')
                if first > last or (previous is not None and first <= previous) or COUNTRY.fullmatch(text) is None \
                        or text == UNKNOWN_COUNTRY:
                    raise GeoDatabaseError(f'{self.path}: record {i} of {"IPv4" if record is V4_RECORD else "IPv6"} is invalid')
                previous = last
        return True


def load_database(path):
    """GeoDatabase at `path`, or None when no file is installed there (no GeoIP: nothing is located)."""
    if not path or not Path(path).is_file():
        return None
    return GeoDatabase(path)


# ---------------------------------------------------------------------------------------------------------------
# Reporter credentials (the gateway's systemd credential)

class Credentials:
    """The network salt and the reporters' keys. repr() never shows a secret."""
    __slots__ = ('salt', 'reporters')

    def __init__(self, salt, reporters):
        self.salt, self.reporters = salt, dict(reporters)

    def __repr__(self):
        return f'Credentials(reporters={sorted(self.reporters)}, secrets redacted)'


def parse_credentials(raw):
    """Credentials of {"salt": "<hex>", "reporters": {"<name>": "<hex key>", ...}}: the salt and every key 32 to 64
    bytes of lower-case hex, 1 to 16 reporters named like 'founder-1', all secrets distinct. CredentialError otherwise
    (the message names the field, never its value)."""
    if not isinstance(raw, (bytes, bytearray)) or len(raw) > MAX_CREDENTIAL_BYTES:
        raise CredentialError('The geo-reporters credential is missing or exceeds 64 KiB')
    try:
        value = json.loads(bytes(raw).decode('utf-8'), object_pairs_hook=_unique_object, parse_constant=_no_constant)
    except (ValueError, UnicodeDecodeError, RecursionError):
        raise CredentialError('The geo-reporters credential is not a JSON object') from None
    if not isinstance(value, dict) or set(value) != {'salt', 'reporters'}:
        raise CredentialError('The geo-reporters credential must hold exactly "salt" and "reporters"')
    salt, reporters = value['salt'], value['reporters']
    if not isinstance(salt, str) or SECRET_HEX.fullmatch(salt) is None:
        raise CredentialError('The geo-reporters "salt" must be 32 to 64 bytes of lower-case hex')
    if not isinstance(reporters, dict) or not 1 <= len(reporters) <= MAX_REPORTERS:
        raise CredentialError(f'The geo-reporters "reporters" must name 1 to {MAX_REPORTERS} reporters')
    keys = {}
    for name, key in reporters.items():
        if REPORTER_NAME.fullmatch(name) is None:
            raise CredentialError('A reporter name must be 1 to 32 characters a-z, 0-9 and inner hyphens')
        if not isinstance(key, str) or SECRET_HEX.fullmatch(key) is None:
            raise CredentialError(f'The key of reporter {name} must be 32 to 64 bytes of lower-case hex')
        keys[name] = bytes.fromhex(key)
    secrets = [bytes.fromhex(salt)] + list(keys.values())
    if len(set(secrets)) != len(secrets):
        raise CredentialError('The salt and every reporter key must be distinct')
    return Credentials(bytes.fromhex(salt), keys)


def load_credentials(directory=None):
    """Credentials from $CREDENTIALS_DIRECTORY/geo-reporters (systemd LoadCredential=geo-reporters:...), or None when
    the credential is not provided (the report route then answers 503 and only observer data is shown). An invalid
    credential raises CredentialError."""
    directory = directory if directory is not None else os.environ.get('CREDENTIALS_DIRECTORY')
    if not directory:
        return None
    path = Path(directory) / CREDENTIAL_NAME
    if not path.is_file():
        return None
    with open(path, 'rb') as stream:
        raw = stream.read(MAX_CREDENTIAL_BYTES + 1)
    return parse_credentials(raw)


def _unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError('duplicate key')
        value[key] = item
    return value


def _no_constant(_):
    raise ValueError('NaN and Infinity are not JSON')


# ---------------------------------------------------------------------------------------------------------------
# Report ingestion (POST /api/geo-report.php)

REPORT_ERRORS = {
    400: 'The report is malformed.',
    401: 'Unknown reporter or bad signature.',
    409: 'The report is stale or replayed.',
    413: 'Request size limit exceeded.',
    415: 'Content-Type must be application/json.',
    503: 'Operator reports are not enabled.',
}
_REPORT_KEYS = {'schema', 'reporter', 'chainId', 'genesisHash', 'at', 'head', 'miner', 'peers', 'unlocated'}
_UNKNOWN_KEY = b'\0' * 32


class ReportState:
    """What verify_report remembers between reports, in memory only: the network served, the GeoIP database (the
    source address's country is looked up at acceptance) and the newest `at` accepted per reporter (replay)."""

    def __init__(self, network, database=None):
        self.chain_id, self.genesis = network_identity(network)
        self.database = database
        self.lock = threading.Lock()
        self.last_at = {}


def network_identity(network):
    """(chainId, genesisHash) of a pocol_rewards.Network or a {"chainId", "genesisHash"} object."""
    if isinstance(network, dict):
        chain, genesis = network.get('chainId'), network.get('genesisHash')
    else:
        chain, genesis = getattr(network, 'chain_id', None), getattr(network, 'genesis', None)
    if type(chain) is not int or chain <= 0 or not isinstance(genesis, str) or HASH.fullmatch(genesis.lower()) is None:
        raise ValueError('A network needs a chain ID and a genesis hash')
    return chain, genesis.lower()


def _header_values(headers, name):
    if headers is None:
        return []
    if hasattr(headers, 'get_all'):
        return list(headers.get_all(name) or [])
    items = headers.items() if isinstance(headers, dict) else headers
    return [value for key, value in items if isinstance(key, str) and key.lower() == name.lower()]


def _int(value, low, high):
    return type(value) is int and low <= value <= high


def verify_report(headers, body, source_ip, credentials, state, now):
    """(status, report) for one POST /api/geo-report.php: 204 with the accepted report, else (status, None) with
    status one of 400 malformed, 401 unknown reporter or bad signature, 409 stale (`at` outside ±120 s of `now`) or
    replayed (`at` not newer than that reporter's last accepted one), 413 body over 16384 bytes, 415 not
    application/json, 503 no credentials. Checks run in that order of cost: size, type, headers, then the signature
    (HMAC-SHA256 of the exact body bytes with the reporter's key, compared in constant time) before the body is parsed.

    The accepted report holds the reporter, `at`, `receivedAt` (= now), the head (height, hash), the miner (lower
    case, or None), the peers as ((id, country or None), ...), `unlocated`, and the reporter node's own `sourceId`
    (keyed id) and `sourceCountry`: the source address is looked up here (when public and a database is installed)
    and then dropped; it is never stored."""
    if credentials is None:
        return 503, None
    if not isinstance(body, (bytes, bytearray)) or len(body) > MAX_REPORT_BYTES:
        return 413, None
    types = _header_values(headers, 'Content-Type')
    if len(types) != 1 or types[0].split(';')[0].strip().lower() != 'application/json':
        return 415, None
    # §4.1 "no Origin": a browser always sends Origin on a cross-site POST; a reporter never does. Refused as
    # malformed (400), the contract's answer list having no 403 for this route.
    if _header_values(headers, 'Origin') or _header_values(headers, 'Transfer-Encoding'):
        return 400, None
    lengths = _header_values(headers, 'Content-Length')
    if len(lengths) > 1 or (lengths and lengths[0].strip() != str(len(body))):
        return 400, None
    names, signatures = _header_values(headers, 'X-PoCol-Reporter'), _header_values(headers, 'X-PoCol-Signature')
    if len(names) != 1 or len(signatures) != 1 or REPORTER_NAME.fullmatch(names[0]) is None \
            or SIGNATURE.fullmatch(signatures[0]) is None:
        return 400, None
    name = names[0]
    key = credentials.reporters.get(name)
    expected = hmac.new(key if key is not None else _UNKNOWN_KEY, bytes(body), hashlib.sha256).hexdigest()
    if not hmac.compare_digest(expected, signatures[0].lower()) or key is None:
        return 401, None
    try:
        value = json.loads(bytes(body).decode('utf-8'), object_pairs_hook=_unique_object, parse_constant=_no_constant)
    except (ValueError, UnicodeDecodeError, RecursionError):
        return 400, None
    report = _report_shape(value, name, state)
    if report is None:
        return 400, None
    at = report['at']
    if abs(at - now) > AT_SKEW_SECONDS:
        return 409, None
    with state.lock:
        if at <= state.last_at.get(name, -1):
            return 409, None
        state.last_at[name] = at
    source = public_address(source_ip)
    report['receivedAt'] = int(now)
    report['sourceId'] = keyed_id(credentials.salt, source) if source is not None else None
    report['sourceCountry'] = state.database.lookup(source) if source is not None and state.database is not None else None
    return 204, report


def _report_shape(value, name, state):
    if not isinstance(value, dict) or set(value) != _REPORT_KEYS:
        return None
    if value['schema'] != REPORT_SCHEMA or value['reporter'] != name:
        return None
    if value['chainId'] != state.chain_id or type(value['chainId']) is not int or value['genesisHash'] != state.genesis:
        return None
    at, head, miner, peers, unlocated = value['at'], value['head'], value['miner'], value['peers'], value['unlocated']
    if not _int(at, 0, 1 << 40):
        return None
    if not isinstance(head, dict) or set(head) != {'height', 'hash'} or not _int(head['height'], 0, 1 << 63) \
            or not isinstance(head['hash'], str) or HASH.fullmatch(head['hash']) is None:
        return None
    if miner is not None and (not isinstance(miner, str) or ADDRESS.fullmatch(miner) is None):
        return None
    if not isinstance(peers, list) or len(peers) > MAX_PEERS or not _int(unlocated, 0, MAX_OBSERVED_PEERS):
        return None
    entries, seen = [], set()
    for peer in peers:
        if not isinstance(peer, dict) or set(peer) != {'id', 'country'}:
            return None
        ident, country = peer['id'], peer['country']
        if not isinstance(ident, str) or KEYED_ID.fullmatch(ident) is None or ident in seen:
            return None
        if country is not None and (not isinstance(country, str) or COUNTRY.fullmatch(country) is None):
            return None
        seen.add(ident)
        entries.append((ident, None if country == UNKNOWN_COUNTRY else country))
    return {'reporter': name, 'at': at, 'head': (head['height'], head['hash']), 'miner': miner.lower() if miner else None,
            'peers': tuple(entries), 'unlocated': unlocated}


# ---------------------------------------------------------------------------------------------------------------
# The public answer

def _holders_inputs(registry, window, rewards):
    """({record id: record}, {record id: blocks won}, {record id: wei} or None) from the holders index's answers,
    checked so that no malformed value reaches the public answer (ValueError otherwise)."""
    if not isinstance(registry, dict) or not _int(registry.get('height'), 0, 1 << 63) \
            or not isinstance(registry.get('blockHash'), str) or HASH.fullmatch(registry['blockHash'].lower()) is None \
            or not _int(registry.get('thetaInact'), 1, 1 << 32) or not isinstance(registry.get('records'), list):
        raise ValueError('The holders index registry snapshot has another shape')
    records = {}
    for record in registry['records']:
        ident = record.get('id') if isinstance(record, dict) else None
        if not isinstance(ident, str) or ADDRESS.fullmatch(ident) is None or ident.lower() in records \
                or not all(_int(record.get(k), 0, 1 << 64) for k in ('status', 'tomb', 'bondState', 'lastShare', 'graceStart')):
            raise ValueError('A registry record has another shape')
        records[ident.lower()] = record
    if not isinstance(window, dict) or not all(_int(window.get(k), 0, 1 << 63) for k in ('from', 'to', 'blocks')) \
            or not isinstance(window.get('miners'), dict):
        raise ValueError('The holders index block window has another shape')
    winners = {}
    for miner, blocks in window['miners'].items():
        if not isinstance(miner, str) or ADDRESS.fullmatch(miner) is None or not _int(blocks, 0, 1 << 63):
            raise ValueError('The block window names a winner that is not an address')
        winners[miner.lower()] = winners.get(miner.lower(), 0) + blocks
    if sum(winners.values()) > window['blocks']:
        raise ValueError('The block window credits more blocks than it holds')
    if rewards is not None:
        if not isinstance(rewards, dict) or not all(isinstance(k, str) and ADDRESS.fullmatch(k) and type(v) is int and v >= 0
                                                    for k, v in rewards.items()):
            raise ValueError('The miner rewards have another shape')
        rewards = {k.lower(): v for k, v in rewards.items()}
    return records, winners, rewards


class Geography:
    """`pocol-miners-geography-v2` (§5.3) from three inputs:

    - holders_index (the gateway's holders index, duck-typed):
        registry() -> None | {"height": int, "blockHash": "0x<64>", "thetaInact": int,
                              "records": [{"id", "status", "tomb", "bondState", "lastShare", "graceStart"}, ...]}
          (every registry record at one snapshot height, §2 definitions);
        winners(window) -> None | {"from": int, "to": int, "blocks": int, "miners": {"0x<record id>": blocks}}
          (the block winners, `miner` = record ID, over the last `window` blocks up to the tip);
        miner_rewards() -> None | {"0x<record id>": wei int}   (optional; None or absent: rewards are not exact)
    - observer_status_fn() -> the raw pocol_status of the gateway host's node (its `peers` list), or raises;
    - operator reports accepted by report() (verify_report), kept in memory, the newest per reporter.

    Without credentials (credentials=None) report() answers 503 and only the observer's peers are shown. Without a
    database nothing is located (database null, every node unlocated, every miner Unknown); reporters' peer countries
    are then not shown either, since the answer names no geolocation source. answer() is None while the holders
    index has no registry snapshot or no block window (the route then answers 503).

    `salt` keys the ids. With credentials it must be the credential's salt (None takes it): reporters compute their
    peers' ids with the same salt, so one node seen by several servers has one id. Without credentials None means a
    random per-process salt (ids never leave memory). Per-country `miners.active` counts every located active record
    and `miners.inactive` the active ones among them that are inactive (§2: inactive miners are active miners)."""

    def __init__(self, database, salt, network, holders_index, observer_status_fn, clock=time.time, credentials=None):
        self.database = database
        self.credentials = credentials
        if credentials is not None and salt is not None and not hmac.compare_digest(salt, credentials.salt):
            raise ValueError('The geography salt must be the geo-reporters credential salt')
        if credentials is not None:
            salt = credentials.salt
        elif salt is None:
            salt = os.urandom(32)
        if not isinstance(salt, bytes) or len(salt) < 16:
            raise ValueError('The geography salt must be at least 16 bytes')
        self.salt = salt
        self.chain_id, self.genesis = network_identity(network)
        self.holders = holders_index
        self.observer_status_fn = observer_status_fn
        self.clock = clock
        self.state = ReportState({'chainId': self.chain_id, 'genesisHash': self.genesis}, database)
        self.lock = threading.Lock()
        self.reports = {}                  # reporter -> newest accepted report (stale ones are pruned)
        self.reported_at = {}              # reporter -> receivedAt of its newest accepted report
        self.observed = None               # (time, {id: country}, unlocated) of the last good observer reading
        self.observe_attempt = None        # time of the last observer read attempt (throttle)

    @property
    def reports_enabled(self):
        """True when the geo-reporters credential is loaded (otherwise POST /api/geo-report.php answers 503)."""
        return self.credentials is not None

    def report(self, headers, body, source_ip):
        """The status of one POST /api/geo-report.php (verify_report); an accepted report replaces that reporter's."""
        now = self.clock()
        status, report = verify_report(headers, body, source_ip, self.credentials, self.state, now)
        if status == 204:
            with self.lock:
                kept = self.reports.get(report['reporter'])
                if kept is None or kept['at'] < report['at']:
                    self.reports[report['reporter']] = report
                    self.reported_at[report['reporter']] = report['receivedAt']
        return status

    def _observe(self, now):
        """(time, {id: country} or None when stale, unlocated) of the newest good reading of the observer node's
        peers, or None before the first. A failed or foreign-network reading keeps the previous one while it is fresh.
        The node is read at most once per OBSERVE_SECONDS whatever the request rate."""
        with self.lock:
            due = self.observe_attempt is None or not 0 <= now - self.observe_attempt < OBSERVE_SECONDS
            if due:
                self.observe_attempt = now
        status = None
        if due and self.observer_status_fn is not None:
            try:
                status = self.observer_status_fn()
            except Exception:
                status = None
        reading = None
        if isinstance(status, dict) and type(status.get('chainId')) is int and status.get('chainId') == self.chain_id \
                and str(status.get('genesisHash', '')).lower() == self.genesis and isinstance(status.get('peers'), list):
            nodes, unlocated = observe_peers(status['peers'], self.salt, self.database)
            reading = (now, nodes, unlocated)
        with self.lock:
            if reading is not None:
                self.observed = reading
            observed = self.observed
        if observed is not None and now - observed[0] > FRESH_SECONDS:
            observed = (observed[0], None, 0)
        return observed

    def answer(self):
        now = self.clock()
        registry = self.holders.registry() if self.holders is not None else None
        window = self.holders.winners(BLOCKS_WINDOW) if registry else None
        if not registry or not window:
            return None
        rewards_fn = getattr(self.holders, 'miner_rewards', None)
        rewards = rewards_fn() if rewards_fn is not None else None
        records, winners, rewards = _holders_inputs(registry, window, rewards)
        theta = registry['thetaInact']
        # The index tip: the newer of the registry snapshot and the newest indexed block (a reporter's node head is
        # compared with it for "connected"); inactivity is evaluated at the registry snapshot's own height.
        height, tip = registry['height'], max(registry['height'], window['to'])

        def active(r):
            return r['status'] == 1 and r['tomb'] == 0 and r['bondState'] in (1, 4)

        def inactive(r):
            return active(r) and height + 1 >= max(r['lastShare'], r['graceStart']) + theta

        with self.lock:
            for name in [n for n, r in self.reports.items() if now - r['receivedAt'] > FRESH_SECONDS]:
                del self.reports[name]
            reports = sorted(self.reports.values(), key=lambda r: r['reporter'])
            reported_at = dict(self.reported_at)
        observed = self._observe(now)

        # Miners: placed only by a fresh report naming a registry record as its node's miner. One miner reported by
        # several servers counts once; reports that place it in different countries leave it Unknown (the gateway
        # cannot tell which is true), and it is located only when every located report agrees.
        claims, connected = {}, set()
        for r in reports:
            miner = r['miner']
            if miner is None or miner not in records:
                continue
            claims.setdefault(miner, set())
            if r['sourceCountry'] is not None and self.database is not None:
                claims[miner].add(r['sourceCountry'])
            if abs(r['head'][0] - tip) <= CONNECTED_BLOCKS:
                connected.add(miner)
        placed = {m: next(iter(c)) for m, c in claims.items() if len(c) == 1}

        # Nodes: the observer's peers, every reporter's peers and every reporter's own node, by keyed id. The
        # gateway's own lookup (observer peers, reporters' source addresses) decides an id's country; otherwise
        # the reporters' countries when they agree. Addresses never looked up (not public) count as unlocated.
        own, claimed, unlocated = {}, {}, 0
        if observed is not None and observed[1] is not None:
            own.update(observed[1])
            unlocated += observed[2]
        for r in reports:
            unlocated += r['unlocated']
            if r['sourceId'] is not None:
                own[r['sourceId']] = r['sourceCountry'] if own.get(r['sourceId']) is None else own[r['sourceId']]
            else:
                unlocated += 1
            for ident, country in r['peers']:
                claimed.setdefault(ident, set())
                if country is not None:
                    claimed[ident].add(country)
        node_country = {}
        for ident in set(own) | set(claimed):
            country = own.get(ident)
            if country is None and ident in claimed and len(claimed[ident]) == 1:
                country = next(iter(claimed[ident]))
            node_country[ident] = country if self.database is not None else None

        countries = {}

        def entry(code):
            return countries.setdefault(code, {'code': code, 'miners': {'located': 0, 'active': 0, 'inactive': 0, 'connected': 0},
                                               'nodes': 0, 'blocks': 0, 'rewards': 0})

        for miner, code in placed.items():
            e = entry(code)
            e['miners']['located'] += 1
            e['miners']['active'] += active(records[miner])
            e['miners']['inactive'] += inactive(records[miner])
            e['miners']['connected'] += miner in connected
            e['blocks'] += winners.get(miner, 0)
            if rewards is not None:
                e['rewards'] += rewards.get(miner, 0)
        for code in node_country.values():
            if code is not None:
                entry(code)['nodes'] += 1
        total_blocks = window['blocks']
        rows = []
        for e in sorted(countries.values(), key=lambda e: (-e['miners']['located'], -e['nodes'], e['code'])):
            rows.append({'code': e['code'], 'miners': e['miners'], 'nodes': e['nodes'], 'blocks': e['blocks'],
                         'blocksSharePercentE10': str(e['blocks'] * 10 ** 12 // total_blocks if total_blocks else 0),
                         'rewardsWei': str(e['rewards']) if rewards is not None else None})
        located_blocks = sum(e['blocks'] for e in countries.values())
        located_nodes = sum(1 for c in node_country.values() if c is not None)
        sources = [{'kind': 'observer', 'name': 'explorer',
                    'fresh': observed is not None and observed[1] is not None,
                    'reportedAt': int(observed[0]) if observed is not None else None}]
        names = set(self.credentials.reporters) if self.credentials is not None else set()
        fresh = {r['reporter'] for r in reports}
        for name in sorted(names | set(reported_at)):
            sources.append({'kind': 'operator', 'name': name, 'fresh': name in fresh, 'reportedAt': reported_at.get(name)})
        values = list(records.values())
        return {
            'schema': ANSWER_SCHEMA, 'chainId': self.chain_id, 'genesisHash': self.genesis, 'generatedAt': int(now),
            'height': height, 'blockHash': registry['blockHash'].lower(),
            'database': self.database.descriptor() if self.database is not None else None,
            'miners': {'registered': len(values), 'active': sum(map(active, values)), 'inactive': sum(map(inactive, values)),
                       'exited': sum(1 for r in values if r['status'] == 2), 'punished': sum(1 for r in values if r['tomb'] != 0),
                       'located': len(placed), 'unknown': len(values) - len(placed), 'connected': len(connected),
                       'thetaInact': theta},
            'nodes': {'observed': len(node_country) + unlocated, 'located': located_nodes,
                      'unlocated': len(node_country) - located_nodes + unlocated},
            'sources': sources,
            'blocksWindow': {'from': window['from'], 'to': window['to'], 'blocks': total_blocks},
            'countries': rows,
            'unknown': {'miners': len(values) - len(placed), 'blocks': total_blocks - located_blocks},
        }


# ---------------------------------------------------------------------------------------------------------------
# Command line

def main(argv=None):
    parser = argparse.ArgumentParser(description='PoCol GeoIP database tool (DB-IP IP to Country Lite).')
    commands = parser.add_subparsers(dest='command', required=True)
    make = commands.add_parser('build', help='convert a DB-IP country CSV (or .csv.gz) into the binary database')
    make.add_argument('--input', required=True)
    make.add_argument('--release', required=True, help='the DB-IP release month, YYYY-MM')
    make.add_argument('--output', required=True)
    info = commands.add_parser('info', help='check a built database and print its header')
    info.add_argument('database')
    find = commands.add_parser('lookup', help='the country of one public address')
    find.add_argument('database')
    find.add_argument('address')
    args = parser.parse_args(argv)
    try:
        if args.command == 'build':
            n4, n6, digest = build(args.input, args.release, args.output)
            with GeoDatabase(args.output) as db:
                db.verify()
            print(f'BUILT {args.output}: release {args.release}, {n4} IPv4 and {n6} IPv6 ranges, source SHA-256 {digest}')
        elif args.command == 'info':
            with GeoDatabase(args.database) as db:
                db.verify()
                print(json.dumps({'provider': PROVIDER, 'license': LICENSE, 'release': db.release, 'sourceSha256': db.source_sha256,
                                  'ipv4Ranges': db.ipv4_ranges, 'ipv6Ranges': db.ipv6_ranges}, sort_keys=True))
        else:
            with GeoDatabase(args.database) as db:
                address = public_address(args.address)
                print(db.lookup(address) if address is not None else 'not a public unicast address (never looked up)')
    except (OSError, GeoDatabaseError) as error:
        print(f'FAILED: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())
