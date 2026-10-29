# Annex C32: defensive JSON-RPC envelope and retry-delay validation for HeaderNetCheck

**For root review. Not approved. The author ran nothing.**

## The defect

Root found this in REVIEW-0.26 (`coordination/review-001/v1-malformed-error-probes-0.26.json`). In the 0.26 reference checker, a −32021 reply with:
- missing `data` raised KeyError;
- `data: null` raised TypeError;
- `data: {}` raised KeyError for `retryAfterMs`.

The 0.26 suite never sent such replies. A malformed delay could also have caused an unbounded wait.

## The repair (`tools/v1_ref_027.py`; the 0.26 module is unchanged)

1. **Literal replies.** Replies are handled as literal JSON text and parsed strictly. The tokens `NaN` and `Infinity` are rejected (P-C32-4).
2. **Envelope (P-C32-1).** A reply is acted on only if:
   - it is an object with `jsonrpc: "2.0"`;
   - it has exactly one of `result` or `error`;
   - an `error` is an object with an integral `code` and a string `message`.

   The `message` rule follows the accepted 0.6 envelope precedent.
3. **−32021 rule.** A −32021 reply is retried only if `data` is an object with:
   - `reason` equal to `busy` or `rate` (network.md:364);
   - `retryAfterMs` an integral number:
     - in [250, 2000] for `busy` (network.md:234);
     - in [0, 2000] for `rate` (P-C32-2; the source gives no range for `rate`, so the cap reuses the only approved −32021 range).
4. **Integral values (P-C32-3).** "Integral" is the accepted C19 value semantics: a finite JSON number with an integral value, so `500.0`, `5e2` and `-32021.0` are accepted, and booleans and strings are never numbers.
5. **Retries.** A valid −32021 is retried after exactly `retryAfterMs`, at most 3 times (browser.md:41, P-V1-10 unchanged). A fourth valid −32021 gives viewIncomplete.
6. **Everything else gives viewIncomplete.** That covers a malformed reply, −32018, any other error code, and a malformed `eth_blockNumber` reply. The outcome is:
   - immediately, with no wait and no exception;
   - no frame;
   - 4901 for pending account and sign requests;
   - the log −32019 `{rule: viewIncomplete}`.

## What does not change

- **Honest servers.** An RpcGuard-conforming server (network.md:217, 234) only sends delays inside these ranges, so its behaviour is unchanged.
- **Approved retry semantics.** "Retry after retryAfterMs up to 3 times, then viewIncomplete" is unchanged.
- **Total waiting.** It is at most 3 · 2000 = 6000 ms, inside the 10 s RP protection of network.md:374.
- **No new rejection class.** Malformed replies already ended in viewIncomplete under P-V1-9 ("other errors"). This repair makes that outcome total, so no input escapes it.

## Fixtures (`../vectors/c32-error-cases.json`, 41 cases)

- **The three root failures:** `C32-root-missingData`, `C32-root-dataNull`, `C32-root-missingDelay`.
- **Near-neighbour malformed inputs:**
  - `data` that is an array or a string;
  - `reason` missing or unknown;
  - a delay that is null, boolean, a string, negative, fractional, NaN, Infinity, `1e400` (overflow), or 10^20;
  - the out-of-range delays 249 (busy), 2001 (busy) and 2001 (rate);
  - a `code` that is a string, a boolean, fractional or missing;
  - a missing `message`;
  - an `error` that is a string, both `result` and `error`, neither, no `jsonrpc`, an array reply, truncated JSON;
  - a retry followed by a malformed reply, another error code, a malformed `eth_blockNumber`.
- **Legitimate replies and exact boundaries:**
  - busy at 250 and at 2000;
  - rate at 0 and at 2000;
  - integral float values;
  - an exponent form;
  - exactly three mixed retries (accepted);
  - four busy replies at 2000 (exhausted after a total wait of 6000 ms).

Every case lists the literal reply texts, the request times, the total wait and the outcome. The runner also asserts that no case raises.
