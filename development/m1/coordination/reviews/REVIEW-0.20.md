# Codex independent review — revision 0.20, author turn 018

The preserved review copy ran on Python 3.11.1: 646 entries, 570 passed, 76 recorded, zero failures, 1.6 seconds, exit 0. All 516 entries from 0.19 ran again. Client transitions, branch data, request logs, full results, limits, nine client controls and the fifteen original sink controls are unchanged. All original sink controls retain their exact expected rule sets.

C29 is repaired. Only abort and commit can terminate an attempt. Unknown events are reported and do not hide a missing terminal. The checker validates event shape, integer identifiers and counters, and the commit anchor. The original checker remains available as a negative control: the new fixture assertions expose its false acceptance, crashes and incorrect rule reports rather than rewriting that history.

Twenty-three additional root probes passed against the actual strict checker. They cover the original unknown-terminal counterexample, an unknown event followed by a real abort, malformed events, boolean and floating identifiers, every commit counter with boolean/float/negative inputs, and valid abort/commit controls. This is independent evidence beyond the author's prepared fixture suite.

Verdict: accept this specification/reference-fixture batch; close C29 as a checker defect. V2 is Partial: 39 Partial, 2 Not started (V3–V4), 0 Complete. The proposed contract grammar is consistent with S-a's named terminal events; accepting the checker does not approve unrelated owner policies or replace the baseline.

Deferred TS/Chrome/RPC experiments now have recorded specification inputs and pass criteria. Their actual implementation outcomes are not required by this task. Genuine definition gaps remain explicit: the RG3b hook/adoption signal, portable expanded JSON replies, and the before-begin request-budget wording. The final completeness audit must resolve or precisely classify these instead of using absence of production execution as a blocker. Model conventions P-V2-1 through P-V2-10 remain distinguishable from baseline authority.

Existing owner decisions, CONF_DEPTH, the full specification gate, CR-E4 representation proposals and RF-E6-1 remain unchanged. Continue V3/V4 and the final specification audit. No merge or deployment.
