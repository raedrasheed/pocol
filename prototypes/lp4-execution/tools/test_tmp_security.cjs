// Run after the locked npm ci in this directory. All files and symlink targets
// belong to a fresh test directory; the SMT process below is a local fixture.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const { createRequire } = require('node:module');
const os = require('node:os');
const path = require('node:path');
const test = require('node:test');

const tools = path.resolve(process.env.POCOL_COMPILER_TOOLS || __dirname);
const fromTools = createRequire(path.join(tools, 'package.json'));
// Resolve from solc itself so a nested override is checked as well.
const fromSolc = createRequire(fromTools.resolve('solc'));
const tmp = fromSolc('tmp');

function directories(t) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'pocol-tmp-test-'));
  const sandbox = path.join(root, 'sandbox');
  fs.mkdirSync(sandbox);
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return { root, sandbox };
}

function rejectsUnsafeOptions(options, pattern, sandbox) {
  let file;
  try {
    assert.throws(() => {
      file = tmp.fileSync(options);
      // On the historical dependency, record the actual write before failing.
      const relative = path.relative(sandbox, fs.realpathSync(file.name));
      console.log(JSON.stringify({ unsafe_options_accepted: true, relative }));
    }, pattern);
  } finally {
    if (file) file.removeCallback();
  }
}

test('SMT-style temporary file can be written and manually removed', t => {
  const { sandbox } = directories(t);
  const file = tmp.fileSync({ tmpdir: sandbox, dir: sandbox, postfix: '.smt2' });
  try {
    assert.equal(path.dirname(fs.realpathSync(file.name)), fs.realpathSync(sandbox));
    assert.ok(file.name.endsWith('.smt2'));
    fs.writeFileSync(file.name, '(check-sat)\n');
    assert.equal(fs.readFileSync(file.name, 'utf8'), '(check-sat)\n');
    if (process.platform !== 'win32') {
      assert.equal(fs.statSync(file.name).mode & 0o777, 0o600);
    }
  } finally {
    file.removeCallback();
  }
  assert.equal(fs.existsSync(file.name), false);
});

for (const [option, value] of [
  ['prefix', '../escape-prefix-'],
  ['postfix', '/../../escape-postfix.smt2'],
  ['prefix', ['../escape-array-prefix-']],
  ['postfix', ['/../../escape-array-postfix.smt2']],
]) {
  test(`rejects ${Array.isArray(value) ? 'array' : 'string'} traversal in ${option}`, t => {
    const { sandbox } = directories(t);
    rejectsUnsafeOptions(
      { tmpdir: sandbox, dir: sandbox, [option]: value },
      /Relative value not allowed|option must be a string/,
      sandbox,
    );
  });
}

test('rejects a dir symlink leading outside the selected temporary root', t => {
  const { root, sandbox } = directories(t);
  const outside = path.join(root, 'outside');
  const link = path.join(sandbox, 'redirect');
  fs.mkdirSync(outside);
  fs.symlinkSync(outside, link, process.platform === 'win32' ? 'junction' : 'dir');
  rejectsUnsafeOptions(
    { tmpdir: sandbox, dir: link, postfix: '.smt2' },
    /dir option must be relative/,
    sandbox,
  );
  assert.deepEqual(fs.readdirSync(outside), []);
});

function shellQuote(value) {
  return "'" + value.split("'").join("'\\''") + "'";
}

test('solc SMT helper keeps its file API and cleans up after a local solver fixture', {
  skip: process.platform === 'win32' ? 'Local solver fixture uses POSIX shell quoting' : false,
}, t => {
  const { root } = directories(t);
  const script = path.join(root, 'solver.cjs');
  const report = path.join(root, 'solver-report.json');
  fs.writeFileSync(script, `
    const fs = require('node:fs');
    const [report, input] = process.argv.slice(2);
    if (fs.readFileSync(input, 'utf8') !== '(check-sat)\\n') process.exit(2);
    fs.writeFileSync(report, JSON.stringify({ input }));
    process.stdout.write('unsat\\n');
  `);
  const { smtSolver } = fromTools('solc/smtsolver');
  const output = smtSolver('(check-sat)\n', {
    name: 'local compatibility fixture',
    command: shellQuote(process.execPath),
    params: [script, report].map(shellQuote).join(' '),
  });
  assert.equal(output, 'unsat\n');
  const { input } = JSON.parse(fs.readFileSync(report, 'utf8'));
  assert.ok(input.endsWith('.smt2'));
  assert.equal(fs.existsSync(input), false);
});
