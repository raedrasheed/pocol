"""Independent S-a counterexample: an unknown event is not abort or commit."""
import importlib.util,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
p=ROOT/'m1-draft-0.19/tools/logclient_ref.py';s=importlib.util.spec_from_file_location('sink_c29',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
a={'number':1,'hash':'0x'+'11'*32}
entries=[{'seq':0,'kind':'sink','ev':['begin',1,a]},{'seq':1,'kind':'sink','ev':['unknownTerminal',1,'fake']}]
got=m.SinkChecker().check(entries,1,1)
r={'source':'reference/browser.md:125 S-a requires abort or commit','entries':entries,'expectedRule':'S-a','actual':got,'pass':any(x['rule']=='S-a' for x in got)}
Path(__file__).with_suffix('.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps(r));sys.exit(0 if r['pass'] else 1)
