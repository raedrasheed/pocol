#!/usr/bin/env python3
"""Check independent testnet nodes through operator-supplied HTTPS endpoints or SSH tunnels."""
import argparse
import json
import os
from pathlib import Path
import sys
import time
import urllib.parse
import urllib.request
from node_operator import read_json, write_json


def rpc(url, method, params=None):
    parsed = urllib.parse.urlsplit(url)
    local = parsed.scheme == 'http' and parsed.hostname == '127.0.0.1' and parsed.port
    if not (local or parsed.scheme == 'https') or not parsed.hostname or parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in ('', '/'):
        raise ValueError('Use HTTPS or http://127.0.0.1:PORT; no credentials, queries or alternate paths')
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *args, **kwargs): raise ValueError('RPC redirect refused')
    handlers = [NoRedirect()]
    if local: handlers.append(urllib.request.ProxyHandler({}))
    request = urllib.request.Request(url, json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params or []}).encode(), {'Content-Type': 'application/json'})
    with urllib.request.build_opener(*handlers).open(request, timeout=10) as response:
        raw = response.read(2 * 1024 * 1024 + 1)
    if len(raw) > 2 * 1024 * 1024: raise ValueError('Oversized RPC response')
    value = json.loads(raw)
    if value.get('jsonrpc') != '2.0' or type(value.get('id')) is not int or value['id'] != 1 or 'error' in value or 'result' not in value:
        raise ValueError('Invalid RPC response or RPC error')
    return value['result']


def snapshot(release, urls):
    values = {}
    for name, url in urls.items():
        s = rpc(url, 'pocol_status')
        for field in ['chainId', 'genesisHash', 'protocolVersion', 'compatibilitySha256']:
            if s[field] != release[field]: raise ValueError('Wrong ' + field + ' on ' + name)
        if s['name'] != name or not s['stateStore'] or s['admission']['scope'] != 'experimental-devnet':
            raise ValueError('Unexpected node mode or name on ' + name)
        if s['compatiblePeers'] < 1: raise ValueError('No compatible peers on ' + name)
        if s['head']['height'] >= s['lifetime']['closingHeight'] - 100:
            raise ValueError('Finite testnet approaching closing height on ' + name)
        if s['stats']['historyDamaged']: raise ValueError('Damaged history on ' + name)
        values[name] = s
    return values


def check(release, urls, observe=180):
    if not 180 <= observe <= 3600: raise ValueError('Observe 180..3600 seconds (three mining-attempt durations minimum)')
    if set(urls) != set(release['bundleManifestSha256']) or len(set(urls.values())) != len(urls):
        raise ValueError('Provide one distinct endpoint for every release node')
    before = snapshot(release, urls)
    until = time.monotonic() + observe
    while time.monotonic() < until: time.sleep(min(1, max(0, until - time.monotonic())))
    after = snapshot(release, urls)
    if any(after[n]['head']['height'] <= before[n]['head']['height'] for n in urls): raise ValueError('A node stopped advancing')
    height = min(s['head']['height'] for s in after.values()) - 6
    if height < 1: raise ValueError('Not enough confirmed blocks')
    blocks = [rpc(u, 'eth_getBlockByNumber', [hex(height), False]) for u in urls.values()]
    common = {(b['hash'], b['stateRoot'], b['receiptsRoot']) for b in blocks}
    if len(common) != 1: raise ValueError('Nodes disagree on the common block or roots')
    if any(rpc(u, 'eth_getBlockByNumber', [hex(height), False])['hash'] != blocks[0]['hash'] for u in urls.values()):
        raise ValueError('Reorganization during observation; retry a fresh sample')
    return {'result': 'PASS', 'observedSeconds': observe, 'checkedAtUnix': int(time.time()),
            'commonHeight': height, 'commonHash': blocks[0]['hash'], 'publicLaunchReady': False,
            'nodes': {n: {'height': s['head']['height'], 'compatiblePeers': s['compatiblePeers']} for n, s in after.items()}}


def metrics(result):
    lines = ['# HELP pocol_observation_success Last bounded multi-node observation succeeded.',
             '# TYPE pocol_observation_success gauge', 'pocol_observation_success 1',
             f'pocol_observation_timestamp_seconds {result["checkedAtUnix"]}',
             f'pocol_common_height {result["commonHeight"]}']
    for name, node in result['nodes'].items():
        label = json.dumps(name)
        lines += [f'pocol_node_height{{node={label}}} {node["height"]}', f'pocol_compatible_peers{{node={label}}} {node["compatiblePeers"]}']
    return '\n'.join(lines) + '\n'


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--release', type=Path, required=True); p.add_argument('--rpc', action='append', required=True)
    p.add_argument('--observe', type=int, default=180); p.add_argument('--out', type=Path, required=True)
    a = p.parse_args(); pairs = [x.split('=', 1) for x in a.rpc]; urls = dict(pairs)
    if len(urls) != len(pairs): raise ValueError('Duplicate node endpoint')
    result = check(read_json(a.release), urls, a.observe)
    # Immutable observations; a collector may select the latest .prom by timestamp.
    write_json(a.out, result)
    with a.out.with_suffix('.prom').open('x') as stream: stream.write(metrics(result)); stream.flush(); os.fsync(stream.fileno())
    print(json.dumps(result))


if __name__ == '__main__':
    try: main()
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('testnet monitoring failed: ' + str(error), file=sys.stderr); sys.exit(1)
