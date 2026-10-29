# Annex E1–E3: SR reference, P items, reviewer findings, E4–E7 boundaries

## 1. Reference model (`tools/sr_ref.py`)

Python standard library only. It runs on a manual clock, with a FakeBackend whose operations stay pending until the schedule **applies**, **settles** or **drops** them:
- settle ok = applied;
- settle fail = not applied (A15d);
- apply without settle = the D107 `apply` event.

It is **specification fixture tooling, not a Chrome implementation**.

**Pure functions:**
- `epoch_name` / `parse_epoch_name`, `record_name` / `parse_record_name` (strict, no normalization);
- `valid_record`, `lookup` (largest valid record, ≤ 4 reads);
- `epoch_value_ok`;
- `late_gens` and `site_admission`: D112 **adapters**, used only for h6.

**`World`:** generations with the fence (name gate → window gate → write → confirm → sweep), one shared recovery per key, checkpoint, data writes, record GC, death, viewer teardown and banner, and user-click reloads.

**Recorded every event:**
- epoch-name count, the visible maximum and resurrections;
- greatest-record survival;
- record-name reuse;
- data-writer epochs;
- replies and snapshots, with the generation that produced them;
- the FakeBackend metric.

**Faults:** `epochNonceNames`, `epochNoGate`, `epochSweepEarly`, `epochSweepByBootMs`, `singleItem`, `noEpochConfirm`, `noCheckpoint`, `reuseSeq`.

## 2. Oracle method: not circular

- **Golden first.** Goldens are hand calculations from cited lines, or hand-derived rule tables: the BR22a k1/E rules and the h5 literal rows. The model is compared *to* them.
- **Enumeration.** Enumerated runs (BR22a, 520 combinations; h5, 2 × 729) are judged by the **baseline's own criteria**, not by model output. The distinct-schedule counts are recorded, not assumed.
- **Provenance and hashes.** Literal anchors are checked in the cited source lines. The SHA-256 of every input is recorded.
- **Controls.** Every control must fail its named vector, **and** the same vector must pass without the fault.

## 3. Modelled choices not fixed by the baseline (P)

| ID | Choice | Citation |
|---|---|---|
| P-E1-1 | `hex16`/`hex8`: lowercase, zero-padded, exact length; other spellings are not items | browser.md:508, 534 |
| P-E1-2 | EPOCH_BYTES_MAX measured as compact JSON of `{bootMs}` | browser.md:508 |
| P-E1-3 | abstract record value; the fmt-2 codec belongs to E4 | browser.md:535 |
| P-E1-4 | a tomb carries `{}` | browser.md:535; D108 |
| P-E2-1 | STORE_EPOCH_RETRY = 3 read as 1 + 3 same-name issues | browser.md:514, 516 |
| P-E2-4 | after a window wait: re-list and recompute E | browser.md:711 |
| P-E2-5 | a generation confirmed after boot + T_LATE sweeps at confirmation | browser.md:517 |
| P-E2-6 | BR22h dead-generation lifetimes: 10 ms (1000 spacing), 100 ms (70000 spacing); survivor data request at boot + 10 | validation.md:515–517 |
| P-E2-7 | h5 dead generations never settle their F op; the baseline's settled-G2/G3 example is a separate literal case | validation.md:529–533, 551 |
| P-E3-1 | timing conventions and same-millisecond priority (annex E2 §6, E3 §5) | — |
| P-E3-2 | L5 "after B" is placed after C settles | validation.md:406, 418 |
| P-E3-3 | P4/P5 combine with every first death; "third death" = the recovering generation dying before any data write | validation.md:393–399, 408 |
| P-E3-4 | the BR22a initial confirmed k0 is record (1,1) of a live, ready gen1 with `seq` next = 2 | validation.md:392 |

## 4. Reviewer findings (not owner decisions)

- **RF-1: control names.**
  - The task names `epochNoNameGate` and `epochSweepByBootMs`. The baseline names `epochNonceNames`, `epochNoGate` and `epochSweepEarly` (governance.md:276–278; validation.md:549–551).
  - All three baseline controls are kept with their baseline expectations, and `epochNoNameGate` is treated as an alias of `epochNoGate`.
  - `epochSweepByBootMs` is an **added** author control: sweeping by stored bootMs contradicts browser.md:517 "whatever its stored bootMs". Its detection vector is author-designed.
- **RF-2: A15d versus the BR22a fail variant.**
  - BR22a asks for "a rejected promise that is applied later" (validation.md:408). That is exactly what A15d (threat.md:49) assumes never happens.
  - Reading: criteria (1)–(6) are D103 ordering properties and must still hold. The resolution lemma does not rely on A15d.
  - The site and disk bounds (browser.md:664, 695, 702) are conditional on A15c/A15d and are **not claimed** for that variant.
  - The runner checks (1)–(6) and (7a) there. (7b) is a scoped metric only.
- **RF-3: retry count.** "Failure is retried up to STORE_EPOCH_RETRY = 3 times" (browser.md:516) is ambiguous between 3 retries (4 issues) and 3 attempts. Modelled as 4 issues (vectors `E2-retry-*`); the other reading changes only those two vectors.
- **RF-4: h5 settlement.** h5 does not say whether dead generations confirm. Its sweep-early example (validation.md:551) needs G2 and G3 confirmed. Both are covered (P-E2-7).
- **RF-5: the f2 duplication in BR22f** is out of scope (E5).
- **RF-6: clock ties.** The baseline gives no same-millisecond order. Every literal time here depends on P-E3-1.

## 5. E4–E7 boundaries (not this batch)

| Row | Not modelled; assumption used by E1–E3 |
|---|---|
| E4 | the RecoveryGate state machine beyond `ready` (`failed` / `busy` / `readTimeout`, `gateId`); the second checkpoint attempt; SeqAlloc exhaustion (BR22e); the fmt-2 codec and RECORD_BYTES_MAX; disk acceptance; tomb semantics. **Assumed here:** the read is atomic, one checkpoint attempt, disk acceptance always ok, abstract values |
| E5 | BR22d (V-a..V-e), BR22f (RecoverySlots), BR22g (ReadSlots, gateId, the 25 s bound). **Assumed here:** no slot contention |
| E6 | DiskLedger, AdminSlots, AdminDelete, BR23/BR24, LATE_NAMES = 24, META_RESERVE, the 45 s bound. **Assumed here:** none; criterion (7) disk is an unexecuted dependency |
| E7 | DeleteOp, sitesLive, pin, TombReaper, BR23 d1/d9/d12–d15. **Used only as an adapter in h6:** sitesLive is an input (60), R_sites = 0, lateGens computed from the visible items per browser.md:665 |

Pending integration: the TypeScript StoreQueue and StoreRecovery over FakeBackend must reproduce these goldens (BR22a, implementation.md), then BR22c in Chrome.
