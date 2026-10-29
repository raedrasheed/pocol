# P4/P5 interface prerequisite — owner decision pending

This packet contains a consolidated NEW LP4 SystemCall contract proposal, not an
implemented/accepted SystemCall or GenesisBuilder. Read SYS-EXECUTION-DECISION.md.
The existing CODE/PAY/CREATE/Registry approvals are recorded and are not reopened.
M1, LP3, prior evidence, production runtime, dependencies and all numeric limits
are unchanged. Every existing LP4 gate remains NOT PASSED and boot remains false.

25 diagnostics execute source-built bytecode using frozen P2 R20 and independent
unmodified py-evm APIs:21 frame comparisons,2 unsigned controls,2 independent
validation rejections. Full roots differ for fresh/shared epochs, and ordinary
controls increment SYSTEM nonce. All rows are explicitly unapproved. The selected
metadata/epoch/gas rule has NOT been implemented or treated as original M1.

The exact compiler output, primary results and independent results reproduced
byte-for-byte in a second owned scratch copy. Reproduce using:

`python reproduce.py SOURCE_REPO OUTPUT CARGO NODE_MODULES PYTHON`

SOURCE_REPO must be a clean runtime checkout of public R20
2c36d0fec08e69ee6005d0362216f4cd60b38c2c (full tree31c9bb503449d0aebf0cb02883f7bb7c52f699c7).
Provision the previously pinned Rust1.99.1, solc0.8.28 and hash-locked Python tools.
The script verifies the source tree/clean runtime, copies only into OUTPUT, applies
the diagnostic overlay, compiles locally and checks all three exact outputs.
Set CARGO_TARGET_DIR to an owned reusable compiler cache if needed. No network,
deployment, existing node activation or selected consensus rule is involved.

Diagnostic sources live under this evidence directory deliberately: no production
runtime path is changed to install an unapproved rule. The overlay's actual
Registry helper comes from the exact R20 source; the probe is not a system program.
Initial reporting/API failures are preserved in the complete local archive and
explained in REVIEW.md. A successful diagnostic is not a full integration gate.

The proposed exact pending-word encoding, SYS ORIGIN metadata channel and epoch/
raw-gas allocation choices require an owner answer before their implementation.
All original GenesisBuilder/system/T2c/SV6/finalization/import/crash/reorg and
generalR18/R21/caps64 requirements remain mandatory after approval.
