# PoCol development

## Current status — 7 October 2026

This page tracks development. Sanitized copies of actual local work through draft 0.7 were imported into `work/m1-import`, preserving the thesis and local sources. Details and results are in [STATUS-EN.md](STATUS-EN.md).

**Locally verified:** the 0.8 author/reviewer cycle corrected the profile reference in 0.7. Codex performed 261 successful checks and recorded three values, without failures. Sanitized review evidence and earlier-cycle failures are preserved; there is no implementation approval or project CI result yet.

M1 here means preparing the specification for web-page storage, associated contracts and test evidence. It does not mean the PoCol network is complete or its security or efficiency has been established.

## Following the work

| Location | What it explains |
|---|---|
| Issues | The problem, impact, resolution requirements and status |
| Pull requests | Proposed changes and reviews before merging |
| Checks | Automated test results tied to the change, once configured and run |
| development/STATUS-EN.md | A short status summary created at import and updated after each cycle |
| development/decisions/ | Decisions required from the project owner and their rationale |

Every status update must explain what changed, why, what was actually tested, what remains and whether Raed needs to decide anything. Code, technical specifications and current progress summaries are in English. Earlier summaries were originally written in Arabic.

## Workflow

1. Preserve the thesis and all existing work. Import the local specification history on a separate branch and open a draft PR.
2. Claude authors scoped changes. Codex independently reviews and runs applicable checks. Use sequential author/reviewer turns to avoid overlapping writes.
3. Link each finding to a requirement, affected revision, proposed fix, and evidence. Distinguish open, proposed fix, verified, deferred, and owner decision.
4. Update the status summary and the PR after each completed cycle. Record commands, tool versions, exit codes, and the exact tested revision. A successful test run does not establish complete coverage.
5. Do not close findings by elapsed time, fixed round count, or agreement alone. Missing tests remain missing. Experimental code is not production approval.
6. No automatic merge, deployment, or expansion beyond specification/fixture work is authorized by this tracking setup.

## Local coordinator handoff

Repository: https://github.com/raedrasheed/Proof-of-Collaboration
Tracking branch: `docs/development-tracking`
Local project reported by the user: `D:\PoCol-Development`

The local coordinator should finish or checkpoint its active author turn before migration, then:

- Read this document and the actual current local issue ledger. Identify the latest complete revision; do not assume 0.7 has completed.
- Inspect Git status, remotes, GitHub CLI availability and authentication. Do not overwrite an existing checkout, change unrelated remotes, reset work, or force-push.
- Fetch the tracking branch into an appropriate checkout and create a separate `work/m1-import` branch from it. Keep the original local project and earlier drafts intact.
- Import project specifications, revision history, deterministic fixtures, source tools and relevant review evidence. If prior Git commits exist, preserve them where practical; copying snapshots alone must not be described as preserving Git history.
- This repository is public. Stage only reviewed project files. Exclude credentials, tokens, private keys, personal account data, `.env`, model session directories, raw private transcripts, runtime binaries, downloaded dependencies and caches. Keep sanitized task/review records needed for reproducibility.
- Create `development/STATUS-EN.md` with actual revision paths, completed work, checks actually run, remaining blockers and plain English explanations of owner questions. Preserve unanswered questions as open.
- Push the import branch and open a draft PR. Put the link and a short English progress explanation in the local session. Do not merge it.
- Thereafter publish sanitized revision/review summaries to the PR so the owner can follow work in the browser without carrying messages between agents. Clearly identify which agent produced each report; a shared account is not independent GitHub approval.

If GitHub access is unavailable, report the exact blocker without requesting secrets in chat. Do not claim a push, PR, or test succeeded without its result.

## Execution boundary

GitHub is the shared record, not an automatic host for the current agents. Claude and Codex currently run through the local coordinator. No cloud runner or agent integration has been configured by this document. CI workflows should be added after inspecting the imported tools and their requirements; do not introduce a permanently green placeholder check.

## References

- [GitHub Issues](https://docs.github.com/en/issues/tracking-your-work-with-issues/learning-about-issues/about-issues)
- [Pull requests](https://docs.github.com/en/pull-requests/reference/pull-requests)
- [GitHub Actions](https://docs.github.com/en/actions/get-started/understand-github-actions)

The status above is based on the user's local-session screenshot from 2026-10-07 and repository inspection. It must be updated from the actual imported artifacts before any implementation gate is evaluated.
