# Windows verification of bc38d41 (node code as b6d57e8: review fixes 1 and 4, stored versions read by diff at open)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40572992585, https://github.com/palcertcom/pocol/actions/runs/40572992585
- **Trigger:** pull_request on `claude/memory-r14-followup` (palcertcom/pocol#2, draft).
- **Commit under test:** `bc38d414b019e3fb474c901216319b9289feae69`, not dirty. It contains:
  - `5abbaf3` and `b6d57e8` (node code);
  - `e6d07b2` (the D-7 test).
- **Job 122431871556:** Windows Server 2022, Rust 1.58.1 / 1.99.1, Node 22.22.0; 19:52:35 to 20:17:39 UTC on
  2026-10-27.
- **Result: PASS on every step.**
  - lp1 store journal 12/0, lp1-node 150/0, **lp4-execution 211/0, pocold 239/0**;
  - extension npm ci, build, and unit tests 234/0.
- **The new and changed tests all pass on Windows** (`logs/`):
  - `diff_lists_what_differs_and_reads_every_node_only_the_second_version_holds`;
  - `diff_of_raw_clustered_keys_is_exactly_the_rows_that_differ`;
  - `e4_checkpoint_open_resumes_and_equals_replay_from_genesis`;
  - `e4_checkpoint_damage_falls_back_to_a_verified_state`, with the middle-version case;
  - `a13_counts_a_handle_for_every_held_version`;
  - `pool_revalidation_at_the_gas_bound_does_not_hold_up_block_import`.
- **Artifact** `audit-r2-windows-bc38d414b019e3fb474c901216319b9289feae69`, id 12766959619, 25,557 bytes, sha256
  `bd9e2c18927b057b42f5de9b4392b0b38fc54a8348f7039017c60cd093933bea`.
  - That is the digest GitHub reports, and the digest of the downloaded file.
  - Its contents are this directory, unchanged.

## The pacer seen from the client (`logs/pocold.log`)

```
PACER {"acceptedBytes":32940,"clientWindowS":0.5043064,"closedMs":1088,"firstByteMs":583,"honestFirstByteMs":562,"os":"windows","pacerLateMaxMs":3,"pacerWaits":10,"sendTimeS":0.503958,"violationS":0.5}
```

| check | bound | Windows |
|---|---|---|
| from the client's first byte to the close, not before GRACE | ≥ 0.45 s | 0.504 s |
| the same, within ENFORCE_SLACK of s_v (0.3 s more for polls) | ≤ 1.0 s | 0.504 s |
| the slow reader's first byte against an honest reader's of the same batch | ≤ 2 × 562 ms + 1 s | 583 ms |
| the pacer's own lateness | ≤ 200 ms | 3 ms |

A Windows pass is not acceptance. The run does not include:
- the Python and Chrome e2e suites;
- the thread-count assertions;
- the ignored tests, among them the scale measurement.
