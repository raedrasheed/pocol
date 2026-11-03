#!/usr/bin/env python3
"""Real native wallet on an earn-in network: a new miner with only a fee's worth of PCOL joins.

Opt-in and slow (about 10 minutes; T_blk=10 s is fixed by the testnet profile). Three genesis
miners run as `pocol start`; a fourth home holds nothing and has never
sent a transaction. Through the wallet pipes it must:
- get its first fee from the fee faucet (deploy/faucet, the real service, signer and node);
- register with a 0 PCOL bond as its first transaction (EARN-IN-SPEC R9: no PAY-02 probe for a self
  reward), so the record is on probation;
- mine on probation, and see its bond grow from its own rewards (accrual through RewardEscrow);
- fill the rest with one top-up, after which it is a full member.
"""
import http.client, json, os, subprocess, sys, tempfile, time
from pathlib import Path
from test_launcher import BIN, EXE, call, identity, port, rpc
from test_wallet import Wallet

PCOL = 10**18

def status(w, predicate, seconds):
    return w.until(lambda v: v.get('type') == 'status' and predicate(v), seconds)

def wei(text):
    whole, _, frac = text.partition('.')
    return int(whole) * PCOL + int((frac + '0' * 18)[:18] or 0)

def run():
    with tempfile.TemporaryDirectory(prefix='pocol-earnin-') as tmp:
        root = Path(tmp); homes = [root / f'home{i}' for i in range(4)]; ids = [identity(h) for h in homes]
        p2p = [port() for _ in range(3)]; api = [port() for _ in range(3)]
        accounts = [{'address': a, 'wei': str(10**24)} for a in ids[:3]]
        cfg = {'continuous': True, 'earnin': True, 'chainId': 777934, 'genesisTime': int(time.time()) + 5, 'registrationPerBlock': 1,
               'miners': [{'id': a, 'reward': a} for a in ids[:3]], 'accounts': accounts}
        (root / 'config.json').write_text(json.dumps(cfg), encoding='utf-8')
        call('pocold', 'init-testnet', '--config', root / 'config.json', '--out', root / 'genesis')
        assert json.loads((root / 'genesis/devnet.json').read_text())['earnin'] is True
        args = ['network', '--from', root / 'genesis', '--out', root / 'network.pocol.json']
        for p in p2p: args += ['--seed', f'127.0.0.1:{p}']
        call('pocol', *args); inv = root / 'network.pocol.json'; processes = []; logs = []; w = None
        try:
            for i in range(3):
                log = open(root / f'node{i}.log', 'w', encoding='utf-8'); logs.append(log)
                processes.append(subprocess.Popen([str(BIN / ('pocol' + EXE)), 'start', '--home', str(homes[i]), '--network', str(inv),
                                                   '--listen', f'127.0.0.1:{p2p[i]}', '--rpc', f'127.0.0.1:{api[i]}'], stdout=log, stderr=log))
            log = open(root / 'wallet.log', 'w', encoding='utf-8'); logs.append(log); w = Wallet(homes[3], inv, log)
            s = status(w, lambda v: v['height'] >= 3 and v['peers'] > 0, 240)
            assert s['earnIn'] is True and s['bondTarget'] == '20' and s['registryStatus'] == 'none', s
            assert s['balance'] == '0' and not s['topUpAvailable'], s
            e = w.request({'method': 'quote_register'}, 'error'); assert 'Insufficient' in e['message'], e
            # The fee faucet, funded by genesis miner 1's moved balance, signs with its key file.
            fcfg = root / 'faucet.json'; fport = port()
            fcfg.write_text(json.dumps({'chainId': 777934, 'keyFile': str(homes[1] / 'identity.pocol-key'), 'ledger': str(root / 'faucet-ledger.json'),
                                        'rpc': f'http://127.0.0.1:{api[1]}', 'port': fport, 'signer': str(BIN / ('pocol-sign' + EXE))}))
            flog = open(root / 'faucet.log', 'w', encoding='utf-8'); logs.append(flog)
            processes.append(subprocess.Popen([sys.executable, str(Path(__file__).resolve().parents[1] / 'faucet/pocol_faucet.py'), '--config', str(fcfg)], stdout=flog, stderr=flog))
            def claim(address):
                for _ in range(50):
                    try:
                        c = http.client.HTTPConnection('127.0.0.1', fport, timeout=10); c.request('POST', '/claim', json.dumps({'address': address}), {'Content-Type': 'application/json'})
                        r = c.getresponse(); return r.status, json.loads(r.read())
                    except ConnectionRefusedError: time.sleep(0.2)
                raise AssertionError('faucet did not start')
            code, gift = claim(ids[3]); assert code == 200 and gift['amountWei'] == str(2 * 10**15), gift
            assert claim(ids[3])[0] == 409 and claim(ids[0])[0] == 409, 'once per address, new accounts only'
            s = status(w, lambda v: v['balance'] == '0.002', 300)
            e = w.request({'method': 'quote_top_up'}, 'error'); assert 'probation record' in e['message'], e
            e = w.request({'method': 'quote_register', 'bond': '21'}, 'error'); assert 'at most 20' in e['message'], e
            # The first transaction of this identity: no prior transfer is needed on the earn-in network.
            q = w.request({'method': 'quote_register'}, 'quote')
            assert q['kind'] == 'registration' and q['amount'] == '0' and q['probation'] is True and q['bondTarget'] == '20', q
            reg = w.request({'method': 'confirm', 'quote_id': q['quoteId']}, 'submitted')
            s = status(w, lambda v: v['registryStatus'] == 'probation', 300)
            registered = s['height']
            assert any(x['hash'] == reg['hash'] and x['state'] == 'Included' for x in s['activity']), s['activity']
            assert s['bondHeld'] == '0' and s['miningEligible'] and s['topUpAvailable'], s
            assert s['probationEndsAt'] > registered and 'Probation' in s['miningState'], s
            miners = rpc(api[0], 'pocol_getMiners')
            mine = [m for m in (miners if isinstance(miners, list) else miners.get('miners', [])) if m.get('id', '').lower() == ids[3].lower()]
            assert mine and mine[0]['bond'] in ('0', '0x0', 0) and mine[0]['registeredAt'], miners
            e = w.request({'method': 'quote_register'}, 'error'); assert 'already has' in e['message'], e
            # Mining on probation: rewards fill the bond before anything is paid out.
            s = w.request({'method': 'mining', 'enabled': True}, 'mining'); assert s['miningEnabled'], s
            s = status(w, lambda v: v['miningActive'] and v['bondHeld'] not in (None, '0'), 900)
            accrued = wei(s['bondHeld']); assert 0 < accrued < 20 * PCOL and s['registryStatus'] == 'probation', s
            assert wei(s['balance']) < 2 * 10**15, ('an accruing record is paid nothing until its bond is full', s)
            # Fund the rest from a genesis miner (signed with its key file, relayed over RPC), then top up.
            e = w.request({'method': 'quote_top_up', 'amount': '25'}, 'error'); assert 'remains' in e['message'], e
            nonce = int(rpc(api[0], 'eth_getTransactionCount', [ids[0], 'latest']), 16)
            raw = subprocess.run([str(BIN / ('pocol-sign' + EXE)), '--key-file', str(homes[0] / 'identity.pocol-key'), '--chain-id', '777934',
                                  '--to', ids[3], '--value', str(20 * PCOL), '--nonce', str(nonce)], capture_output=True, text=True, check=True).stdout.strip()
            funded = rpc(api[0], 'eth_sendRawTransaction', [raw])
            s = status(w, lambda v: wei(v['balance']) >= 20 * PCOL, 300)
            q = w.request({'method': 'quote_top_up'}, 'quote')
            remaining = wei(q['amount']); assert q['kind'] == 'topup' and 0 < remaining <= 20 * PCOL - accrued, (q, accrued)
            held = wei(status(w, lambda v: True, 30)['balance'])
            top = w.request({'method': 'confirm', 'quote_id': q['quoteId']}, 'submitted')
            s = status(w, lambda v: v['registryStatus'] == 'member', 300)
            assert s['bondHeld'] == '20' and not s['topUpAvailable'] and 'Probation' not in s['miningState'], s
            # Rewards accrued after the quote shrink what the top-up keeps; the rest comes back (U2).
            assert held - wei(s['balance']) <= remaining + PCOL // 100, (held, s['balance'], remaining)
            kinds = {x['kind']: x for x in s['activity']}
            assert kinds['topup']['hash'] == top['hash'] and kinds['topup']['state'] == 'Included', kinds
            assert kinds['registration']['hash'] == reg['hash'], kinds
            e = w.request({'method': 'quote_top_up'}, 'error'); assert 'probation record' in e['message'], e
            s = status(w, lambda v: v['blocksMined'] > 0 and v['miningActive'], 600)
            print(json.dumps({'result': 'PASS', 'scope': 'four local native processes on an earn-in genesis; fee faucet, real wallet zero-bond first-transaction registration, probation mining with accrual, top-up promotion',
                              'physicalDevicesTested': False, 'registeredAt': registered, 'accruedBeforeTopUpWei': str(accrued),
                              'faucet': gift['hash'], 'registration': reg['hash'], 'funding': funded, 'topUp': top['hash'], 'topUpWei': str(remaining),
                              'height': s['height'], 'blocksMined': s['blocksMined']}))
        except BaseException:
            for log in logs: log.flush()
            for path in root.glob('*.log'): print(path.name, path.read_text(encoding='utf-8', errors='replace')[-2500:])
            raise
        finally:
            if w: w.close()
            for p in processes:
                if p.poll() is None: p.terminate()
            for p in processes:
                try: p.wait(timeout=12)
                except subprocess.TimeoutExpired: p.kill(); p.wait()
            for log in logs: log.close()
if __name__ == '__main__': run()
