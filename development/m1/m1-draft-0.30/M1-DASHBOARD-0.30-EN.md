# M1 summary — Draft 0.30

**The author ran nothing. M1 is not complete.** Counts below are expected and computed by the run in `dashboard-0.30-en.json`.

## Repairs

- **C34.** N05 is intentional: declared length 24576, chunk sum 24575. Contract and viewer correctly rejected it, but an incorrect generic check required matching sum/length for every negative case, producing four failures.
  - Each case now has its own sum condition.
  - The case itself is unchanged.
  - Eight deliberate data mutations are tested; the check must detect each one.
- **C33.** Every response reaching header checking now passes the receive guard first. The guard checks:
  - Size: 4096 bytes for block number, 98304 for headers;
  - Strict UTF-8;
  - Depth limit 16;
  - Duplicate keys;
  - Request ID.

  Any failure immediately stops display, without retries or waits. All previously valid responses remain valid.

## Seventeen conventions

The coordinator selected them under the user's standing delegation; they are not personal owner approval. Main effects:
- Initial RP navigation has one budget: ten seconds from the first header-check request through the content verdict.
- The warning page ends the first load; a click starts a new budget.

## Freeze

Test hashes were frozen with their inputs:
- 668 earlier hashes verified by three libraries;
- 4024 V1 hashes verified by the root.

Synthetic IDs remain labeled and are not frozen.

## Expected status

- **38 candidate-complete rows.**
- **3 rows awaiting root review:** C3, C4 and V1.
- No row is complete before root review. No production code, deployment or transactions.
