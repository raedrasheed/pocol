# PR #27: resume point for the GitHub steps

[BODY.md](BODY.md) is the description to post on PR #27, verbatim. Post it as the whole body and keep
the PR a **draft**. It was written on 2026-10-25 for head `733a52d`, and updated for the local commits that
followed: `78fe971` (this file), `90c4185` (relay test), `24cc167` (R14/R19 store-mode boot) and the evidence
commit after them.

## Why it is here

At 15:46 UTC on 2026-10-25 this session lost GitHub access, and the update was not posted:
- the GitHub connector asked to sign in again;
- the container's git and `gh` token was refused ("could not read Username", "The token in GH_TOKEN is
  invalid").

The owner re-authorized later, but the running session's credentials did not refresh. Until they do,
nothing below can be done from it.

## Last state seen on GitHub (before the loss)

- **PR #27:** open, draft. Its head was `733a52d` and its base `claude/affectionate-pascal-rjhh2k`
  (`090eae4`).
- **Windows runs** (audit-r2-windows-verify.yml):
  - run 64 (`9bd9bc0`), run 65 (`919a50c`) and run 66 (`8584283`) passed every step;
  - the run on `733a52d` (documents and evidence only; node code as `8584283`) had not been seen.
- **Comments:** the PR reported one conversation comment at the last read. Review threads were not read
  in that pass, so they must be read before anything is posted.

## Steps once access returns

1. Read PR #27:
   - its head (expect the latest commit of `claude/audit-repair-a01-a14`);
   - its draft state;
   - its check runs on that head;
   - any review comments since 2026-10-25 15:44 UTC.
2. If a check failed, investigate it before anything else.
3. Push the commits made locally since `733a52d` (`git push -u origin claude/audit-repair-a01-a14`), so the
   body's commit references resolve.
4. Post BODY.md as the description, with any newer commits added to its "Commit roles" table. Do not
   mark the PR ready.
5. Record the Windows run of the new head in FINDING-STATUS.json `windowsVerification.runs` and in
   evidence/d7-bound/README.md.
