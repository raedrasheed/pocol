# Historical M1 validation after English translation

The English translation replaces working-tree prose. The original text and evidence
remain in Git at `3b861aaafceecdea722a2a262cfe060d04b04f04`.

Historical fixtures contain exact source quotations and line ranges. These establish
what an earlier draft was based on; they are not new acceptance criteria for the
English wording. `historical_source.py` reads files under `reference/` from that
explicit commit when historical acceptance runners call `source_text` or
`source_bytes`. Other files, including the translated models and active vectors,
are read from the current working tree. Missing history is an error: there is no
silent fallback to a different source revision.

Source-literal arrays retain the original quoted bytes through JSON Unicode escapes
and identify `sourceRevision`. Actual Unicode test inputs also retain their decoded
values through escapes, including invalid non-ASCII paths, Arabic-Indic digits,
UTF-8 length cases and lone surrogates. Translating those payload values would change
the tests. User-facing messages, report prose and their matching expected strings
are translated. DNR scenario identifiers are rendered as `DNR-d1` through `DNR-d5`;
they remain distinct from the disk-test `d1` through `d5` identifiers.

A historical evidence digest continues to describe the original artifact, not its
translated presentation. Translation does not turn an old pass into a pass for the
current checkout. Use the repository translation manifest to locate original and
translated versions; do not overwrite historical measured digests to imply that a
new test ran.

Some older runners need sanitized or private review/oracle inputs that are not in
this repository. A missing input remains a missing-input failure. Targeted model
checks and successful source-literal checks do not establish that every historical
suite, browser test, node test or implementation-acceptance gate passes today.

For copied historical runner files, use `--root` or `POCOL_M1_ROOT` to point to this
repository's `development/m1` directory. Keep outputs in a new temporary directory
where the runner supports `--out`; older scripts may have a fixed output directory,
so inspect their entry point before executing them.

## Git history and full archival reruns

A shallow checkout may not contain the pinned commit. Check it explicitly:

```bash
git cat-file -e 3b861aaafceecdea722a2a262cfe060d04b04f04^{commit}
# If absent, fetch that reachable revision using your normal repository access:
git fetch origin 3b861aaafceecdea722a2a262cfe060d04b04f04
```

The adapter never fetches automatically. Current GitHub workflows do not invoke
these historical M1 acceptance scripts. Any future workflow doing so must obtain
the pinned history as well as its required oracle/review inputs.

Some full historical suites compare exact earlier package hashes, copied evidence
and filenames. Translated code, presentation files or renamed reports cannot be
byte-identical to those old packages. Do not regenerate the frozen manifest to
make that claim pass. To reproduce a complete historical acceptance run, use an
isolated checkout of the pinned revision, together with the inputs required by
that run. The English-tree model checks validate current behavior; the historical
source checks validate original provenance. Neither replaces the other.
