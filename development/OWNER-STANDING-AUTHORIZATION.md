# Owner standing authorization — 2026-10-21

Recorded verbatim from the owner (Raed S. Rasheed) in the Claude Code session
https://claude.ai/code/session_01WdDy26DUcwZsDK64zKqRL4 :

> أريد أن تبدأ بإكمال العمل حتى ينتهي وأي طلب موافقة مني خذ الموافقة على الخيار
> الموصى به ولا تتوقف عن العمل حتى يكتمل العمل المشروع بشكل كامل ويحقق جميع
> المواصفات التي قمت انت و codex بالتوافق عليها. أريد تقرير عند نهاية كل مرحلة ما
> تم انجازه وما سيتم انجازه. أكرر لا تتوقف عن العمل حتى تنهي المشروع بالكامل.

English summary: continue the project through LP10 without stopping; for every
pending or future owner decision, the **recommended option** is approved; publish
an Arabic report at the end of every stage (what was done, what is next).

## How this authorization is applied

1. Every decision document that names a recommended option is recorded as approved
   in its own `LP*-...-APPROVAL.md`, citing this file. Decisions without a stated
   recommendation get an explicit engineering recommendation first, then approval
   under this file. Nothing here rewrites M1, LP1–LP3 evidence or earlier approvals.
2. **Boot.** The earlier instruction "boot remains disabled" is superseded only for
   local development profiles of kind `simnet` (local devnet, disposable keys, no
   funds). The `network` profile (chainId 777001) is still not bootable and nothing
   is deployed or published as a production network.
3. **Scope limits that still hold:** no real funds or funded keys, no purchases, no
   production deployment, no merge into `main` without a separate request, and no
   destructive rewrite of existing branches or evidence.
4. Author/reviewer separation continues: implementation is authored on branch
   `claude/affectionate-pascal-rjhh2k`; independent checks are run by separate
   reviewer agents and by independent oracles. A shared GitHub account is not an
   independent human approval.
