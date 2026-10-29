"""Verify the reproducible unapproved diagnostic, not a Ledger acceptance gate."""
from pathlib import Path
import hashlib,json,sys
repo=Path(sys.argv[1]).resolve();p=Path(__file__).resolve().parent
for line in (p/'SHA256SUMS').read_text().splitlines():
 digest,name=line.split('  ',1);f=(p/name).resolve();assert f.is_relative_to(p)
 assert hashlib.sha256(f.read_bytes()).hexdigest()==digest,name
sources=json.loads((p/'SOURCES.json').read_text())
for row in sources['sources']:assert hashlib.sha256((repo/row['path']).read_bytes()).hexdigest()==row['sha256']
d=json.loads((p/'diagnostic/results.json').read_text());assert d['artifactSHA256']==hashlib.sha256((p/'diagnostic/compiler.json').read_bytes()).hexdigest()
assert d['rootsDiffer'] and d['decision']=='NOT APPROVED' and not d['bootable']
a,b=d['cases'];assert not a['approved'] and not b['approved'] and a['beforeRoot']==b['beforeRoot'] and a['afterRoot']!=b['afterRoot']
assert a['counts']=={'1':0,'2':11,'3':13} and b['counts']=={'1':7,'2':11,'3':13}
full=json.loads((repo/'development/LP4-ACCEPTANCE.json').read_text());assert full['ledgerStateDecision']['status']=='NOT APPROVED'
assert len(full['obligations'])==27 and all(x['status']=='NOT PASSED' for x in full['obligations']) and not full['bootable']
assert full['issuanceFundingDecision']['status']==full['systemExecutionDecision']['status']=='APPROVED_ADDITIVE_LP4'
print('VERIFIED unapproved equal-code/prestate retention countermodel/source preservation; actual Ledger and full LP4 NOT PASSED; boot=false')
