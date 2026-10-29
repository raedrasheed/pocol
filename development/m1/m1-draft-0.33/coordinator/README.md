# Coordinator continuation patch 0.33

**Patch bundle for root review. The author executed nothing.** Nothing here is live until root has run the tests, reviewed the bundle and applied it.

- **Target:** `coordination/issue3-repo/tools/local-coordinator`.
- **M1 specification acceptance is unchanged** and remains revision 0.32. This bundle changes coordinator tooling only.

## What changes

### 1. Continuation after an actual review (`src/broker.mjs`)

**When it fires.** When the host records an actual review (`/api/reviewer/review`), the broker decides once whether to queue a continuation. It is **off by default**. It queues one only if the saved `coordination/issue-ledger.json` meets all of these:
- `standingAuthorization.autonomousSequentialCycles === true`;
- `standingAuthorization.noProduction === true`;
- `standingAuthorization.scope` names `M1`;
- `continuationWork.status` is `ready` or `blocked`;
- `continuationWork.remainingIndependentItems` holds at least one well-formed item with scope `M1` that is still ready (no `status`, or `status: "ready"`).

**What disables it.** Any of these turns it off, with a named reason:
- a missing, malformed or `complete` worklist;
- a worklist that is `blocked` with no ready item;
- `autonomousSequentialCycles` that is not the boolean `true`;
- a scope without `M1`.

**What can block it even when allowed.** These gates are checked in order:
1. service stopping;
2. pause, which **defers** the continuation;
3. an active writer or worker lease;
4. an unreviewed receipt;
5. an already-open continuation item.

**What happens when it fires.**
- **One item per review.** Exactly **one** durable item of kind `continuation` is queued. Its idempotency key is derived from the review ID (`cont-<reviewId>`), so a restart cannot create a second.
- **One notification.** The same existing host thread is notified once, with the fixed reason `continuation`. Delivery state, restart uncertainty and explicit retry all work exactly as for existing notifications; nothing is retried automatically.
- **Nothing else.** No review is invented, no author is started, and no loop runs.

**What the host does next.** The host coordinator claims the item and chooses:
- `ack` with no plan: nothing happens;
- `ack` with `dispatch: { mode: "author", taskFile, plannedDir }` (or `smoke`): one `authorJob` is created as `approved`. It is validated exactly like a normal approval, and starts only through the unchanged dispatch gates (pause, writer, lease, unreviewed receipt, attached host, session, `claude.exe`, ledger).

**Durability.**
- Each review is saved with `continuation.status = "pending"` before the decision. A restart finishes a pending decision once.
- An item saved just before a stop is reused, never duplicated.
- Reviews saved by the old broker have no such field and never trigger a continuation.

**Manual resume.** A continuation deferred by pause is decided once, for the latest review only, when someone resumes manually. There is no round limit and no automatic resume.

### 2. Inactive ledger state (`ledgerBlock`)

**What still works.** `activeAuthor.state === "completed"`, or no state at all, is still accepted. This keeps terminal ledgers compatible.

**What is new.** The states `reviewed`, `ended` and `finished` unblock dispatch **only** if all of these hold:
- `activeAuthor.brokerJobId` names an author job that this broker persisted;
- that job has an actual recorded review whose `jobId` matches.

Unknown states, `running`, a mismatched or missing job ID, or an unreviewed job stay blocked.

After reviewing a broker job, root records `activeAuthor.state = "reviewed"` together with its `brokerJobId`.

### 3. Read-only authority display (`authorityView`, `public/index.html`, `public/app.js`)

A new section shows, in Arabic, the standing delegation and the continuation progress. It reads only safe ledger fields:
- the booleans;
- the scope, cut to 300 characters;
- the worklist IDs, kinds and statuses.

It also shows the policy decision, the current gate and the last review's continuation status.

It is displayed separately from the owner questions. Owner answers, including UI-TEST, are untouched, and guidance is never turned into an approval.

### 4. Notifier (`src/notifier.mjs`)

There is a new fixed reason, `continuation`. It uses the same `codex queue --remote unix:// --thread <existing>` command, with no exec, model or permission override.

The message contains only:
- the item ID;
- the private connection-file path;
- fixed text telling the host to read `standingAuthorization`, `continuationWork` and the latest checkpoint, claim the one item, decide the next useful M1 action itself, and ignore acknowledged continuations.

### 5. Saved connection reuse (`src/server.mjs`)

**When it reuses.** On start, a valid private `connection.json` keeps the same port and both capabilities, so an open browser page keeps working without a relaunch. "Valid" means all of these hold:
- the workspace is this workspace;
- the URL is `http://127.0.0.1:<port>/#<64 hex>`;
- the port is in 1024–65535;
- the reviewer capability is 64 hex characters and differs from the control capability;
- the reviewer API URL and header are exactly what this server writes.

**When it does not reuse.** Anything else is **never reused**. New capabilities are created, and a notice names only the reason. `--fresh-connection` forces new capabilities.

**When it refuses to start.** If the saved port is busy, the start fails with an actionable message instead of silently moving to another port.

**What stays the same.**
- The service lock is acquired before any reuse, so a second service still cannot start.
- Capabilities never appear in command-line arguments or `server.log`. The printed local URL is unchanged existing behaviour.
- The reviewer API still refuses any request that carries `Origin` or `Sec-Fetch-Site`, and any browser capability.

### Preserved behaviour

These are all unchanged:
- CSP and the Host/Origin checks;
- the reviewer capability guard;
- the single-writer lease and service lock;
- idempotency keys;
- pause and drain;
- notifier configuration;
- the saved state schema (`pocol-local-coordinator/1`; new fields are optional);
- all queue items, reviews and owner answers.

## Test commands (root)

1. **Assemble an isolated, complete tree.** This copies every unchanged live file byte for byte, overlays the 8 changed files and computes the old and new SHA-256 values:
   ```
   node m1-draft-0.33\coordinator\tools\bundle.mjs assemble --live coordination\issue3-repo\tools\local-coordinator --out <new test dir>
   ```
2. **Run the full suite** in that directory, with Node 22 built-ins only and no install:
   ```
   cd <new test dir>
   node --test test/broker.test.mjs test/history.test.mjs test/server.test.mjs test/worker.test.mjs test/notifier.test.mjs test/continuation.test.mjs test/connection.test.mjs
   ```

**Initial expectation:**
- the 63 baseline tests are unchanged;
- 14 new tests are in `test/continuation.test.mjs`;
- 5 new tests are in `test/connection.test.mjs`;
- **82 tests, 0 failures.**

**If a test fails,** the bundle is not applied.

## Apply (root, only when no author is running and after review)

1. **Stop the coordinator service** (Ctrl+C). Confirm there is no `worker.lease`. An ended author receipt must already be reviewed.
2. **Verify, then apply:**
   ```
   node m1-draft-0.33\coordinator\tools\bundle.mjs verify --live coordination\issue3-repo\tools\local-coordinator --manifest <test dir>\APPLY-MANIFEST.json
   node m1-draft-0.33\coordinator\tools\bundle.mjs apply  --live coordination\issue3-repo\tools\local-coordinator --manifest <test dir>\APPLY-MANIFEST.json --backup <new backup dir> --ui-control coordination\ui-control
   ```
   `apply` refuses in any of these cases:
   - a `worker.lease` or `service.lock` exists;
   - any live file differs from the assembled manifest;
   - the backup directory already exists.

   The old copies of the changed files are kept in `<backup>/files`, with `ROLLBACK.json`.
3. **Restart the service** with the same command line. When the saved connection is valid, the existing browser URL keeps working.
4. **Record in the ledger** (root's own action) that the patch is live, after checking the running service.

## Rollback

1. Stop the service.
2. Run:
   ```
   node m1-draft-0.33\coordinator\tools\bundle.mjs rollback --live coordination\issue3-repo\tools\local-coordinator --backup <backup dir> --ui-control coordination\ui-control
   ```
   This restores the old files after checking their SHA-256 values. An added test file is removed only if it is unchanged since apply.
3. Restart the service.

`ui-control/state.json`, `connection.json` and the ledger are never touched by `apply` or `rollback`. Saved items, reviews, notifications and owner answers stay as they were. The old broker ignores the new optional fields.

## Not included

- No dependencies, binaries, runtime files, raw state, private connection values or transcripts. The tests use synthetic capabilities (`a…a`, `b…b`) and the existing synthetic session helper.
- No dispatcher helper: the existing dispatch path already inspects the actual queue and review status, and refuses an existing planned directory or a second job.
