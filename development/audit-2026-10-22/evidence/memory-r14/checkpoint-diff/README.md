# Seeded faults in the checkpoint-open diff (`b6d57e8`)

- **The code under test:**
  - lp4 `NodeTrie::diff` (node_trie.rs: `diff_ids`, `diff_nodes`, `kid`, `leaves`);
  - pocold `checkpoint::restore`, which reads a stored version through its diff from its parent's version.
- **The tree:** `bc38d41`, whose prototypes are `b6d57e8`'s plus an ignored measurement.
- **The runner:** a scratch program outside the repository, with its own target directory. Only its log is here.
- **The tests run on each fault:**
  - lp4: `node_trie::tests::diff_lists_what_differs_and_reads_every_node_only_the_second_version_holds` and
    `diff_of_raw_clustered_keys_is_exactly_the_rows_that_differ`;
  - pocold: `--test consensus e4_checkpoint_` (7 tests).
- **BASE:** both stages pass.

## Result: 14 of 16 killed, and 2 equivalent in every output

| fault | outcome |
|---|---|
| DF1–DF13: the diff's subtree skip, the parted-prefix case, the nested cases in both directions, a changed leaf, the last nibble, an only-new account's storage, a changed account's storage, the child index | **all killed** by the lp4 tests. Most fail both; DF10–DF12 fail the version test, which checks both the listed changes and that every new node was read |
| RS3: restore moves the parent's summary without the diff | **killed**, 6 of 7 checkpoint tests fail |
| RS1: restore diffs from the grandparent's version | **survives; equivalent in every output.** See below |
| RS2: restore reads every version whole (the code before `b6d57e8`) | **survives; equivalent in every output.** It changes only the cost. See below |

**The log's two runs.** The first run's DF12 did not compile (the fault's text broke a closure), and the runner
stopped at DF13, whose search text occurs twice in node_trie.rs. Both texts were corrected, and DF12, DF13 and
RS1–RS3 were run again, as the log's second header shows.

## Why RS1 and RS2 are equivalent in output

- **RS1:** restore may read a version through its diff from any version it has already read, not only the
  parent's.
  - A node the diff does not read is a node of that base version, and every node of the base was read when it
    was restored.
  - `Summary::moved` from the base's summary, through the accounts and slots that differ, is the version's
    `scan_view`. The lp4 version test checks this for pairs further apart, in both directions.
  - So the grandparent gives the same summaries, the same refusals and the same reads of every node. Only the
    diff is longer.
- **RS2** is the read before `b6d57e8`: every version whole. Its results are the same by construction; only its
  cost differs.
  - That cost is measured at scale, on the same directory: an open of 801 s with every version read whole
    (`d30dc71`), 262 s with the diff (`bc38d41`). See ../scale-d30dc71/README.md.
  - **No unit-level guard.** A scratch test counted the node lookups of the open in
    `e4_checkpoint_open_resumes_and_equals_replay_from_genesis` (cache hits included, nothing replayed).
    It found 10,017 lookups, where reading every version whole holds 6,639 nodes.
  - On that chain's small versions (about 200 nodes each), `Summary::moved`'s path reads from the root cost
    about as much as reading the version whole. That test was not committed.
  - The guard that does hold is the lp4 positive control: between consecutive 400-account versions, the diff
    reads a median 6.7% of the version's nodes.
