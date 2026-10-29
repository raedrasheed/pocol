# Note on this directory's README (2026-10-28)

The e2e_browser failure in whole-state mode (BR18, `fr.click('#home')` timed out after 30 s) stays **open**:
its cause is not established, and the passing runs since (on e453851 and later) do not explain it. The browser
driver now records a Playwright trace of each context (DOM snapshots and screenshots), saved when a check
fails, and keeps it with the node logs and inputs (keepOnFailure). The next such failure will carry that
evidence.
