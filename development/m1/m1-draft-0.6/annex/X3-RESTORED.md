# X3 Restored: Q8–Q12, X1–X7, د1–د5 (English, traceable)

Revision R6-04 (C13). Machine-readable form: `x3-restored.json`.

**Sources.**
- **H31:** recovered historical round-15 proposal, message 31 of the original dialogue state (`coordination/review-001/historical-round15-source.json`, sha256 `b7a3fa38…`). It is the last round-15 proposal that defines Q8.
- **H15e:** an earlier round-15 wording (`historical-test-definition-hits.json`).
- **B:** the current baseline.

The historical text is **evidence, not current authority**. Where it conflicts with B, B wins, and the difference is recorded below (I1–I8). **ADD-M1-01 (0.4) is withdrawn**, together with its T12-site, isolation-substitute and unsupported-feature mappings.

## Q8–Q12: signing and profile races

Setup (H31):
- two anvil instances on two ports, both chainId 777001, each with a Website at the same address but with different content;
- a local proxy that records every `eth_sendRawTransaction`;
- a wallet test hook that counts every sign call.

Executable decision model: `tools/profile_race_ref.py`, run over `vectors/profile-race-cases.json`.

| ID | Restored English definition | Pass criterion | Model scenario | Trace |
|---|---|---|---|---|
| Q8 | A signing request is pending on profile A; a conflicting profile B (same chainId, different genesisHash) is added from the options page | The request is cancelled with **4100** when B commits, before any click; 0 sign calls; 0 sends; a later click has no effect | `Q8` | H31 validation; code 4100 is B (FD:L983) |
| Q9a | B is submitted and commits before the acceptance acquires ProfileMutex | 4100 (at the commit re-evaluation, or at step 1 at the latest); 0 sign; 0 send | `Q9a` | H31 |
| Q9b | B is submitted while the acceptance holds the lock; the hook delays the identity check by 2 s | B waits and cannot commit between the steps; exactly 1 sign and 1 send with the previous state, logged as "sent before the conflict"; then B commits, both profiles are labelled conflicting, and a later request gets 4100 | `Q9b` | H31; the step sequence is B (FD:L1324) |
| Q9c | A bypassing direct write to `profiles:` while the lock is held | If the write precedes step 3 and step 3 sees it, the request is cancelled. A write after step 3 is detected only after the release (declared limit) | `Q9c-before-step3`, `Q9c-after-step3` | H31; the detection mechanism is gap I1 |
| Q10 | networks.json revokes netKey(A) while a request is pending | 4100; 0 sends | `Q10` | H31 |
| Q11 | The site connection is withdrawn, or the wallet is locked, while a request is pending | 4100 | `Q11-disconnect`, `Q11-lock` | H31 |
| Q12 | A direct write to a `profiles:` key from a non-worker context, outside any acceptance | Detected via onChanged; eligibilityEpoch increases; the pending request is cancelled (here 4901: profile field changed) | `Q12` | H31; gap I1 |

**Supporting signEligible definition (X2).** This is the current 7-condition form of FD:L1323. The cancellation codes for conditions 1, 2 and 7 come from B; those for conditions 3–5 come from H31 and are carried as P.

| # | Condition | Code on failure | Label |
|---|---|---|---|
| 1 | The profile exists and matches the frozen fields | 4901 | B, FD:L982 |
| 2 | There is no conflicting profile | 4100 | B, FD:L983 |
| 3 | netKey is not revoked | 4100 | H31 |
| 4 | The connection exists | 4100 | H31 |
| 5 | The wallet is unlocked | 4100 | H31 |
| 6 | The epoch matches | Re-stamp if 1–5 and 7 hold | P, U42 |
| 7 | LN or DEV, or RP with h ≥ 1 and a successful window | 4901 | B, FD:L1001 |

The model also covers these supporting scenarios:
- SE-eligible;
- SE-benign-mutation;
- SE-identity-mismatch (historical Q5);
- SE-endpoint-changed (Q4);
- SE-trust-changed (Q6);
- SE-rp-no-window and SE-rp-window-ok.

## X1–X7: malicious sites (browser, Phase A)

Run (H31, extended by B): every site runs with all layers, then with each layer alone. B adds RtcLockdown as a third layer (FD:L1353), so (P) the runs are: all three layers, then CSP-only, then DNR-only.

Pass in every configuration:
- zero external requests (proxy, DNS log, CanarySink K0–K2 = 0);
- zero access to extension APIs.

| ID | Restored definition | Literal site (in `x3-restored.json`) | Expected |
|---|---|---|---|
| X1 | fetch to an external address | `fetch('https://example.com/x1')` | `__x1 == 'TypeError'`; 0 requests |
| X2 | An external img, with srcset | `<img src>` and `<img srcset>` pointing to example.com | naturalWidth 0; 0 requests |
| X3 | External CSS `@import` and `url()` | `s.css` with `@import url(https://…)` and `background:url(https://…)` | 0 requests; the local CSS still applies |
| X4 | chrome.runtime and chrome.storage attempts | `typeof` probes | Neither exists in the opaque-origin sandbox |
| X5 | window.top / parent access, top navigation | Read `top.location.href`, write `parent.document.title`, assign `top.location` | SecurityError on the reads; no navigation of the viewer |
| X6 | window.open for a new window | `window.open('https://example.com/x6')` | Returns null; 0 windows or tabs |
| X7 | Form submission to an external destination | `form.submit()` with an external action | 0 requests; the frame is not navigated away |

Behaviours that appear only in H15e (a forged `postMessage` to the parent, a transaction on load without a click, 1000 messages per second, an external link stylesheet, and storage_set with another netKey) are listed as **supplementary**, with their current B coverage (T12, BR14, BR19, BR17e/f, د4). They do not renumber the IDs.

## د1–د5: DNR rule lifecycle

All are fail-closed. Executable model: `tools/dnr_ref.py`, run over `vectors/dnr-cases.json`.

Rule literals:
- block: `{id 2t, priority 1, block, tabIds [t], RT}`;
- allow: `{id 2t+1, priority 2, allow, urlFilter '|chrome-extension://<ID>/', RT}`.

RT is every resource type except main_frame. The ids, priorities and urlFilter are B (FD:L1349–1351). tabIds and RT are H31.

| ID | Restored definition | Pass | Trace |
|---|---|---|---|
| د1 | Both rules are installed and verified before the frame is created | getSessionRules shows exactly ids 2t and 2t+1 with these fields; event order: installed and verified, then frame created | H31; B FD:L1352; field check from H15e |
| د2 | An injected updateSessionRules failure means the frame is not created | No frame, no rules. A read-back mismatch also gives no frame, and the rules are removed | H31; B fail-closed |
| د3 | Closing the tab removes both rules | Rules removed (H15e bound: within 1 s) | H31 / H15e |
| د4 | `chrome-extension://<ID>/` resources are allowed; every external destination is blocked for every RT | The allow rule (priority 2) beats the block rule; `https://example.org` and `http://127.0.0.1:18080` (H15e) are blocked; main_frame and other tabs are unaffected | H31 + H15e |
| د5 | A worker restart and a failed ping tear the frame down; leftover rules are verified and removed | Frame torn down; rules 2t and 2t+1 removed | H31 |

## Reconciliation with the current baseline (exact)

| # | Kind | Statement |
|---|---|---|
| I1 | Gap | Q9c and Q12 need out-of-worker write detection: H31 uses `storage.onChanged` with `writeToken`. FD:L1321–1325 states only ProfileMutex and eligibilityEpoch, and neither requires nor forbids the detection. It is carried as P |
| I2 | Superseded | signEligible has 7 conditions in B against 6 in H31. B's form is used |
| I3 | Superseded | H31 sends to the frozen endpoint with a 10 s timeout. B sends via WalletSubmit (FD:L1324, FD:L4933). B's sequence is used; the 5 s identity timeout and endpoint freezing are P |
| I4 | Partial | 4100 for a conflict and 4901 for an identity change are B. 4100 for revocation, disconnection and lock are H31, carried as P |
| I5 | Superseded | H31's X8 is replaced by B's X8 (CanarySink gate, FD:L2645). Only X1–X7 are restored |
| I6 | Gap | B lacks the resource-type list, tabIds scoping, read-back verification and removal triggers for the DNR rules. They are carried from H31 (D37) as P |
| I7 | Note | The Q tests use chainId 777001 on anvil. On pocold, 777001 is the CI2 rejection vector (FD:L845); the Q tests never touch pocold |
| I8 | Choice | An eligible pending request is re-stamped with the new epoch at each commit (P, U42) |

No restored item contradicts the current baseline outright. The gaps (I1, I6) are filled by proposals taken from the historical text, labelled H/P.
