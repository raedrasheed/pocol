# Windows verification of 1074b47 (round 4: checkpoint open, the review's fixes): one test failed

- **Workflow:** audit-r2-windows-verify.yml, run 40680583220, https://github.com/palcertcom/pocol/actions/runs/40680583220
- **Commit under test:** `1074b476999236870b85318991008e56f08cec0a`, not dirty.
- **Artifact** `audit-r2-windows-1074b476999236870b85318991008e56f08cec0a`, id 12815028794, 29,502 bytes, sha256
  `01502b1e9ef6660d8c5241e115cc6f6879a21d161e7ca6c7656bcffb71d72369` (GitHub's digest and the downloaded file's). Its contents are this directory, unchanged.
- **Result: FAIL, one test.** lp1 store journal 12/0, lp1-node 150/0, lp4-execution 211/0, **pocold 252/1**,
  pocold-io-fault 110/0, extension npm ci, build and unit tests 234/0.
- **The failure:** `admission::a_store_mode_boot_is_timed_through_a_scratch_state_store_in_its_data_directory`,
  tests/admission.rs:115. The **whole-state** admission (`pocold::load_spec`) was refused: `-32018 ParamGate
  rejected [R14, R19]`, the boot timing gate. The store-mode admission just before it (line 113) passed.
- **What is known:**
  - the refused call times the calibration blocks through lp4-execution (`admission::measure`);
  - nothing in lp4-execution changed in round 4, and this test was unchanged since 24cc167;
  - it passed on every Windows run from 24cc167 to 22543cf, the run just before this one;
  - the gate fails closed when a phase takes longer than t_exec (1 s here) on the host, which is its specified
    behaviour.
- **What is not known:** the times measured. The gate's error does not carry them, so it cannot be told from
  this run whether the runner stalled or a phase really took longer than 1 s.
- **What was done:** the test now prints, with a refusal, the phase times of a calibration run made right after
  it, so that a recurrence shows its cause. The next push runs Windows again. The cause stays open until then.
- Pacer line (`logs/pocold.log`):

```
PACER {"acceptedBytes":32940,"clientWindowS":0.513077,"closedMs":1302,"firstByteMs":788,"honestFirstByteMs":790,"os":"windows","pacerLateMaxMs":9,"pacerWaits":10,"sendTimeS":0.509052,"violationS":0.5}
```
