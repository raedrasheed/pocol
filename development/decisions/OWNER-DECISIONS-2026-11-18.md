# Owner decisions (2026-11-18)

Status: **decided by the owner on 2026-11-18.** Approving a decision is not approval to deploy: every launch step on
a server still needs its own approval.

## The owner's words (verbatim)

On the launch texts:

> الشبكة ليست جاهزة للاطلاق العام الكامل هذه الجملة يجب أن تحذف تماما

> لا يجب ان يكون هناك نص يذكر شيء غير حقيقي ولا شيء يمنع الاطلاق العام

Answers to the four questions put to the owner (verbatim): "1 أ، 2 لا شيء، 3 لاحقا، 4 نعم".

## Decisions

1. **Owner keys.** The owner made, offline, the extension manifest key (extension ID
   `gggcoflmedlhlkcdhebmoepmggdnafmp`, `prototypes/pocol-extension/release/manifest-key.pub`) and the release signing
   key (fingerprint `SHA256:FlrCFc4LXIOOPMrT2g4dgV6CuOEiw1L30NWKzGPBZt8`, `deploy/release/allowed_signers`). Only the
   public halves are in the repository.
2. **Server founders' reward addresses.** Three owner-held addresses, made offline with `pocol-key`, given to the
   ceremony as `-ServerRewardAddresses` (a ceremony parameter, not committed).
3. **Reward key storage (question 4: yes).** The recorded storage of the three reward keys is offline: an encrypted
   backup and paper copies, held by the owner. `pocol-key export` (the passphrase-encrypted `export-key` format) is
   available for a further backup.
4. **Truthful texts, nothing presented as preventing the launch.** No text in the repository or on the sites may
   state anything untrue, or present the public launch as prevented or conditional. The node's readiness report
   (`publicLaunchReady`, `launchBlockers`) is node-local, refuses nothing, and stays as computed: a gate is marked
   closed only when its acceptance criterion is met, with evidence.
5. **Launch disclosures.** They carry no "not public-launch-ready" item (question 2: and no list of ongoing
   engineering work). The other draft items still await the owner's review of their wording.
6. **Newcomers' first fee (question 1: a).** There is no faucet on mainnet (owner decision 2026-11-16). A newcomer's
   first registration fee (about 0.0012 PCOL of gas) is transferred by the founders on request. The request channel
   is published on pocol.net with the launch.
7. **Independent security review (question 3: later).** Gate Q1 stays open; the owner commissions an independent
   review after the launch.
