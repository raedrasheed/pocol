# Codex independent review: LP1 revision 2 (transport 0.35)

Verdict: revise. Actual Claude receipt completed successfully; only its four changed files were imported, followed by mechanical rustfmt. Accepted M1 0.32 remains unchanged.

LP1-I01 compile and LP1-I02 C19 binary64 IDs are repaired: compilation succeeds; dedicated unit/RPC cases and independent process HTTP oracle pass. Formatting was applied; final check remains required.

New blocker LP1-I04: cargo test aborts with Windows stack overflow 0xc00000fd in rlp::tests::depth_bound. Default depth 16 rejects 2000 nested lists, but public decode_with_depth(...,2001) permits unsafe recursive depth, and its positive assertion crashes. Preserve a safe hard ceiling at 16 for this prototype and reject excessive requested limits explicitly. Do not increase thread stack, skip the test, or loosen accepted M1 depth rules.

New blocker LP1-I05: genesis_corpus_never_panics counts two unchanged generated inputs as hash collisions. Independent faithful xorshift replay finds byte-identical trials 2037 and 3053 of 5000, matching both failures. Distinguish no-op cases and assert their identity; continue checking genuinely changed inputs. Evidence: genesis-noop-independent-v2.json. This is a harness classification fault, not evidence for changing genesis hashing.

Passing evidence: verify-fixtures checks 22 windows, 1027 ASERT vectors, 3677 hashes plus derived preimages/signatures; independent ASERT output comparison confirms all 1027 decimal targets. Remaining integration tests: 12 fixture tests, 2 HTTP loopback tests; allocation probe 1069 calls, zero allocations, counter probe active. Independent actual-process HTTP: 35/35; author process HTTP harness: 45/45. Neither passing subsets nor fixture agreement satisfies full A1/A2/A8: full cargo suite aborted and adversarial suite failed. Preserve all logs and snapshots; request focused revision 3 then rerun full suite with real subprocess exit capture.

No production/deployment/full M3 claim. No specification change or personal owner approval.
