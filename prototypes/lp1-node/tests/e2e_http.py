"""A7 process-level end-to-end check for the LP1 node binary. Python standard library only.

Root runs it after building, for example:
    python tests/e2e_http.py --bin target/debug/lp1-node.exe --fixtures fixtures

It starts the real binary from a temporary working directory on 127.0.0.1 with an ephemeral port,
reads the single JSON ready record, sends positive and negative HTTP/JSON-RPC requests over raw
sockets, checks bounded shutdown (--max-requests) and exit codes, and checks that non-loopback bind
addresses are refused. Expected header bytes are computed here from chain.json, independently of the
Rust code. Prints one JSON summary line; exit status 0 only if every check passed.
"""

import argparse
import json
import os
import socket
import subprocess
import sys
import tempfile
import time

CHECKS = []


def check(name, cond, detail=''):
    CHECKS.append({'check': name, 'ok': bool(cond), 'detail': '' if cond else str(detail)[:300]})


def rlp_list(payload):
    n = len(payload)
    if n < 56:
        return bytes([0xc0 + n]) + payload
    ln = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([0xf7 + len(ln)]) + ln + payload


def exchange(port, raw, timeout=10.0):
    s = socket.create_connection(('127.0.0.1', port), timeout=timeout)
    try:
        s.sendall(raw)
        out = b''
        while True:
            try:
                chunk = s.recv(65536)
            except (ConnectionResetError, socket.timeout):
                break
            if not chunk:
                break
            out += chunk
        return out
    finally:
        s.close()


def split(resp):
    head, _, body = resp.partition(b'\r\n\r\n')
    try:
        status = int(head.split(b' ')[1])
    except (IndexError, ValueError):
        status = 0
    return status, head.decode('latin-1'), body


def post(port, body):
    if isinstance(body, str):
        body = body.encode('utf-8')
    raw = b'POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: %d\r\n\r\n' % len(body) + body
    return split(exchange(port, raw))


def rpc(port, method, params=None, rid=1):
    req = {'jsonrpc': '2.0', 'id': rid, 'method': method}
    if params is not None:
        req['params'] = params
    status, head, body = post(port, json.dumps(req, separators=(',', ':')))
    try:
        return status, json.loads(body.decode('utf-8'))
    except ValueError:
        return status, {'unparsed': body[:200].decode('latin-1')}


def start(binary, fixtures, extra, cwd):
    args = [binary, 'serve', '--fixtures', fixtures] + extra
    p = subprocess.Popen(args, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    line = p.stdout.readline().decode('utf-8').strip()
    try:
        ready = json.loads(line)
    except ValueError:
        ready = {}
    return p, ready, line


def finish(p, timeout=30):
    try:
        rc = p.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        p.kill()
        p.wait()
        return None, ''
    rest = p.stdout.read().decode('utf-8').strip().splitlines()
    p.stderr.read()
    return rc, rest[-1] if rest else ''


def refusal(binary, fixtures, cwd, addr):
    p = subprocess.run([binary, 'serve', '--fixtures', fixtures, '--bind', addr, '--max-requests', '1'], cwd=cwd,
                       stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
    check('refuse bind %s' % addr, p.returncode == 2 and b'"ready"' not in p.stdout, (p.returncode, p.stdout[:200]))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--bin', required=True)
    ap.add_argument('--fixtures', required=True)
    a = ap.parse_args()
    binary = os.path.abspath(a.bin)
    fixtures = os.path.abspath(a.fixtures)
    cwd = tempfile.mkdtemp(prefix='lp1-e2e-')
    with open(os.path.join(fixtures, 'chain.json'), 'rb') as f:
        chain = [bytes.fromhex(h['rlp_hex']) for h in json.load(f)['headers']]
    check('chain fixture has 20 headers', len(chain) == 20, len(chain))

    for addr in ('0.0.0.0', '::', '::1', '127.0.0.2', '192.0.2.1'):
        refusal(binary, fixtures, cwd, addr)

    plan_requests = 31
    p, ready, line = start(binary, fixtures, ['--max-requests', str(plan_requests), '--max-runtime-ms', '120000',
                                              '--conn-timeout-ms', '500'], cwd)
    check('ready record', ready.get('event') == 'ready' and ready.get('addr') == '127.0.0.1' and isinstance(ready.get('port'), int)
          and ready.get('port') > 0 and ready.get('head') == 20 and ready.get('m7') is False, line)
    port = ready.get('port', 0)
    sent = 0

    def counted(fn, *args, **kw):
        nonlocal sent
        sent += 1
        return fn(*args, **kw)

    st, r = counted(rpc, port, 'eth_chainId', [])
    check('eth_chainId', st == 200 and r.get('result') == '0xbdb2a' and r.get('id') == 1, r)
    st, r = counted(rpc, port, 'eth_blockNumber')
    check('eth_blockNumber', r.get('result') == '0x14', r)
    for frm, cnt in ((1, 20), (7, 14), (20, 1)):
        st, r = counted(rpc, port, 'pocol_getHeaders', [hex(frm), hex(cnt)])
        want = '0x' + rlp_list(b''.join(chain[frm - 1:frm - 1 + cnt])).hex()
        check('getHeaders %d %d bytes' % (frm, cnt), r.get('result') == want, str(r)[:200])
    negatives = [
        (['0x0', '0x1'], -32018, {'reason': 'fromZero'}),
        (['0x1', '0x0'], -32018, {'reason': 'countRange'}),
        (['0x1', '0x201'], -32018, {'reason': 'countRange'}),
        (['0x15', '0x1'], -32018, {'reason': 'fromAboveHead'}),
        (['0x14', '0x2'], -32018, {'reason': 'beyondHead'}),
        (['0x1', '0x200'], -32018, {'reason': 'beyondHead'}),
        (['0x01', '0x1'], -32602, {'path': 'params[0]'}),
        (['0x1', '0x10000000000000000'], -32602, {'path': 'params[1]'}),
        (['0x1'], -32602, {'path': 'params'}),
        ([1, '0x1'], -32602, {'path': 'params[0]'}),
    ]
    for params, code, data in negatives:
        st, r = counted(rpc, port, 'pocol_getHeaders', params)
        e = r.get('error', {})
        check('F0-F4 %s' % params, st == 200 and e.get('code') == code and e.get('data') == data, r)
    for m in ('eth_sendRawTransaction', 'eth_sendTransaction', 'eth_accounts', 'eth_requestAccounts', 'personal_sign'):
        st, r = counted(rpc, port, m, ['0x00'])
        check('forbidden %s' % m, r.get('error', {}).get('code') == -32601, r)
    st, body = None, None
    st, h, body = counted(post, port, '{"jsonrpc":"2.0","id":7.0,"method":"eth_chainId"}')
    check('id normalised', json.loads(body).get('id') == 7, body)
    st, h, body = counted(post, port, '{"jsonrpc":"2.0","id":"7","method":"eth_chainId"}')
    check('string id refused', json.loads(body).get('error', {}).get('code') == -32600 and json.loads(body).get('id') is None, body)
    st, h, body = counted(post, port, '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":{"a":1,"a":2}}')
    check('nested duplicate key', json.loads(body).get('error', {}).get('data') == {'reason': 'duplicateKey'}, body)
    st, h, body = counted(post, port, '[' * 17 + ']' * 17)
    check('depth 17', json.loads(body).get('error', {}).get('data') == {'reason': 'depth'}, body)
    st, h, body = counted(post, port, b'"\xff"')
    check('invalid UTF-8', json.loads(body).get('error', {}).get('data') == {'reason': 'utf8'}, body)
    resp = split(counted(exchange, port, b'POST / HTTP/1.1\r\nContent-Length: 4097\r\n\r\n' + b'x' * 4097))
    check('body limit 413', resp[0] == 413, resp[1])
    resp = split(counted(exchange, port, b'POST / HTTP/1.1\r\nX-F: ' + b'a' * 9000 + b'\r\nContent-Length: 2\r\n\r\n{}'))
    check('head limit 431', resp[0] == 431, resp[1])
    resp = split(counted(exchange, port, b'POST / HTTP/1.1\r\nContent-Length: 5\r\n\r\n{}'))
    check('short body times out 408', resp[0] == 408, resp[1])
    resp = split(counted(exchange, port, b'POST / HTTP/1.1\r\nContent-Length: abc\r\n\r\n{}'))
    check('malformed length 400', resp[0] == 400, resp[1])
    # Client disconnect mid-request; the server must continue.
    sent += 1
    s = socket.create_connection(('127.0.0.1', port), timeout=5)
    s.sendall(b'POST / HTTP/1.1\r\nContent-Le')
    s.close()
    time.sleep(0.2)
    st, r = counted(rpc, port, 'eth_blockNumber', [], rid=42)
    check('recovery after disconnect', r.get('result') == '0x14' and r.get('id') == 42, r)
    check('request plan size', sent == plan_requests, sent)
    rc, last = finish(p)
    check('bounded shutdown exit 0', rc == 0, rc)
    try:
        sd = json.loads(last)
    except ValueError:
        sd = {}
    check('shutdown record', sd.get('event') == 'shutdown' and sd.get('served') == plan_requests and sd.get('reason') == 'maxRequests', last)

    p, ready, line = start(binary, fixtures, ['--empty-chain', '--max-requests', '2', '--max-runtime-ms', '60000'], cwd)
    port = ready.get('port', 0)
    check('empty ready', ready.get('head') == 0, line)
    st, r = rpc(port, 'eth_blockNumber')
    check('empty eth_blockNumber', r.get('result') == '0x0', r)
    st, r = rpc(port, 'pocol_getHeaders', ['0x1', '0x1'])
    check('empty fromAboveHead', r.get('error', {}).get('data') == {'reason': 'fromAboveHead'}, r)
    rc, last = finish(p)
    check('empty shutdown exit 0', rc == 0, rc)

    p, ready, line = start(binary, fixtures, ['--max-runtime-ms', '300'], cwd)
    rc, last = finish(p)
    check('max-runtime shutdown', rc == 0 and '"maxRuntime"' in last, last)

    failed = [c for c in CHECKS if not c['ok']]
    print(json.dumps({'e2e': 'lp1-http', 'checks': len(CHECKS), 'failed': len(failed), 'failures': failed}, ensure_ascii=False))
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
