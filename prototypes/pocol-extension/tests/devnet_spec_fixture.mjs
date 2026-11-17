// W13: data/devnet-spec-v3.hex is what the Rust tool writes, not a hand-made file. This reruns the `pocold init-devnet`
// command recorded in rp_fixture.mjs (the one place it is written down) into a temporary directory and compares the
// genesis.spec.hex it writes with the committed fixture, byte for byte. The committed file may carry one final newline
// that the tool does not write; nothing else may differ. data/devnet-spec.hex stays the spec-1 fixture: it is checked
// to be a different genesis, never rewritten.
//
// Usage: node tests/devnet_spec_fixture.mjs [path/to/pocold]   (default: POCOLD_BIN_DIR/pocold, else
// prototypes/pocold/target/release/pocold). A missing binary is a failure, not a skip.
import { readFileSync, mkdtempSync, rmSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { tmpdir } from 'node:os';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const HERE = dirname(fileURLToPath(import.meta.url));
const DATA = join(HERE, 'data');
const RECORD = join(HERE, 'rp_fixture.mjs');
const WORKFLOW = join(HERE, '..', '..', '..', '.github', 'workflows', 'pocol-browser-verify.yml');
const STEP_NAME = 'Devnet spec fixture regenerates byte for byte (W13)';

/** The init-devnet arguments recorded in rp_fixture.mjs's comment, with `--out DIR` taken out. */
export function recordedInitDevnetArgs(src = readFileSync(RECORD, 'utf8')) {
  const lines = src.split('\n');
  const starts = lines.flatMap((l, i) => (/^\/\/\s+pocold init-devnet\s/.test(l) ? [i] : []));
  if (starts.length !== 1) throw new Error(`rp_fixture.mjs must record the pocold init-devnet command exactly once (found ${starts.length})`);
  let text = '';
  for (let i = starts[0]; ; i++) {
    if (i >= lines.length || !lines[i].startsWith('//')) throw new Error('the recorded init-devnet command ends in a line continuation');
    const part = lines[i].replace(/^\/\/\s*/, '').trimEnd();
    if (!part.endsWith('\\')) { text += part; break; }
    text += part.slice(0, -1) + ' ';
  }
  const words = text.trim().split(/\s+/);
  if (words[0] !== 'pocold' || words[1] !== 'init-devnet') throw new Error('not an init-devnet command: ' + text);
  const args = words.slice(2);
  // Only plain flags and values: the command is run without a shell, and nothing in it may name another path.
  for (const a of args) if (!/^(--[a-z][a-z-]*|[A-Za-z0-9,]+)$/.test(a)) throw new Error('unexpected word in the recorded command: ' + a);
  const out = args.indexOf('--out');
  if (out < 0 || args[out + 1] !== 'DIR' || args.indexOf('--out', out + 1) >= 0) throw new Error('the recorded command must write to --out DIR once');
  args.splice(out, 2);
  if (args.includes('DIR')) throw new Error('DIR may appear only after --out');
  return args;
}

/** null when `committed` is `generated`, or `generated` plus one final newline; otherwise why not. */
export function fixtureMismatch(generated, committed) {
  const body = committed.length === generated.length + 1 && committed[committed.length - 1] === 0x0a ? committed.subarray(0, -1) : committed;
  if (Buffer.compare(Buffer.from(body), Buffer.from(generated)) === 0) return null;
  let i = 0; while (i < body.length && i < generated.length && body[i] === generated[i]) i++;
  return `differs at byte ${i} (committed ${committed.length} bytes, generated ${generated.length} bytes)`;
}

/**
 * null when the fixture step in pocol-browser-verify.yml cannot cause any other step to be skipped; otherwise why not.
 * A step with no `if:` (or one without always()) runs only when every earlier step passed, so a fixture mismatch, the
 * failure this step exists to report, would also hide the evidence of every such step after it.
 */
export function workflowGatingProblem(src) {
  const steps = [];
  for (const line of src.split('\n')) {
    const m = /^      - (?:name: (.*)|.*)$/.exec(line);
    if (m) { steps.push({ name: (m[1] || line.trim()).trim(), if: null }); continue; }
    const c = /^        if: (.*)$/.exec(line);
    if (c && steps.length) steps[steps.length - 1].if = c[1].trim();
  }
  const at = steps.findIndex((s) => s.name === STEP_NAME);
  if (at < 0 || steps.findIndex((s, i) => i > at && s.name === STEP_NAME) >= 0) return `the workflow must have the step "${STEP_NAME}" exactly once`;
  if (!/^always\(\) && steps\.build\.outcome == 'success'$/.test(steps[at].if || '')) return 'the fixture step must run after a successful build whatever else failed';
  const gated = steps.slice(at + 1).filter((s) => !/\balways\(\)/.test(s.if || '')).map((s) => s.name);
  return gated.length ? `a fixture failure would skip later steps: ${gated.join('; ')}` : null;
}

export function devnetSpecFixtureChecks(pocold) {
  const checks = [], ok = (name, pass, detail = '') => checks.push({ name, pass: !!pass, detail });
  const committed = readFileSync(join(DATA, 'devnet-spec-v3.hex'));
  const spec1 = readFileSync(join(DATA, 'devnet-spec.hex'));
  let args; try { args = recordedInitDevnetArgs(); } catch (e) { ok('the init-devnet command is recorded in rp_fixture.mjs', false, e.message); return checks; }
  ok('the init-devnet command is recorded in rp_fixture.mjs', true, args.join(' '));
  if (!existsSync(pocold)) { ok('pocold is built', false, pocold); return checks; }
  const work = mkdtempSync(join(tmpdir(), 'pocol-devnet-spec-'));
  try {
    const out = join(work, 'devnet');
    const r = spawnSync(pocold, ['init-devnet', '--out', out, ...args], { encoding: 'utf8', timeout: 120000 });
    ok('pocold init-devnet runs the recorded command', r.status === 0, r.status === 0 ? '' : `exit ${r.status} ${r.error ? r.error.message : ''} ${(r.stderr || '').slice(-300)}`);
    if (r.status !== 0) return checks;
    const generated = readFileSync(join(out, 'genesis.spec.hex'));
    const why = fixtureMismatch(generated, committed);
    ok('data/devnet-spec-v3.hex is byte for byte the genesis.spec.hex the recorded command writes', why === null, why || '');
    ok('data/devnet-spec.hex (the spec-1 fixture) is a different genesis, kept as it is', fixtureMismatch(generated, spec1) !== null);
    // The comparison itself: a single changed byte, a missing byte or an extra one is caught.
    const flipped = Uint8Array.from(generated); flipped[flipped.length >> 1] ^= 1;
    ok('the comparison refuses a changed, truncated or extended fixture',
      fixtureMismatch(generated, flipped) !== null && fixtureMismatch(generated, generated.subarray(0, -1)) !== null
      && fixtureMismatch(generated, Buffer.concat([generated, Buffer.from('\n\n')])) !== null
      && fixtureMismatch(generated, Buffer.concat([generated, Buffer.from('0')])) !== null);
  } finally {
    rmSync(work, { recursive: true, force: true }); // init-devnet's throwaway devnet files never outlive the check
  }
  return checks;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const pocold = process.argv[2] || join(process.env.POCOLD_BIN_DIR || join(HERE, '..', '..', 'pocold', 'target', 'release'), 'pocold');
  const refuses = (src) => { try { recordedInitDevnetArgs(src); return false; } catch { return true; } };
  const checks = [
    { name: 'the command reader refuses a missing, repeated, shell-like or unterminated record', pass:
      refuses('') && refuses('//   pocold init-devnet --out DIR --c 2\n//   pocold init-devnet --out DIR --c 2\n')
      && refuses('//   pocold init-devnet --out DIR --c $(x)\n') && refuses('//   pocold init-devnet --out /tmp/x --c 2\n')
      && refuses('//   pocold init-devnet --out DIR --c 2 \\\nconst x = 1;\n')
      && JSON.stringify(recordedInitDevnetArgs('//   pocold init-devnet --out DIR --miners a,b \\\n//     --c 2\n')) === '["--miners","a,b","--c","2"]', detail: '' },
    (() => {
      const why = workflowGatingProblem(readFileSync(WORKFLOW, 'utf8'));
      return { name: 'pocol-browser-verify runs this check where a failure skips no other step', pass: why === null, detail: why || '' };
    })(),
    { name: 'the workflow reader catches a later step that a fixture failure would skip', pass:
      workflowGatingProblem(`      - name: ${STEP_NAME}\n        if: always() && steps.build.outcome == 'success'\n      - name: later\n        run: true\n`) !== null
      && workflowGatingProblem(`      - name: ${STEP_NAME}\n        if: always() && steps.build.outcome == 'success'\n      - name: later\n        if: always()\n`) === null
      && workflowGatingProblem(`      - name: ${STEP_NAME}\n      - name: later\n        if: always()\n`) !== null
      && workflowGatingProblem('      - name: other\n') !== null, detail: '' },
    ...devnetSpecFixtureChecks(pocold),
  ];
  let failed = 0;
  for (const c of checks) { console.log((c.pass ? 'PASS ' : 'FAIL ') + c.name + (c.detail ? ' — ' + c.detail : '')); if (!c.pass) failed++; }
  console.log(`${checks.length - failed}/${checks.length} devnet spec fixture checks pass`);
  process.exit(failed ? 1 : 0);
}
