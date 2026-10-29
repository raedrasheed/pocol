# Codex independent review ? revision 0.17 / turn015

Preserved-copy execution:1194 entries,1037pass,156recorded,1FAIL,63.6s,exit1. C27 full timeline corrected: exact first witness at6000,count3; second K2 attempt at11000; final maximum4/six sets; unchanged correct path and old checks.

C28 checker snapshot alias: inherited R16.admin_cells collects admin_actual('violations') as the live e.violations list. At5000 it is empty, but later run(6000) appends the violation to this same list. The recorded5000 actual then falsely contains future6000. Thus suite016.BR24-x9.faultyPath.adminEarlyRelease.row@5000.violations fails. This is a real checker defect, not a reason to delete the5000 assertion. Deep-copy every actual and expected cell at observation time (rows and final, nested structures), using a new revision wrapper in actual runner globals, preserving older source. Add adversarial regression that a later nested mutation cannot change an earlier captured cell. Ensure complete history remains checked.

RF-E6-1 supplemental timelines prove global noCancel total2 (op1/op2 each1), normal0, analysis op1-only total1; zero stale allocations/tickets/sets and correct recreated K1. Source literal total1 is not reconciled by silently calling it per-op: retain precise blocker and options; recommend supplemental source correction preserving global fault rather than narrowing it. No owner decision made.

21 independently authored site/disk/pin/reaper probes pass against unchanged0.16 SiteLedger. Historical scope still partial, realChrome/A15c/A15d and sweepMax gaps remain. Verdict revise C28; accepted0.15 remains. No merge/deploy/owner approval.
