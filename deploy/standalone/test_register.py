#!/usr/bin/env python3
"""`pocol register` (Stage 4 PR A1) beside running Linux nodes: a server miner registers without the wallet.

Run on its own: the unittest suites do not run it; CI runs it in public-mainnet-e2e.yml (Linux). Five native
`pocol start` processes on a local v3 devnet with a small set (M_max 4) and short clocks (Theta_inact 3, E_win 8,
W_act 4, U 12, C_react 12, the rehearsal fill test's values): three genesis miners and two new identities A and B,
which sync but mine nothing while they hold no record. With every node running (each holds its home's lock):
- status and quote read only, and every refusal comes before anything is signed: no balance, a bond above B_reg, a
  non-loopback --rpc, a reserved reward address, a confirmation answered "no" or not at all;
- A, funded with the fee faucet's 0.002 PCOL only, registers below a full set with `register(address)` as its first
  transaction after a typed "yes"; its running node then mines with the identity (its record's lastShare advances);
  a second registration is refused;
- the set is now full; genesis miner 2 stops, and once it is removable B registers by replacement
  (`registerReplacing`), its victim drawn from pocol_getRemovable, and mines in its seat.
A run takes under a minute; its waits are bounded (about 105 minutes in the worst case, with three replacement tries).
"""
import json, os, subprocess, sys, tempfile, time
from pathlib import Path
from test_launcher import BIN, EXE, call, identity, port, rpc

PCOL = 10**18
CHAIN = 777936
OUTPUTS = []  # every `pocol register` stdout and stderr, searched for the secret keys at the end

def register(home, action, *args, answer=None, ok=True, rpc_port):
    """One `pocol register` run; its JSON lines and stderr. `answer` is what is typed (None: stdin closed); `ok` True
    requires success, False failure, None either."""
    p = subprocess.run([str(BIN / ('pocol' + EXE)), 'register', action, '--home', str(home), '--rpc', f'127.0.0.1:{rpc_port}', *map(str, args)],
                       input=answer if answer is not None else '', capture_output=True, text=True, encoding='utf-8', timeout=300)
    OUTPUTS.append(p.stdout + p.stderr)
    lines = [json.loads(line) for line in p.stdout.splitlines() if line.strip()]
    if ok and p.returncode:
        raise AssertionError(f'register {action} failed: {p.stderr}\n{p.stdout}')
    if ok is False and not p.returncode:
        raise AssertionError(f'register {action} succeeded: {p.stdout}')
    return lines, p.stderr

def wei(text):
    whole, _, frac = text.partition('.')
    return int(whole) * PCOL + int((frac + '0' * 18)[:18] or 0)

def until(what, predicate, seconds):
    end = time.time() + seconds
    while time.time() < end:
        try:
            value = what()
            if predicate(value): return value
        except (OSError, ValueError, KeyError): pass
        time.sleep(1)
    raise AssertionError(f'timed out after {seconds} s')

def run():
    with tempfile.TemporaryDirectory(prefix='pocol-register-') as tmp:
        root = Path(tmp); homes = [root / f'home{i}' for i in range(5)]; ids = [identity(h) for h in homes]
        p2p = [port() for _ in range(5)]; api = [port() for _ in range(5)]
        cfg = {'chainId': CHAIN, 'genesisTime': int(time.time()) + 5, 'registrationPerBlock': 1,
               'cp': {'M_max': 4, 'Theta_inact': 3, 'E_win': 8, 'W_act': 4, 'U': 12, 'C_react': 12},
               'miners': [{'id': a, 'reward': a} for a in ids[:3]], 'accounts': [{'address': a, 'wei': str(10**24)} for a in ids[:3]]}
        (root / 'config.json').write_text(json.dumps(cfg), encoding='utf-8')
        call('pocold', 'init-testnet', '--config', root / 'config.json', '--out', root / 'genesis')
        args = ['network', '--from', root / 'genesis', '--out', root / 'network.pocol.json']
        for p in p2p[:3]: args += ['--seed', f'127.0.0.1:{p}']
        call('pocol', *args); inv = root / 'network.pocol.json'; processes = []; logs = []
        try:
            for i in range(5):
                log = open(root / f'node{i}.log', 'w', encoding='utf-8'); logs.append(log)
                processes.append(subprocess.Popen([str(BIN / ('pocol' + EXE)), 'start', '--home', str(homes[i]), '--network', str(inv),
                                                   '--listen', f'127.0.0.1:{p2p[i]}', '--rpc', f'127.0.0.1:{api[i]}'], stdout=log, stderr=log))
            A, B = homes[3], homes[4]; a, b = ids[3], ids[4]
            until(lambda: (int(rpc(api[3], 'eth_blockNumber'), 16), int(rpc(api[3], 'net_peerCount'), 16)), lambda v: v[0] >= 3 and v[1] > 0, 300)
            until(lambda: int(rpc(api[4], 'eth_blockNumber'), 16), lambda h: h >= 3, 300)
            # Read only, beside the running node (which holds A's launcher.lock).
            (s,), _ = register(A, 'status', rpc_port=api[3])
            assert s['type'] == 'status' and s['address'].lower() == a.lower() and s['state'] == 'unregistered', s
            assert s['balance'] == '0' and s['network']['matchesHome'] and s['seats'] == {'active': 3, 'seats': 4, 'full': False, 'removable': 0, 'pendingReplacements': 0}, s
            _, e = register(A, 'quote', ok=False, rpc_port=api[3]); assert 'Insufficient available balance' in e, e
            # Fund A with the fee faucet's amount (deploy/faucet) and B with a little more, signed by genesis miner 0.
            nonce = int(rpc(api[0], 'eth_getTransactionCount', [ids[0], 'latest']), 16)
            for k, (to, value) in enumerate([(a, 2 * 10**15), (b, 10**16)]):
                raw = subprocess.run([str(BIN / ('pocol-sign' + EXE)), '--key-file', str(homes[0] / 'identity.pocol-key'), '--chain-id', str(CHAIN),
                                      '--to', to, '--value', str(value), '--nonce', str(nonce + k)], capture_output=True, text=True, check=True).stdout.strip()
                rpc(api[0], 'eth_sendRawTransaction', [raw])
            until(lambda: register(A, 'status', rpc_port=api[3])[0][0], lambda v: v['balance'] == '0.002', 300)
            until(lambda: register(B, 'status', rpc_port=api[4])[0][0], lambda v: v['balance'] == '0.01', 300)
            # Every refusal comes before anything is signed.
            for extra, expect in [(['--bond', '21'], 'at most 20'), (['--reward-address', '0x0000000000000000000000000000000000c0c001'], 'reserved system address'),
                                  (['--bond', '-1'], '--bond')]:
                _, e = register(A, 'quote', *extra, ok=False, rpc_port=api[3]); assert expect in e, (extra, e)
            p = subprocess.run([str(BIN / ('pocol' + EXE)), 'register', 'quote', '--home', str(A), '--rpc', f'0.0.0.0:{api[3]}'], capture_output=True, text=True)
            assert p.returncode and 'loopback' in p.stderr, p.stderr
            p = subprocess.run([str(BIN / ('pocol' + EXE)), 'register', 'status', '--home', str(A), '--bond', '1'], capture_output=True, text=True)
            assert p.returncode and 'unknown option --bond' in p.stderr, p.stderr
            for action in [[], ['sign'], ['--home', str(A)]]:
                p = subprocess.run([str(BIN / ('pocol' + EXE)), 'register', *action], capture_output=True, text=True)
                assert p.returncode and 'usage: pocol register status' in p.stderr, (action, p.stderr)
            for answer in ['no\n', 'YES\n', '', 'yes please\n']:
                lines, e = register(A, 'confirm', answer=answer, ok=False, rpc_port=api[3])
                assert [x['type'] for x in lines] == ['quote'] and 'nothing was signed or sent' in e, (answer, lines, e)
            assert rpc(api[3], 'eth_getTransactionCount', [a, 'pending']) == '0x0', 'nothing was sent'
            (q,), _ = register(A, 'quote', rpc_port=api[3])
            assert q['method'] == 'register' and q['gas'] == 400000 and q['bond'] == '0' and q['probation'] is True and q['reward'].lower() == a.lower(), q
            assert q['chainId'] == CHAIN and q['nonce'] == 0 and q['maxPriorityFeePerGas'] == str(10**9), q
            assert q['maximumFee'] == q['maximumTotal'] and wei(q['maximumFee']) == 400000 * int(q['maxFeePerGas']), q
            # A reward account the registry would not pay yet (never sent a transaction) is refused; a durable one is quoted.
            fresh = identity(root / 'fresh')
            _, e = register(A, 'quote', '--reward-address', fresh, ok=False, rpc_port=api[3]); assert 'cannot receive registry rewards yet' in e, e
            (r,), _ = register(A, 'quote', '--reward-address', ids[0], rpc_port=api[3]); assert r['reward'].lower() == ids[0].lower(), r
            # Register: A's first transaction, after a typed yes, while A's node runs.
            lines, _ = register(A, 'confirm', answer='yes\n', rpc_port=api[3])
            assert [x['type'] for x in lines] == ['quote', 'sent', 'receipt'], lines
            receipt = lines[2]; assert receipt['status'] == 'included' and receipt['registered'] and receipt['miningEligible'], receipt
            _, e = register(A, 'confirm', answer='yes\n', ok=False, rpc_port=api[3]); assert 'already has a registry record' in e, e
            (s,), _ = register(A, 'status', rpc_port=api[3])
            assert s['state'] == 'eligible' and s['record']['bondStateName'] == 'ACCRUING' and s['seats']['full'], s
            # The node that was already running with A's identity mines with it: no restart.
            mined = until(lambda: register(A, 'status', rpc_port=api[3])[0][0], lambda v: v['record']['lastShare'] > v['record']['registeredAt'], 900)
            removable = rpc(api[4], 'pocol_getRemovable')
            assert removable['full'] and removable['seats'] == 4 and removable['pendingVictims'] == [], removable
            # A full set: genesis miner 2 stops, and once it is removable B takes its seat.
            processes[2].terminate(); processes[2].wait(timeout=30)
            listed = until(lambda: rpc(api[4], 'pocol_getRemovable'), lambda v: ids[2].lower() in [x.lower() for x in v['removable']], 600)
            # The registry judges the drawn member again at inclusion. With Theta_inact 3 a running member can miss three
            # blocks of credited shares, be drawn, and mine again before the replacement lands: the transaction then
            # reverts (NotRemovable), the fee is spent, and the operator runs confirm again, as here (at most 3 tries).
            retries = 0
            for attempt in range(3):
                lines, e = register(B, 'confirm', answer='yes\n', ok=None, rpc_port=api[4])
                # The same race can surface before signing: the registry's removalStatus view already refuses the drawn
                # member (it was credited meanwhile), or no member is removable for the moment. Nothing is sent then.
                if not lines and ('refused the replacement' in e or 'none can be replaced yet' in e or 'pending replacement' in e):
                    retries += 1
                    until(lambda: rpc(api[4], 'pocol_getRemovable'), lambda v: ids[2].lower() in [x.lower() for x in v['removable']], 600)
                    continue
                assert [x['type'] for x in lines] == ['quote', 'sent', 'receipt'], (lines, e)
                q, receipt = lines[0], lines[2]
                if receipt['status'] == 'included': break
                assert receipt['status'] == 'reverted' and 'the registration reverted' in e and not receipt['registered'], (receipt, e)
                retries += 1
                until(lambda: rpc(api[4], 'pocol_getRemovable'), lambda v: ids[2].lower() in [x.lower() for x in v['removable']], 600)
            else:
                raise AssertionError(f'no replacement was included in 3 tries ({retries} retried)')
            assert q['method'] == 'registerReplacing' and q['gas'] == 500000 and q['reward'].lower() == b.lower(), q
            # The victim is drawn among the members removable at the quote (genesis miner 2 at least; another member that
            # missed Theta_inact blocks of credited shares can be drawn too, and is then the one replaced).
            assert q['replacing']['victim'].lower() in [x.lower() for x in ids[:4]], q
            assert receipt['status'] == 'included' and receipt['miningEligible'], receipt
            victim = q['replacing']['victim'].lower()
            record = [m for m in rpc(api[4], 'pocol_getMiners') if m['id'].lower() == victim][0]
            assert record['status'] != 1, ('the replaced member left the active set', record)
            replaced = until(lambda: register(B, 'status', rpc_port=api[4])[0][0], lambda v: v['record']['lastShare'] > v['record']['registeredAt'], 900)
            # No secret key reaches any output: the key files hold the 32 raw secret bytes.
            text = '\n'.join(OUTPUTS).lower()
            for home in (A, B):
                secret = (home / 'identity.pocol-key').read_bytes()
                assert len(secret) == 32 and secret.hex() not in text, 'a secret key appears in pocol register output'
            print(json.dumps({'result': 'PASS', 'scope': 'five local native pocol start processes on a v3 devnet (M_max 4); pocol register status, quote and confirm beside the running nodes: refusals, a 0.002 PCOL first-transaction register, mining without restart, a full-set registerReplacing',
                              'physicalDevicesTested': False, 'replacedAtBlock': receipt['block'], 'aLastShare': mined['record']['lastShare'],
                              'victim': victim, 'replacementRetries': retries, 'bLastShare': replaced['record']['lastShare']}))
        except BaseException:
            for i in (4, 3, 0):
                try: print('registry at node', i, json.dumps({'head': rpc(api[i], 'eth_blockNumber'), 'removable': rpc(api[i], 'pocol_getRemovable'), 'miners': rpc(api[i], 'pocol_getMiners')})); break
                except Exception: pass
            for log in logs: log.flush()
            for path in root.glob('*.log'): print(path.name, path.read_text(encoding='utf-8', errors='replace')[-2500:])
            raise
        finally:
            for p in processes:
                if p.poll() is None: p.terminate()
            for p in processes:
                try: p.wait(timeout=12)
                except subprocess.TimeoutExpired: p.kill(); p.wait()
            for log in logs: log.close()
if __name__ == '__main__': run()
