# LP3 amendment 01 — approved resource-fact contract

Approved by the owner in this conversation on 2026-10-17. Applies to LP3 only;
M1 and all earlier evidence remain immutable. This resolves the option-1 question
recorded in S5 PR #15, not a retroactive pass of the original requirements.

R18/R21 in LP3 evaluate numeric resource facts bound to the complete CP/profile
identity and resource-model version. Missing, mismatched or incomplete facts prevent
acceptance. No caller-supplied success flags are permitted. Provenance distinguishes
synthetic fixtures, measurements and proven bounds; none authorizes boot in LP3.
R14/R19 use explicit measured/test numeric durations. Test fixture outcomes establish
evaluator behavior only. Actual performance/system/EVM/builder facts remain LP4.

The original general R18/R21 derivations and caps/64 minimum SYS_SLOT_MAX obligation
are **DEFERRED TO LP4, NOT PASSED**. They must not be represented as satisfied under
the original LP3 criteria. All other LP3 criteria remain in force. Boot stays disabled.

## Mandatory LP4 acceptance tests

1. Specify and independently verify general SYS slots, phase keys/accounts, per-call
   and block gas including RET for all supported parameter domains. Reproduce the
   reference phase counts, changed-RET and end/full cases; mutate each bound.
2. Specify the historical-trie retention/sharing derivation over the complete profile;
   compare predicted and actual retained bytes/horizon against independently counted
   trie nodes. Test boundary budget, changed K_hist/H_END and the simnet-only
   retentionStress exception. Measurements alone do not prove a general bound.
3. Derive caps/64's minimum SYS_SLOT_MAX: minimum passes, minimum-1 fails R18,
   and independently counted real execution reaches the documented requirement.
4. Obtain real source-bound EVM/SYS/GenesisBuilder measurements, allocRoot and
   sysCodeHash, including ordinary/closing/final and configured RET coverage.
   Reject missing provenance, model/profile mismatch and incomplete phase coverage.
5. Replace every synthetic LP3 resource fixture needed for production acceptance.
   Independently review any claimed proof. No boot activation follows from LP3 success.

See LP3-ACCEPTANCE-MATRIX.md for original versus amended obligations and status.
