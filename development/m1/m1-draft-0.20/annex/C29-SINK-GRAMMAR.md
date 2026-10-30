# Annex C29: closed SinkChecker grammar and V2 deferred experiments (M1 draft 0.20)

**Proposed for review. Not approved. Nothing here was executed by the author.**

## 1. Defect

- The 0.19 `SinkChecker` treated every event other than `begin` or `piece` as an attempt's terminal.
- Root's minimal counterexample (`coordination/review-001/sink-unknown-terminal-probe.json`), `begin(1)` + `['unknownTerminal', 1, 'fake']`, returned `[]`.
- browser.md:126 (S-a) requires exactly one terminal, and it must be `abort` or `commit`.
- The same weakness accepted:
  - a short `['abort', 1]`;
  - `True`/`1.0` as attempt id or piece seq;
  - `True`/`1.0` as commit counters (Python `True == 1`);
  - a commit anchor that differs from begin.
- Some malformed inputs crashed it: an empty event, a 1-element range, a non-dict summary.

The client FSM, branches, source goldens and limits were correct and are unchanged.

## 2. Closed grammar (proposed supplement P-V2-9; `tools/sink_checker_strict.py`)

`int` below means exactly an integer: `bool` and `float` are rejected.

| Element | Exact form | If violated |
|---|---|---|
| timeline entry | `{seq: int or float (not bool), kind: 'req' or 'sink', …}` | S-e `malformedEntry:<seq or kind>` |
| request | `['H', 'latest' or int]`, or `['L', a:int, b:int, anchorHash:str]` | S-e `malformedEntry:request` |
| begin | `['begin', id:int, {number: int ≥ 0, hash: str}]` | S-a `malformedEvent:…` |
| piece | `['piece', id:int, seq:int ≥ 0, [a:int, b:int] with a ≤ b, logs:list]` | S-a `malformedEvent:…` |
| abort | `['abort', id:int, reason: non-empty str]` | S-a `malformedEvent:…` |
| commit | `['commit', id:int, summary]` | S-a `malformedEvent:…` |
| any other kind, wrong arity, not a list, empty | — | S-a `malformedEvent:unknownKind:<k> / arity:<k>/<n> / notList / noKind` |
| summary | exactly `{pieces, logs, logRequests, headRequests, totalRequests: int ≥ 0, anchor}` | S-e `malformedSummary:…`; the commit still settles the attempt |
| summary.anchor | equals the attempt's begin anchor | S-e `anchorMismatch` |

**Effects:**
- A malformed event never begins, extends or settles an attempt. So:
  - an unknown "terminal" leaves the attempt without a terminal (S-a `noTerminal`);
  - an unknown event before or after a real terminal is reported (S-a) and does not change the real terminal.
- Well-formed events are judged exactly as in 0.19.

**The 15 existing 0.19 sink controls keep their exact rule sets.**
- No adjustment was needed.
- `twoCommits` gains the additional reason `anchorMismatch`, because attempt 2's summary anchor is B3000 while begin(1) is A3000. Its rule set {S-c, S-d, S-e} is unchanged.

## 3. Installation and replay

- `run_checks_020.py` loads `m1-draft-0.19/tools/run_checks_019.py` as a module and keeps the 0.19 class as the pristine checker.
- It then sets `SinkChecker = StrictSinkChecker` on the loaded 0.19 model module object (`R19.M`), which the 0.19 runner resolves at call time. No file of 0.19 is edited.
- It replays the 0.19 main body without the 0.19 result write:
  - every LC case and variant, with literal requests, events, results and SHA-256 (43 SinkChecker runs, now strict);
  - the step table and the client controls;
  - the 15 sink controls, exact;
  - the P-V2 gaps, coverage and input SHAs.

## 4. C29 controls (`vectors/c29-controls.json`)

25 timelines: the root minimal case, adversarial cases, and three positive controls. Each is judged twice:
- by the strict checker: exact rules **and** reasons must match;
- by the pristine checker: the derived old behaviour is asserted, so the defect is shown, not assumed.

The pristine checker is green on 11 of the cases that must be rejected:
- root;
- shortAbort;
- bool/float attempt id;
- bool/float piece seq;
- bool/float counter;
- anchorMismatch;
- bad entry kind;
- malformed request.

It crashes on 3: emptyEvent, pieceRange and summaryNotDict. It reports the wrong rule (S-b instead of S-a) for unknown-then-abort and unknown-after-commit.

| Control | Strict rules | Strict reasons |
|---|---|---|
| root (begin + unknownTerminal) | S-a | malformedEvent:unknownKind:unknownTerminal, noTerminal |
| unknown then real abort | S-a | malformedEvent:unknownKind:unknownTerminal |
| unknown after commit (LC1) | S-a | malformedEvent:unknownKind:unknownTerminal |
| no real terminal | S-a | noTerminal |
| short abort / not list / empty | S-a | malformedEvent:arity:abort/2 / notList / noKind, plus noTerminal |
| bool / float attempt id | S-a | malformedEvent:attemptIdNotInt:<type>, eventWithoutBegin |
| attempt id 0 | S-b | attemptIdNotSequential |
| bool / float piece seq, bad range (LC1) | S-a, S-d, S-e | malformedEvent:…, pieceSeq, rangesNotExactPartition, summaryMismatch |
| bool / float / negative counter, not a dict, missing key | S-e | malformedSummary:… |
| anchor mismatch | S-e | anchorMismatch |
| bad entry kind / bool entry seq / malformed request | S-e | malformedEntry:kind / seq / request |
| valid abort, valid commit (LC1, LC15b) | — | — |

## 5. V2 deferred experiments: specifications, not executions (`vectors/v2-deferred-experiments.json`)

**Scope.** M1 needs each experiment's definition: literal inputs and pass criteria. It does not need a production TypeScript, Chrome or pocold run. The absence of an implementation is therefore not a definition gap.

| Experiment | Inputs | Pass criteria |
|---|---|---|
| TS-LC | 0.19 LC fixtures and the C29 controls | Identical request logs, sink events and full results/SHA-256. Identical step actions. The strict-checker rule sets. Fault builds change their witnesses. |
| BR10 | page eth_getLogs 1..100 and 1..1025, 17-address filter, anvil | Only pocol_getLogs and eth_getBlockByNumber are seen. −32020 {range} with zero requests. −32602. −32601 gives unsupported. |
| RG3 | chain fixture validation.md:985–989, ReferenceLogs | 559104 logs byte-equal, no loss or duplicate. Every reply ≤ 8 MiB. Strict checker clean. Counters equal RpcRecorder (recorded). |
| RG3-bridge | the same, through fetchAll | tooLarge, no partial result. |
| RG3b and variants | branches B and A′, hook after the third piece | The literal trace. Attempt 2 equals ReferenceLogs(B). The A→B→A′ and pre-anchor-check variants. |
| RG9 | RG5 load | Matching commit or explicit error, never partial. The ratio is recorded (it is a measurement). |

**Genuine definition gaps (recorded):**
- **DG-V2-1:** the RG3b hook mechanism (pause, submit B, adoption signal and time limit) is undefined.
- **DG-V2-2:** literal JSON replies for non-Python harnesses. The 0.19 fixtures use generator templates. Proposal P-V2-10: publish the expanded replies.
- **DG-V2-3:** the sink grammar was only implicit. Proposal P-V2-9 is §2.
- **DG-V2-4:** RG3 counters are recorded rather than literal, by design. Noted so it is not mistaken for a missing golden.

## 6. Unchanged

- P-V2-1..8 remain explicit, unapproved proposals. P-V2-9 and P-V2-10 are new proposals, also unapproved.
- No policy change.
- Owner decisions, the full gate, CONF_DEPTH, the representation proposals and RF-E6-1 are unchanged.
