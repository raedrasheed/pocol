#!/usr/bin/env python3
"""Real native wallet pipes + P2P: no mock balances, signing, mining or registration."""
import json, queue, subprocess, tempfile, threading, time
from pathlib import Path
from test_launcher import BIN, EXE, call, identity, port, rpc

class Wallet:
    def __init__(self, home, invitation, log):
        self.q=queue.Queue(); self.snapshot={}
        self.p=subprocess.Popen([str(BIN/('pocol'+EXE)),'wallet','--home',str(home),'--network',str(invitation)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,text=True,encoding='utf-8',bufsize=1)
        def reader():
            for line in self.p.stdout:
                try:self.q.put(json.loads(line))
                except ValueError:pass
        threading.Thread(target=reader,daemon=True).start()
    def until(self,predicate,seconds=240):
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            if self.p.poll() is not None:raise AssertionError('wallet exited')
            try:v=self.q.get(timeout=1)
            except queue.Empty:continue
            if v.get('type')=='status':self.snapshot=v
            if predicate(v):return v
        raise AssertionError('wallet response timeout: '+str(self.snapshot))
    def request(self,value,expected):
        while True:
            try:self.q.get_nowait()
            except queue.Empty:break
        self.p.stdin.write(json.dumps(value)+'\n');self.p.stdin.flush()
        return self.until(lambda v:v.get('type')==expected,30)
    def close(self):
        if self.p.poll() is None:
            self.p.stdin.write('{"method":"stop"}\n');self.p.stdin.flush()
            try:self.p.wait(timeout=12)
            except subprocess.TimeoutExpired:self.p.kill();self.p.wait()

def run():
    with tempfile.TemporaryDirectory(prefix='pocol-wallet-') as tmp:
        root=Path(tmp);homes=[root/f'home{i}' for i in range(5)];ids=[identity(h) for h in homes];keys=[(h/'identity.pocol-key').read_bytes() for h in homes]
        p2p=[port() for _ in range(3)];api=[port() for _ in range(3)]
        cfg={'continuous':True,'chainId':777932,'genesisTime':int(time.time())+5,'registrationPerBlock':1,'miners':[{'id':a,'reward':a} for a in ids[:3]],'accounts':[{'address':a,'wei':str(10**24)} for a in ids]}
        (root/'config.json').write_text(json.dumps(cfg),encoding='utf-8')
        call('pocold','init-testnet','--config',root/'config.json','--out',root/'genesis')
        args=['network','--from',root/'genesis','--out',root/'network.pocol.json']
        for p in p2p:args+=['--seed',f'127.0.0.1:{p}']
        call('pocol',*args);inv=root/'network.pocol.json';processes=[];wallets=[];logs=[]
        try:
            for i in range(3):
                log=open(root/f'node{i}.log','w',encoding='utf-8');logs.append(log)
                processes.append(subprocess.Popen([str(BIN/('pocol'+EXE)),'start','--home',str(homes[i]),'--network',str(inv),'--listen',f'127.0.0.1:{p2p[i]}','--rpc',f'127.0.0.1:{api[i]}'],stdout=log,stderr=log))
            for i in [3,4]:
                log=open(root/f'wallet{i}.log','w',encoding='utf-8');logs.append(log);wallets.append(Wallet(homes[i],inv,log))
            a,b=wallets
            for w in wallets:
                s=w.until(lambda v:v.get('type')=='status' and v['height']>=3 and v['peers']>0)
                assert s['lifetime']['continuous'] and s['lifetime']['finalHeight'] is None
                assert not s['miningEnabled'] and not s['miningEligible'] and s['balance']=='1000000',s
            e=a.request({'method':'mining','enabled':True},'error');assert 'eligible' in e['message']
            e=a.request({'method':'quote_send','to':ids[4],'amount':'1e2'},'error');assert 'digits' in e['message']
            q=a.request({'method':'quote_send','to':ids[4],'amount':'1.234567890123456789'},'quote')
            assert q['amount']=='1.234567890123456789' and q['to']==ids[4]
            assert b.snapshot['balance']=='1000000','quote must not send'
            e=a.request({'method':'confirm','quote_id':'wrong'},'error');assert 'expired or changed' in e['message']
            e=a.request({'method':'confirm','quote_id':q['quoteId']},'error');assert 'first' in e['message']
            q=a.request({'method':'quote_send','to':ids[4],'amount':'1.234567890123456789'},'quote')
            sent=a.request({'method':'confirm','quote_id':q['quoteId']},'submitted')
            a.request({'method':'confirm','quote_id':q['quoteId']},'error')
            received=b.until(lambda v:v.get('type')=='status' and v['balance']=='1000001.234567890123456789')
            s=a.until(lambda v:v.get('type')=='status' and any(x['hash']==sent['hash'] and x['state']=='Included' for x in v['activity']))
            q=a.request({'method':'quote_register'},'quote');assert q['kind']=='registration' and q['amount']=='100'
            a.request({'method':'confirm','quote_id':q['quoteId']},'submitted')
            a.until(lambda v:v.get('type')=='status' and v['miningEligible'])
            s=a.request({'method':'mining','enabled':True},'mining');assert s['miningEnabled']
            s=a.until(lambda v:v.get('type')=='status' and v['blocksMined']>0,360)
            s=a.request({'method':'mining','enabled':False},'mining');assert not s['miningEnabled'];mined=s['blocksMined'];height=s['height']
            # One exhausted range/absent proposer can require D_att + fallback wait (~67s).
            # Three new blocks must allow multiple honest fallback attempts, not only 100s.
            s=a.until(lambda v:v.get('type')=='status' and v['height']>=height+3,300)
            assert not s['miningEnabled'] and s['blocksMined']==mined,'mining off must leave synchronization alive without local mining'
            assert [(h/'identity.pocol-key').read_bytes() for h in homes]==keys
            a.close();wallets.remove(a)
            log=open(root/'restart.log','w',encoding='utf-8');logs.append(log);a=Wallet(homes[3],inv,log);wallets.append(a)
            s=a.until(lambda v:v.get('type')=='status' and v['height']>=height)
            assert s['address']==ids[3] and not s['miningEnabled']
            assert any(x['hash']==sent['hash'] for x in s['activity'])
            assert (homes[3]/'identity.pocol-key').read_bytes()==keys[3]
            print(json.dumps({'result':'PASS','scope':'five local native processes, continuous genesis, real wallet transfer/registration/mining/restart','physicalDevicesTested':False,'transfer':sent['hash'],'height':s['height']}))
        except BaseException:
            for log in logs:log.flush()
            for path in root.glob('*.log'):print(path.name,path.read_text(encoding='utf-8',errors='replace')[-2500:])
            raise
        finally:
            for w in wallets:w.close()
            for p in processes:
                if p.poll() is None:p.terminate()
            for p in processes:
                try:p.wait(timeout=12)
                except subprocess.TimeoutExpired:p.kill();p.wait()
            for log in logs:log.close()
if __name__=='__main__':run()
