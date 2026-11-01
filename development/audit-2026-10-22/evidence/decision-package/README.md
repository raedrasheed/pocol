# Decision package evidence (2026-10-24)

- `a13-network-profile.log`: the network-profile model before C-1 S2, run by `tests/network_budget.rs`.
  Total 150.83 GiB; the gate needs 172.4 GiB.
- **C-1 S2 (code counted once per lineage):**
  - `a13-network-profile-s2.log`: after S2. Total 105.83 GiB, the code group 9.69 GiB (was 54.69), and
    the gate needs 120.95 GiB. The test still asserts that the profile does **not** fit 6 GiB.
  - `a13-devnet-breakdown-s2.log`: the devnet breakdown after S2. Total 5,487,223,178 B (was
    5,581,426,268 B), pinned by `consensus::a13_bound_breakdown`.
  - What the model now relies on, `journal_measure::retained_states_of_one_lineage_share_their_code_buffers`:
    - every canonical state holding three 32 KiB runtimes points at the head's buffers;
    - so does the `contracts` map;
    - its control: the same deploy executed on another chain object has a separate buffer.
  - Counted unshared, one full set each: side states, a rebuild or restore in progress, and the pool's
    pending state (resources.rs).
- **consensus.md:101 "the peer is punished"** (review R1-5):
  - test `consensus::an_invalid_template_from_the_drawn_proposer_punishes_its_peer`, with a valid
    template and one for another parent as controls;
  - mutation control `../reviews-2026-10-24/r1-fixes/r1-5-mutant.log`;
  - the test drives the node in process, so it counts the penalty (`templatePenalties`) but does not
    observe the closed socket.
- **Suites on this tree** (release): `suite-lp4.log` 173 passed / 0 failed, and `suite-pocold.log`
  184 passed / 0 failed / 15 ignored.
- `finding-status-commits.json`: the input given to `tools/make_finding_status.py` for this update.
