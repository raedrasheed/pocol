# Codex independent review — M1 author turn007 / revision0.9

User guidance arrived through the live PoCol broker. Claude authored only the new0.9 package using the original saved session and default permission controls. No production implementation, deployment, transactions or model loops were started.

Independent execution in a preserved review copy: Python3.11.1, exit0,235 entries:197 pass,38 recorded,0 fail,24.1s. The author runner restores24 schedules and13 fault expectations covering8 distinct fault controls. Input hashes are recorded, not assertions of identity by themselves; original139 protected file hashes were independently compared with the prior checkpoint and remain identical.

Additional independent probes:31/31 pass, exit0. Fourteen are based on the installed Node22.13.1 TextEncoder, including Arabic, non-BMP and lone-surrogate strings. Hand-derived schedules independently check exact acceptance+10000 expiry, the5000 write-timeout retaining ownership/seat/counters, late settlement with no second response, teardown after timeout, session drain, full dictionary content at snapshot (not merely key/length projection), frame-derived network separation, and refusal of duplicate backend settlement.

Reviewed the literal q1–q10,r1–r6,e/f/f2 checkpoints against reference/browser.md and reference/validation.md. In particular, retained61507/68,246300,3936448,300000,987133/987137 and quota-before-global-seat behavior. Negative controls are exercised against literal golden expectations; replay through the same SiteStorageRef is consistency evidence only.

Verdict: accepted as a baseline-preserving specification/reference-fixture batch, not M1 completion or implementation approval. Q1–Q4 move from NotStarted to Partial:31 Partial,10 NotStarted (E1–E7,V2–V4),0 Complete. Real TypeScript/MV3/Chrome storage atomicity and BR21c/BR21d performance/loader tests remain unexecuted. The0.9 runner does not rerun the0.8 suite; earlier inputs are unchanged and earlier evidence remains separately recorded.

P-Q1-1/2/5 and P-Q2-3 largely follow explicit TextEncoder, one-write and teardown rules; they grant no owner policy. Same-tick ordering, wait scope after another frame's new write, badge updates and test setup remain labelled model choices to be checked at implementation. The legacy quotaNoRelease/f2 wording conflict is disclosed; separate quotaAfterSeat evidence preserves both adverse behaviors without hiding the attribution ambiguity.

Owner U01/U02/U10/U14/CR-M1-01 remain unanswered. CONF_DEPTH remains a required operational input. Full acceptance gate remains. Next owner-independent scope: E1–E3 generation fence/recovery specifications and fixtures, then E4–E7 and V2–V4. No merge/deploy.
