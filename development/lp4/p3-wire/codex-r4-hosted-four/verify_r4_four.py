from pathlib import Path
import json,hashlib,re,sys
p=Path(sys.argv[1]);repo=Path(sys.argv[2]);controls=p/'lp4-p2-controls/lp4-controls';native=p/'lp4-native-cancun-reference';results={'run':39428650062,'sourceHead':'f3fbe13863030adfed6912450e255acd986238b5','sourceTree':'540fa389c5a9e3c2d0d81b6a8f3f71b4387479e5'}
for a in json.loads((p/'artifacts-verified.json').read_text()):
 b=(p/(a['name']+'.zip')).read_bytes();assert len(b)==a['bytes'] and hashlib.sha256(b).hexdigest()==a['sha256']
for file in [controls/'rust-tests.log',p/'lp4-native-windows/rust-tests.log']:
 text=file.read_text();assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',text)))==90;assert 'test result: FAILED' not in text
for name,relative in [('wire-envelopes.json','wire-envelopes.json'),('london-fees.json','london-fees.json'),('user-phase.json','user-phase.json'),('profile-execution.json','profile-execution.json')]:assert (controls/name).read_bytes()==(repo/'prototypes/lp4-execution/tests/data'/relative).read_bytes()
raw=(controls/'signed-admission/raw-signed-inputs.jsonl').read_text().splitlines();actual=[json.loads(s) for s in (controls/'signed-admission/cases.jsonl').read_text().splitlines()];independent=[json.loads(s) for s in (native/'signed-geth-results/cases.jsonl').read_text().splitlines()];published=[json.loads(s) for s in (repo/'development/lp4/p3-wire/codex-r3/signed-negative-cases.jsonl').read_text().splitlines()];assert len(raw)==len(actual)==len(independent)==len(published)==1173
seen=set()
for data,a,b,c in zip(raw,actual,independent,published,strict=True):
 data=json.loads(data);assert set(data)=={'raw','pre','env'}
 assert all(set(v)=={'balance','nonce','code','storage'} for v in data['pre'].values());assert len(bytes.fromhex(data['raw']))<=131072
 key=(a['file'],a['name'],a['variant']);assert key not in seen;seen.add(key)
 for k in ['file','name','variant','indexes','expectedException','kind','rawSHA256','category']:assert a[k]==b[k]==c[k]
 assert hashlib.sha256(bytes.fromhex(data['raw'])).hexdigest()==a['rawSHA256'];assert a['rust']==c['rust'];assert b['independentGeth']==c['independentGeth'];assert not b['independentGeth']['accepted'];assert a['unmodifiedSecondEngine']['rejected']==c['unmodifiedPyEvmRejected']
counts=json.loads((controls/'signed-admission/summary.json').read_text())['counts'];assert counts=={'POCOL_TYPE3_REJECTION_NOT_ORIGINAL_CATEGORY':157,'unmodifiedAccepted':73,'unmodifiedRejected':1100,'SIGNED_NATIVE_CATEGORY_AND_INPUT_PREDICATE':1014,'SIGNED_PRIORITY_WIRE_PRECEDENCE':1,'SIGNED_OVER_U256_WIRE_INTEGER':1}
proof=json.loads((native/'signed-geth/source-proof.json').read_text());local=json.loads((repo/'development/lp4/p3-wire/codex-r3/geth-source-proof.json').read_text());assert proof['unchangedSourceFiles']==2000 and proof['changedBuildMetadataOnly']==[] and proof['upstreamZipSHA256']==local['upstreamZipSHA256'];assert proof['helperSHA256']==local['helperSHA256']==hashlib.sha256((repo/'prototypes/lp4-execution/tests/gst_signed_geth.go').read_bytes()).hexdigest()
compiled={}
for name,count in [('lp4-pool-mutations',10),('lp4-wire-mutations',13),('lp4-user-mutations',6)]:
 rows=json.loads((p/'lp4-p2-controls'/name/'results.json').read_text());assert len(rows)==count and all(r['buildExit']==0 and r['assertionKilled'] for r in rows);compiled[name]=count
rows=json.loads((native/'signed-geth-mutations/results.json').read_text());assert len(rows)==3 and all(r['buildExit']==0 and r['assertionKilled'] for r in rows);compiled['independentSigned']=3
primary=controls/'gst/cases.jsonl';assert hashlib.sha256(primary.read_bytes()).hexdigest()=='1bd455f96ded27e7f7f65253a3a3912f8044089ec60394cc036e994cb3c5a28b'
legacy=json.loads((p/'lp4-legacy-windows/results.json').read_text())['counts'];assert legacy=={'PASS':16,'FAIL':0,'BLOCKED BY PLATFORM':8,'NOT RUN':2}
results.update(completedArtifactsVerified=4,rustLinux=90,rustWindows=90,independentProfileCases=9,allSignedCaseCertificatesMatchPublished=True,signedCases=1173,unmodifiedPyEvmAccepted=73,unmodifiedSignedAudit='NOT PASSED',independentSignedGethRejected=1173,compiled=compiled,primaryCaseBytesEqualR4=True,independentGethUnchangedSourceFiles=2000,legacy=legacy,scope='completed job artifact evidence only, independent long comparison pending; no P2/P3/LP4 acceptance',bootable=False)
(p/'verified-completed.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({k:results[k] for k in ['rustLinux','rustWindows','signedCases','independentSignedGethRejected','allSignedCaseCertificatesMatchPublished']}))

controls=p/'lp4-p2-controls/lp4-controls'
rows=json.loads((p/'lp4-p2-controls/lp4-code-mutations/results.json').read_text());assert len(rows)==12 and all(r['buildExit']==0 and r['assertionKilled'] for r in rows)
for name in ['code-caps-execution.json','code-caps-witness.json']:assert (controls/name).read_bytes()==(repo/'prototypes/lp4-execution/tests/data'/name).read_bytes()
assert (controls/'code-deltas-independent/cases.json').read_bytes()==(repo/'development/lp4/p3-wire/codex-r4/individual-replacements.json').read_bytes()
allrows=[json.loads(line) for line in (controls/'code-deltas/all-cases.jsonl').read_text().splitlines()];ids={(r['file'],r['name'],r['variant']) for r in allrows};assert len(allrows)==len(ids)==20661 and sum(r['changed'] for r in allrows)==14
primary=[json.loads(line) for line in (controls/'gst/cases.jsonl').read_text().splitlines()];assert len(primary)==20661 and all(r['controlPass'] for r in primary) and {(r['file'],r['name'],r['variant']) for r in primary}==ids
results.update(codeMutationsBuiltAndDetected=12,codeCases=76,codeFollowUpTransactions=2,codeDeltaIdentities=20661,unchangedCodeCases=20647,independentAmendedExecution=8,independentInvalidInputSnapshotAdmissionOnly=6,actualGenesisBuilder='NOT PASSED',scope='Four completed artifacts only; independent long comparison pending; no P2/P3/LP4 acceptance');(p/'verified-completed.json').write_text(json.dumps(results,indent=2)+'\n');print(json.dumps({'completedArtifacts':4,'codeMutations':12,'all20661CaseIdentities':True,'individualReplacementCertificates':14}))
