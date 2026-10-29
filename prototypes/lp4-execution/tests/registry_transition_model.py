"""Separate domain-object model of approved Registry/PAY transitions.
No bytecode execution or Rust imports; encoding is the documented explicit layout.
"""
import copy
from eth_utils import keccak
REG=f'0x{0xc0c001:040x}';ESC=f'0x{0xc0c002:040x}';CAP=f'0x{0xc0c004:040x}';SYS=f'0x{2**160-2:040x}'
MASK=2**64-1
class Reject(Exception):pass
def key(n):return f'0x{n:064x}'
def address(n):return f'0x{n:040x}'
def rec(a):return int.from_bytes(keccak(bytes(12)+bytes.fromhex(a[2:])+(7).to_bytes(32,'big')),'big')
def base(n):return int.from_bytes(keccak(n.to_bytes(32,'big')),'big')
def transition(row):
    original={a.lower():copy.deepcopy(v) for a,v in row['pre'].items()};state=copy.deepcopy(original)
    caller=row['caller'].lower();height=row['height'];data=bytes(row['input']);value=row['value']
    state[caller]['nonce']+=1
    def s(n):return int(original[REG]['storage'].get(key(n),'0'),16)
    def cp(n):return int(original[CAP]['storage'].get(key(n),'0'),16)
    def fail(error):raise Reject(error)
    def checked(n):
        if n>MASK:fail('HeightOverflow()')
        return n
    def pay(source,to,amount,error):
        before=original.get(to);dest=state.get(to)
        if to==address(0) or before is None or not(before['nonce'] or before['code']!='0x') or dest is None or not(dest['nonce'] or dest['code']!='0x' or int(dest['balance'])):fail(error)
        if int(state[source]['balance'])<amount or (source!=to and int(dest['balance'])+amount>=2**256):fail(error)
        if source!=to:state[source]['balance']=str(int(state[source]['balance'])-amount);dest['balance']=str(int(dest['balance'])+amount)
    def decode(a):
        r=rec(a);w=s(r);t=s(r+3)
        return dict(id=a,status=w&255,tomb=(w>>8)&255,bondState=(w>>16)&255,recPos=(w>>24)&(2**32-1),actPos=(w>>56)&(2**32-1),lastShare=(w>>88)&MASK,reward=address(s(r+1)&(2**160-1)),bond=s(r+2),exit=t&MASK,due=(t>>64)&MASK,burn=(t>>128)&MASK,grace=t>>192,reserved=w&~(2**152-1))
    members=[decode(address(s(base(5)+i))) for i in range(s(0))];active=[address(s(base(6)+i)) for i in range(s(1))]
    byid={m['id']:m for m in members};c=s(3);meta=dict(ops=c&65535,adds=(c>>16)&255,begin=(c>>24)&MASK,initialized=bool(c&(1<<88)),closing=bool(c&(1<<89)),cursor=(c>>90)&(2**32-1),closeInit=bool(c&(1<<122)),mark=(c>>123)&MASK,closeH=(c>>187)&MASK,reserved=c&~(2**251-1));sweep=s(2);held=s(4)
    signatures=['register(address)','deregister()','removeInactive(address)','reactivate()','setRewardAddress(address)','claimBond()','markClose()','beginBlock()']
    sig=next((sig for sig in signatures if keccak(sig.encode())[:4]==data[:4]),None)
    def isheld(m):return m is not None and m['status']!=0 and m['tomb']==0 and m['bondState']==1
    def refund(m):
        nonlocal held
        if not isheld(m):fail('BondUnavailable()')
        pay(ESC,m['reward'],m['bond'],'PaymentFailed()')
        if held<m['bond']:fail('Panic(uint256)')
        held-=m['bond']
    def erase(m):
        i=members.index(m);last=members.pop()
        if i<len(members):members[i]=last
        byid.pop(m['id'])
    def deactivate(m):
        i=active.index(m['id']);last=active.pop()
        if i<len(active):active[i]=last
        m['actPos']=0;m['status']=2
    try:
        if sig in ['markClose()','beginBlock()']:
            if caller!=SYS:fail('Unauthorized()')
            if len(data)!=4:fail('BadCalldata()')
            checked(height)
            if sig=='markClose()':
                if not height or meta['mark']>=height:fail('BadPhase()')
                meta['mark']=height
                if meta['closing'] or height>=cp(47) or 0>=cp(48):
                    if not meta['closeInit']:meta.update(closing=True,closeInit=True,cursor=0,closeH=height)
                    end=min(len(members),meta['cursor']+cp(50))
                    for m in members[meta['cursor']:end]:
                        if isheld(m):refund(m);m['bondState']=3
                    meta['cursor']=end
            else:
                if not height or(meta['initialized'] and height<=meta['begin'])or meta['mark']!=height:fail('BadPhase()')
                meta.update(ops=0,adds=0,begin=height,initialized=True)
                if not meta['closing']:
                    if not members:sweep=0
                    else:
                        scanned=deleted=0
                        while members and scanned<min(cp(22),64) and deleted<min(cp(21),16):
                            sweep%=len(members);m=members[sweep];scanned+=1;remove=False
                            if isheld(m) and m['status']==2 and height>=m['due']:refund(m);remove=True
                            elif m['tomb'] and m['bondState']!=1 and height>=checked(m['burn']+cp(26)+cp(11)+1):remove=True
                            if remove:erase(m);deleted+=1
                            else:sweep+=1
                        sweep=sweep%len(members) if members else 0
        else:
            if sig is None:fail('BadCalldata()')
            hasarg=sig in ['register(address)','setRewardAddress(address)','removeInactive(address)']
            if len(data)!=(36 if hasarg else 4)or(hasarg and any(data[4:16])):fail('BadCalldata()')
            arg='0x'+data[16:36].hex() if hasarg else None
            registering=sig=='register(address)';adding=registering or sig=='reactivate()'
            if value!=(cp(16)if registering else 0):fail('BadValue()')
            if meta['closing']:fail('Closing()')
            if meta['ops']>=cp(23)or(adding and meta['adds']>=cp(17)):fail('RegOpsLimit()')
            checked(height)
            if not meta['initialized']or meta['begin']!=height:fail('BadPhase()')
            target=arg if sig=='removeInactive(address)' else caller;m=byid.get(target)
            if registering:
                if target in [address(0),SYS]or 0xc0c001<=int(target,16)<=0xc0c0ff or s(rec(target)):fail('BadState()')
                if len(members)>=cp(20)or len(active)>=cp(18):fail('Capacity()')
                state[caller]['balance']=str(int(state[caller]['balance'])-value);state[REG]['balance']=str(int(state[REG]['balance'])+value)
                pay(REG,arg,0,'RewardNotDurable()');pay(REG,ESC,value,'PaymentFailed()')
                m=dict(id=target,status=1,tomb=0,bondState=1,recPos=len(members),actPos=len(active),lastShare=0,reward=arg,bond=value,exit=0,due=0,burn=0,grace=height,reserved=0)
                members.append(m);active.append(target);byid[target]=m;held+=value
            else:
                if not isheld(m):fail('BadState()')
                if sig=='setRewardAddress(address)':pay(REG,arg,0,'RewardNotDurable()');m['reward']=arg
                elif sig in ['deregister()','removeInactive(address)']:
                    if m['status']!=1 or len(active)<=cp(19):fail('BadState()')
                    if sig=='removeInactive(address)'and height<checked(max(m['lastShare'],m['grace'])+cp(25)):fail('BadState()')
                    due=checked(height+cp(27));deactivate(m);m.update(exit=height,due=due)
                elif sig=='reactivate()':
                    if m['status']!=2 or height<checked(m['exit']+cp(24)):fail('BadState()')
                    if len(active)>=cp(18):fail('Capacity()')
                    m.update(status=1,actPos=len(active),exit=0,due=0,grace=height);active.append(target)
                else:
                    if m['status']!=2 or height<m['due']:fail('BadState()')
                    refund(m);erase(m)
            meta['ops']+=1
            if adding:meta['adds']+=1
    except Reject as error:
        failed=copy.deepcopy(original);failed[caller]['nonce']+=1
        return failed,False,list(keccak(str(error).encode())[:4])
    dest=state[REG]['storage']
    def store(n,v):
        if v:dest[key(n)]=key(v)
        else:dest.pop(key(n),None)
    # Encode domain lists/records, not the implementation's sequence of writes.
    for i in range(s(0)):store(base(5)+i,0)
    for i in range(s(1)):store(base(6)+i,0)
    for a in set([address(s(base(5)+i))for i in range(s(0))])-set(byid):
        for k in range(4):store(rec(a)+k,0)
    for i,m in enumerate(members):m['recPos']=i;store(base(5)+i,int(m['id'],16))
    for i,a in enumerate(active):byid[a]['actPos']=i;store(base(6)+i,int(a,16))
    for m in members:
        r=rec(m['id']);store(r,m['reserved']|m['status']|(m['tomb']<<8)|(m['bondState']<<16)|(m['recPos']<<24)|(m['actPos']<<56)|(m['lastShare']<<88));store(r+1,int(m['reward'],16));store(r+2,m['bond']);store(r+3,m['exit']|(m['due']<<64)|(m['burn']<<128)|(m['grace']<<192))
    store(0,len(members));store(1,len(active));store(2,sweep);store(4,held)
    m=meta;store(3,m['reserved']|m['ops']|(m['adds']<<16)|(m['begin']<<24)|(int(m['initialized'])<<88)|(int(m['closing'])<<89)|(m['cursor']<<90)|(int(m['closeInit'])<<122)|(m['mark']<<123)|(m['closeH']<<187))
    return state,True,[]
