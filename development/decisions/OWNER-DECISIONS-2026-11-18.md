# Owner decisions (2026-11-18)

Status: **decided by the owner on 2026-11-18.** Approving a decision is not approval to deploy: every launch step on
a server still needs its own approval.

## The owner's words (verbatim)

On the launch texts:

> الشبكة ليست جاهزة للاطلاق العام الكامل هذه الجملة يجب أن تحذف تماما

> لا يجب ان يكون هناك نص يذكر شيء غير حقيقي ولا شيء يمنع الاطلاق العام

Answers to the four questions put to the owner (verbatim): "1 أ، 2 لا شيء، 3 لاحقا، 4 نعم".

Later the same day, on funding newcomers' first fee by hand (verbatim):

> الارسال للبريد ستكون عملية مضنية لانني يجب ان اتبع البريد وهذا عمل مضني

and, on the automated fee faucet with the limits put to the owner (verbatim):

> موافق على الصنبور الآلي بهذه الحدود

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
   closed only when its acceptance criterion is met, with evidence. (Superseded in part by owner decision 2026-11-20
   D4, [OWNER-DECISIONS-2026-11-20.md](OWNER-DECISIONS-2026-11-20.md): the wording "stays as computed" and "refuses
   nothing" is refined. Launch approval is computed apart from technical readiness; the open gates are reported in
   `technicalReadiness` and do not withhold approval; every value stays computed and truthful. This decision's
   truthfulness rule stays.)
5. **Launch disclosures.** They carry no "not public-launch-ready" item (question 2: and no list of ongoing
   engineering work). The other draft items still await the owner's review of their wording.
6. **Newcomers' first fee: the automated fee faucet on mainnet.** The owner first chose a transfer by the founders on
   request (question 1: a), then, the same day, replaced it with the automated fee faucet, which supersedes the
   rehearsal-only decision of 2026-11-16. The faucet (`deploy/faucet`) runs on mainnet with these limits, which the
   code enforces as ceilings (`MAINNET_LIMITS`; a config may set lower values, never higher):
   - 0.002 PCOL per address, once ever;
   - new accounts only: never sent a transaction, no code, a balance below the amount;
   - at most 200 transfers a UTC day (0.4 PCOL a day) and 3 per client a UTC day;
   - from a dedicated operator key that holds at most a 10 PCOL float; a claim is refused while it holds more.

   The operator key is a hot key, generated on the server that runs the faucet; it is not a reward, miner or release
   key. The faucet creates no coins and does not touch consensus. Deploying it on a server, and funding its float,
   needs the owner's approval like every other launch step. Any PCOL holder can still send a newcomer its first fee.
7. **Independent security review (question 3: later).** Gate Q1 stays open; the owner commissions an independent
   review after the launch.
