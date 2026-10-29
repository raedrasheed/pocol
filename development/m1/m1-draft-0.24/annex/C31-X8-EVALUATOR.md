# Annex C31: X8 evidence-evaluator integrity and the C7 per-path plan (M1 draft 0.24)

**Proposed for review. Not approved. Nothing here was executed by the author. No browser or CanarySink measurement is claimed.**

## 1. Defect

- The 0.23 evaluator indexed observation windows by (K, C) in a dictionary, so the last window for a cell won.
- Root's probe recorded a K0/C1 reach and then appended an empty K0/C1 window. The evaluator returned **PASS**, erasing a release-blocking failure.
- The same structure:
  - silently ignored unknown K/C identifiers and unknown keys;
  - accepted `18080.0` as port 18080, even as self-test sensitivity evidence;
  - crashed on a non-object run or window.

## 2. Contract (`tools/x8_eval_v2.py`)

The signature and output keys are those of 0.23, plus two new keys:
- `integrity`: the list of findings;
- `gatingReaches`: every observed gating-zero reach, with the indices of the windows it came from.

**Rule 1 — integrity before verdict.** The whole input is validated before any verdict. Each finding makes the verdict **INVALID** and is reported deterministically, in input order:

| Finding | Meaning |
|---|---|
| `runNotObject`, `runKeys` | the run must be an object with exactly `selftests` and `windows` |
| `selftestsNotObject`, `selftestUnknownK`, `selftestNotList` | the self-test map has known K keys and list values |
| `windowsNotList`, `windowNotObject`, `windowKeys` | each window has exactly K, C, events, secondary and dns; the secondary and DNS collections are required evidence |
| `unknownK`, `unknownC` | identifiers outside K0–K5 / C1–C8, case-sensitive |
| `eventsNotList`, `eventNotObject`, `eventProto`, `eventPortType`, `eventPortUnknown` | proto is `tcp` or `udp`; port is an `int` that is not `bool` (floats are rejected); port is one of the five canary ports. Extra event fields are allowed. |
| `secondaryNotList`, `dnsNotList`, `secondaryEntryNotObject`, `dnsEntryNotObject` | the evidence collections are well formed |
| `duplicateWindow` | more than one window for the same (K, C) is **ambiguous**, in either order |

**Rule 2 — nothing is lost.**
- Every window with a known K and C is inspected, and evidence is **aggregated** per cell across all its windows.
- Any element of an events list in a gating-zero cell, even a malformed one, is an observed reach.
- Observed reaches are kept in `fail` and `gatingReaches` even when the verdict is INVALID.
- So later clean data can never erase a failing observation, and dictionary order cannot change the outcome.

**Rule 3 — precedence.** INVALID (an integrity finding, or a self-test missing a port) > FAIL > PENDING > PASS. A missing window is PENDING, as in 0.23.

**Self-test sensitivity.** Each K block needs one well-formed observation on every canary port. A missing block, a missing port, or a bool/float port is INVALID: a zero without proven sensitivity is never a success.

## 3. Installation and replay

- The runner loads the unchanged 0.23 runner, keeps the published evaluator as **old**, and sets `evaluate` on the 0.23 `x8_eval` module object that the runner actually uses.
- The whole 0.23 main body is then replayed without its result write: 16 assets, configuration, canonical/historical provenance, the 14 synthetic verdicts (unchanged), no-network, coverage and preservation.
- The asset manifest aggregate is compared with the review copy of the 0.23 results.

## 4. Controls (`vectors/c31-controls.json`)

There are 34 hand-derived controls. Each is evaluated twice by the new evaluator (the results must be identical), and once by the old evaluator, whose recorded outcome shows the defect.

- **Duplicates:**
  - reach then empty: new INVALID with fail kept; old PASS;
  - empty then reach (reversed): new INVALID; old FAIL;
  - two clean windows: INVALID;
  - duplicate K4 control: INVALID; old PENDING;
  - reversed K5/C6: INVALID with fail kept.
- **Unknown identifiers:** K9, C9, a lowercase `k0`, and a self-test for K6.
- **Malformed input:**
  - a run that is not an object (old: crash);
  - an unknown run key, or missing self-tests;
  - windows not a list (old: PENDING), or a window not an object (old: crash);
  - a window missing its events or secondary collection;
  - events not a list; an event not an object in a gating cell (fail kept);
  - `TCP` in upper case; a bool, float or string port; an unknown port;
  - a malformed secondary or DNS collection.
- **Sensitivity:** a missing K3 self-test block; a bool self-test port; a float self-test port (old **PASS**, a false sensitivity proof).
- **Valid controls:** the base (PASS), extra canary event fields (PASS), a single real reach (FAIL), a missing window (PENDING), the root base (PASS), the root duplicate (INVALID).

**Further checks:**
- The root probe is replicated (37 cases), with the duplicate case now INVALID.
- A sweep covers all 27 gating-zero cells in both duplicate orders (54 runs). None passes, and the reach always remains in `fail`.

## 5. Supplemental C7 per-path plan (`vectors/c7-path-plan.json`, `pages-supplemental/C7-P1…P5`)

This is a technical supplement for root review, not an owner-policy decision. The 0.23 C7 page, its aggregate window and its cell rules are unchanged.

- **Why.** The five C7 paths ran in one window, so a reach from one path cannot show that another path (notably `document.write`, PB6) was testable.
- **Pages.** There is one page per path: createElement, innerHTML, createElementNS, template + importNode, document.write. Each page records `realm` and `pc` in its log and closes its peer connections at 12000 ms.
- **Windows.** There are 30: `X8C7-<K>-<P>` for K0–K5 × P1–P5, each 15000 ms in a new tab, followed by a silent 3000 ms gap. They run after each K block's self-test and C1–C8 windows.
- **Attribution.** Attribution is by window only, because STUN carries no path. A canary event inside a gap makes both neighbouring windows ambiguous. No other activity may run during the block.
- **Cleanup.** The page closes its peers at 12000 ms, the harness reads the log at 14000 ms and closes the tab at 15000 ms, and the gap must be silent. A missing `closed` flag makes the window ambiguous.
- **K4 positive control, per path.** Three pieces of evidence are required:
  - E1: at least one udp/13478 event in that path's window;
  - E2: the page log for that window shows `path`, `realm = true` and `pc = true`;
  - E3: silent gaps and `closed = true`.

  The outcome is one of: testable, `untestable:noRealm`, `untestable:noTraffic`, or `ambiguous`. A path that is not testable keeps C7 PENDING.
- **K0, K1, K2 and K5.** Every path window must have zero events, and the gaps must be silent. Any event is FAIL; in K0 it blocks release. A zero counts only if the same path's K4 control was testable in the same run.
- **K3.** Informational; reach is expected per path.
- **Still open.**
  - PB6: whether an untestable `document.write` path is acceptable for the gate is a source decision.
  - PB8: the timings are proposals.
  - Both remain unresolved in the source.

## 6. Unchanged

- The 0.23 assets, synthetic goldens, source clauses, flags, experiment claims and privacy exclusions.
- Owner questions, the CR-E4 proposals, RF-E6-1, CONF_DEPTH and the full gate.
