"""Independent Python integer London oracle; deterministic synthetic inputs only."""
import random,json,sys
r=random.Random(777902);rows=[]
for base in [0,1,7,8,10**9,2**255,2**256-1]+[r.randrange(2**256) for _ in range(25)]:
 for limit,used in [(8_000_000,0),(8_000_000,4_000_000),(8_000_000,4_000_001),(8_000_000,8_000_000),(2**64-1,2**64-1),(1,0)]:
  target=limit//2
  if target==0: expected='ParentGas'
  else:
   delta=base*abs(used-target)//target//8
   value=base if used==target else base+max(1,delta) if used>target else base-delta
   expected='Overflow' if value>=2**256 else str(value)
  rows.append(dict(base=str(base),limit=limit,used=used,expected=expected))
with open(sys.argv[1],'w') as f:json.dump(rows,f,indent=2);f.write('\n')
print('Generated192 independent Python-bigint London boundary vectors')
