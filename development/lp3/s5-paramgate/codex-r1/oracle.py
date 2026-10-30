"""Independent unlimited-integer/Fraction R26 oracle. Imports no project arithmetic."""
import hashlib, json, random, subprocess, sys
from fractions import Fraction

M = 1024**2
DEFAULT = dict(conn_max=32, req_buf=M, conn_buf=65536, send_sock_buf=32768,
 heavy_active=4, light_active=16, light_queue=32, heavy_resp_mem=48*M,
 light_resp_mem=16*M, resp_max=8*M, light_resp_max=512*1024,
 batch_resp_max=16*M, scratch_heavy=16*M, scratch_light=M, mem_total=192*M,
 batch_max=20, err_item_max=256, logs_count_max=10000, logs_scan_bytes=32*M,
 rate=20, bucket_cap=20, queue_wait_ms=2000, compute_heavy_ms=5000,
 cancel_max_ms=100, grace_ms=2000, min_send_rate=65536, batch_wall_ms=420000,
 enforce_slack_ms=200, timer_eps_ms=1)

def expected(changes):
 p = DEFAULT | changes
 if p['min_send_rate'] == 0:
  return dict(error='ZeroSendRate', r26=False)
 mem = p['heavy_resp_mem']+p['light_resp_mem']+p['heavy_active']*p['scratch_heavy']+p['light_active']*p['scratch_light']+p['conn_max']*(p['req_buf']+p['conn_buf'])
 hard = p['batch_resp_max']+p['batch_max']*p['err_item_max']+64
 send = p['grace_ms']+Fraction(1000*hard,p['min_send_rate'])
 watch = send+p['enforce_slack_ms']
 wall = p['batch_max']*(p['queue_wait_ms']+p['enforce_slack_ms']+p['compute_heavy_ms']+p['cancel_max_ms'])+watch
 ok = all([
  p['heavy_resp_mem'] >= p['resp_max'],
  p['light_resp_mem'] >= p['conn_max']*p['light_resp_max'],
  mem <= p['mem_total'], p['batch_resp_max'] >= p['resp_max'],
  p['light_queue'] >= p['conn_max'], p['batch_max']*p['err_item_max']+64 <= p['conn_buf'],
 ])
 # Keep the literal wire/log arithmetic separate from the production expression.
 predicates = [p['resp_max'] >= 345088, p['logs_count_max'] >= 546,
  p['logs_scan_bytes'] >= 32768, p['light_resp_max'] >= 394240,
  p['batch_wall_ms'] >= wall, p['rate'] >= 1, p['bucket_cap'] >= p['batch_max'],
  0 < p['timer_eps_ms'] < p['enforce_slack_ms'] <= 500, p['send_sock_buf'] <= 65536]
 def ratio(v): return [str(v.numerator), str(v.denominator)]
 return dict(memory=str(mem), hard=str(hard), send=ratio(send), watch=ratio(watch),
             wall=ratio(wall), r26=ok and all(predicates))

def run(probe):
 seed = 0x533520261017
 rng = random.Random(seed)
 rows = [{}]
 for field, default in DEFAULT.items():
  for value in {0,1,max(0,default-1),default,default+1,2**32-1}:
   rows.append({field:value})
 for _ in range(5000):
  row = {}
  for field in rng.sample(list(DEFAULT), rng.randint(1,5)):
   v = DEFAULT[field]
   row[field] = rng.choice([0,1,max(0,v-1),v+1,rng.randrange(2**32)])
  rows.append(row)
 text = ''.join(json.dumps(x,sort_keys=True)+'\n' for x in rows)
 proc = subprocess.run([probe],input=text,text=True,capture_output=True)
 assert proc.returncode == 0, proc.stderr
 actual = [json.loads(x) for x in proc.stdout.splitlines()]
 assert len(actual) == len(rows)
 mismatches = [(i, row, expected(row), got) for i,(row,got) in enumerate(zip(rows,actual)) if expected(row) != got]
 result = dict(seed=seed,cases=len(rows),mismatches=len(mismatches),
  firstMismatches=mismatches[:3],inputSha256=hashlib.sha256(text.encode()).hexdigest(),
  outputSha256=hashlib.sha256(proc.stdout.encode()).hexdigest(),rustExit=proc.returncode)
 print(json.dumps(result,sort_keys=True))
 assert not mismatches

if __name__ == '__main__': run(sys.argv[1])
