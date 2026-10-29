// Independent BigInt oracle from consensus.md:126-137. No network or author model import.
import fs from 'node:fs';
const MAX = (1n << 256n) - 1n;
const floor = (a, b) => { const q = a / b, r = a % b; return r < 0n ? q - 1n : q; };
const bits = (x) => (x < 0n ? -x : x).toString(2).length;
function compute(target, gts, blockTime, tau, h, ts) {
  const dt = ts - gts - blockTime * h;
  const e = floor(dt * 65536n, tau), s = floor(e, 65536n), f = e - 65536n * s;
  if (s >= 256n) return { target: MAX.toString(), early: 'high', maxShiftBits: 0 };
  if (s <= -257n) return { target: '1', early: 'low', maxShiftBits: 0 };
  const F = 65536n + (195766423245049n * f + 971821376n * f * f + 5127n * f * f * f + (1n << 47n)) / (1n << 48n);
  const X = target * F, k = s - 16n;
  const Y = k >= 0n ? X << k : X >> -k;
  return { target: (Y < 1n ? 1n : Y > MAX ? MAX : Y).toString(), early: null, maxShiftBits: Math.max(bits(X), bits(Y)) };
}
const rows = [];
for (const target of [1n, 1n << 240n, MAX]) {
  for (const dt of [-257n * 600n, -256n * 600n, -1n, 0n, 1n, 4n * 600n, 255n * 600n, 256n * 600n, 1n << 62n]) {
    const gts = 1700000000n, blockTime = 10n, h = 20n, ts = gts + blockTime * h + dt;
    rows.push({ targetG: target.toString(), gts: gts.toString(), blockTime: '10', tau: '600', h: '20', ts: ts.toString(), ...compute(target, gts, blockTime, 600n, h, ts) });
  }
}
let seed = 0x31415926;
const next = () => { seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0; return seed; };
for (let i = 0; i < 1000; i++) {
  let target = 0n; for (let j = 0; j < 8; j++) target = (target << 32n) | BigInt(next());
  if (!target) target = 1n;
  const h = BigInt(next()), gts = 1700000000n, blockTime = BigInt(next() % 1000 + 1), tau = BigInt(next() % 10000 + 1);
  const dt = BigInt((next() % 6000000) - 3000000), ts = gts + blockTime * h + dt;
  rows.push({ targetG: target.toString(), gts: gts.toString(), blockTime: blockTime.toString(), tau: tau.toString(), h: h.toString(), ts: ts.toString(), ...compute(target, gts, blockTime, tau, h, ts) });
}
if (rows.some((r) => r.maxShiftBits > 512)) throw Error('Source width bound exceeded');
const report = { node: process.version, source: 'reference/consensus.md:126-137', seed: '0x31415926', checks: rows.length,
  maxIntermediateBits: Math.max(...rows.map((r) => r.maxShiftBits)), rows };
fs.writeFileSync(process.argv[2], JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ checks: report.checks, maxIntermediateBits: report.maxIntermediateBits }));
