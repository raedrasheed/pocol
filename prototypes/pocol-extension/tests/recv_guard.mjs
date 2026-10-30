// RecvGuard unit gates (browser.md D98/D100, MR10-style): a hostile HTTP server
// in Node serves oversized, streamed, malformed and deep responses.
import { createServer } from 'node:http';
import { pathToFileURL } from 'node:url';
import { join } from 'node:path';

export async function recvGuardChecks(dist) {
  const T = await import(pathToFileURL(join(dist, 'lib', 'transport.js')).href);
  const B = await import(pathToFileURL(join(dist, 'lib', 'bridge.js')).href);
  const checks = []; const ok = (name, c, d = '') => checks.push({ name, pass: !!c, detail: d });
  let mode = 'ok'; let bytesSent = 0;
  const srv = createServer((req, res) => {
    let body = ''; req.on('data', (c) => { body += c; }); req.on('end', async () => {
      const send = (s, h = {}) => { res.writeHead(200, { 'content-type': 'application/json', ...h }); res.end(s); };
      if (mode === 'ok') return send('{"jsonrpc":"2.0","id":1,"result":"0x10"}');
      if (mode === 'cl') return send('{"jsonrpc":"2.0","id":1,"result":"' + 'a'.repeat(5000) + '"}');
      if (mode === 'stream') { res.writeHead(200, { 'content-type': 'application/json' }); res.write('{"jsonrpc":"2.0","id":1,"result":"'); bytesSent = 0;
        for (let i = 0; i < 160; i++) { if (res.destroyed) break; res.write('b'.repeat(1024)); bytesSent += 1024; await new Promise((r) => setTimeout(r, 5)); } res.end('"}'); return; }
      if (mode === 'utf8') { res.writeHead(200, { 'content-type': 'application/json' }); res.end(Buffer.from([0x7b, 0x22, 0x72, 0xff, 0x22, 0x3a, 0x31, 0x7d])); return; }
      if (mode === 'deep') { let v = '1'; for (let i = 0; i < 17; i++) v = '[' + v + ']'; return send('{"jsonrpc":"2.0","id":1,"result":' + v.slice(1, -1) + '}'); }
      if (mode === 'deep16') { let v = '1'; for (let i = 0; i < 15; i++) v = '[' + v + ']'; return send('{"jsonrpc":"2.0","id":1,"result":' + v + '}'); }
      if (mode === 'dup') return send('{"jsonrpc":"2.0","id":1,"result":"0x1","result":"0x2"}');
      if (mode === 'err') return send(JSON.stringify({ jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'm'.repeat(1000), data: 'd'.repeat(5000) } }));
      if (mode === 'err-small') return send(JSON.stringify({ jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'x', data: { a: 1 } } }));
      if (mode === 'slow') { await new Promise((r) => setTimeout(r, 1500)); return send('{"jsonrpc":"2.0","id":1,"result":"0x1"}'); }
      if (mode === 'proto') return send('{"jsonrpc":"2.0","id":1,"result":{"__proto__":{"polluted":true}}}');
      send('{}');
    });
  });
  await new Promise((r) => srv.listen(0, '127.0.0.1', r)); const url = `http://127.0.0.1:${srv.address().port}/`;
  const pool = new T.Pool(T.SITE_POOL, 48);
  const call = async (m, method = 'eth_blockNumber', extra = {}) => { mode = m; try { return { r: await T.request(url, method, [], { pool, ...extra }) }; } catch (e) { return { t: e.transport, rpc: e.rpc }; } };
  ok('RecvGuard legit response', (await call('ok')).r === '0x10');
  ok('Content-Length > recvLimit (4096) -> recvLimit before reading', (await call('cl')).t === 'recvLimit');
  const st = await call('stream', 'eth_getCode'); await new Promise((r) => setTimeout(r, 300));
  ok('streamed body over recvLimit (68 KiB) aborted at the limit -> recvLimit', st.t === 'recvLimit' && bytesSent < 160 * 1024, `sent=${bytesSent}`);
  ok('invalid UTF-8 -> recvParse', (await call('utf8')).t === 'recvParse');
  ok('depth 17 -> recvDepth', (await call('deep')).t === 'recvDepth');
  ok('depth 16 accepted', Array.isArray((await call('deep16')).r));
  ok('duplicate keys in a response -> recvParse', (await call('dup')).t === 'recvParse');
  { const e = await call('err', 'eth_call'); ok('remote error: message clipped to 256, data > 4 KiB dropped', e.rpc && e.rpc.message.length === 256 && e.rpc.data === undefined && e.rpc.code === -32000); }
  { const e4 = await call('err'); ok('error body over the method limit -> recvLimit', e4.t === 'recvLimit'); const e = await call('err-small'); ok('remote error: small data passed through', e.rpc && e.rpc.data && e.rpc.data.a === 1); }
  ok('method without RECV_LIMIT is never sent', (await call('ok', 'debug_traceTransaction')).t === 'unsupported');
  ok('deadline -> transport', (await call('slow', 'eth_blockNumber', { deadline: 300 })).t === 'transport');
  { const r = await call('proto', 'eth_call'); ok('__proto__ key stays data (null-prototype objects)', r.r && ({}).polluted === undefined && Object.getPrototypeOf(r.r) === null); }
  // Pool: FIFO reservation, busy after the wait.
  { const p = new T.Pool(8192, 48); const a = await p.reserve(8192, 50); const b = await p.reserve(1, 50); p.release(8192); const c = await p.reserve(8192, 50);
    ok('Pool: exhausted reservation -> busy after wait, release frees', a === true && b === false && c === true); }
  { const p = new T.Pool(1 << 30, 2); await p.reserve(1); await p.reserve(1); ok('Pool: inflight slots bound', (await p.reserve(1, 50)) === false); }
  // Linear parser: 8 MiB response parses quickly.
  { const big = '{"a":[' + Array.from({ length: 200000 }, (_, i) => `"${'x'.repeat(30)}${i}"`).join(',') + ']}'; const t0 = Date.now(); const v = B.parseStrict(big, 16); const dt = Date.now() - t0;
    ok('parseStrict is linear: ~7 MiB in well under 2 s', v.a.length === 200000 && dt < 2000, `${(big.length / 1048576).toFixed(1)} MiB in ${dt} ms`); }
  srv.close(); return checks;
}
