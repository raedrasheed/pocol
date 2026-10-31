#!/usr/bin/env python3
"""Real processes on separate loopback IPs: not a remote-host acceptance test."""
import argparse
import json
from pathlib import Path
import socket
import subprocess
import sys
import time

from private_testnet import HERE, ROOT, rpc, write_json
from runtime import digest


def available_port(ip):
    with socket.socket() as sock:
        sock.bind((ip, 0))
        return sock.getsockname()[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--work', type=Path, required=True)
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=False)
    work = args.work.resolve()
    inventory = {'name': 'private-rehearsal', 'chainId': 777911, 'nodes': []}
    used_rpc = set()
    for i in range(3):
        ip = f'127.0.0.{i + 2}'
        port = available_port('127.0.0.1')
        while port in used_rpc:
            port = available_port('127.0.0.1')
        used_rpc.add(port)
        inventory['nodes'].append({'name': f'node{i + 1}', 'ip': ip,
                                   'p2pPort': available_port(ip), 'rpcPort': port})
    write_json(work / 'inventory.json', inventory)
    release = work / 'release'
    subprocess.run([sys.executable, str(HERE / 'private_testnet.py'), 'prepare', '--rehearsal',
        '--inventory', str(work / 'inventory.json'), '--out', str(release),
        '--genesis-time', str(int(time.time()) + 5)], check=True)
    urls = {n['name']: f"http://127.0.0.1:{n['rpcPort']}" for n in inventory['nodes']}
    processes, logs = {}, []
    checks = []

    def record(name):
        checks.append(name)
        print('PASS ' + name, flush=True)

    def start(name):
        log = (work / (name + '.log')).open('ab')
        logs.append(log)
        processes[name] = subprocess.Popen([sys.executable, str(release / name / 'runtime.py'),
            'run', '--data', str(work / 'data' / name)], stdout=log, stderr=subprocess.STDOUT)

    def stop(name):
        process = processes.pop(name)
        process.terminate()
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)

    def wait(predicate, seconds, label):
        end, last = time.monotonic() + seconds, ''
        while time.monotonic() < end:
            for name, process in processes.items():
                if process.poll() is not None:
                    raise RuntimeError(name + ' exited; inspect its log')
            try:
                if predicate():
                    return
            except (OSError, ValueError, KeyError) as error:
                last = str(error)
            time.sleep(1)
        raise RuntimeError(label + ' timed out: ' + last)

    try:
        for name in urls:
            start(name)
        wait(lambda: all(rpc(url, 'pocol_status')['peers'] for url in urls.values()), 60, 'connections')
        record('three packaged nodes connect on distinct loopback addresses')
        # A second wrapper must fail before touching a live node's calibration/store.
        duplicate = subprocess.run([sys.executable, str(release / 'node1/runtime.py'), 'run',
            '--data', str(work / 'data/node1')], capture_output=True, timeout=15)
        assert duplicate.returncode != 0 and b'temporarily unavailable' in duplicate.stderr
        record('duplicate process refused by data lock')
        info = json.loads((release / 'network/devnet.json').read_text())
        bob = next(a for a in info['accounts'] if a['label'] == 'bob')
        raw = subprocess.check_output([str(release / 'node1/bin/pocol-sign'), '--chain-id', '777911',
            '--account', 'alice', '--to', bob['address'], '--value', '12345', '--nonce', '0'], text=True).strip()
        tx = rpc(urls['node1'], 'eth_sendRawTransaction', [raw])
        # A receipt on each moving tip does not imply one canonical inclusion block:
        # concurrent miners can include the transaction on transient sibling branches.
        # Keep the equality requirement and wait for a shared inclusion six blocks deep.
        def transfer_converged():
            receipts = [rpc(url, 'eth_getTransactionReceipt', [tx]) for url in urls.values()]
            if not all(receipts) or len({r['blockHash'] for r in receipts}) != 1:
                return False
            height = int(receipts[0]['blockNumber'], 16)
            return all(rpc(url, 'pocol_status')['head']['height'] >= height + 6 and
                       rpc(url, 'eth_getBlockByNumber', [hex(height), False])['hash'] == receipts[0]['blockHash']
                       for url in urls.values())
        wait(transfer_converged, 240, 'confirmed convergent transfer')
        receipts = [rpc(url, 'eth_getTransactionReceipt', [tx]) for url in urls.values()]
        assert all(r['status'] == '0x1' for r in receipts)
        assert len({r['blockHash'] for r in receipts}) == 1
        assert all(int(rpc(url, 'eth_getBalance', [bob['address'], 'latest']), 16) == int(bob['wei']) + 12345 for url in urls.values())
        record('test currency transfer included on every node with identical receipt block and balance')
        wait(lambda: min(rpc(url, 'pocol_status')['head']['height'] for url in urls.values()) >= 8, 240, 'eight blocks')
        command = [sys.executable, str(HERE / 'private_testnet.py'), 'check', '--release', str(release),
                   '--observe', '180', '--report', str(work / 'health.json')]
        for name, url in urls.items():
            command += ['--rpc', name + '=' + url]
        subprocess.run(command, check=True, timeout=300)
        record('health check verifies identity, progress and common block/state/receipt roots')
        reference = json.loads((work / 'health.json').read_text())
        height, common_hash = reference['commonHeight'], reference['commonHash']
        stop('node2')
        # Check that rebinding existing storage fails rather than replacing its chain.
        marker = work / 'data/node2/network-binding.json'
        saved = marker.read_bytes()
        altered = json.loads(saved)
        altered['chainId'] = 1
        write_json(marker, altered)
        refused = subprocess.run([sys.executable, str(release / 'node2/runtime.py'), 'run',
            '--data', str(work / 'data/node2')], capture_output=True, timeout=15)
        marker.write_bytes(saved)
        assert refused.returncode != 0 and b'different network' in refused.stderr
        record('wrong data-network binding refused')
        start('node2')
        wait(lambda: rpc(urls['node2'], 'eth_getBlockByNumber', [hex(height), False])['hash'] == common_hash,
             60, 'restart/replay')
        wait(lambda: abs(rpc(urls['node1'], 'pocol_status')['head']['height'] -
                         rpc(urls['node2'], 'pocol_status')['head']['height']) <= 1, 60, 'rejoin')
        record('restarted node retains common block and rejoins')
        report = {'result': 'PASS', 'checks': checks, 'scope': 'single-host-loopback-rehearsal',
                  'distributedDeployment': False, 'health': reference,
                  'binarySha256': digest(release / 'node1/bin/pocold'), 'testTransactionHash': tx}
        write_json(work / 'result.json', report)
    finally:
        for name in list(processes):
            stop(name)
        for log in logs:
            log.close()


if __name__ == '__main__':
    main()
