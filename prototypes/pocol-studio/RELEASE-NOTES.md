# PoCol Studio 0.1.1 — validation

This release targets `studio.pocol.net` with a matching Chrome extension and the `gateway.pocol.net/studio/rpc` relay.

## Verified locally

- Editor: 9 model, migration and ZIP tests passed; package and JavaScript syntax checks passed.
- Extension: 234 existing regression tests, 25 Studio integration/keyfile tests, and 42 publisher protocol tests passed.
- Relay and installer: 20 tests passed, including bounded HTTP requests and rollback behavior; Python and shell syntax checks passed.
- Total: 330 automated tests passed across the components. The editor suite was rerun for 0.1.1; unchanged extension and gateway results are retained from 0.1.0.
- English is the default UI, initial HTML and new-project content language. A one-time migration translates only an untouched, unpublished Arabic personal starter, preserving customized drafts and published site identities.
- Browser checks covered page editing, template creation, Arabic/English UI, theme switching, desktop/mobile preview selection, local draft persistence, and the missing-extension installation flow.
- Publication tests use real transaction serialization/signatures against a simulated chain. No live blockchain transaction was sent.
- Expiry cleanup now holds the publication job lock and rereads persisted state, preventing deletion during a signed checkpoint write.
- Resuming a pending chunk reconciles its signed transaction even if a different publisher has since stored that chunk, preventing nonce reuse.
- Gateway rollback continues restoring service files when Caddy reload times out and reports incomplete recovery explicitly.

## Deployment checks still required

- Create the separate cPanel subdomain and document root, upload the static package, and verify DNS/HTTPS.
- Run the gateway installer preflight and deployment on the node, then verify HTTPS from an external client.
- Install the matching extension on Chrome and verify a funded PoCol Mainnet account's first live publication and update.
- Run the Windows DPAPI export helper on the user's Windows account. Portable-file crypto compatibility was tested in Node, but Windows DPAPI execution was unavailable in this Linux environment.
- Chrome Web Store submission/review is not completed. The provided extension package uses Chrome's developer installation flow.

The private hosted editor preview does not activate DNS or the production relay. RN is a trusted remote-node connection, not local full-chain verification.
