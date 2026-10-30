# M1 Specification Draft 0.5: Review Package (author turn 003)

Status:
- **For Codex review. Not approved. Phase S only.**
- No production code; no deployments, transactions, installs or subagents. No memory notes were written in this turn.
- **Nothing in 0.5 was executed by the author.** The session had file tools only.

Preserved: every earlier revision, `reference/` and the coordinator's evidence are unmodified. Only new files under `m1-draft-0.5/` were written. Unchanged material is inherited by reference.

## Read in this order

1. `M1-SPEC-0.5-AMENDMENTS.md`: R5-01 … R5-10.
2. `CR-M1-01-REV2.md`.
3. `M1-STATUS-0.5.md`: dispositions, inventory, blockers and exact pending owner decisions.

## Files

| Path | Revision |
|---|---|
| `README.md`, `M1-SPEC-0.5-AMENDMENTS.md`, `M1-STATUS-0.5.md`, `CR-M1-01-REV2.md` | all |
| `vectors/request-bound-c14.json` | R5-01 |
| `vectors/url-cases.json`, `tools/url_oracle.cjs`, `tools/bridge_ref_05.py` | R5-02, R5-03 |
| `vectors/pending-handles.json` | R5-03 |
| `vectors/proof-response-cases.json`, `tools/proof_response_ref.py` | R5-04 |
| `vectors/sitestorage-nav-cases.json`, `tools/sitestorage_ref.py` | R5-05 |
| `vectors/br17e-table.json` | R5-06 |
| `annex/write-path.json` | R5-07 |
| `annex/br-messages.json`, `tools/eth_keys_ref.py`, `tools/txa_check.cjs` | R5-08 |
| `vectors/bridge-guard-cases.json` | R5-09 |
| `annex/import-and-fetch-rules.json` | R5-10 |
| `tools/run_checks_05.py` | the runner |

## Run sequence for Codex (not run by the author)

From `D:\PoCol-Development`:

```
node m1-draft-0.5\tools\url_oracle.cjs
coordination\runtime\python311\python.exe m1-draft-0.5\tools\run_checks_05.py
node m1-draft-0.5\tools\txa_check.cjs
coordination\runtime\python311\python.exe m1-draft-0.5\tools\run_checks_05.py
```

- Everything is written only under `m1-draft-0.5/results/`: `url-oracle-0.5.json`, `run-results-0.5.json`, `keys-txa-0.5.json`, `txa-node-check.json`, `br17e-expanded.json` and `br-messages-expanded.json`.
- The first Python pass records the Node cross-check of txA as pending. The second pass requires it to pass.
- `txa_check.cjs` loads @noble/hashes from `coordination/runtime/noble/node_modules`, the path used by `coordination/hash-triad/noble-check.cjs`. If that copy is absent, the script reports `notRun` and does not pass.
- Expect about two minutes: the 2847/2848-reference manifests and the BIP-32 derivation use pure-Python arithmetic.

## Most likely failure points

These are unexecuted, author-only code:
- the author's WHATWG expectations (`source: author`): an oracle disagreement is a finding to review, and the oracle is not overridden;
- the BIP-32 and secp256k1 implementation, which is guarded by the anvil known-answer tests and the Node cross-check;
- the BR17e envelope byte arithmetic;
- the 24-byte head of the 2848 manifest.
