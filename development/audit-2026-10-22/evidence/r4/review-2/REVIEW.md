# Independent review of e028068, eb85bed, 87d2392, 5cd7d43, 50f9207 (2026-10-28)

A reviewer that had not written the code read the five commits' changes to `prototypes/pocold/src`. It was given
the code and seven questions, not this work's conclusions. Read-only: no build, no run.

| question | finding | severity | outcome |
|---|---|---|---|
| Q1: can `Chain::side` differ from the non-canonical entries? | no defect: every mutation of `blocks`, `canonical` and `head` traced; re-inserts, genesis, a failed restore | — | — |
| Q2: can a non-head entry keep a trie after settle? | no defect: only `insert` stores an entry with a trie, and it is recorded in `tried` | — | — |
| Q3: is shed exact from `prune_cursor`, and without side blocks? | no defect; suggested `debug_assert!(prune_holds())` in shed | — | assertion added |
| Q4: is the incremental `rebuild_canonical` a full walk? | no defect (backward head, shorter path, cleared vector) | — | — |
| Q5: checkpoint decode | no defect: 126 is the true minimum, the guard still bounds the allocation, seal and trailing checks hold | — | — |
| Q6: I/O failures | **F-A**: `StateStore::open` and `History::open` set a store aside on any open error, an I/O error included (and an I/O error inside `check_integrity` read as "integrity check failed"): an EIO at open lost the checkpoints and replayed from genesis; for history.redb it also deleted state.redb | should-fix | fixed: an I/O error fails the open and keeps the file; only a damaged store is set aside. Test: `e4_an_open_that_cannot_read_its_storage_fails_and_keeps_the_store`, case "state.redb read at open" (the `io-fault` backend now fails reads too). It fails without the fix ("the open succeeded (resumed from None)", [fa-without-fix.log](fa-without-fix.log)). history.redb has no fault backend; its fix is the same code, untested |
| Q6 | **F-B**: a journal record at an offset open had verified, read back corrupt during a restore, was a refusal (the checkpoint dropped) | minor | fixed: a local failure (`LOCAL`). No test: there is no fault that changes the journal between open's two passes |
| Q7: memory accounting | **F-C**: the side set priced at 48 B a slot; a hash table of 32-byte keys can take up to about 76 B an element after it grows and never shrinks, and a reorg moves up to r_keep hashes in before retain trims | minor | fixed: its own A13 term, (side entries + r_keep) × 76 B. Devnet total 3,234,220,320 B; network store mode 5,581,944,077 B (5.199 GiB), 6,379,364,660 B required. The protect-set slot and the evicted FIFO use the same 48 B convention and are **not** changed: open |
| Q6 | no path found where a local I/O failure marks a block invalid | — | — |

Suites after the fixes ([suite-review-fixes.log](suite-review-fixes.log)): release all targets, `io-fault` (state_store,
consensus), `crash-test`, and debug assertions (consensus, state_store): 23 test binaries, 0 failed.
