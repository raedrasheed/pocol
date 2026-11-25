#!/usr/bin/env python3
"""PoCol operator node reporter for the explorer's miners map (development/explorer/MINERS-MAP-HOLDERS.md §4, §4.1).

One run (started every 60 s by pocol-geo-reporter.timer, install_geo_reporter.py) does exactly this:

1. one JSON-RPC batch to this host's own node on 127.0.0.1:8545 (pocol_status and pocol_getParams), and a check that
   the node serves the network in the configuration;
2. its peers' countries from the local GeoIP database (pocol_geoip.py, installed beside this file): only public
   unicast addresses are looked up, and every IP is replaced by its keyed id HMAC-SHA256(network salt, IP)[:24 hex]
   at once; the IPs themselves never leave this host and are never printed;
3. a `pocol-geo-report-v1` report naming the node's head and its own mining address (`pocol_status.miner`, the
   miner key's address, which is the registry record ID: pocold keys its nonce assignment by it, node.rs
   assignment_range(ctx, key.address())), signed with HMAC-SHA256(reporter key, body);
4. one HTTPS POST to the configured explorer URL with full certificate and host name verification and a short timeout.

It prints one line and exits: 0 accepted (or --dry-run printed the report), 1 local setup (configuration,
credential or database), 2 the node could not be read or serves another network, 3 the explorer refused the report
(4xx), 4 the explorer could not be reached or failed (network, TLS, 5xx). It never writes a file and never touches
the node beyond that one read.

    python3 -I -B /opt/pocol-geo-reporter/pocol_geo_reporter.py --config /etc/pocol-geo-reporter/config.json
    sudo python3 -I -B /opt/pocol-geo-reporter/pocol_geo_reporter.py --config ... \\
        --credential /etc/pocol-geo-reporter/credential.json --dry-run

Configuration (JSON, not secret): {"chainId": N, "genesisHash": "0x..", "explorerUrl":
"https://explorer.pocol.net/api/geo-report.php", "database": "/etc/pocol-geo-reporter/geoip-country.bin"}.
Credential (secret, a systemd credential `geo-reporter`, read from $CREDENTIALS_DIRECTORY): {"reporter": "founder-1",
"key": "<hex>", "salt": "<hex>"}: the key is the one the explorer gateway holds for that reporter and the salt is the
gateway's network salt (so the same peer seen by several servers has one id). Standard library only.
"""
import argparse
import hashlib
import hmac
import http.client
import json
import os
from pathlib import Path
import re
import ssl
import sys
import time
from urllib.parse import urlsplit

HERE = Path(__file__).resolve().parent
# `python3 -I` does not put the script's folder on sys.path: the installed pocol_geoip.py sits beside this file, and
# in the repository it is deploy/rewards/pocol_geoip.py.
for folder in (HERE, HERE.parent / 'rewards'):
    if (folder / 'pocol_geoip.py').is_file():
        sys.path.insert(0, str(folder))
        break
import pocol_geoip  # noqa: E402

RPC_HOST, RPC_PORT = '127.0.0.1', 8545
RPC_TIMEOUT, POST_TIMEOUT = 5, 10
MAX_RPC_REPLY = 4 << 20
MAX_CONFIG = 4096
CREDENTIAL_NAME = 'geo-reporter'
EXPLORER_URL = re.compile(r'https://[a-z0-9](?:[a-z0-9.-]{0,251}[a-z0-9])?(?::[1-9][0-9]{0,4})?/api/geo-report\.php')
USER_AGENT = 'pocol-geo-reporter/1'
OK, SETUP, NODE, REFUSED, UNREACHABLE = 0, 1, 2, 3, 4


class Failure(Exception):
    def __init__(self, code, message):
        super().__init__(message)
        self.code = code


def _strict_json(raw):
    return json.loads(raw.decode('utf-8'), object_pairs_hook=pocol_geoip._unique_object, parse_constant=pocol_geoip._no_constant)


def load_config(path):
    """The validated configuration: chainId, genesisHash (lower case), explorerUrl (https, the report route) and the
    database path."""
    try:
        with open(path, 'rb') as stream:
            raw = stream.read(MAX_CONFIG + 1)
        if len(raw) > MAX_CONFIG:
            raise ValueError('larger than 4096 bytes')
        value = _strict_json(raw)
    except (OSError, ValueError, UnicodeDecodeError) as error:
        raise Failure(SETUP, f'configuration {path}: {error}') from None
    if not isinstance(value, dict) or set(value) != {'chainId', 'genesisHash', 'explorerUrl', 'database'}:
        raise Failure(SETUP, 'configuration: expected exactly chainId, genesisHash, explorerUrl and database')
    try:
        chain, genesis = pocol_geoip.network_identity(value)
    except ValueError:
        raise Failure(SETUP, 'configuration: chainId and genesisHash do not name a network') from None
    if value['genesisHash'] != genesis:
        raise Failure(SETUP, 'configuration: genesisHash must be lower case')
    url = value['explorerUrl']
    if not isinstance(url, str) or EXPLORER_URL.fullmatch(url) is None:
        raise Failure(SETUP, 'configuration: explorerUrl must be https://HOST[:PORT]/api/geo-report.php')
    database = value['database']
    if not isinstance(database, str) or not (database.startswith('/') or Path(database).is_absolute()) or len(database) > 512:
        raise Failure(SETUP, 'configuration: database must be an absolute path')
    return {'chainId': chain, 'genesisHash': genesis, 'explorerUrl': url, 'database': value['database']}


def load_credential(path=None):
    """(reporter name, key bytes, salt bytes) from the credential file; the message of a failure never shows a secret."""
    if path is None:
        directory = os.environ.get('CREDENTIALS_DIRECTORY')
        if not directory:
            raise Failure(SETUP, 'no credential: $CREDENTIALS_DIRECTORY is not set (LoadCredential=geo-reporter:...)')
        path = Path(directory) / CREDENTIAL_NAME
    try:
        with open(path, 'rb') as stream:
            raw = stream.read(MAX_CONFIG + 1)
        value = _strict_json(raw) if len(raw) <= MAX_CONFIG else None
    except (OSError, ValueError, UnicodeDecodeError):
        raise Failure(SETUP, 'the reporter credential cannot be read as JSON') from None
    if not isinstance(value, dict) or set(value) != {'reporter', 'key', 'salt'}:
        raise Failure(SETUP, 'the reporter credential must hold exactly reporter, key and salt')
    name, key, salt = value['reporter'], value['key'], value['salt']
    if not isinstance(name, str) or pocol_geoip.REPORTER_NAME.fullmatch(name) is None:
        raise Failure(SETUP, 'the reporter credential names no valid reporter')
    for field, secret in (('key', key), ('salt', salt)):
        if not isinstance(secret, str) or pocol_geoip.SECRET_HEX.fullmatch(secret) is None:
            raise Failure(SETUP, f'the reporter credential {field} must be 32 to 64 bytes of lower-case hex')
    if key == salt:
        raise Failure(SETUP, 'the reporter credential key and salt must differ')
    return name, bytes.fromhex(key), bytes.fromhex(salt)


def read_node(timeout=RPC_TIMEOUT):
    """(pocol_status result, pocol_getParams result) of the node on 127.0.0.1:8545, in one batch request."""
    batch = [{'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_status', 'params': []},
             {'jsonrpc': '2.0', 'id': 2, 'method': 'pocol_getParams', 'params': []}]
    conn = http.client.HTTPConnection(RPC_HOST, RPC_PORT, timeout=timeout)
    try:
        conn.request('POST', '/', json.dumps(batch), {'Content-Type': 'application/json', 'Connection': 'close'})
        reply = conn.getresponse()
        raw = reply.read(MAX_RPC_REPLY + 1)
        if reply.status != 200 or len(raw) > MAX_RPC_REPLY:
            raise Failure(NODE, f'the node RPC answered HTTP {reply.status}')
        answers = json.loads(raw)
    except (OSError, ValueError, http.client.HTTPException) as error:
        raise Failure(NODE, f'the node RPC on {RPC_HOST}:{RPC_PORT} cannot be read ({type(error).__name__})') from None
    finally:
        conn.close()
    by_id = {a.get('id'): a for a in answers if isinstance(a, dict)} if isinstance(answers, list) else {}
    results = [by_id.get(i, {}).get('result') for i in (1, 2)]
    if not all(isinstance(r, dict) for r in results):
        raise Failure(NODE, 'the node did not answer pocol_status and pocol_getParams')
    return results[0], results[1]


def build_report(status, params, config, credential, database, now):
    """(body bytes, signature hex, report object) of one report; Failure(NODE) when the node is not the configured
    network or its answer has another shape. The report carries keyed ids and countries only, never an IP."""
    name, key, salt = credential
    chain, genesis = config['chainId'], config['genesisHash']
    for answer in (params, status):
        if type(answer.get('chainId')) is not int or answer.get('chainId') != chain \
                or str(answer.get('genesisHash', '')).lower() != genesis:
            raise Failure(NODE, f'the node does not serve the configured network (chain {chain})')
    head = status.get('head')
    if not isinstance(head, dict) or type(head.get('height')) is not int or head['height'] < 0 \
            or not isinstance(head.get('hash'), str) or pocol_geoip.HASH.fullmatch(head['hash'].lower()) is None:
        raise Failure(NODE, 'pocol_status has no head height and hash')
    miner = status.get('miner')
    if miner is not None and (not isinstance(miner, str) or pocol_geoip.ADDRESS.fullmatch(miner) is None):
        raise Failure(NODE, 'pocol_status names a miner that is not an address')
    peers = status.get('peers')
    if not isinstance(peers, list):
        raise Failure(NODE, 'pocol_status has no peer list')
    nodes, unlocated = pocol_geoip.observe_peers(peers, salt, database)
    ids = sorted(nodes)
    # The explorer takes at most MAX_PEERS peers in at most MAX_REPORT_BYTES (about 390 peers fit; pocold keeps far
    # fewer connections). Peers left out of the report are counted in `unlocated`: the explorer cannot locate them.
    listed = min(len(ids), pocol_geoip.MAX_PEERS)
    while True:
        report = {'schema': pocol_geoip.REPORT_SCHEMA, 'reporter': name, 'chainId': chain, 'genesisHash': genesis,
                  'at': int(now), 'head': {'height': head['height'], 'hash': head['hash'].lower()},
                  'miner': miner.lower() if miner else None,
                  'peers': [{'id': i, 'country': nodes[i]} for i in ids[:listed]],
                  'unlocated': min(unlocated + len(ids) - listed, pocol_geoip.MAX_OBSERVED_PEERS)}
        body = json.dumps(report, separators=(',', ':'), sort_keys=True).encode()
        if len(body) <= pocol_geoip.MAX_REPORT_BYTES:
            return body, hmac.new(key, body, hashlib.sha256).hexdigest(), report
        listed -= max(1, (len(body) - pocol_geoip.MAX_REPORT_BYTES) // 48)


def tls_context():
    """The system's trust store, certificate and host name verification required (never disabled)."""
    context = ssl.create_default_context()
    context.check_hostname = True
    context.verify_mode = ssl.CERT_REQUIRED
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    return context


def post(url, name, body, signature, timeout=POST_TIMEOUT):
    """The explorer's HTTP status for the report."""
    parts = urlsplit(url)
    if parts.scheme != 'https' or EXPLORER_URL.fullmatch(url) is None:
        raise Failure(SETUP, 'the explorer URL must be https')
    conn = http.client.HTTPSConnection(parts.hostname, parts.port or 443, timeout=timeout, context=tls_context())
    try:
        conn.request('POST', parts.path, body, {'Content-Type': 'application/json', 'X-PoCol-Reporter': name,
                                                'X-PoCol-Signature': signature, 'User-Agent': USER_AGENT,
                                                'Connection': 'close'})
        reply = conn.getresponse()
        reply.read(4096)
        return reply.status
    except (OSError, http.client.HTTPException) as error:
        raise Failure(UNREACHABLE, f'the explorer {parts.hostname} could not be reached ({type(error).__name__})') from None
    finally:
        conn.close()


def run(config_path, credential_path=None, dry_run=False, clock=time.time, out=print):
    config = load_config(config_path)
    credential = load_credential(credential_path)
    try:
        database = pocol_geoip.GeoDatabase(config['database'])
    except (OSError, ValueError) as error:
        raise Failure(SETUP, f'GeoIP database: {error}') from None
    try:
        status, params = read_node()
        body, signature, report = build_report(status, params, config, credential, database, clock())
    finally:
        database.close()
    located = sum(1 for p in report['peers'] if p['country'])
    summary = (f'reporter {credential[0]}, head {report["head"]["height"]}, miner {"yes" if report["miner"] else "none"}, '
               f'{len(report["peers"])} peers ({located} located, {report["unlocated"]} not locatable)')
    if dry_run:
        out(body.decode())
        out(f'DRY RUN: not sent; {summary}')
        return OK
    answer = post(config['explorerUrl'], credential[0], body, signature)
    if answer == 204:
        out(f'SENT: {summary}')
        return OK
    if 400 <= answer < 500 and answer != 429:
        raise Failure(REFUSED, f'the explorer refused the report: HTTP {answer} ({summary})')
    raise Failure(UNREACHABLE, f'the explorer did not accept the report: HTTP {answer}')


def main(argv=None):
    parser = argparse.ArgumentParser(description='Send one signed geography report of this node to the PoCol explorer.')
    parser.add_argument('--config', required=True, help='the reporter configuration (JSON)')
    parser.add_argument('--credential', help='the credential file (default: $CREDENTIALS_DIRECTORY/geo-reporter)')
    parser.add_argument('--dry-run', action='store_true', help='print the report it would send (keyed ids, no IP); send nothing')
    args = parser.parse_args(argv)
    try:
        return run(args.config, args.credential, args.dry_run)
    except Failure as failure:
        print(f'FAILED: {failure}', file=sys.stderr)
        return failure.code


if __name__ == '__main__':
    sys.exit(main())
