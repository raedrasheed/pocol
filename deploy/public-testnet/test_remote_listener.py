#!/usr/bin/env python3
"""Handshake through this host's non-loopback IP, without contacting another host.

Required CI control for the former loopback-source filter; not multi-host acceptance.
"""
import argparse
import ipaddress
import json
from pathlib import Path
import socket
import subprocess
import tempfile
import time
from monitor import rpc
from node_operator import LOCAL_ARGS


def rlp(data):
    if len(data) == 1 and data[0] < 128: return data
    if len(data) < 56: return bytes([128+len(data)]) + data
    size = len(data).to_bytes((len(data).bit_length()+7)//8,'big')
    return bytes([183+len(size)]) + size + data


def hello(info, build):
    integer = lambda n: n.to_bytes((n.bit_length()+7)//8,'big')
    genesis = bytes.fromhex(info['genesisHash'][2:])
    payload = b''.join(rlp(x) for x in [b'', integer(info['chainId']), genesis, genesis, b'', bytes(32),
        b'non-loopback-control', integer(build['protocolVersion']), bytes.fromhex(build['compatibilitySha256'])])
    size = len(payload).to_bytes((len(payload).bit_length()+7)//8,'big')
    encoded = bytes([247+len(size)]) + size + payload
    return len(encoded).to_bytes(4,'big') + encoded


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--binary', type=Path, required=True); p.add_argument('--network', type=Path, required=True)
    a = p.parse_args()
    interfaces = json.loads(subprocess.check_output(['ip','-j','-4','address','show'], text=True))
    addresses = [e['local'] for i in interfaces for e in i.get('addr_info',[]) if not ipaddress.ip_address(e['local']).is_loopback]
    if not addresses: raise RuntimeError('Required test needs a non-loopback IPv4 interface')
    info = json.loads((a.network / 'devnet.json').read_text())
    build = json.loads(subprocess.check_output([str(a.binary), 'build-info'], text=True))
    held = [socket.socket(), socket.socket()]
    held[0].bind(('0.0.0.0',0)); held[1].bind(('127.0.0.1',0))
    port, rpc_port = [s.getsockname()[1] for s in held]
    for s in held: s.close()
    with tempfile.TemporaryDirectory(prefix='pocol-interface-') as temp:
        log = Path(temp) / 'node.log'
        with log.open('wb') as stream:
            proc = subprocess.Popen([str(a.binary),'run','--network',str(a.network),'--data',temp+'/data',
                '--name','interface-test','--listen',f'0.0.0.0:{port}','--rpc',f'127.0.0.1:{rpc_port}',
                '--state-store'] + LOCAL_ARGS, stdout=stream, stderr=subprocess.STDOUT)
        try:
            url = f'http://127.0.0.1:{rpc_port}'; until = time.monotonic()+30
            while True:
                if proc.poll() is not None: raise RuntimeError(log.read_text())
                try: rpc(url,'pocol_status'); break
                except OSError: pass
                if time.monotonic() > until: raise RuntimeError('Node startup timeout')
                time.sleep(.1)
            with socket.create_connection((addresses[0],port),timeout=5) as peer:
                assert not ipaddress.ip_address(peer.getsockname()[0]).is_loopback
                peer.sendall(hello(info,build)); until = time.monotonic()+5
                while rpc(url,'pocol_status')['compatiblePeers'] != 1:
                    if time.monotonic() > until: raise AssertionError('Non-loopback peer was not admitted')
                    time.sleep(.05)
                print(json.dumps({'result':'PASS','scope':'one-host non-loopback TCP interface',
                    'compatiblePeers':1,'protocolVersion':build['protocolVersion'],'multiHostProven':False}))
        finally:
            proc.terminate()
            try: proc.wait(timeout=10)
            except subprocess.TimeoutExpired: proc.kill(); proc.wait(timeout=5)


if __name__ == '__main__': main()
