import sys
"""Independent Decimal-60 ASQ grid, not imported from authored models."""
from decimal import Decimal,localcontext
from pathlib import Path
import hashlib,json,time
start=time.monotonic();maximum=Decimal(0);at=0;digest=hashlib.sha256();previous=0
with localcontext() as c:
 c.prec=60
 for f in range(65536):
  poly=195766423245049*f+971821376*f*f+5127*f*f*f+(1<<47);F=65536+(poly>>48);assert F>=previous;previous=F;digest.update((str(F)+'\n').encode());true=c.power(Decimal(2),Decimal(f)/Decimal(65536));relative=abs(Decimal(F)/Decimal(65536)/true-1)
  if relative>maximum:maximum=relative;at=f
 assert maximum<=Decimal('0.0002')
result=dict(cases=65536,precision=60,maxRelativeError=str(maximum),atF=at,Fsha256=digest.hexdigest(),monotonic=True,seconds=round(time.monotonic()-start,3));(Path(sys.argv[1]).resolve()/'asq-python.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
