# S5 design decision: general R18 and R21 derivation contract

Status: OPEN. Proposed interpretation only; M1 and the accepted LP3 plan are unchanged.

This is a specification completeness finding, not a failed network request, missing secret,
or lack of authorization to implement/push. The GitHub connector works. S5 must not be
accepted while the general rules are replaced with guessed equations or caller-supplied
`sysOk`/`trieOk` flags.

## Source evidence

- `development/m1/reference/governance.md:97`: R18 is "SYS, including the cost of RET
  records within G_SYS". No general SYS slot/key/account/gas derivation is stated there.
- `development/m1/reference/execution.md:86-93`: KN=1752, KC=1482, KF=1395,
  AN=404, AC=388, AF=257; normal/final gas are approximate 26M/37M and measured by T14.
  The text explicitly calls for ParamGate.derive to recheck the resource accounting in M4.
  These are reference results, not equations over arbitrary CP values.
- `development/m1/reference/governance.md:100`: R21 is historical trie, with the only
  exception simnet plus retentionStress.
- `development/m1/reference/storage.md:66-67`: live entries <=447 B, historical <=1.6 GiB,
  P(d)=568d+186+32(d+1). This does not specify how historical entries/path copies are
  counted over K_hist, dirty keys/accounts, H_END, shared paths or SYS entries.
- `development/m1/reference/validation.md:2064`: caps/64 requires the *smallest*
  SYS_SLOT_MAX passing R18 via ParamGate.derive. A fixed network literal cannot answer it.
- `development/LP3-MILESTONE.md`: S5 requires all R1-R26, no supplied-ok shortcuts,
  exact derivations, mutation controls, and explicitly labeled measured test fixtures.

Searched the accepted reference sections and FINAL_DESIGN, plus preserved Python tools
through M1 0.32. No general R18/R21 formula implementation was found. The preserved
network-profile tools handle schema/identity; they are not an R1-R26 resource gate.

## Why guessing changes acceptance

For illustration only (neither expression is proposed as the correct R21 rule), using
K_hist=32, dirty keys+accounts=4096+2048 and d=64:

- Entry-only accounting: 32*6144*447 = 87,883,776 bytes, below 1,717,986,918.
- Path-per-entry accounting: 32*6144*P(64) = 7,592,607,744 bytes, above that budget.

The available numbers do not determine which paths/entries are retained and shared.
Choosing an accounting model silently can flip the required network PG-END3 result.
Similarly, the six fixed SYS counts do not determine new counts when RET, M_max,
S_max, E_max or other resource-driving CP fields change. The full end/full and caps/64
acceptance cases require such changes.

## Concrete proposed resolution

Choose and record one contract in a new LP3 amendment; do not overwrite M1:

1. **Trace-backed LP3 gate (recommended for the existing LP3/LP4 separation):** R18/R21
   compare numeric resource facts against budgets. Require facts bound to the complete
   CP/profile identity and resource model/version, with explicit ordinary/closing/final
   phase coverage, user/system key/account counts, peak live slots, per-call and total
   system gas (including the configured RET work), historical-trie bytes and retained
   horizon. R19 uses explicit measured phase durations and R14 measured validate p99.
   Missing, mismatched or incomplete facts are a named non-ok result, not zero/default
   success. LP3 tests may provide labeled synthetic numeric fixtures; they cannot prove
   worst-case bounds or authorize boot. Real measured/builder facts remain LP4 gates.
   `derive` must label observations versus predictions, and caps/64's minimum SYS slot
   requirement stays explicitly pending the LP4 resource model rather than fabricated.
2. **Deterministic general derivation:** supply/approve complete equations for SYS live
   slots, distinct keys/accounts for all phases, RET/system gas and historical-trie
   retention, including domains, rounding and sharing. Then implement and independently
   mutate those equations and keep real timing/code/builder checks gated to LP4.

The accepted plan allows synthetic measured fixtures but does not expressly resolve
whether observational maxima are sufficient for the general *derive* API and the
caps/64 smallest-slot requirement. That semantic boundary is the decision requested.

## Work safely completed before this decision

Implemented only the fully specified R16/R20/R26 arithmetic in `param_gate.rs`, with
private exact rational milliseconds, checked arithmetic, required named PG-RPC vectors,
registry/end/full derivation values and explicit zero/overflow rejection. No full
ParamGate result, production boot integration, or synthetic `ok` profile was added.
The draft PR is a checkpoint, not a merge candidate. Full S5 and S6-S9 remain pending.
