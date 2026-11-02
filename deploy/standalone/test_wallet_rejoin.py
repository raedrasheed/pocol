#!/usr/bin/env python3
"""Real native wallet: an idle registered member rejoins mining after one confirmation.

Opt-in and slow (about 15 minutes; T_blk=10 s and W_act=32 blocks are fixed by the testnet
profile). Three genesis miners run as `pocol start`; a fourth home registers through the wallet
pipes, keeps mining off until it is outside the active set, then confirms one rejoin quote. The
wallet must exit, reactivate inside the Registry window by itself, turn mining on, and mine.
"""
import json, subprocess, tempfile, time
from pathlib import Path
from test_launcher import BIN, EXE, call, identity, port
from test_wallet import Wallet

def status(w, predicate, seconds):
    return w.until(lambda v: v.get('type') == 'status' and predicate(v), seconds)

def run():
    with tempfile.TemporaryDirectory(prefix='pocol-rejoin-') as tmp:
        root = Path(tmp); homes = [root / f'home{i}' for i in range(4)]; ids = [identity(h) for h in homes]
        p2p = [port() for _ in range(3)]; api = [port() for _ in range(3)]
        cfg = {'continuous': True, 'chainId': 777933, 'genesisTime': int(time.time()) + 5, 'registrationPerBlock': 1,
               'miners': [{'id': a, 'reward': a} for a in ids[:3]], 'accounts': [{'address': a, 'wei': str(10**24)} for a in ids]}
        (root / 'config.json').write_text(json.dumps(cfg), encoding='utf-8')
        call('pocold', 'init-testnet', '--config', root / 'config.json', '--out', root / 'genesis')
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
            assert s['registryStatus'] == 'none' and not s['rejoinAvailable'], s
            e = w.request({'method': 'quote_rejoin'}, 'error'); assert 'Register first' in e['message'], e
            # PAY-02: the reward address (this identity) must have nonce > 0 before the registration.
            # The wallet refuses the review instead of letting it revert, so send one transfer first.
            e = w.request({'method': 'quote_register'}, 'error'); assert 'Send one transfer first' in e['message'], e
            q = w.request({'method': 'quote_send', 'to': ids[0], 'amount': '1'}, 'quote')
            sent = w.request({'method': 'confirm', 'quote_id': q['quoteId']}, 'submitted')
            status(w, lambda v: any(x['hash'] == sent['hash'] and x['state'] == 'Included' for x in v['activity']), 300)
            q = w.request({'method': 'quote_register'}, 'quote'); w.request({'method': 'confirm', 'quote_id': q['quoteId']}, 'submitted')
            s = status(w, lambda v: v['miningEligible'] and v['miningActive'], 300)
            registered = s['height']
            assert s['registryStatus'] == 'member' and s['rejoin']['step'] == 'active' and not s['rejoinAvailable'], s
            e = w.request({'method': 'quote_rejoin'}, 'error'); assert 'already in the active' in e['message'], e
            # Mining stays off: after W_act blocks of grace without shares the account has no nonce range.
            s = status(w, lambda v: v['rejoinAvailable'], 900)
            assert s['rejoin']['step'] == 'exit' and s['miningActive'] is False and s['miningEligible'], s
            assert 'Rejoin mining' in s['miningState'], s
            q = w.request({'method': 'quote_rejoin'}, 'quote')
            assert q['kind'] == 'rejoin' and q['amount'] == '0' and q['transactions'] == 4 and len(q['steps']) == 4, q
            w.request({'method': 'cancel'}, 'cancelled')
            e = w.request({'method': 'confirm', 'quote_id': q['quoteId']}, 'error'); assert 'first' in e['message'], e
            q = w.request({'method': 'quote_rejoin'}, 'quote')
            exit_tx = w.request({'method': 'confirm', 'quote_id': q['quoteId']}, 'submitted')
            e = w.request({'method': 'quote_rejoin'}, 'error'); assert 'in progress' in e['message'], e
            s = status(w, lambda v: v['registryStatus'] == 'exited', 300)
            assert s['rejoin']['step'] == 'wait' and s['rejoin']['inProgress'] and 'Rejoining' in s['miningState'], s
            opens = s['rejoin']['opensAt']
            done = w.until(lambda v: v.get('type') == 'rejoin' and v['state'] in ('Rejoined', 'Stopped'), 1200)
            assert done['state'] == 'Rejoined', done
            s = status(w, lambda v: v['miningActive'] and v['miningEnabled'], 60)
            assert s['registryStatus'] == 'member' and s['height'] >= opens and not s['rejoin']['inProgress'], s
            kinds = {x['kind']: x for x in s['activity']}
            assert kinds['rejoin']['hash'] == exit_tx['hash'] and kinds['rejoin']['state'] == 'Included', kinds
            assert kinds['reactivation']['state'] == 'Included', kinds
            # 'member' needs a HELD bond: the record was never swept, so the bond was not refunded and re-paid.
            s = status(w, lambda v: v['blocksMined'] > 0, 600)
            print(json.dumps({'result': 'PASS', 'scope': 'four local native processes; real wallet registration, inactivity, rejoin (exit + reactivate) and mining',
                              'physicalDevicesTested': False, 'registeredAt': registered, 'reactivationOpenedAt': opens,
                              'exit': exit_tx['hash'], 'reactivation': kinds['reactivation']['hash'], 'height': s['height'], 'blocksMined': s['blocksMined']}))
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
