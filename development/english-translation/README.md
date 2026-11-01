# English translation of the PoCol repository

The owner requested English throughout the project, chose replacement of the original text files, and explicitly excluded the signed thesis PDF. This change translates documentation, approvals, reports, comments, built-in UI/messages, test descriptions and historical text evidence. Historical dates, findings, accepted/proposed status, numerical limits and test results retain their original meaning. Translation does not amend consensus or approve outstanding decisions.

## Scope and provenance

The source revision is `3b861aaafceecdea722a2a262cfe060d04b04f04`:
https://github.com/palcertcom/pocol/tree/3b861aaafceecdea722a2a262cfe060d04b04f04

`MANIFEST.json` maps original paths and SHA-256 digests to translated paths and digests. Reports previously suffixed `-AR.md` are now `-EN.md`, and current Markdown/code references follow them. The combined `FINAL_DESIGN.md` is rebuilt from the same twelve translated sections and 143 translated decisions, plus its translated prefix and final risk sections.

Historical SHA manifests, source digests, test counts and approval dates remain attestations of the original bytes and runs. They are **not** newly issued attestations of the English presentation. Retrieve original bytes using the source revision and original path in the translation manifest. The new manifest separately verifies translated files. Do not rewrite old hashes to make an old run appear to test changed code.

Two historical Playwright trace archives have translated textual UI records and HTML resources with updated internal content-addressed references. Their added `ENGLISH-TRANSLATION.json` records original archive/resource hashes. Timings, numeric outcomes and binary screenshots remain original. They are translated recordings, not fresh test runs.

## Deliberate preservation

- The signed thesis PDF is unchanged, as explicitly requested.
- Historical binary screenshots retain the interface as it was recorded. They were not fabricated or edited into new evidence.
- Unicode test payloads keep their exact decoded values, including Arabic letters/digits needed to test UTF-8 lengths, invalid names and non-ASCII input rejection. Source spellings use escapes where appropriate.
- Exact historical source quotations used by assertions also retain their decoded values and explicit source revision. `development/m1/historical_source.py` reads those original reference bytes from Git; executable models and other inputs still come from the current checkout. Missing history fails explicitly. See [historical validation](../m1/HISTORICAL-VALIDATION.md).
- Three accepted, byte-pinned GenesisSpec vectors retain their **exact original bytes**, including historical source quotations: `m1-draft-0.21/vectors/v3-gsv1.json`, `m1-draft-0.22/vectors/c30-depth.json` and `m1-draft-0.22/vectors/v3-e05-supplement.json` under `development/m1/`. The translation had only escaped quotations and added provenance metadata, but this changed their immutable SHA-256 attestations. They are restored from the source revision above; the original test pins remain unchanged. The manifest and verifier explicitly allow only these three paths and still enforce their original hashes. These are canonical evidence fixtures, not interface text or default content.
- Coordinator API fields such as `summaryAr` retain their names for saved-state compatibility; built-in content is English. User-supplied historical state is not silently rewritten.

## Verification

Evidence in `evidence/` records:

- Extension: 234 unit checks passed; build passed.
- Rust RP window: two targeted library tests passed on Rust 1.99.1; formatting passed. A broader filtered run also passed, but is not counted here as additional unique coverage.
- M1: 221 StoreQueue unit/schedule checks, 22 RP cases, 10 DNR/X3 checks and 176 exact historical-source assertions passed. Counts describe different scopes and must not be summed as unique tests.
- Independent review confirmed that 34 modified Python tool files retain control-flow/assertion structure, numeric JSON tokens are unchanged, and actual Unicode payloads retain their decoded values. Original provenance still rejects a deliberately wrong literal.
- Reference translations preserve original line counts and numerical token sequences. Separate semantic review corrected wording and subsection cross-reference inconsistencies.
- Both translated trace ZIPs pass CRC, resource-address/hash and reference checks, with original numeric/timing data and screenshots preserved.

The coordinator is an eight-file overlay bundle; its unchanged baseline modules and test helpers are absent from this repository, so its full 82-test suite could not be run here. Changed JavaScript files pass syntax checks. No live coordinator was started and no notifications were sent.

Local real-Chrome verification is blocked: the Chrome for Testing 151.0.7922.34 download was not a valid ZIP. Its failure log is preserved; this is not a browser-test pass. No new Windows pass is claimed. Existing PR workflows can test the published revision separately.

Full archival M1 acceptance is not claimed for the translated checkout: historical byte-identical package/evidence checks require the original revision and, for some suites, original oracle inputs absent here. The source adapter preserves historical quotation checks without substituting old executable implementations.

Run `python3 development/english-translation/verify.py` to check the translation manifest, original Git bytes, current file digests, reference numbers and remaining raw Arabic text. The language check excludes the unchanged thesis, binary screenshots and the three hash-verified immutable vectors above; intentional escaped Unicode/source-quotation payloads are reported separately.
