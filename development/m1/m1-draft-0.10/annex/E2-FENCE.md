# Annex E2: the generation fence, D134 (I74)

**Status: Partial.** Source: browser.md:507–531 and 707–712; validation.md:510–551; governance.md:276–278. Assumptions: threat.md:48–49.

## 1. Assumptions (explicit, not verified)

- **A15c** (threat.md:48):
  - every operation of a dead generation is applied or dropped within `T_LATE = 60 s` of the boot of the next generation;
  - the wall clock behind `bootMs` does not jump backwards by more than that.
- **A15d** (threat.md:49): a `set` or `remove` whose promise was **rejected** in the live generation is never applied later.

Both are hypotheses that BR22c is meant to measure. **This package does not claim that Chrome behaves this way.**
- A15c underpins the writer, name-limit and coverage lemmas, and every disk bound.
- Violating A15c invalidates the disk bound only, not the D103 ordering (threat.md:48).

## 2. Boot sequence (browser.md:509–516)

1. **List** `pocol:epoch:*` with `getKeys`. EN is the number of names, M the largest, and E = M + 1.
2. **Name gate**, before anything else: `EN + 1 ≤ EPOCH_NAMES_MAX = 8` (inclusive). On a breach:
   - nothing is issued, and `epochGateBlocks` is incremented;
   - wait until `T_LATE` has passed on the generation's **monotonic** clock since its boot;
   - **sweep** (§3), then re-list and re-run the gate **once**;
   - if the breach persists: every `storage_set` replies −32603 `{storeRecovery, reason: 'epochNames'}` for the whole generation, there is no snapshot, and **operational action 25** is raised.
3. **Generation window**, after the name gate (browser.md:707–712): if `GEN_WINDOW_MAX = 4` visible epoch items have `bootMs ≥ now − T_LATE`, issue nothing. Wait until the oldest leaves the window.
   - A waiting generation has written no item, so it is not counted.
   - P-E2-4: when the wait ends, the generation re-lists and re-runs both gates, so E is recomputed.
4. **Write** `F(E) = {bootMs}` and wait for it to succeed.
   - Retries reuse **the same name**, so they never add names.
   - `STORE_EPOCH_RETRY = 3`. P-E2-1: read as one write plus up to 3 same-name retries, four issues in all. The alternative reading, three issues in all, is recorded as finding RF-3.
   - After the last failure: `storage_set` → −32603 `{reason: storeRecovery}`, and no snapshot.
5. **No data write, session or snapshot before the confirmation** (browser.md:515).

## 3. Sweep (browser.md:517–520)

- **When:** once the generation has existed for `T_LATE` on its monotonic clock. It runs in a confirmed generation and in a name-gate-blocked one.
- **What:** remove every epoch name **smaller than the largest visible name at sweep time, whatever its stored `bootMs`**.
  - The largest name is never removed, so the visible maximum never decreases.
  - A failed remove still counts in EN.
- P-E2-5: a generation confirmed after its own boot + T_LATE (for example after a long window wait) sweeps at the confirmation.

## 4. Lemmas (restated; each holds under the stated assumptions)

| Lemma | Statement | Rests on |
|---|---|---|
| Writers (browser.md:521) | Let M_L be the largest visible name when the live generation L boots. Every writer of a name X ≤ M_L booted before L, so its successor booted ≤ bootMs(L), and all its operations settle before bootMs(L) + T_LATE. So a swept name never returns | **A15c** |
| Name limit (browser.md:522–527) | Every name ≤ M was visible earlier. Removed names do not return. So the present or possible names are the visible ones plus {M+1}. The gate keeps EN + 1 ≤ 8 at every issue. So there are at most 8 epoch items at all times, however many generations died before confirming | **A15c** (through the writer lemma) |
| Fence (browser.md:528–531) | A data-writing generation confirmed its item first, so a later generation sees it and takes a strictly larger E. An unconfirmed generation wrote no data. Of the generations that share an E, at most one confirms | the atomicity of a single item; the order of confirmation |
| Coverage (browser.md:670–672) | For a dead generation D that wrote data, every writer of the name E_D + 1 booted after D. So the stored bootMs of that item is ≥ the boot of a successor of D, and the item stays in the window while D's operations may still apply | **A15c**, and **A15d** for the site bound (browser.md:664) |
| Window (browser.md:708–710) | A data writer sees ≤ 3 items in the window before it issues. After that only its own name can appear. So `lateGens ≤ 4` for its whole life | A15c |

The lemmas are **textual**. The reference model checks their consequences on enumerated schedules: names ≤ 8, the maximum never decreasing, `epochResurrected = 0`, strictly increasing epochs of data writers. That is not a proof, and not evidence about Chrome (FINAL_DESIGN.md:6223).

## 5. BR22h vectors (validation.md:510–551)

**Initial state:** F(5) with bootMs −200000, and the S_A record (5, 3) holding {k0: 'a'}.

The FakeBackend holds every op until apply, settle or drop.

| Vector | Schedule | Expected (hand derived) |
|---|---|---|
| h1 | G1..G40 at k·1000. Each issues F(6), dies after 10 ms unsettled. All 40 apply after G40 boots, in a fixed-seed order (seed 2201) | every E = 6; the names are {F5, F6} at every moment; `epochGateBlocks` 0 |
| h2 | G1..G40 at k·70000. Each op is applied at boot+50; death at boot+100 (before T_LATE, so no sweep) | G1..G7 write F6..F12 with EN 1..7; G8..G40 see EN = 8 and issue nothing; `epochGateBlocks` = 33; ≤ 8 names |
| h3 | h2, then G41 at 2870000 stays alive | EN = 8, so it blocks (34 in all). Sweep at 2930000 removes F5..F11. Re-list at 2930001 gives EN = 1, so it issues F(13), confirmed at 2930002. Checkpoint (13,0) at 2930005, data (13,1) at 2930006, `null` at 2930007. No data `set` before 2930002 |
| h4 | as h3, with every remove failing | after the sweep EN is still 8, so the generation is disabled (`epochNames`) and action 25 is raised. Both writes get −32603 `{storeRecovery, epochNames}`. Zero F and data `set`s by G41. (5, 3) is unchanged |
| h5 | G1..G6 at spacings 1000 and 70000. Each F op is applied *before* the next boot (next−1), *after* it within A15c (next + T_LATE − 1, the latest moment allowed), or *dropped*. That is 3^6 = 729 assignments per spacing. G7 stays alive and writes data | every run keeps all invariants; G7 confirms and gets `null`. Literal rows: 70000/BBBBBB → E 6..11, G7 12, final {F12}. 70000/DDDDDD → all 6. 1000/BBBBBB → G1..G4 = 6..9, G5/G6 blocked by the window, G7 = 10 confirmed at 61002. 1000/DDDDDD → all 6. 1000/DDDDAB → G7 = 7. The number of distinct schedules is recorded by the runner |
| h6 | literal (validation.md:535–540) | G2 confirms F(6) with bootMs 70000. G1's late apply at 70600 makes it 0. G3 confirms F(7). At 71100: lateGens = 1, LATE_SITES = 4, 60+0+4+1 = 65 > 64, so B gets 4300 `{sites, 64}`. The late A applied at 100000 ≤ bootMs(F7) + T_LATE = 131000, so at most 61 keys |

**Controls** (governance.md:276–278):
- `epochNonceNames` gives 41 names in h1.
- `epochNoGate` (the task calls it `epochNoNameGate`) gives 41 names in h2.
- `epochSweepEarly` is detected in h5:
  - the literal baseline combination is in case `h5-baseline-example`;
  - in the enumeration it must at least detect 1000/DDDDAB, where G5's F6 lands after G7's early sweep and so `epochResurrected > 0`.
- **Author-added** `epochSweepByBootMs` (finding RF-1): a sweep by stored `bootMs` removes the overwritten F(6) maximum in `E2-sweep-ignores-bootMs`, so the visible maximum decreases.

## 6. Timing conventions of the model (P-E3-1)

- The fence is issued at the attempt. An alive generation's operations settle 1 ms after issue, unless the scenario makes them never settle. Removes settle 1 ms after issue.
- Same-millisecond priority: placement (apply/drop), then death, then settle, then generation steps, then user/page events.
- These conventions fix the literal times quoted above. The baseline does not prescribe them.
