# M1 — Draft 0.10: Amendments R10-01 to R10-04 (annex rows E1–E3)

**Status:** review draft, **NOT approved**. Phase S only.

- **Nothing in 0.10 was executed by the author.** The session had file tools only.
- Only new files under `m1-draft-0.10/` were written. Earlier drafts, `reference/`, the ledger, the Dialogue source, reviews and checkouts are unchanged.

**Task.** `coordination/task-008.md`, on the owner's dashboard guidance.

**Scope.** Rows E1–E3 of `m1-draft-0.2/M1-ANNEX-CHECKLIST-0.2.md:71–73`: worker-generation recovery (D103, D134). Not the protocol-chain epoch, and not the UI broker.

## R10-01: E1

`annex/E1-NAMES-RECORDS.md`:
- the shared `F(E)` name with no nonce, its value `{bootMs}` and its bound;
- record names and ranges;
- records never overwritten, and `seq` never reused;
- largest valid (E, seq) wins, with no normalization;
- the meaning of a tomb;
- the fmt-2 codec staged to E4.

## R10-02: E2

`annex/E2-FENCE.md`:
- the name gate (EN + 1 ≤ 8), then the window, then the write, then confirmation;
- same-name retries;
- the post-T_LATE sweep by name order;
- the persistent `epochNames` failure and action 25;
- the writer, name-limit, fence, coverage and window lemmas, with **A15c/A15d stated explicitly**;
- BR22h h1–h6, with the three baseline controls plus one author control.

## R10-03: E3

`annex/E3-RECOVERY.md`:
- fence → one shared gate per key → read → checkpoint → snapshots, then data;
- the resolution, allocation and survival lemmas;
- memory reset on death;
- viewer teardown and banner, with reload only by user action;
- the storage-API assumptions;
- BR22a (full enumeration, criteria, rule tables, four controls);
- BR22b literal;
- BR22c as a future specification with the 20-attempt inconclusive rule and its measurement obligations.

## R10-04: reference, vectors, runner

**Tools.**
- `tools/sr_ref.py`: the SR-model.
- `tools/run_checks_010.py`: the runner. It writes only `results/run-results-0.10.json`.

**Vectors.**
- `vectors/e1-units.json`
- `vectors/e2-fence-cases.json`
- `vectors/e3-br22a.json`

**Annex.** `annex/E-REFERENCE-AND-FINDINGS.md` holds the P items, the reviewer findings RF-1 to RF-6, and the E4–E7 boundaries.

**Reuse of 0.9.** None. The 0.9 StoreQueue model covers D101 inside one generation. E3 needs only the per-key FIFO of the recovering generation, which the SR-model implements directly as a boundary adapter (one write in flight per key). The 0.9 files are read only, and hashed as inputs.
