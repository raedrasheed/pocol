# Correction to this directory's README (2026-10-28)

The README says of the failed e2e_integrated run (whole-state, run 1): "The fault is in the test … No node code
is involved." That conclusion rests on node logs that were read and **not kept** (the runner's cleanup removed
the work directory). **It is not established by kept evidence, and the failure stays open.** What is
established since:
- fork choice by total work in both directions of a heal is correct, with mining controlled. The heavier side
  wins, whether it is c or a and b, in the in-process tests `d133_heal_*` and the process-level
  `e2e_heal.mjs` (../../r4/README.md § Reorganization);
- e2e_integrated now measures each head with no node mining and heals with no miner, so the outcome no longer
  depends on who mines while the nodes reconnect;
- the e2e drivers now keep the node logs and inputs of a failed run before anything cleans it.

A rerun passing does not explain the failure.
