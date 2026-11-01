# M1 — Draft 0.11: Amendment R11-01 (C24 repair only)

**Status:** review draft, **NOT approved**. Phase S only.

- **Nothing in 0.11 was executed by the author.** The session had file tools only.
- Only new files under `m1-draft-0.11/` were written. 0.10, its results and every earlier package, the ledger, the GUI, reviews and checkouts are unchanged.
- The last independently accepted revision is still 0.9.

**Task.** `coordination/task-009.md`.

**Review input.**
- `coordination/review-001/REVIEW-0.10.md`.
- The independent oracle `epoch-independent-probes-0.10.json`: 24 checks, 17 pass, **7 FAIL**.
- The 0.10 default suite: 113 entries, 86 pass, 27 recorded, 0 fail.

Both results stay preserved as they are.

## C24 defects (verbatim counterexamples)

| # | Counterexample | 0.10 result | Required |
|---|---|---|---|
| 1 | `parse_epoch_name(epoch_name(6) + '\n')` | 6 | None |
| 2 | `parse_record_name(record_name('N','A',6,0) + '\n', 'N', 'A')` | (6, 0) | None |
| 3 | `epoch_name(True)` | accepted | refused |
| 4 | `record_name('N','A',1,True)` | accepted | refused |
| 5 | `epoch_value_ok({'bootMs': True})` | True | False |
| 6 | `valid_record((1,0), {…'epoch': True…})` | True | False |
| 7 | `valid_record((1,1), {…'seq': True…})` | True | False |

**Causes.**
- In Python `re`, `$` also matches before a final `\n`.
- `bool` is a subclass of `int`, and `True == 1`.

## R11-01 repair (`tools/sr_ref.py`, drop-in)

1. **Names.** `re.fullmatch` on the exact string, with no trim and no normalization. A name must also be exactly a `str`.
   - Rejected: LF, CRLF, CR, TAB, NUL, spaces before or after, nonce or other suffixes, foreign prefixes, uppercase, short or long hex, `0x`, non-ASCII digits, epoch 0.
   - Kept unchanged: the canonical baseline forms. That is the `pocol:epoch:` and `site:…:r:` namespaces, 16/8 lowercase zero-padded hex, `E ∈ [1, 2^64−1]` and `seq ∈ [0, 2^32−1]`.
2. **Integer fields.** `type(x) is int` for `epoch`, `seq`, `fmt` and `bootMs`, both in the generators and in stored values. Ranges are checked too. `tomb` must be exactly `bool`. Wrong types raise `FieldTypeError`, a subclass of both TypeError and ValueError.
3. **Integral floats (P-C24-2): rejected.**
   - 6.0 is not accepted as 6, because that would rely on Python float==int equality.
   - JS `JSON.parse` would read `6.0` as 6. But the declared u64/u32 ranges exceed what a JS Number holds exactly (2^53−1), and the stored numeric representation belongs to the fmt-2 codec, which is row E4 and pending.
   - This is a stricter reference rule, stated openly. It is not a new owner policy, and it is open to reviewer override.
4. **bootMs range (P-C24-4).** |bootMs| ≤ 2^53−1, the JS safe-integer range of `Date.now()`.
5. **World runtime.**
   - The 0.10 source is loaded read-only, and its module globals are rebound to the strict helpers. So `World.epochs`, `lookup` (and through it `valid_record`), `record_name` in `_name`, `_observe` and `World.__init__` all use the repairs.
   - `World.epochs` is replaced so that `bootMs` follows the strict value rule.
   - `self_check()` reports the wiring, and the runner asserts it.
6. **bootMs of an item with a valid name but an invalid value (P-C24-3).**
   - The item still counts in EN and M, because counting is by name.
   - It contributes no time to the window or to `lateGens`.
   - This is the 0.10 treatment of non-dict values, extended to bool, float and out-of-range values.
   - Finding RF-8: a more conservative reading (count it as inside the window) would need a rule for when it leaves the window. That is reviewer-level.
7. **Unchanged:** the epoch gate, name gate, window, scheduler, retry caps, checkpoints, disk adapters, sequence allocation and every owner policy. There is no E4–E7 expansion. **No claim of fmt-2 wire validation.**

Finding RF-9: the baseline says EN counts the `pocol:epoch:*` items returned by `getKeys` (browser.md:509). Following C24, malformed names under that prefix are not epoch items and are not counted. This matters only for names that no generation writes. It is flagged for review.

## Evidence plan (`tools/run_checks_011.py`)

1. **Preserved failures.** The 24 independent probes run against an **unpatched** copy of 0.10. They must reproduce exactly the 7 recorded failures. The Node oracle cases are cross-checked against the review file `epoch-name-oracle.json`.
2. **Repaired probes.** All 24 probes must pass on the repaired module.
3. **Entire 0.10 suite re-run** with the repaired module bound as `sr_ref`:
   - E1;
   - BR22h h1–h6, with 2 × 729 h5 assignments;
   - BR22b, retry and sweep;
   - all 520 BR22a combinations;
   - every legacy control.
4. **New regressions** (`vectors/c24-regressions.json`):
   - 19 epoch-name and 14 record-name rejections, plus canonical accepts including max u64/u32;
   - generator type and range rejections and accepts;
   - epoch-value and record-value cases;
   - four World scenarios:
     - **W-A1:** an LF-tailed epoch name does not move E (repaired E = 6; 0.10 gave 10).
     - **W-A2:** seven LF-tailed names do not fill the name gate (0.10 blocked).
     - **W-B:** bool `bootMs` contributes no window time (repaired confirms at 1001; 0.10 waited until 60003).
     - **W-C:** recovery skips records with bool or float numeric fields and an LF-tailed record name, and reads (1, 0) after exactly 4 reads (0.10 read the LF name).
   - Each pristine (0.10) outcome is also checked against its hand-written defect expectation.
