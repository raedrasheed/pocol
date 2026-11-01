# Acceptance on 051dfaa (2026-10-28)

Same runner and order as accept-719da04 (local nodes, loopback RPC, a test Chrome profile). Node code: the side
set (87d2392). The H_END run of § 4 (nice 10) ran beside it. `kept/` holds what the harness kept of each failed
run, with the Playwright traces, without the `blocks.journal` files (their sums are in each manifest).

| step | whole-state | store |
|---|---|---|
| e2e_devnet, e2e_site | pass | pass |
| e2e_browser | **fail** (1 check) | **fail** (1 check) |
| e2e_integrated, runs 1 and 2 | pass, pass | pass, pass |
| e2e_heal | pass | pass |

Witnesses ([witnesses.out](witnesses.out)): 17 NOT_REPRODUCED, 15 controls OK.

The two e2e_integrated failures of 719da04 did not recur after the catch-up fix (4 of 4 runs pass).

## The two e2e_browser failures: the harness, shown by the traces

Both are a Playwright call on a site frame that the call itself causes to be replaced. The viewer replaces the
frame on every navigation (BR18), and the call does not return once its frame is gone.

1. **Whole-state: `frame.click: Timeout 30000ms exceeded`** on `fr.click('#docs')`, after D99.
   - In the trace, the click's log ends at "performing click action".
   - The snapshot taken when it timed out shows the navigation done:
     - the viewer's status reads `/docs/index.html`;
     - the stage holds a new site frame (`data-frame` 5);
     - that frame shows "Docs page from PoCol state".

   So the link worked; the click on the torn-down frame never returned. This is the signature of the BR18 failure
   on ad301e7 (`fr.click('#home')` timing out), which was kept open because that run had no trace. It now has a
   shown cause in this run. For ad301e7 itself the cause is inferred from the same signature, not shown.
2. **Store: `frame.evaluate: Frame was detached`** on `navigate(fr, '/')`. That helper evaluates
   `setTimeout(() => pocol.navigate(p), 0)` in the frame. The frame was replaced 24 ms later, before the
   evaluate's own result reached Playwright.

Fixes (test only, `e2e_browser.mjs`):
- the clicks on in-site links are started and not awaited; the new frame is what the test waits for and checks;
- `navigate` takes "Frame was detached" as the navigation happening; any other error is rethrown, and the
  caller's wait for the new frame is still the check.
A click or navigation that does not happen still fails, at that wait (60 s).
