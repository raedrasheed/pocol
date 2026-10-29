# M1 — Draft 0.13: Amendments R13-01 to R13-03 (annex rows E4–E5)

**Status:** review draft, **NOT approved**. Phase S only.

- **Nothing in 0.13 was executed by the author.** The session had file tools only.
- Only new files under `m1-draft-0.13/` were written. All earlier packages, `reference/`, the ledger, the GUI, reviews and checkouts are unchanged.

**Task.** `coordination/task-011.md`.

**Scope.** Rows E4 and E5 of `m1-draft-0.2/M1-ANNEX-CHECKLIST-0.2.md:74–75`. E6, E7 and V2–V4 are not touched.

## R13-01: E4 normative annex

`annex/E4-E5-RECOVERY.md` §§1–5 covers:
- the RecoveryGate FSM, with the gateId fence and the 25 s bound;
- ReadSlots and RecoverySlots, released only on settle;
- the two checkpoint attempts with a single SeqAlloc, allocated before each issue and never reused;
- monotonic `confirmed` and late-success semantics (failed never becomes ready);
- the exact fmt-2 codec, with byte-sorted UTF-8 keys, be32 lengths, strict base64, and the corrupt and limit checks;
- the RECORD_BYTES_MAX derivation;
- the DiskLedger reserve/ticket/settleSeq/refresh rules, with the LATE_RESERVE, LATE_NAMES, EPOCH_NAMES_MAX and META_RESERVE literals.

Deferred, with no coverage claimed:
- AdminDelete, AdminSlots and tombs (E6);
- sites, pins and TombReaper (E7).

## R13-02: E5 canonical tables and fixtures

`vectors/e5-schedules.json`, all hand derived:
- BR22d base, V-a..V-e and the failure variant;
- BR22e;
- BR22f base, f2, f3, f4 (f4 through a narrow E6 adapter);
- BR22g g1–g6, including both g4 variants (g5 through the E6 adapter).

Each case gives literal timings, set counts and versions, statuses, the complete reply and snapshot sequences with full dictionaries, and full decoded backend records.

There are four baseline controls and four author controls. BR22f-f5 is E6 and is not included.

`vectors/e4-units.json`:
- codec known answers in hex and base64;
- rejections;
- the surrogate collision;
- the maximum-record size;
- DiskLedger boundaries and the ticket lifecycle.

## R13-03: reference and runner

- `tools/fmt2_codec.py`: strict codec, lossy-TextEncoder demonstration, `lookup_fmt2`.
- `tools/recovery_ref.py`: engine with FakeBackend (apply, settle ok/fail, drop, held reads, held removes), the slots, DiskLedger, a StoreQueue boundary adapter (per-key FIFO, at most 2 active) and the tomb adapter.
  - Names come from the 0.12 strict helpers, imported read-only.
- `tools/run_checks_013.py`: provenance and hashes; wiring (identity, plus a behavioural test that an LF-tailed record name is invisible to recovery); E4 units; E5 rows and controls.
  - It also re-runs the full 0.12 suite in memory (and through it 0.11 and 0.10), so the strict-name, type and physical-namespace repairs are re-verified.

## Open items (for review or owner; not approved)

- **CR-E4-01 / P-E4-1:** the u64 representation of `epoch` and `seq` in the value. This reference takes JSON numbers ≤ 2^53 − 1.
- **CR-E4-02 / P-E4-2:** the unpaired-surrogate reconciliation. Proposal: scalar-value validation at the bridge before persistence. It needs a baseline change.
- **P-E4-3 / P-E4-4:** the enc metric and the netKey length.
- **P-E5-1 to P-E5-7:** conventions.
- **RF-E5-1, RF-E5-2:** see annex §7.
