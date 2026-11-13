# Owner decisions (2026-11-13): a brand-new PoCol network

Status: **decided by the owner on 2026-11-13.** Approving a design direction is not approval to deploy.

## The owner's words (verbatim)

> سنقوم ببناء الشبكة من الصفر ولن نحتاج اي شيء من الشبكة الحالية سنقوم بوقفها تماما والبدء من جديد في عملية النشر.

Answers to the 16 questions put to the owner (verbatim):

> 1. نعم بداية نظيفة تماما
> 2. ابدأ بمفاتيح جديدة تماما وكأنه لا يوجد شبكة سابقة.
> 3. لا تبقي شئا يتعلق بالشبكة القديمة نريد اطلاق شبكة جديدة تماما
> 4. سأوقفها بمساعدة ChatGPT ولا حاجة للاحتفاظ ببيانات سابقة
> 5. نعم استخدم رقم جديد
> 6. اطلاق شبكة رئيسية عامة
> 7. أين الجدول لأقرر
> 8. أقرر أن يكون لمعدنين الأوائل الأربعة كل منهم يحصل على مليون PCOL
> 9. المعدنون المؤسسون 4
> 10. زمن الكتلة غير محدد حسب الصعوبة أو حسب حل اللغز ولا يحدد بزم. ولا يوجد حد اقصى مطلقا أي شخص يريد التعدين يمكنه ذلك. لا افضل تحديد مجمع اعضاء تحت الاختبار ولا اريد رسوم انشاء حساب فقط حجز 20 PCOL من التعدين لمنح التصرف السيء
> 11. اتركها كما هي
> 12. موافق على ما توصي به
> 13. موافق على ما توصي به
> 14. نعم
> 15. 3 خوادم في أمريكا وجهازي وجوالي
> 16. نعم سيبقى PoCol والعملة PCOL

## Summary (translation)

1. Fully clean start: no balance, bond or account carries over.
2. Brand-new keys, as if no earlier network existed.
3. Keep nothing related to the old network.
4. The owner stops the old network (with ChatGPT's help); no old data is kept. Claude does not touch the old servers.
5. A new chain id.
6. Launch a public mainnet directly (no public testnet).
7. Issuance table: pending (the owner asked to see the table).
8. Genesis allocation: each of the first four miners gets 1,000,000 PCOL.
9. Four founding miners.
10. Block time set by difficulty / puzzle solving, not fixed by time; no maximum on the number of miners (anyone may mine);
    no probation pool; no account-creation fee; only 20 PCOL reserved from mining as a bond against misbehaviour.
    (Clarifications on 10 requested: see the follow-up.)
11. D-V3-21 other A03 cap halts: leave as they are (declared gas).
12. N2: two settings, `safe` about 6 blocks and the reorg alarm at 60 (recommendation approved).
13. E4: a pool-contest victim may reactivate into a free seat after U (recommendation approved; may become moot under 10).
14. N3: a node mode with no internal miner, off by default: yes.
15. Infrastructure: 3 servers in the USA, the owner's computer and the owner's phone.
16. Name PoCol, coin PCOL.

Earlier constraints about the old servers (no re-initialisation, never run Initialize-PoColNetwork.ps1, never delete wallet
data, preserve api/config.php) concerned the old network, which the owner now retires; Claude still does not act on
any server without explicit approval, and deployment of the new network stays held until the owner approves it.

## Follow-up answers (2026-11-13, verbatim)

> 7. اختار (ب) 10PCOL
>
> 10. أ) أفضل أن متوط زمن الكتلة 10 ثواني
>         ب) موافق
>         ج) موافق
>         د) موافق
>
> 6. موافق
>
> 15. موافق
>
> الدومين pocol.net

Summary:
- 7: issuance option (b): a starting reward of 10 PCOL per block (halving every 4 years, tail S0/32). Open: with a 10 s
  average block the per-block amount gives three times the yearly issuance of the table (computed at 30 s); asked the
  owner to confirm per block or per year.
- 10a: difficulty adjustment kept, with a 10 s average block time (instead of 30 s).
- 10b: no seat cap or seat contest; registrations rate-limited per block; inactive members removed after about an hour
  (they may return).
- 10c: new miners register for free; the first 20 PCOL of their rewards is held as a bond, forfeited on misbehaviour;
  after that rewards are paid in full. The four founders' 20 PCOL bond comes from their genesis allocation.
- 10d: no account-creation fee; new accounts pay ordinary gas only.
- 6: a closed rehearsal on the three US servers before the public launch.
- 15: founding miners: the three US servers and the owner's computer; the phone is a wallet/observer.
- Domain: pocol.net.
