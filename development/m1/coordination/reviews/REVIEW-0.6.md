# Codex independent review of 0.6 / author turn 004

CLI completed successfully, default permission mode, no permission denials. Twenty new files preserved by copying before verification. Decision: revision required.

Executed Node URL/id/BR16 oracles, then Python: 190 entries, 188 pass, 1 recorded, 1 FAIL, 17.2 seconds. Python and JS BR16 streams match: bafd5b01baec76e6f3e91ed046f2adc802ee7910d592b911357fce71d883212b. C14 prefix and C19 numeric id cases pass; these issues may close at spec/tool level. Restoration is substantively correct and withdraws ADD-M1-01; historical proposals remain distinct from current authority.

BR19 failure is a checker collision: send() saves {'tokensBefore': before, **r}, then r's tokensBefore=None for size rejection overwrites the snapshot. BridgeRef itself writes tokensBefore_mt from its independent snapshot correctly. Fix the checker, do not change the bucket's size-rejection semantics.

C20 confirmed mutation-instrumentation gap (rule-disable-probe.json): /a//.. normally fails path.grammar; disabling only grammar incorrectly ACCEPTS instead of failing path.dotSegment. /./% similarly ACCEPTS after disabling dotSegment instead of failing grammar. Existing neg-path-empty labels its isolation full, even though disabling length still leaves missing-leading-slash grammar. Make disabled-rule traversal faithful, amend affected fixture isolation claims, rerun all existing normal outcomes and targeted mutants. Also ensure disabling a fetch-stage rule cannot populate a trusted shared cache and silently bypass the corresponding enabled rule at another stage; test this or keep mutant fetches uncached.

C21 reject U42 re-stamping: sign_eligible must be a predicate over the request's captured epoch. Independent epoch-probe.json: captured 0, world moves to 1, the function rewrites request to 1 and signs/sends. This does not implement the baseline's epoch-match condition (FD:L1323). Preserve capture and invalidate stale approval; require a fresh request instead of silent refresh. Update SE-benign-mutation to the baseline-compatible result. Rejection is a reviewer decision to preserve the baseline, not a new owner question.

Reviewer dispositions: approve restored original purposes/IDs with documented later-baseline reconciliation; I1 out-of-worker detection and I6 DNR scoping/read-back are compatible explicit proposals. Test keys/txA fields are approved for declared reference fixtures only; CONF_DEPTH remains required operational input, no invented default. Full gate remains. Browser, actual DNR, MPT, compiler, EVM and real transport integration remain unexecuted future checks.

Pending material decisions are unchanged: U01/U02/U10/U14 and CR-M1-01. Independent annex work can continue while they await the owner.
