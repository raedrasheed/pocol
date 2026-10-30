# Owner standing authorization — 2026-10-21

English translation of the owner's statement (Raed S. Rasheed) in the Claude Code session
https://claude.ai/code/session_01WdDy26DUcwZsDK64zKqRL4 :

> I want you to start completing the work and continue until it is finished. For any
> approval request, take my approval for the recommended option. Do not stop until
> the entire project is complete and meets all specifications that you and Codex
> agreed on. I want a report at the end of each stage explaining what has been done
> and what will be done next. Again, do not stop until the whole project is finished.

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

## Current report language — 29 October 2026

The owner subsequently requested English throughout the repository. Current reports and interfaces use English. The historical authorization above is retained as a translated record; its earlier Arabic-report preference does not override the later language instruction.
