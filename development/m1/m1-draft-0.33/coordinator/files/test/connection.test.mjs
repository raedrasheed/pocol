import test from 'node:test';
import assert from 'node:assert/strict';
import http from 'node:http';
import { writeFileSync } from 'node:fs';
import path from 'node:path';
import { createApp, resolveConnection, savedConnection } from '../src/server.mjs';
import { Controller } from '../src/broker.mjs';
import { fakeSpawner, makeWorkspace } from './helpers.mjs';

// Clearly synthetic capabilities; real values live only in the private connection.json and are never read here.
const CTL = 'a'.repeat(64), REV = 'b'.repeat(64), PORT = 45678;

function writeConn(ws, patch = {}, raw) {
  const dir = path.join(ws, 'coordination', 'ui-control');
  const file = path.join(dir, 'connection.json');
  const base = { url: `http://127.0.0.1:${PORT}/#${CTL}`, port: PORT, pid: 1, startedAt: 't', workspace: ws,
    reviewerApi: `http://127.0.0.1:${PORT}/api/reviewer/`, reviewerHeader: 'X-PoCol-Reviewer', reviewerToken: REV };
  new Controller({ workspace: ws, isAlive: () => false });               // creates ui-control as the service would
  writeFileSync(file, raw !== undefined ? raw : JSON.stringify({ ...base, ...patch }));
  return file;
}

test('a valid saved loopback connection for this workspace is reused: same port and capabilities', () => {
  const ws = makeWorkspace();
  const file = writeConn(ws);
  const saved = savedConnection(file, ws);
  assert.deepEqual(saved, { status: 'valid', reason: null, port: PORT, controlToken: CTL, reviewerToken: REV });
  const c = resolveConnection({ saved });
  assert.deepEqual([c.port, c.controlToken, c.reviewerToken, c.reused, c.notice], [PORT, CTL, REV, true, null]);
  assert.equal(resolveConnection({ saved, argPort: String(PORT) }).reused, true, 'the same explicit port keeps the URL');
});

test('invalid saved connections fail closed: never reused, explicit reason, no capability in the notice', () => {
  const ws = makeWorkspace();
  const other = makeWorkspace();
  const cases = [
    [{ workspace: other }, 'otherWorkspace'],
    [{ port: 80 }, 'port'],
    [{ port: 70000 }, 'port'],
    [{ port: '45678' }, 'port'],
    [{ reviewerToken: REV.toUpperCase() }, 'reviewerCapability'],
    [{ reviewerToken: 'short' }, 'reviewerCapability'],
    [{ url: `http://localhost:${PORT}/#${CTL}` }, 'url'],
    [{ url: `http://127.0.0.1:${PORT + 1}/#${CTL}` }, 'url'],
    [{ url: `http://127.0.0.1:${PORT}/?t=${CTL}` }, 'url'],
    [{ url: `http://127.0.0.1:${PORT}/#${REV}` }, 'sameCapabilities'],
    [{ reviewerApi: `http://192.168.1.2:${PORT}/api/reviewer/` }, 'reviewerApi'],
    [{ reviewerHeader: 'Authorization' }, 'reviewerApi'],
  ];
  for (const [patch, reason] of cases) {
    const saved = savedConnection(writeConn(ws, patch), ws);
    assert.deepEqual([saved.status, saved.reason], ['invalid', reason], JSON.stringify(patch));
    assert.equal(saved.controlToken, undefined);
    const c = resolveConnection({ saved });
    assert.equal(c.reused, false);
    assert.notEqual(c.controlToken, CTL); assert.notEqual(c.reviewerToken, REV);
    assert.match(c.notice, new RegExp(reason));
    assert.ok(!c.notice.includes(CTL) && !c.notice.includes(REV), 'no capability in the notice');
  }
  for (const raw of ['', '{"url":', 'null', '[]', '"x"']) {
    assert.equal(savedConnection(writeConn(ws, {}, raw), ws).status, 'invalid', `raw ${JSON.stringify(raw)}`);
  }
  assert.equal(savedConnection(path.join(ws, 'missing.json'), ws).status, 'none');
});

test('--fresh-connection and a different --port create new capabilities; a bad --port is refused', () => {
  const ws = makeWorkspace();
  const saved = savedConnection(writeConn(ws), ws);
  const f = resolveConnection({ saved, fresh: true });
  assert.equal(f.reused, false); assert.notEqual(f.controlToken, CTL); assert.match(f.notice, /--fresh-connection/);
  const p = resolveConnection({ saved, argPort: '45679' });
  assert.equal(p.reused, false); assert.equal(p.port, 45679);
  assert.throws(() => resolveConnection({ saved, argPort: 'abc' }), /--port/);
  assert.throws(() => resolveConnection({ saved, argPort: '70000' }), /--port/);
  const none = resolveConnection({ saved: { status: 'none' } });
  assert.deepEqual([none.port, none.reused, none.notice], [0, false, null]);
  assert.match(none.controlToken, /^[0-9a-f]{64}$/);
});

test('a reused connection keeps the open browser working and the reviewer API unreachable from the browser', async (t) => {
  const ws = makeWorkspace();
  const controller = new Controller({ workspace: ws, claudeBin: 'claude-fake', spawnImpl: fakeSpawner().spawnImpl, isAlive: () => false });
  const { server } = createApp({ controller, controlToken: CTL, reviewerToken: REV });
  await new Promise((r) => server.listen(0, '127.0.0.1', r));
  t.after(() => new Promise((r) => { server.closeAllConnections(); server.close(r); }));
  const port = server.address().port;
  const req = (p, headers = {}, method = 'GET', body) => new Promise((resolve, reject) => {
    const q = http.request({ host: '127.0.0.1', port, method, path: p, headers: { host: `127.0.0.1:${port}`, ...headers } }, (res) => {
      const chunks = []; res.on('data', (c) => chunks.push(c)); res.on('end', () => resolve({ status: res.statusCode, text: Buffer.concat(chunks).toString('utf8') }));
    });
    q.on('error', reject); if (body) q.write(JSON.stringify(body)); q.end();
  });
  const state = await req('/api/state', { 'x-pocol-control': CTL });
  assert.equal(state.status, 200, 'the previous page capability still works');
  assert.ok(!state.text.includes(CTL) && !state.text.includes(REV), 'capabilities never echoed');
  assert.equal((await req('/api/reviewer/queue', { 'x-pocol-reviewer': REV })).status, 200, 'the host capability still works');
  assert.equal((await req('/api/reviewer/queue', { 'x-pocol-control': CTL })).status, 403, 'browser capability cannot reach the reviewer API');
  assert.equal((await req('/api/reviewer/ack', { 'x-pocol-reviewer': REV, origin: `http://127.0.0.1:${port}`, 'content-type': 'application/json' }, 'POST', { itemId: 'x' })).status, 403,
    'a browser-originated request never reaches the reviewer API, even with the host capability');
  assert.equal((await req('/api/reviewer/queue', { 'x-pocol-reviewer': REV, 'sec-fetch-site': 'same-origin' })).status, 403);
  assert.match((await req('/')).text, /التفويض الدائم والمتابعة/, 'the read-only authority section is served');
});

test('a saved valid connection never bypasses the single-service lock', () => {
  const ws = makeWorkspace();
  writeConn(ws);
  const a = new Controller({ workspace: ws, isAlive: () => false });
  const b = new Controller({ workspace: ws, isAlive: () => false });
  a.acquireService(null);
  assert.throws(() => b.acquireService(null), /تعمل بالفعل/, 'a second service cannot start or take over');
  a.releaseService();
});
