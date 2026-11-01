# Reproduction

Use the pinned Python requirements and source archive/extraction instructions from
.github/workflows/lp4-execution-controls.yml. No fixture secret keys are printed or
emitted in receipts. GST files contain public fixture keys; never upload the corpus.
Use an isolated Python environment and the R9 Rust1.99.1/CARGO_HOME instructions.

```
python prototypes/lp4-execution/tests/gst_independent.py CORPUS OUTPUT --timeout 10
python prototypes/lp4-execution/tests/gst_independent.py CORPUS ROOT_OUTPUT --profile lp4-create01-modexp --selected-receipts ROOT_DIFFERENCES.jsonl
python prototypes/lp4-execution/tests/gst_independent.py CORPUS RETRY_OUTPUT --profile lp4-create01-modexp --selected-receipts TIMEOUTS.jsonl --timeout 60
python prototypes/lp4-execution/tests/gst_comparison_checks.py
python prototypes/lp4-execution/tests/gst_exception_audit.py CORPUS PRIMARY_CASES.jsonl AUDIT.json
python prototypes/lp4-execution/tests/gst_exception_mutations.py MUTATION_OUTPUT CORPUS PRIMARY_CASES.jsonl
```

Filter root differences/timeouts by the corresponding comparison field in preserved
unmodified receipts. The primary receipts are byte-identical to approved R4's gzipped
source-final cases, as verified in R11. Each selection is explicit in its summary;
unknown/missing selected cases fail. Hash all actual source versions, input archives,
requirements and outputs before interpreting results. The first unmodified run uses
the preserved independent-unmodified-runner.py source, not the later adapted source.
Linux SIGALRM/process-pool budget controls are test infrastructure, not consensus gas.
