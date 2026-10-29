"""Independent root probes: fixed boundaries derived directly from D104/D112/D113."""
import importlib.util,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
rev=sys.argv[1]; p=ROOT/('m1-draft-'+rev)/'tools/sites_ref.py'
s=importlib.util.spec_from_file_location('site_probe_'+rev,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
rows=[]
def ck(n,a,b):rows.append({'check':n,'actual':a,'expected':b,'pass':a==b})
def at(l,t):l.t=t;return l
keys=['k'+str(i) for i in range(64)]
l=at(m.SiteLedger(keys_on_disk=keys,epochs=[{'E':2,'bootMs':0}],live_E=2),100)
ck('existing64key_no_site_seat',l.admit('k0','data',1),None)
ck('full64_no_retry_hint',l.admit('new','data',1),{'code':4300,'reason':'sites','limit':64})
l=at(m.SiteLedger(keys_on_disk=keys[:60],epochs=[{'E':2,'bootMs':0}],live_E=2),100)
ck('late_only_retry59900',l.admit('new','data',1),{'code':4300,'reason':'sites','limit':64,'retryAfterMs':59900})
l.issue_refresh();l.t=60000;l.ev_refreshComplete({})
ck('pre_expiry_refresh_keeps_late',l.late_sites(),4)
l.issue_refresh();l.ev_refreshComplete({})
ck('post_expiry_completed_refresh_drops_late',l.late_sites(),0)
ck('late_expiry_admits_new',l.admit('new','data',1),None)
for n,want in [(991,None),(992,{'code':4300,'reason':'disk','limit':1024})]:
 l=at(m.SiteLedger(names=n),0);ck('names_tomb_'+str(n),l.admit('k','tomb',1),want)
base=m.DISK_HARD-m.LATE_RESERVE-m.META_RESERVE-10
for delta,want in [(0,None),(1,{'code':4300,'reason':'disk','limit':m.DISK_HARD})]:
 l=at(m.SiteLedger(in_use=base+delta),0);ck('bytes_inclusive_'+str(delta),l.admit('k','data',10),want)
ck('tomb_byte_exemption',at(m.SiteLedger(in_use=m.DISK_HARD),0).admit('k','tomb',512),None)
l=at(m.SiteLedger(keys_on_disk=['k']),0);l.ev_op({'id':'w','key':'k','kind':'data','bytes':1});ck('existing_write_pin',sorted(l.pinned()),['k'])
l.issue_refresh();l.items={};l.t=1;l.ev_settle({'id':'w','ok':False});l.t=2;l.ev_refreshComplete({})
ck('refresh_issued_before_settle_keeps_pin',sorted(l.pinned()),['k'])
ck('absent_pinned_key_counted',sorted(l.sites_live),['k'])
ck('serialized_followup_exists',l.refreshing is not None,True)
l.t=3;l.ev_refreshComplete({});ck('followup_retires_pin',sorted(l.pinned()),[]);ck('absent_key_capacity_released',sorted(l.sites_live),[])
l=at(m.SiteLedger(keys_on_disk=['k'],tomb_keys=['k'],sessions=['k']),0);l.scan();ck('live_session_blocks_reaper',len(l.ops),0)
l.sessions=set();l.scan();ck('eligible_tomb_reaper_issues_remove',len(l.ops),1)
ck('remove_zero_live_tickets',len(l.live_tickets()),0)
ck('issued_remove_does_not_release_measured_capacity',sorted(l.sites_live),['k'])
report={'revision':rev,'checks':len(rows),'passed':sum(r['pass'] for r in rows),'failed':sum(not r['pass'] for r in rows),'results':rows}
out=Path(__file__).with_name('site-independent-probes-'+rev+'.json');out.write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k!='results'}))
for r in rows:
 if not r['pass']:print(json.dumps(r))
sys.exit(1 if report['failed'] else 0)
