// Reference JSON codec for M1 draft 0.31 (issue C35). SPECIFICATION FIXTURE TOOLING ONLY; not a production dependency.
// Reads ONE JSON-RPC reply that the Python receive guard (tools/guard_ref_031.py) has already accepted (size, fatal UTF-8,
// depth <= 16, no duplicate keys, no NaN/Infinity, envelope id) from stdin, and reports the native facts of
// browser.md:283-285 using the JavaScript engine's own JSON.parse / JSON.stringify:
//   - error.message cut to 256 UTF-16 code units (String.prototype.slice);
//   - error.data kept only if Buffer.byteLength(JSON.stringify(data), 'utf8') <= 4096 (well-formed JSON.stringify:
//     lone surrogates are escaped, numbers follow IEEE 754 / Number::toString, overflow parses to Infinity -> null);
//   - textForChecker: JSON.stringify of the clipped reply ONLY when clipping or dropping changed it, else null
//     (the caller then keeps the original reply text byte for byte).
// Built-ins only (node:process, node:buffer, global TextDecoder). No arguments, files, network, eval, child processes or
// environment reads. Output: one JSON object on stdout; exit code 0 on success, 2 on any refusal.
import process from 'node:process';
import { Buffer } from 'node:buffer';

const MAX_IN = 98304;
const MESSAGE_UNITS_MAX = 256;
const DATA_BYTES_MAX = 4096;

function finish(obj, code) {
  process.stdout.write(JSON.stringify(obj));
  process.exitCode = code;
}

function isObject(v) {
  return v !== null && typeof v === 'object' && !Array.isArray(v);
}

const chunks = [];
let received = 0;
let over = false;
process.stdin.on('data', (chunk) => {
  received += chunk.length;
  if (received > MAX_IN) {
    over = true;
  } else if (!over) {
    chunks.push(chunk);
  }
});
process.stdin.on('end', () => {
  if (over) {
    finish({ ok: false, error: 'inputTooLarge' }, 2);
    return;
  }
  let text;
  try {
    text = new TextDecoder('utf-8', { fatal: true }).decode(Buffer.concat(chunks));
  } catch {
    finish({ ok: false, error: 'utf8' }, 2);
    return;
  }
  let msg;
  try {
    msg = JSON.parse(text);
  } catch {
    finish({ ok: false, error: 'parse' }, 2);
    return;
  }
  const r = { ok: true, node: process.version, changed: false, errorObject: false, messageUnits: null, messageClipped: false,
              dataBytes: null, dataDropped: false, textForChecker: null };
  const e = isObject(msg) && Object.prototype.hasOwnProperty.call(msg, 'error') ? msg.error : undefined;
  if (isObject(e)) {
    r.errorObject = true;
    if (typeof e.message === 'string') {
      r.messageUnits = e.message.length;
      if (e.message.length > MESSAGE_UNITS_MAX) {
        e.message = e.message.slice(0, MESSAGE_UNITS_MAX);
        r.messageClipped = true;
        r.changed = true;
      }
    }
    if (Object.prototype.hasOwnProperty.call(e, 'data')) {
      r.dataBytes = Buffer.byteLength(JSON.stringify(e.data), 'utf8');
      if (r.dataBytes > DATA_BYTES_MAX) {
        delete e.data;
        r.dataDropped = true;
        r.changed = true;
      }
    }
  }
  if (r.changed) {
    r.textForChecker = JSON.stringify(msg);
  }
  finish(r, 0);
});
