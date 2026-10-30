# Public import and reproduction boundary

This tree imports curated local M1 filesystem snapshots through draft 0.8. The original workspace remains unchanged. No prior Git history is claimed.

PUBLICATION-MANIFEST.json records local/public SHA256 and whether a file was curated. Raw agent sessions, original private dialogue history, explicit private-key/seed values, personal home paths, downloaded packages, runtimes and caches are not published. Approved project baseline documents and English restored specifications are included; private historical quotes are removed from the restoration JSON.

Python 3.11.1 was used for local model checks. Node 22.13.1 was used for URL/id/BR16 oracles. Hash checks used Rust sha3 0.9.1, @noble/hashes 1.7.1 and pycryptodome 3.24.0. Source requirements are retained; dependencies must be installed separately and are not committed.

The public key generator requires POCOL_TEST_MNEMONIC as local input; there is no committed seed. Known-answer key checks in the publication copy compare public addresses rather than literal secret scalars. Generated key-bearing outputs are ignored. Missing input must produce a failure/pending result, never a fabricated pass. Do not use live wallet material.

Evidence records describe exact local tested revisions and failures. Privacy-curated source files have not all been re-executed as a new full package. Browser, EVM, actual node/MPT, sockets and production lint integration remain unverified. No permanently green CI placeholder is added.
