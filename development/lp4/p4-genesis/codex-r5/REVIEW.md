# Separate retention review

Reviewed the producer and upload list after R3 completion. The producer's actual
cases.jsonl was absent from artifacts; summary success cannot establish archived
individual evidence. Historical artifacts and all failed/local evidence are
preserved, and this gap is explicitly reported rather than silently repaired by
combining rows from different runs.

The new archiver reads only published result metadata and frozen public corpus
posts. A strict field allowlist excludes private signing/prestate input material.
Each source path is relative and rejects traversal. Every source digest, case
identity, indexes, expected exception, root and log hash is checked. The allowed
exception classification remains EXCEPTION_PRESENCE_ONLY; no exception reason
or signed transaction equivalence is implied. The complete set and counts must
match the frozen20661/2807 corpus, summary and empty incomplete file.

The output refuses to overwrite an existing compressed archive, uses mtime0 and
no gzip filename, and compares decompressed SHA256 with the original file before
writing a receipt. New CI uploads both archive and receipt, including on failure
for existing diagnostics; validation failure still fails the job. No check was
removed, bypassed or counted as passing before execution.

Positive full-run validation remains pending the actual new producer; the
negative actual incomplete historical receipt is a separate check, not positive
execution evidence. Complete LP4 acceptance remains NOT PASSED and boot=false.
