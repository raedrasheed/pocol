# Owner decisions (2026-11-23): the extension's store item, site connections, Studio's version and an RPC fix

Status: **decided by the owner on 2026-11-23**, in the working session of that day, as answers to the questions of the
production readiness report (branch `claude/production-readiness`). Approving a decision is not approval to deploy or
to publish: the Chrome Web Store upload, the signed release and every upload of a site still need the owner's own
steps and approval.

The decisions are numbered D1 to D6 within this record; cite them with the date ("owner decision 2026-11-23 D2"),
since the records of 2026-11-20 and 2026-11-21 also number theirs D1 to D7.

## The owner's words (verbatim)

> يبقى الحال كذلك في هذا الإصدار

> قبول المعرّف الذي يمنحه Chrome Web Store

> يبقى في نسخة المتجر من الإضافة

> لا رابط ولا بصمة قبل الإصدار الموقّع

> يقفز من 0.1.1 إلى 0.5.0. نعم

> نعم طبق الاصلاح

> نعم طبق الاصلاح وسجل القرارات

Each answer is quoted again under its decision with an English rendering. The last line is the request that applied
the fix of D6 and asked for this record.

## Decisions

1. **D1 (EXT-03): only Studio and the IDE connect to the wallet in this release; arbitrary sites are deferred.**
   - *Question.* The extension has no permission-based connection for other websites on PoCol Mainnet: there is no
     general provider, and a website opened in the viewer gets no account and no signature on PoCol Mainnet. Should a
     per-site, optional-permission connection be designed and built for this release, or does this release keep
     Studio and the IDE as the only sites that connect?
   - *Answer.* "يبقى الحال كذلك في هذا الإصدار" ("It stays as it is in this release.")
   - *Meaning.* Only PoCol Studio (`https://studio.pocol.net`) and PoCol IDE (`https://ide.pocol.net`) connect, through
     the extension's content scripts on those two origins (`prototypes/pocol-extension/build.mjs`), and each operation
     opens the extension's own confirmation window. A website opened in the viewer gets no account and no signature on
     PoCol Mainnet: `seedWalletAllowed` in `prototypes/pocol-extension/src/sw.js` refuses the RN profile and the public
     network key, and `eth_accounts` answers an empty list. The viewer's local wallet stays for local development
     networks the user adds, after the user's approval. No code changes.
   - *What stays open.* Connections for arbitrary sites are deferred, not rejected: a later release may design them
     (per-site optional permissions); that needs its own owner decision.
   - *Affected.* Findings ledger F-99 ([FINDINGS.md](../public-network/FINDINGS.md)), `prototypes/pocol-extension`
     README-IDE.md, README-AR.md and README-STUDIO.md, `store/listing.md`.
2. **D2: the Chrome Web Store item keeps the id the store assigns (route A).**
   - *Question.* A store item's id is fixed by its first upload. Route A: upload the store zip without a key and accept
     the id the store assigns (the officially documented route). Route B: the historical, unofficial `key.pem` upload,
     to give the store item the side-loaded release's id `gggcoflmedlhlkcdhebmoepmggdnafmp`. Which route?
   - *Answer.* "قبول المعرّف الذي يمنحه Chrome Web Store" ("Accept the id that the Chrome Web Store grants.")
   - *Meaning.* The owner uploads the normal store zip (`npm run package:store`: the release build with `manifest.json`
     minus `key`, no private key in it) and the store item gets its own id. That id differs from the side-loaded
     release's `gggcoflmedlhlkcdhebmoepmggdnafmp`: Chrome treats the two as separate extensions with separate storage,
     and users move a wallet between them with its encrypted backup (`store/README.md`, "How users keep their
     wallets"). Route B is not used: no private key goes into an upload. `scripts/package-store.py` keeps refusing
     `--first-upload-key` without `--unofficial-key-pem` (checked on 2026-11-23 by `tests/package_store_test.mjs`, "the
     unofficial key.pem method is never the default"); no code changes.
   - *What stays open.* The store item's id exists only after the owner's first upload. pocol.net then publishes it with
     the store link (W16), beside the side-loaded release's id. Whether a later side-loaded release adopts the store
     item's public key as `release/manifest-key.pub` (so that both share the store's id) is a later owner decision;
     until then side-loaded releases keep `gggcoflmedlhlkcdhebmoepmggdnafmp`.
   - *Affected.* F-69 and F-100, `prototypes/pocol-extension/store/README.md`, README-STUDIO.md, README-IDE.md,
     README-AR.md, `deploy/websites/README.md` (SITE-03).
3. **D3: the on-chain site viewer stays in the store build; the remote-code answer is "Yes".**
   - *Question.* The viewer runs the code of websites published on PoCol in the manifest's sandboxed page. Keep it in
     the store build and answer the dashboard's remote-code question "Yes" with the justification, or build a store
     variant without the viewer (answer "No")?
   - *Answer.* "يبقى في نسخة المتجر من الإضافة" ("It stays in the store version of the extension.")
   - *Meaning.* The store zip is the release build (viewer, sandbox page, omnibox and declarativeNetRequest rules
     included), and the dashboard's remote-code answer is "Yes, I am using remote code" with the justification and
     test instructions of `store/listing.md`. The store variant without the viewer is not built.
   - *What stays open.* The store's review decides; a longer review is expected. If the review objects to the viewer,
     what to do then (a variant without it, or another answer) is a new owner decision.
   - *Affected.* F-100, `store/listing.md` ("Remote code"), `store/README.md`.
4. **D4 (SITE-01): no extension link and no extension digest on any site before the signed release.**
   - *Question.* The sites track withdrew every extension package link and every listed extension package digest
     until the signed release (SITE-01). Does the owner approve that policy, with the Chrome Web Store as the official
     channel?
   - *Answer.* "لا رابط ولا بصمة قبل الإصدار الموقّع" ("No link and no digest before the signed release.")
   - *Meaning.* The policy already implemented on this branch stands: no page of pocol.net, the guide, Studio's install
     dialog or the IDE links an extension package, and no site lists an extension package digest
     (`SHA256SUMS.txt`, `keptOut`, `.sha256` files), until `deploy/websites/tools/release_page.py` records a signed
     release (`deploy/websites/tests/test_downloads.py`). The extension ID and the release key fingerprint that users
     compare are not package digests and stay in the snapshot (SITE-03). No code changes.
   - *What stays open.* The signed release itself (W16) and the uploads of the sites.
   - *Affected.* F-57, `deploy/websites/README.md` ("Production readiness, sites track").
5. **D5: PoCol Studio's version goes from 0.1.1 to 0.5.0.**
   - *Question.* The step-7 release set PoCol Studio's `package.json` to 0.5.0, the extension's version, after 0.1.1.
     Does the owner confirm the jump?
   - *Answer.* "يقفز من 0.1.1 إلى 0.5.0. نعم" ("It jumps from 0.1.1 to 0.5.0. Yes.")
   - *Meaning.* PoCol Studio 0.5.0 (`prototypes/pocol-studio/package.json`, `RELEASE-NOTES.md`) is confirmed; its
     version follows the extension's from now on. No code changes.
   - *What stays open.* Nothing for the version; Studio's upload to `studio.pocol.net` is the owner's step.
   - *Affected.* `prototypes/pocol-studio/RELEASE-NOTES.md`.
6. **D6: the pocold RPC work-deadline fix is applied.**
   - *Question.* In `prototypes/pocold/src/rpc.rs` (`conn`), a connection thread that wakes late can send as 200 an
     answer the node completed after the work deadline, because its wait returns a queued answer before it looks at
     the time; RPCGUARD.md and the second review state that after the work deadline the connection answers 504 and
     sends no answer. Apply the proposed patch (each reply records when it was complete, and a reply completed at or
     after the work deadline is answered 504)?
   - *Answer.* "نعم طبق الاصلاح" ("Yes, apply the fix.")
   - *Meaning.* Applied on this branch in the commit "pocold RPC: a reply finished after the work deadline is answered
     504, as RPCGUARD documents", with a deterministic regression test that fails without it. It is local RPC code,
     not consensus: `pocold/src/rpc.rs` is not in `prototypes/pocold/consensus-sources.lock`, and the lock is unchanged.
     The node's informational build digest (`compatibilitySha256`) changes with any node source, as with every node
     change; no test or document pins its value.
   - *What stays open.* Nothing for the fix; it reaches users with the release built from the merged tree.
   - *Affected.* [RPCGUARD.md](../audit-2026-10-22/RPCGUARD.md) §14.

## Where it stands (2026-11-23)

D1, D3, D4 and D5 confirm what the branch already does; D2 chooses the documented store route and leaves the code as
it is; D6 is applied. The texts that presented these as open questions or recommendations now state the decisions:
`prototypes/pocol-extension/store/README.md` and `store/listing.md`, the extension READMEs, the Studio release notes,
`deploy/websites/README.md`, [FINDINGS.md](../public-network/FINDINGS.md) (F-57 and F-69, and the accepted risks
F-99 and F-100) and [STATUS-EN.md](../STATUS-EN.md). Still the owner's: the Chrome Web Store account, the first upload
and the review, the item name and category (`store/listing.md`), the signed release and the site uploads (W16).
