// The explorer's Holders page and Miners map (development/explorer/MINERS-MAP-HOLDERS.md §6): the anchored bundle edits
// (tools/explorer-insights-edits.json, applied after W7, the site-truth and the production edits by
// tools/site_identity.py --edits) and the pure functions of assets/pocol-insights.js.
//   node --test deploy/websites/tests/explorer_insights.test.mjs
// 1. The edits file is what the served bundle carries: each `new` once and each `old` not at all; undone in reverse and
//    redone they give the served bundle byte for byte. English only, no chain ID or genesis literal. They add the
//    Holders tab, the two subtitles, the three empty mount points in place of the old map component (whose v1 poll of
//    /api/miner-locations.php no longer runs), and expose PocolExplorerShared from the bundle's own identity variables.
// 2. index.html loads the module and its stylesheet with their content versions; both are in the S1 include list; the
//    module names no hashed bundle, no identity literal, no outside host but its two attribution links, and builds no
//    HTML from strings.
// 3. The module's pure functions (imported from a copy, as the browser ignores its exports): amounts from BigInt,
//    percentages from integer ratios, the validators of the three answers (each broken field refused), clustering,
//    zoom bounds, ring segments, choropleth ranges, axis ticks, the language choice, and EN/AR text parity.
// The pages themselves run in explorer_insights_browser.mjs.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url)), ROOT = join(HERE, '..', '..', '..');
const SITE = join(ROOT, 'deploy', 'websites', 'explorer.pocol.net');
const page = readFileSync(join(SITE, 'index.html'), 'utf8');
const served = [...page.matchAll(/<script\b[^>]*\bsrc="\/(assets\/index-[A-Za-z0-9_-]+\.js)"/g)].map((m) => m[1]);
assert.equal(served.length, 1, 'index.html loads one explorer bundle');
const BUNDLE = readFileSync(join(SITE, served[0]), 'utf8');
const EDITS = JSON.parse(readFileSync(join(ROOT, 'deploy', 'websites', 'tools', 'explorer-insights-edits.json'), 'utf8'));
const DECLARED = JSON.parse(readFileSync(join(ROOT, 'deploy', 'websites', 'site-identity.json'), 'utf8'));
const MODULE_PATH = join(SITE, 'assets', 'pocol-insights.js'), CSS_PATH = join(SITE, 'assets', 'pocol-insights.css');
const SOURCE = readFileSync(MODULE_PATH, 'utf8'), CSS = readFileSync(CSS_PATH, 'utf8');
const count = (text, part) => text.split(part).length - 1;
const sha12 = (data) => createHash('sha256').update(data).digest('hex').slice(0, 12);

const dir = mkdtempSync(join(tmpdir(), 'pocol-insights-'));
writeFileSync(join(dir, 'insights.mjs'), SOURCE);
const M = await import(pathToFileURL(join(dir, 'insights.mjs')).href);
rmSync(dir, { recursive: true, force: true });

test('the edits file is exactly what the served bundle carries, and undone and redone it is the served bundle', () => {
  assert.ok(Array.isArray(EDITS) && EDITS.length === 5);
  for (const [i, e] of EDITS.entries()) {
    assert.deepEqual(Object.keys(e).sort(), ['count', 'file', 'new', 'old'], `edit ${i}`);
    assert.equal(e.file, 'explorer-bundle'); assert.equal(e.count, 1);
    assert.equal(count(BUNDLE, e.new), 1, `edit ${i}: its new text occurs once: ${e.new.slice(0, 80)}`);
    assert.equal(count(BUNDLE, e.old), 0, `edit ${i}: its old text is gone: ${e.old.slice(0, 80)}`);
  }
  let base = BUNDLE;
  for (const e of [...EDITS].reverse()) base = base.replace(e.new, () => e.old);
  for (const gone of ['PocolExplorerShared', 'pocol-holders', 'pocol-miners-map', '`Holders`']) assert.equal(count(base, gone), 0, gone);
  let redone = base;
  for (const [i, e] of EDITS.entries()) { assert.equal(count(redone, e.old), 1, `edit ${i}: its old text was unique`); redone = redone.replace(e.old, () => e.new); }
  assert.equal(createHash('sha256').update(redone).digest('hex'), createHash('sha256').update(BUNDLE).digest('hex'));
  const added = EDITS.map((e) => e.new).join('\n');
  assert.doesNotMatch(added, /[֐-ࣿיִ-﷿ﹰ-﻿]/, 'the bundle stays English (only the module is bilingual)');
  for (const literal of [String(DECLARED.chainId), DECLARED.chainId.toString(16), DECLARED.genesisHash.slice(2)]) assert.ok(!added.toLowerCase().includes(literal.toLowerCase()), literal);
});

test('the bundle mounts the new views in place of the old map and exposes its identity variables', () => {
  assert.equal(count(BUNDLE, '[`Transactions`,C],[`Holders`,De],[`Miners`,Ee],[`Miner map`,w],[`Network`,he]];'), 1, 'the Holders tab (wallet icon)');
  // The identity comes from the bundle's own constants (Ae, je: the anchored `Ae=<chain>,je=`<genesis>`;`) and shapes.
  assert.equal(count(BUNDLE, 'globalThis.PocolExplorerShared=Object.freeze({chainId:Ae,genesisHash:je,countries:Ke})'), 1);
  assert.equal(count(BUNDLE, ',Ae=' + DECLARED.chainId + ',je=`' + DECLARED.genesisHash + '`;'), 1, 'Ae and je are the module-level identity constants');
  for (const id of ['pocol-holders', 'pocol-miners-map', 'pocol-miners-map-compact']) assert.equal(count(BUNDLE, `(0,N.jsx)(\`div\`,{id:\`${id}\`,className:\`pocol-insights-mount\`})`), 1, id);
  assert.equal(count(BUNDLE, 't===`Holders`&&(0,N.jsx)(`div`,{id:`pocol-holders`'), 1);
  assert.equal(count(BUNDLE, 't===`Miner map`&&(0,N.jsx)(`div`,{id:`pocol-miners-map`'), 1);
  // The old one-node map component is no longer rendered and its v1 poll no longer runs (the hook is not called).
  assert.equal(count(BUNDLE, '(0,N.jsx)(nt,'), 0);
  assert.equal(count(BUNDLE, '$e(`/api/miner-locations.php`)'), 0);
  assert.equal(count(BUNDLE, 'function F(){let[t,n]=(0,h.useState)(`Overview`)'), 1);
  // The subtitles: the new Miner map branch comes first, so the production edit's notice (kept byte for byte for its
  // own replay test) is never reached.
  assert.equal(count(BUNDLE, 't===`Holders`?`PCOL balances of every discovered account at one block height, checked against the supply.`:t===`Miner map`?`Registered miners by country where an operator node reports it; every other miner is Unknown.`:t===`Miner map`?`Miner locations are not published: the network does not record them.`:'), 1);
});

test('index.html loads the module and stylesheet by content version; the S1 list names them; the module keeps the rules', () => {
  const js = /<script type="module" src="\/assets\/pocol-insights\.js\?v=([0-9a-f]{12})"><\/script>/.exec(page);
  const css = /<link rel="stylesheet" href="\/assets\/pocol-insights\.css\?v=([0-9a-f]{12})">/.exec(page);
  assert.ok(js && css, 'index.html references both');
  assert.equal(js[1], sha12(readFileSync(MODULE_PATH)), 'the module version is its content version');
  assert.equal(css[1], sha12(readFileSync(CSS_PATH)), 'the stylesheet version is its content version');
  assert.ok(page.indexOf('pocol-insights.js') > page.indexOf(served[0]), 'the module runs after the bundle (module scripts run in order)');
  const include = readFileSync(join(ROOT, 'deploy', 'websites', 'tools', 'explorer-s1-include.txt'), 'utf8').split('\n');
  for (const f of ['assets/pocol-insights.js', 'assets/pocol-insights.css']) assert.equal(include.filter((l) => l === f).length, 1, f);
  for (const text of [SOURCE, CSS]) {
    assert.doesNotMatch(text, /assets\/index-/, 'names no hashed bundle');
    for (const literal of [String(DECLARED.chainId), DECLARED.chainId.toString(16), DECLARED.genesisHash.slice(2)]) assert.ok(!text.toLowerCase().includes(literal.toLowerCase()), literal);
    assert.doesNotMatch(text, /@import|url\(/, 'no outside stylesheet, font or image');
  }
  assert.doesNotMatch(SOURCE, /innerHTML|outerHTML|insertAdjacentHTML|document\.write|\beval\(|new Function|import\(/, 'no HTML from strings, no code from strings');
  const hosts = new Set([...SOURCE.matchAll(/https?:\/\/([a-z0-9.-]+)/gi)].map((m) => m[1]));
  assert.deepEqual([...hosts].sort(), ['creativecommons.org', 'db-ip.com', 'www.naturalearthdata.com', 'www.w3.org'], 'only the attribution and licence links (and the SVG namespace)');
  assert.match(SOURCE, /const DBIP_URL = 'https:\/\/db-ip\.com', LICENSE_URL = 'https:\/\/creativecommons\.org\/licenses\/by\/4\.0\/';/);
  assert.deepEqual([...SOURCE.matchAll(/getData\(API\.(\w+)/g)].map((m) => m[1]).sort(), ['geography', 'holders', 'stats']);
  assert.match(SOURCE, /holders: '\/api\/holders\.php', stats: '\/api\/holders-stats\.php', geography: '\/api\/miner-geography\.php'/);
  for (const m of SOURCE.matchAll(/localStorage/g)) assert.match(SOURCE.slice(SOURCE.lastIndexOf('\n', m.index), m.index), /try \{/, 'storage only inside try');
  assert.match(CSS, /@media \(prefers-reduced-motion: reduce\) \{ \.pocol-marker-glow\.is-animated \{ animation: none; \} \}/);
});

test('amounts and percentages: BigInt from decimal strings, truncated for display, exact in the title', () => {
  const W = 10n ** 18n;
  assert.deepEqual(M.formatPcol(1234567n * W + 891n * 10n ** 13n), { text: '1,234,567.0089', full: '1,234,567.00891' });
  assert.deepEqual(M.formatPcol(1n), { text: '<0.0001', full: '0.000000000000000001' });
  assert.deepEqual(M.formatPcol(0n), { text: '0', full: '0' });
  assert.deepEqual(M.formatPcol(5n * W + 999999n * 10n ** 12n, 2), { text: '5.99', full: '5.999999' }, 'truncated, never rounded up');
  assert.equal(M.formatPcol(-3n * W).text, '−3');
  assert.throws(() => M.formatPcol(1), TypeError);
  assert.deepEqual(M.formatPercentE10(123456789012n), { text: '12.34%', full: '12.3456789012%' });
  assert.equal(M.formatPercentE10(100n * 10n ** 10n).text, '100.00%');
  assert.equal(M.formatPercentE10(1n).text, '<0.01%');
  assert.equal(M.formatPercentE10(0n).text, '0.00%');
  assert.equal(M.shareE10(3, 10), 300000000000n);
  assert.equal(M.shareE10(1n, 3n), 333333333333n, 'floor');
  assert.equal(M.shareE10(5, 0), 0n);
  assert.equal(M.pageCount(0, 25), 1); assert.equal(M.pageCount(140, 25), 6); assert.equal(M.pageCount(150, 50), 3);
  assert.equal(M.groupDigits('1234567890'), '1,234,567,890');
});

// ---- validators ----
const SHARED = M.readShared({ chainId: DECLARED.chainId, genesisHash: DECLARED.genesisHash, countries: [
  { id: 'USA', code: 'US', name: 'United States of America', continent: 'North America', longitude: -97.48, latitude: 39.54, path: 'M1,2L3,4Z' }] });
const W = 10n ** 18n, h64 = (c) => '0x' + c.repeat(64), a40 = (n) => '0x' + n.toString(16).padStart(40, '0');
const ID = { chainId: DECLARED.chainId, genesisHash: DECLARED.genesisHash };
const clone = (x) => JSON.parse(JSON.stringify(x));
function holdersFixture() {
  const supply = 1000n * W, totals = [600n * W, 300n * W, 100n * W];
  return { schema: 'pocol-holders-v1', ...ID, page: 1, size: 25, order: 'balance_desc', type: 'all', q: '', total: 3,
    snapshot: { height: 50, hash: h64('a'), timestamp: 1795300000 }, index: { state: 'ready', head: 51, historyHeight: 51, updatedAt: 1795300005 },
    supply: { currentSupplyWei: supply.toString() }, verification: { complete: true },
    rows: [
      { rank: 1, address: a40(1), type: 'reward', label: null, minerIds: [a40(2)], rewardAddress: null, totalWei: totals[0].toString(), availableWei: totals[0].toString(), bondedWei: '0', sharePercentE10: '600000000000', transactions: 2, blocksMined: null, rewardsWei: '5' },
      { rank: 2, address: a40(2), type: 'miner', label: null, minerIds: [], rewardAddress: a40(1), totalWei: totals[1].toString(), availableWei: (280n * W).toString(), bondedWei: (20n * W).toString(), sharePercentE10: '300000000000', transactions: 1, blocksMined: 7, rewardsWei: null },
      { rank: 3, address: '0x0000000000000000000000000000000000c0c002', type: 'system', label: 'RewardEscrow', minerIds: [], rewardAddress: null, totalWei: totals[2].toString(), availableWei: (120n * W).toString(), bondedWei: '0', sharePercentE10: '100000000000', transactions: 0, blocksMined: null, rewardsWei: '0' }] };
}
const REQ = { page: 1, size: 25, order: 'balance_desc', type: 'all', q: '', pin: null };

test('a holders page is validated before it is shown; every broken field is refused', () => {
  const ok = M.validateHoldersPage(holdersFixture(), REQ, SHARED);
  assert.equal(ok.rows.length, 3); assert.equal(ok.rows[1].bondedWei, 20n * W); assert.equal(ok.rows[0].share, 600000000000n);
  const asc = holdersFixture(); asc.order = 'balance_asc'; asc.rows.reverse(); asc.rows.forEach((r, i) => { r.rank = 3 - i; });
  assert.equal(M.validateHoldersPage(asc, { ...REQ, order: 'balance_asc' }, SHARED).rows[0].rank, 3);
  const pinned = { ...REQ, page: 1, pin: { height: 50, hash: h64('a') } };
  assert.equal(M.validateHoldersPage(holdersFixture(), pinned, SHARED).snapshot.height, 50);
  const broken = {
    'schema': (d) => { d.schema = 'pocol-holders-v2'; }, 'chain ID': (d) => { d.chainId += 1; }, 'genesis hash': (d) => { d.genesisHash = h64('b'); },
    'query echo': (d) => { d.size = 50; }, 'row count': (d) => { d.total = 30; }, 'rank': (d) => { d.rows[1].rank = 5; },
    'total = available + bonded': (d) => { d.rows[1].availableWei = '1'; }, 'share of supply': (d) => { d.rows[0].sharePercentE10 = '600000000001'; },
    'type': (d) => { d.rows[0].type = 'whale'; }, 'label': (d) => { d.rows[2].label = 'Bank'; }, 'address': (d) => { d.rows[0].address = d.rows[0].address.toUpperCase(); },
    'total': (d) => { d.rows[0].totalWei = '1e21'; }, 'duplicate row': (d) => { d.rows[1].address = d.rows[0].address; },
    'reward address': (d) => { d.rows[0].rewardAddress = a40(9); }, 'miner IDs': (d) => { d.rows[0].minerIds = []; },
    'bonded': (d) => { d.rows[0].bondedWei = '1'; d.rows[0].availableWei = (600n * W - 1n).toString(); },
    'index state': (d) => { d.index.state = 'done'; }, 'snapshot hash': (d) => { d.snapshot.hash = '0x12'; }, 'current supply': (d) => { d.supply.currentSupplyWei = '0'; },
    'verification': (d) => { d.verification = {}; }, 'transactions': (d) => { d.rows[0].transactions = -1; }, 'rewards': (d) => { d.rows[0].rewardsWei = 5; },
    'order': (d) => { [d.rows[0].totalWei, d.rows[1].totalWei] = [d.rows[1].totalWei, d.rows[0].totalWei]; },
  };
  for (const [what, change] of Object.entries(broken)) {
    const d = holdersFixture(); change(d);
    assert.throws(() => M.validateHoldersPage(d, REQ, SHARED), M.Invalid, what);
  }
  assert.throws(() => M.validateHoldersPage(holdersFixture(), { ...REQ, pin: { height: 49, hash: h64('a') } }, SHARED), /pinned snapshot/);
  assert.throws(() => M.validateHoldersPage(holdersFixture(), { ...REQ, type: 'miner' }, SHARED), /query echo/);
  const filtered = holdersFixture(); filtered.type = 'miner'; filtered.total = 1; filtered.rows = [filtered.rows[1]];
  assert.equal(M.validateHoldersPage(filtered, { ...REQ, type: 'miner' }, SHARED).rows[0].rank, 2, 'a filtered page keeps the global rank');
  const searched = holdersFixture(); searched.q = '0x00000000000000000000000000000000000000'; searched.total = 2; searched.rows = searched.rows.slice(0, 2);
  assert.equal(M.validateHoldersPage(searched, { ...REQ, q: searched.q }, SHARED).rows.length, 2);
  searched.rows[1].address = '0x1' + searched.rows[1].address.slice(3);
  assert.throws(() => M.validateHoldersPage(searched, { ...REQ, q: searched.q }, SHARED), /search match/);
});

function statsFixture() {
  const S = 1000n * W, base = 3n, prot = 7n, genesis = 900n * W, minted = S + base + prot;
  const B = [0n, 1n, 10n, 100n, 1000n, 10000n, 100000n, 1000000n].map((x) => x * W);
  const distribution = B.map((lo, i) => ({ minWei: lo.toString(), maxWei: i + 1 < B.length ? B[i + 1].toString() : null, accounts: 0, totalWei: '0' }));
  distribution[3].accounts = 2; distribution[3].totalWei = (900n * W).toString();
  return { schema: 'pocol-holders-stats-v1', ...ID, snapshot: { height: 50, hash: h64('a'), timestamp: 1 }, index: { state: 'syncing', head: 60, historyHeight: 50, updatedAt: 2 },
    supply: { genesisWei: genesis.toString(), issuedWei: (minted - genesis).toString(), mintedWei: minted.toString(), baseFeeBurnedWei: '3', protocolBurnedWei: '7', totalBurnedWei: '10',
      currentSupplyWei: S.toString(), circulatingWei: (880n * W).toString(), availableWei: (1000n * W).toString(), bondedWei: (20n * W).toString(), systemWei: (100n * W).toString(), pendingRewardsWei: (100n * W).toString() },
    verification: { complete: true, balancesSumWei: '1', expectedSumWei: '1', unlocatedWei: '0', accountsIndexed: 3, accountsOnChain: 3, accountsMissing: 0 },
    holders: { count: 2, withAvailable: 2, withBonded: 1, new24h: null, new24hFrom: null },
    concentration: { top1: { totalWei: (600n * W).toString(), sharePercentE10: '600000000000' }, top10: { totalWei: (900n * W).toString(), sharePercentE10: '900000000000' },
      top50: { totalWei: (900n * W).toString(), sharePercentE10: '900000000000' }, top100: { totalWei: (900n * W).toString(), sharePercentE10: '900000000000' } },
    distribution, history: { from: 40, points: [{ height: 40, timestamp: 1, holders: 1, currentSupplyWei: '5', top10SharePercentE10: '0' }, { height: 50, timestamp: 2, holders: 2, currentSupplyWei: '6', top10SharePercentE10: '900000000000' }] },
    rewards: { from: 30, to: 50 } };
}

test('the statistics are validated: identities of the supply, shares, buckets, history order', () => {
  const ok = M.validateStats(statsFixture(), SHARED);
  assert.equal(ok.supply.currentSupplyWei, 1000n * W); assert.equal(ok.history.points.length, 2); assert.equal(ok.rewards.from, 30);
  const noRewards = statsFixture(); delete noRewards.rewards; assert.equal(M.validateStats(noRewards, SHARED).rewards, null);
  const broken = {
    'minted = genesis + issued': (d) => { d.supply.issuedWei = '1'; }, 'burned = base fees + protocol': (d) => { d.supply.totalBurnedWei = '11'; },
    'supply = minted − burned': (d) => { d.supply.currentSupplyWei = '5'; }, 'concentration share': (d) => { d.concentration.top10.sharePercentE10 = '1'; },
    'concentration order': (d) => { d.concentration.top1.totalWei = (950n * W).toString(); d.concentration.top1.sharePercentE10 = '950000000000'; },
    'distribution bounds': (d) => { d.distribution[2].maxWei = '5'; }, 'distribution count': (d) => { d.holders.count = 3; },
    'distribution total in its range': (d) => { d.distribution[3].totalWei = '1'; }, 'last bucket open': (d) => { d.distribution[7].maxWei = '1'; },
    'history order': (d) => { d.history.points.reverse(); d.history.from = 50; }, 'history after the snapshot': (d) => { d.history.points[1].height = 51; },
    'history start': (d) => { d.history.from = 41; }, 'new holders pair': (d) => { d.holders.new24h = 1; },
    'verification': (d) => { d.verification.unlocatedWei = '1'; }, 'stats chain': (d) => { d.chainId = 1; }, 'schema': (d) => { d.schema = 'pocol-holders-v1'; },
  };
  for (const [what, change] of Object.entries(broken)) { const d = statsFixture(); change(d); assert.throws(() => M.validateStats(d, SHARED), M.Invalid, what); }
  const none = statsFixture(); none.history = { from: null, points: [] }; assert.equal(M.validateStats(none, SHARED).history.from, null);
  const incomplete = statsFixture(); Object.assign(incomplete.verification, { complete: false, expectedSumWei: '10', unlocatedWei: '9', accountsOnChain: 5, accountsMissing: 2 });
  assert.equal(M.validateStats(incomplete, SHARED).verification.complete, false);
  // The account count read at another block is published as null (pocol_holders.py); unlocated may be negative.
  const noCount = statsFixture(); Object.assign(noCount.verification, { accountsOnChain: null, accountsMissing: null });
  assert.equal(M.validateStats(noCount, SHARED).verification.accountsMissing, null);
  const over = statsFixture(); Object.assign(over.verification, { complete: false, balancesSumWei: '3', unlocatedWei: '-2' });
  assert.equal(M.validateStats(over, SHARED).verification.unlocatedWei, -2n);
  for (const [what, change] of Object.entries({
    'one count null': (v) => { v.accountsOnChain = null; }, 'complete with a difference': (v) => { v.unlocatedWei = '-1'; v.balancesSumWei = '2'; },
    'unlocated not expected − balances': (v) => { v.complete = false; v.unlocatedWei = '5'; }, 'minus zero': (v) => { v.unlocatedWei = '-0'; },
  })) { const d = statsFixture(); change(d.verification); assert.throws(() => M.validateStats(d, SHARED), M.Invalid, what); }
});

function geoFixture() {
  return { schema: 'pocol-miners-geography-v2', ...ID, generatedAt: 100, height: 8640, blockHash: h64('c'),
    database: { provider: 'DB-IP Lite', release: '2026-11', license: 'CC BY 4.0' },
    miners: { registered: 4, active: 4, inactive: 1, exited: 0, punished: 0, located: 3, unknown: 1, connected: 3, thetaInact: 360 },
    nodes: { observed: 5, located: 5, unlocated: 0 },
    sources: [{ kind: 'observer', name: 'explorer', fresh: true, reportedAt: 99 }, { kind: 'operator', name: 'founder-1', fresh: true, reportedAt: 90 }, { kind: 'operator', name: 'founder-2', fresh: false, reportedAt: null }],
    blocksWindow: { from: 1, to: 8640, blocks: 8640 },
    countries: [{ code: 'US', miners: { located: 3, active: 3, inactive: 1, connected: 3 }, nodes: 5, blocks: 6480, blocksSharePercentE10: '750000000000', rewardsWei: '12' }],
    unknown: { miners: 1, blocks: 2160 } };
}

test('the geography answer is validated: counts add up, shares are exact, nothing finer than a country', () => {
  const ok = M.validateGeography(geoFixture(), SHARED);
  assert.equal(ok.countries[0].blocksShare, 750000000000n); assert.equal(ok.unknownActive, 1); assert.equal(ok.unknownInactive, 0);
  const noDb = geoFixture(); noDb.database = null; assert.equal(M.validateGeography(noDb, SHARED).database, null);
  const broken = {
    'located + unknown': (d) => { d.miners.unknown = 2; }, 'inactive ≤ active': (d) => { d.miners.inactive = 5; }, 'nodes': (d) => { d.nodes.unlocated = 1; },
    'country code': (d) => { d.countries[0].code = 'usa'; }, 'country totals': (d) => { d.countries[0].nodes = 4; }, 'block share': (d) => { d.countries[0].blocksSharePercentE10 = '1'; },
    'blocks add up': (d) => { d.unknown.blocks = 1; }, 'database': (d) => { d.database.provider = 'Other'; }, 'release': (d) => { d.database.release = '2026-13'; },
    'source kind': (d) => { d.sources[0].kind = 'peer'; }, 'fresh without time': (d) => { d.sources[1].reportedAt = null; }, 'duplicate source': (d) => { d.sources[2].name = 'founder-1'; },
    'window': (d) => { d.blocksWindow.blocks = 9000; }, 'country miners': (d) => { d.countries[0].miners.active = 4; },
    'blocks without a located miner': (d) => { d.countries.push({ code: 'DE', miners: { located: 0, active: 0, inactive: 0, connected: 0 }, nodes: 0, blocks: 1, blocksSharePercentE10: '1157407407', rewardsWei: '0' }); d.unknown.blocks -= 1; },
    'rewards known for some only': (d) => { d.nodes = { observed: 6, located: 6, unlocated: 0 }; d.countries.push({ code: 'DE', miners: { located: 0, active: 0, inactive: 0, connected: 0 }, nodes: 1, blocks: 0, blocksSharePercentE10: '0', rewardsWei: null }); },
    'chain': (d) => { d.genesisHash = h64('d'); },
  };
  for (const [what, change] of Object.entries(broken)) { const d = geoFixture(); change(d); assert.throws(() => M.validateGeography(d, SHARED), M.Invalid, what); }
  assert.throws(() => M.readShared(undefined), M.Invalid, 'without the bundle\'s identity nothing is checked');
  assert.throws(() => M.readShared({ chainId: '1', genesisHash: h64('a'), countries: [] }), M.Invalid);
  assert.equal(SHARED.shapes.get('US').name, 'United States of America');
});

test('the map: projection, zoom bounds, clustering at the current zoom, ring segments, choropleth ranges', () => {
  assert.deepEqual(M.project(-180, 90), [0, 0]); assert.deepEqual(M.project(180, -90), [1000, 500]); assert.deepEqual(M.project(0, 0), [500, 250]);
  assert.deepEqual(M.clampView({ x: -50, y: 900, w: 4000 }), { x: 0, y: 0, w: 1000, h: 500 });
  const z = M.zoomView({ x: 0, y: 0, w: 1000, h: 500 }, 2, 250, 125);
  assert.deepEqual(z, { x: 125, y: 62.5, w: 500, h: 250 }, 'the point under the cursor stays put');
  assert.equal(M.zoomView(z, 100).w, 1000 / 16, 'at most 16 times');
  assert.equal(M.zoomView(z, 0.01).w, 1000);
  const pts = [{ key: 'DE', x: 527, y: 108, weight: 2 }, { key: 'FR', x: 507, y: 120, weight: 1 }, { key: 'US', x: 229, y: 140, weight: 3 }];
  const world = M.cluster(pts, 1000 / 1100), zoomed = M.cluster(pts, 250 / 1100);
  assert.deepEqual(world.map((c) => c.members.map((m) => m.key)), [['US'], ['DE', 'FR']], 'close countries share a marker at the world view');
  assert.equal(zoomed.length, 3, 'and separate once zoomed in');
  assert.deepEqual(M.ringSegments(3, 3, 1).map((s) => [s.kind, +s.from.toFixed(3), +s.length.toFixed(3)]), [['active', 0, 0.667], ['inactive', 0.667, 0.333]]);
  assert.deepEqual(M.ringSegments(4, 2, 0).map((s) => s.kind), ['active', 'other']);
  assert.deepEqual(M.ringSegments(0, 0, 0), []);
  // The marker core follows the legend: blue only while a located miner is active and sharing (F2).
  assert.equal(M.markerCore(null), '', 'the nodes layer is blue');
  assert.equal(M.markerCore({ located: 3, active: 3, inactive: 1 }), '');
  assert.equal(M.markerCore({ located: 2, active: 2, inactive: 2 }), 'is-inactive', 'every active one inactive: grey');
  assert.equal(M.markerCore({ located: 1, active: 0, inactive: 0 }), 'is-other', 'located, none active: not the active blue');
  assert.equal(M.markerCore({ located: 3, active: 1, inactive: 1 }), 'is-inactive');
  assert.match(CSS, /\.pocol-marker-core\.is-other \{ fill: #fff; stroke: #c9d6e0; \}/);
  // Each doc comment sits directly above the function it describes.
  assert.match(SOURCE, /\/\*\* Ring segments [^/]*\*\/\nexport function ringSegments\(/, 'the ring segments comment is above ringSegments');
  assert.match(SOURCE, /\/\*\* The marker core's class [^/]*\*\/\nexport function markerCore\(/, 'the marker core comment is above markerCore');
  assert.deepEqual(M.choroplethLevels(3), [{ level: 1, lo: 1, hi: 1 }, { level: 2, lo: 2, hi: 2 }, { level: 3, lo: 3, hi: 3 }]);
  assert.deepEqual(M.choroplethLevels(100).map((l) => [l.lo, l.hi]), [[1, 25], [26, 50], [51, 75], [76, 100]]);
  assert.deepEqual(M.choroplethLevels(0), []);
  assert.deepEqual(M.niceTicks(52), [0, 20, 40, 60]); assert.deepEqual(M.niceTicks(3), [0, 1, 2, 3]); assert.deepEqual(M.niceTicks(0), [0, 1]);
});

test('the language: stored choice first, else the browser\'s; every text in both languages', () => {
  assert.equal(M.pickLanguage('ar', 'en-US'), 'ar'); assert.equal(M.pickLanguage('en', 'ar-EG'), 'en');
  assert.equal(M.pickLanguage(null, 'ar-SA'), 'ar'); assert.equal(M.pickLanguage(null, 'fr'), 'en'); assert.equal(M.pickLanguage('xx', undefined), 'en');
  const keys = (o, p = '') => Object.entries(o).flatMap(([k, v]) => (v && typeof v === 'object' && !Array.isArray(v) ? keys(v, p + k + '.') : [p + k])).sort();
  assert.deepEqual(keys(M.TEXTS.ar), keys(M.TEXTS.en), 'the same keys in Arabic and English');
  for (const k of ['notes', 'mapNotes', 'attribution']) assert.equal(M.TEXTS.ar[k].length, M.TEXTS.en[k].length, k);
  const arabic = JSON.stringify(Object.values(M.TEXTS.ar).map((v) => (typeof v === 'function' ? v(1, 2, 3, 4) : v)));
  assert.doesNotMatch(arabic, /[٠-٩]/, 'Western digits only');
  // The stated limits (§7) are in both languages.
  assert.equal(M.TEXTS.en.mapNotes.length, 4); assert.equal(M.TEXTS.en.notes.length, 5);
  // DB-IP's own link-back wording (verbatim in both languages), the CC BY 4.0 licence and the conversion notice (§3(a)).
  assert.equal(M.TEXTS.en.attribution.join(''), 'Map: Natural Earth · IP Geolocation by DB-IP, licensed under CC BY 4.0 and converted to PoCol’s internal format');
  assert.equal(M.TEXTS.ar.attribution.join(''), 'الخريطة: Natural Earth · IP Geolocation by DB-IP، مرخّصة بموجب CC BY 4.0 ومحوّلة إلى صيغة PoCol الداخلية');
  for (const lang of ['en', 'ar']) assert.deepEqual([1, 3, 5].map((i) => M.TEXTS[lang].attribution[i]), ['Natural Earth', 'IP Geolocation by DB-IP', 'CC BY 4.0'], lang);
});

test('the Websites workflow runs on a change of what its publisher step builds and runs, on pull requests and pushes alike', () => {
  // The "Explorer S1 publication plan" step runs npm ci, npm run build and tests/publisher_test.mjs in the extension.
  const workflow = readFileSync(join(ROOT, '.github', 'workflows', 'websites.yml'), 'utf8').replace(/\r\n/g, '\n');
  assert.match(workflow, /npm ci --ignore-scripts\n\s+npm run build\n\s+node --test tests\/publisher_test\.mjs\n\s+working-directory: prototypes\/pocol-extension\n/);
  const filters = (event) => {
    const block = new RegExp(`\\n  ${event}:\\n[\\s\\S]*?\\n    paths:\\n((?:      .*\\n)+)`).exec(workflow);
    assert.ok(block, `the ${event} trigger has path filters`);
    return block[1].split('\n').map((l) => /^      - '([^']+)'$/.exec(l)?.[1]).filter(Boolean);
  };
  const pr = filters('pull_request'), push = filters('push');
  for (const path of ['prototypes/pocol-extension/tests/publisher_test.mjs', 'prototypes/pocol-extension/build.mjs', 'prototypes/pocol-extension/package-lock.json',
    'prototypes/pocol-extension/package.json', 'deploy/websites/**']) {
    assert.ok(pr.includes(path), `pull_request paths name ${path}`);
    assert.ok(push.includes(path), `push paths name ${path}`);
  }
  assert.deepEqual(push, pr, 'push has the same paths as pull_request');
});
