# Separate sequential review — PAY-02 native component

Same author, separate review after initial execution tests; no external reviewer claimed.
Reviewed native/pay diffs, immutable transaction identity, current-state conditions,
rollback, code-identity authorization, independent oracle, mutation isolation and CI.

- Initial independent oracle reader used make_state_root without persisting the
  resulting trie nodes; construction failed with MissingAccountTrieNode. No PASS
  was counted. Fixed by persisting the actual state before constructing an immutable
  reader at that root. The final comparisons execute 44 boundary and two attack cases.
- Existing PAY-01 account-create single-guard mutation became redundant after PAY-02:
  either independent eligibility barrier rejects absence. Retain historical positive
  control receipts; add a compiled composite removing both barriers, plus omission,
  balance-only and AND-vs-OR mutations. Never label a redundant survivor as killed.
- Full snapshot cloning is an unbounded experimental representation, not an R18/R21
  proof. No external/partial database backend is silently treated as complete.
- Actual Registry/Escrow admission/refund bridge, GenesisBuilder and all associated
  execution/resource/finalization gates remain unimplemented or unverified. This
  review approves publication of a partial component, not P2 merge or LP4 acceptance.

Historical R4 proposal and positive lifetime witnesses remain byte-identical.
