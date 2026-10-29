# Codex independent review - revision 0.27, author turn 025

Actual Claude receipt: completed successfully, exit 0; no duplicate job. Earlier 0.26 draft and review evidence: 40/40 saved files unchanged.

Preserved-copy runner: 320 entries, 248 passed, 72 recorded, zero failures, 97.4 seconds. The inventory remains 34 CompleteCandidate and seven Partial rows; passing computation does not resolve owner policy.

Independent verification:
- 26 independently selected malformed JSON-RPC replies: controlled viewIncomplete/no frame, zero wait, zero exceptions. The three original C32 failures are repaired.
- 18 additional direct SHA-function/retry checks: all passed. The measurements intercept actual hashlib.sha256 calls, independently of the author's counters. Valid busy delays at 250 and 2000 ms retry exactly three times; the fourth busy reply terminates without a fourth wait. Rate cases only verify the explicitly proposed cap branch.
- 3677 exported preimages matched Node native SHA256 or both independent JavaScript Keccak libraries. 31 new supplied fixture signatures verified with read-only Rust libsecp256k1 / sha3. No signing/submission/private-key export by these independent checks.
- 1027 native BigInt ASERT oracle cases matched the new reference; all intermediates stayed within 512 bits.

V1-SHA-COST is now accurately investigated: cached counts are 27 for the share-less full window, 258 for a single 256-share header, and 3355 for the full 13-header/256-share window. The latter has 3355 distinct preimages; disabling TemplateID caching gives 6721 calls. Caching cannot meet the literal source limit of 13. P-V1-3 is withdrawn. The source is unchanged, and a properly explicit owner change request proposes category bounds while retaining all checks. Recommend option A: <=14 TemplateID + <=13 PoW + <=3328 share hashes = <=3355. Owner authorization is pending.

Qualified technical decisions: accept the tested C32 malformed-envelope repair and P-C32-1/-3/-4 (envelope precedent, existing integral-value semantics and rejection of non-JSON constants). This does not adopt P-C32-2. The rate interval [0,2000] is not specified in the source; it is a proposed client policy, not an established approved range. Its honest-server compatibility claim needs proof or withdrawal and explicit owner routing.

Required documentation revision: 6000 ms is the maximum RETRY SLEEP under the proposed rate cap, not a proof of an overall 10-second load deadline. Response latencies and worker time are not represented by ScriptedRpc. network.md:374 states a 10s protection, while U14 remains the owner choice of per-request versus whole-load semantics. Remove the claim that the sleep arithmetic establishes that protection. Preserve U14 unanswered and define conditional cases without adopting either branch.

Verdict: revise only the policy/status claims and routing; accept the independently demonstrated C32 crash repair and SHA accounting at specification/fixture scope. V1 remains owner-blocked. U01/U02/U10/U14/CR-M1-01/U08/RF-E6-1 remain unchanged. No production implementation, merge or deployment. No content restriction blocked these checks.
