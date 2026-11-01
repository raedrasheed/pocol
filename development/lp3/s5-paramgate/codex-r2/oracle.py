"""Separate unlimited-integer reference for amended R1-R26. No Rust implementation imports."""
import json,random,subprocess,hashlib,sys
def expected(v):
 end=v.get('end',False)
 c=dict(T_blk=10 if end else 60,tau=600 if end else 3600,c=2,D_att=60 if end else 300,dFbWait=6 if end else 25,D=16,U=2200,E_win=2048,S_max=256,kappa=2,m=32,B_code_max=32768,TX_MAX=131072,BODY_MAX=196608,M_min=1,M_max=1024 if end else 256,H_END=700 if end else 129600,Z_close=147 if end else 160,H_CLOSE=553 if end else 129440,T_END=1710368000,T_CLOSE=1710358400,R_max=0 if end else 2,REG_RECORDS_MAX=1024 if end else 16384,SWEEP_MAX=16,E_max=4,RET=8 if end else 128,P2=2,SYS_SLOT_MAX=131072,SYS_KEYS_BLOCK_MAX=2048,SYS_KEYS_USER_MAX=256,SYS_ACCT_BLOCK_MAX=512,G_SYS=96000000,B_reg=100000000000000000000,subsidy=80000 if end else 2000000000000000000)
 c.update(v.get('cp',{}))
 l=dict(delta=1 if end else 5,phi=1 if end else 5,capital_phi=2 if end else 10,t_exec=1 if end else 3,delta_rel=8 if end else 26,gamma=5 if end else 30,x_h=6 if end else 22,x_w=10 if end else 36,r_keep=2048,k_hist=32,k_final=60,k_view=12,hist_trie_budget=1717986918)
 l.update(v.get('local',{}));d=l['delta'];phi=l['capital_phi'];t=l['t_exec'];f=c['dFbWait'];sigma=max(d,phi+1)+t;xh=max(d+t,phi+1)+d+2*t;xw=max(phi+1+f,d+t+l['delta_rel'])
 stale=v.get('stale',False) and any(k in v for k in ['cp','local','stress','gossip','synthetic','chain','v2','rpcwall'])
 rf=not (stale or v.get('resourceMissing') or v.get('modelBad'));identity=not(stale or v.get('identityMissing'));cal=not(stale or v.get('calMissing'))
 phases=[dict(system_keys=1496 if j==0 else 1395,user_keys=256 if j==0 else 0,system_accounts=388 if j==0 else 257,user_accounts=16 if j==0 else 0,live_slots=100000,gas=[13000000]*2,duration_ms=500,retired_records=c['RET']) for j in range(3)]
 change=v.get('phase',{});phases[change.get('index',0)].update({k:n for k,n in change.items() if k!='index'})
 coverage=rf and not v.get('omitPhase')
 sysbad=not coverage or any(p['live_slots']>c['SYS_SLOT_MAX'] or p['system_keys']+p['user_keys']>c['SYS_KEYS_BLOCK_MAX'] or p['user_keys']>c['SYS_KEYS_USER_MAX'] or p['system_accounts']+p['user_accounts']>c['SYS_ACCT_BLOCK_MAX'] or p['user_accounts']>16 or (j!=0 and (p['user_keys'] or p['user_accounts'])) or not p['gas'] or max(p['gas'])>30000000 or sum(p['gas'])>c['G_SYS'] or (j==1 and p['retired_records']<c['RET']) for j,p in enumerate(phases))
 gossip=[131072,197294,3067,43,1368,243]
 if 'gossip' in v:gossip[v['gossip'][0]]=v['gossip'][1]
 node=v.get('node',not end);kind='synthetic' if v.get('synthetic') else ('simnet' if end else 'network');chain=v.get('chain',777905 if end else 777001);members=1024 if end else 4
 lmax=max(c['U'],2*c['E_win']+c['D']+1)
 rules=[phi!=2*l['phi'],l['delta_rel']<sigma+d+2*t,f<phi+d+2*t,phi>=f or l['delta_rel']<t,3*f>c['D_att'],l['x_h']!=xh or l['x_w']!=xw,c['D_att']+l['gamma']<2*(xw+c['c']*c['T_blk'])+phi,c['D']<2 or c['U']<c['E_win']+c['D'] or c['E_win']>l['r_keep'],c['S_max']<4*c['kappa']*c['m'] or c['B_code_max']<24577,c['BODY_MAX']<c['TX_MAX']+16384,l['k_final']>l['r_keep'] or l['k_view']>=l['k_hist'] or l['k_view']+2>512,not(c['M_max']>=members>=c['M_min']>=1),c['H_END']<=c['Z_close'] or c['H_CLOSE']+c['Z_close']>c['H_END'] or c['T_CLOSE']>c['T_END'],not rf or v.get('p99',500)>t*1000,any(a<b for a,b in zip(gossip,[c['TX_MAX'],c['BODY_MAX']+686,3067,43,1368,243])),c['REG_RECORDS_MAX']<c['M_max']+c['R_max']*(lmax+2048),c['SWEEP_MAX']<c['R_max']+c['E_max'],sysbad,not coverage or any(p['duration_ms']>t*1000 for p in phases),c['Z_close']<((c['REG_RECORDS_MAX']+c['RET']-1)//c['RET']+c['P2']+c['D']+1),not rf or v.get('retained',32)<min(l['k_hist'],c['H_END']) or (v.get('hist',1000000)>l['hist_trie_budget'] and not(kind=='simnet' and not node and v.get('stress'))),(node and kind!='network') or ((kind=='network')!=(chain==777001)),not identity or v.get('supplyBad') or (10**18+members*c['B_reg']+c['subsidy']*c['H_END']>=2**127),not identity or v.get('rootBad') or v.get('v2') or chain==0,not cal or v.get('calBad') or (kind=='network' and c['tau']<10*c['T_blk']) or c['T_blk']!=(10 if end else 60),v.get('rpcwall',420000)<404280]
 violated=['R'+str(i+1) for i,x in enumerate(rules) if x]
 return dict(violations=violated,accepted=not violated,bootable=False)
rows=[]
for end in [False,True]:
 for flag in ['baseline','node','v2','synthetic','modelBad','rootBad','supplyBad','calBad','resourceMissing','identityMissing','calMissing','omitPhase']:
  rows.append({'end':end,**({flag:True} if flag!='baseline' else {})})
 for k,values in dict(U=[0,2063,2064,2200,2**32-1],E_win=[0,2048,2049],D=[0,1,2,16,65535],D_att=[0,74,75,300],dFbWait=[0,10,20,21,25],tau=[1,599,600,3600],M_min=[1,4,5,1025],H_CLOSE=[0,129584,129600],Z_close=[0,146,147,160],REG_RECORDS_MAX=[0,1024,12577,12578,16384],SWEEP_MAX=[0,5,6,16],RET=[1,8,127,128,129],S_max=[0,255,256],G_SYS=[0,26000000,95999999,96000000]).items():
  rows.extend(dict(end=end,cp={k:n}) for n in values)
 for field in ['delta','phi','capital_phi','t_exec','delta_rel','gamma','x_h','x_w','r_keep','k_hist','k_final','k_view','hist_trie_budget']:
  for n in [0,1,5,32,2**32-1]:rows.append(dict(end=end,local={field:n}))
 for flag,values in dict(p99=[0,1000,1001,3000,3001],hist=[0,1717986918,1717986919],retained=[0,31,32],rpcwall=[404000,404279,404280,405000]).items():
  rows.extend(dict(end=end,**{flag:n}) for n in values)
 for index in range(3):
  for field,values in dict(system_keys=[0,1792,1793,2049],user_keys=[0,256,257],system_accounts=[0,496,497,513],user_accounts=[0,16,17],live_slots=[0,131072,131073],duration_ms=[1000,1001,3001],retired_records=[0,7,127,128]).items():
   rows.extend(dict(end=end,phase=dict(index=index,**{field:n})) for n in values)
 rows.extend([dict(end=end,phase={'index':0,'gas':[]}),dict(end=end,phase={'index':1,'gas':[30000001]}),dict(end=end,phase={'index':2,'gas':[30000000]*4})])
rng=random.Random(0x533501)
base=rows.copy()
for _ in range(1000):
 row=dict(rng.choice(base));row.update(rng.choice([{}, {'node':True},{'p99':3001},{'rpcwall':404000}]))
 rows.append(row)
payload=''.join(json.dumps(v,sort_keys=True)+'\n' for v in rows)
p=subprocess.run([sys.argv[1]],input=payload,text=True,capture_output=True);assert p.returncode==0,p.stderr
actual=[json.loads(l) for l in p.stdout.splitlines()];assert len(actual)==len(rows)
mismatch=[dict(index=j,input=v,expected=expected(v),actual=a) for j,(v,a) in enumerate(zip(rows,actual)) if expected(v)!=a]
print(json.dumps(dict(cases=len(rows),mismatches=len(mismatch),firstMismatches=mismatch[:5],inputSha256=hashlib.sha256(payload.encode()).hexdigest(),outputSha256=hashlib.sha256(p.stdout.encode()).hexdigest())))
assert not mismatch
