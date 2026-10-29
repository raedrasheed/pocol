// Independent fixture hash cross-check. Dependencies stay in a private test directory.
// node e05-keccak-oracle.mjs INPUT.json PRIVATE_MODULE_DIRECTORY OUTPUT.json
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { pathToFileURL } from 'node:url';
const [inputFile, moduleDirectory, outputFile] = process.argv.slice(2);
if (!inputFile || !moduleDirectory || !outputFile) throw Error('Need input, module directory, output');
const noblePath = path.resolve(moduleDirectory, '@noble/hashes/sha3.js');
const jsPath = path.resolve(moduleDirectory, 'js-sha3/build/sha3.mjs');
const noble = await import(pathToFileURL(noblePath));
const sha3 = await import(pathToFileURL(jsPath));
const input = JSON.parse(fs.readFileSync(inputFile));
const results = input.inputs.map((row) => {
  const bytes = Buffer.from(row.hex, 'hex');
  const a = Buffer.from(noble.keccak_256(bytes)).toString('hex');
  const b = sha3.keccak_256(bytes);
  return { id: row.id, bytes: bytes.length, inputSha256: crypto.createHash('sha256').update(bytes).digest('hex'),
    pythonKeccak: row.pythonKeccak, nobleKeccak: a, jsSha3Keccak: b, knownExpected: row.knownExpected ?? null,
    pass: a === b && a === row.pythonKeccak && (!row.knownExpected || a === row.knownExpected) };
});
const metadata = (name, source) => ({ name, version: JSON.parse(fs.readFileSync(path.resolve(moduleDirectory, name, 'package.json'))).version,
  loadedSourceSha256: crypto.createHash('sha256').update(fs.readFileSync(source)).digest('hex') });
const report = { node: process.version, libraries: [metadata('@noble/hashes', noblePath), metadata('js-sha3', jsPath)],
  pythonLibrary: input.pythonLibrary, pythonLibrarySha256: input.pythonLibrarySha256,
  checks: results.length, passed: results.filter((r) => r.pass).length, failed: results.filter((r) => !r.pass).length, results };
fs.writeFileSync(outputFile, JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ checks: report.checks, passed: report.passed, failed: report.failed }));
if (report.failed) process.exitCode = 1;
