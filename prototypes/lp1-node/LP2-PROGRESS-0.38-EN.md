# LP2 progress — Author revision 2 (handoff 0.38)

**Status:** two focused corrections, not compiled or run by the author. No acceptance claim.

| Item | Correction |
|---|---|
| LP2-I01 | The batch test now sends valid H8 followed by invalid H9 genuinely at the next height. Winner signature v=2 is rejected by item 8 at index 1; an unlinked parent is rejected by item 2. Height stays 7 and the journal hash remains unchanged. The conflicting-sibling case remains separate and expects `notLinear`. |
| LP2-I02 | Partial guard/checksum bytes are now compared with the recomputed value as soon as their inputs are available. Any mismatching byte is classified `corrupt` (exit code 2), not a torn tail. Only consistent prefixes remain tails requiring recovery. Added tests for every partial length and available byte and both reviewer probes, preserving the source. Removed unused `h256`. |

**Limits (unchanged):** exclusive writer on Windows only, no directory-entry synchronization. The journal is linear and experimental; status always remains candidate, awaiting H-full, `executedHeight = 0`, `consensus = false`. A partial blockHash field cannot be verified before the complete header is available.

**Awaiting the root:** full offline build/test, rustfmt, oracles, HTTP, 105 independent CLI checks and the two known probes (both must exit 2). LP3 starts only after actual acceptance. M1 0.32 is unchanged.
