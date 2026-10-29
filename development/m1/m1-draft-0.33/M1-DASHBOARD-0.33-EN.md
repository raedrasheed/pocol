# Draft 0.33 summary: local coordinator improvement

**The author ran nothing.** This is a review patch bundle, not yet applied. M1 specification acceptance remains at revision 0.32.

## Added behavior

**One follow-up request after each actual review.** When standing authorization saved in the coordinator ledger is enabled, the broker queues one follow-up request and sends one notification to the same existing Codex thread. Activation requires:
- Sequential cycles enabled;
- A recorded “no production” condition;
- M1 scope;
- At least one ready independent worklist item.

**What it does not do:**
- No automatic review.
- No autonomous author start.
- No loop.

The host coordinator receives the request and decides the next step, or acknowledges it without action.

**When nothing is queued:**
- Authorization missing or invalid.
- Worklist complete or empty.
- An active author or worker lease exists.
- An unreviewed receipt exists.
- An open follow-up request already exists.
- Server shutting down.

**While paused:** defer the request and decide once on manual resume.

## “Completed and reviewed” ledger state

Allows a second author only when it references the same broker task and that task has a saved actual review. Unknown, running or mismatched states remain blocked.

## Authorization display

A new read-only section shows:
- Standing authorization;
- Independent worklist;
- Follow-up decision.

This authorization is not owner answers. Owner questions and answers remain separate and unchanged.

## Restart at the same link

If the saved connection file is valid for this workspace, the server reuses the same port and tokens so the open page continues without reopening.

If invalid, none of its tokens is used: a new link is created with a clear reason.

Tokens do not appear in commands or logs.

## Expected tests

- 63 original tests;
- 19 new tests;
- Total 82, no failures.

Only the root runs them, in an isolated copy.

## Remaining root work

1. Assemble the isolated copy and run tests.
2. Review the bundle.
3. Stop the service and apply with a backup.
4. Restart.
5. Verify and record in the coordinator ledger.

Rollback from the backup is possible at any time; saved state is untouched.
