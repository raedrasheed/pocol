// Ground truth from native JavaScript JSON and UTF-16/UTF-8 behavior, not an author model.
import fs from 'node:fs';
const rows = [];
for (const n of [9007199254740991n, 9007199254740993n, 18446744073709551615n]) {
  const number = JSON.parse('{"epoch":' + n.toString() + '}').epoch;
  const hex = n.toString(16).padStart(16, '0');
  const decimal = n.toString();
  const h = JSON.parse(JSON.stringify({ epoch: hex })).epoch;
  const d = JSON.parse(JSON.stringify({ epoch: decimal })).epoch;
  rows.push({ integer: decimal, parsedNumberExactInteger: BigInt(number).toString(),
    numberPreservesValue: BigInt(number) === n, hexRoundTrip: BigInt('0x' + h) === n, decimalRoundTrip: BigInt(d) === n });
}
const strings = ['\ud800', '\ud801', '\ufffd', '\ud83d\ude00', 'A'];
const utf8 = strings.map((s) => ({ utf16Units: Array.from({ length: s.length }, (_, i) => s.charCodeAt(i).toString(16).padStart(4, '0')),
  utf8Hex: Buffer.from(new TextEncoder().encode(s)).toString('hex'),
  utf8Bytes: new TextEncoder().encode(s).length,
  jsonRoundTripPreservesUnits: JSON.parse(JSON.stringify(s)) === s }));
const report = { node: process.version, numeric: rows, strings: utf8,
  distinctLoneSurrogateKeysRemainDistinctBeforeEncoding: strings[0] !== strings[1],
  textEncoderCollision: utf8[0].utf8Hex === utf8[1].utf8Hex && utf8[1].utf8Hex === utf8[2].utf8Hex,
  mathematicalU64MaxLosesOneOnNativeNumericParse: rows[2].parsedNumberExactInteger === '18446744073709551616' };
if (!report.mathematicalU64MaxLosesOneOnNativeNumericParse || !report.textEncoderCollision || !rows.every((r) => r.hexRoundTrip && r.decimalRoundTrip)) {
  throw Error('Unexpected native behavior; reassess representation proposals');
}
fs.writeFileSync(process.argv[2], JSON.stringify(report, null, 2) + '\n');
console.log(JSON.stringify({ numericCases: rows.length, stringCases: utf8.length,
  u64NumericLossConfirmed: report.mathematicalU64MaxLosesOneOnNativeNumericParse, utf8CollisionConfirmed: report.textEncoderCollision }));
