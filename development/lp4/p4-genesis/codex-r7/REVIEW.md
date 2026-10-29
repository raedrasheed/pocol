# Separate sealed-genesis/factory integration review

Reviewed the full path from supplied compiled program order through sysCodeHash,
sealed genesis MPT/root, actual signed transaction recovery, USER fees/receipt/
CREATE2 code/nonce, pre-mint Escrow state, internal credit and exact closeBlock.
The factory hash is checked against its independently compiled artifact at the
prescribed system address, rather than accepting a self-consistent witness image.

The witness-factory mutation deliberately recomputes the synthetic code/profile/
genesis identity; the failing assertion must therefore detect the wrong actual
program role, not just an unrelated root mismatch. A second control omits the
actual signed USER transaction and must fail the real created-account/nonce check.
No control is accepted before successful compilation and an assertion exit101.

Reviewed the independent oracle: signature validation is py-evm's actual decoder,
USER origin remains the signer, SYS origin is scoped to the separate fresh SYSTEM
epoch, and CALLER authorization remains SYSTEM. The independent engine executes
the actual factory constructor and compares complete post-state/root and receipt,
not a caller success flag. It independently rebuilds the complete genesis state.

The program replacement increases logical code by152 bytes. Numeric facts are
rebound to both complete new synthetic identities and code hashes/model version;
old measurements cannot authorize this image. All measurements remain explicitly
synthetic, with general R18/R21/caps64 unproven. No numeric production cap changes.

The witness Ledger and partial Escrow do not supply missing phase, finalization,
SV6 or resource acceptance. The final funding witness remains synthetic and leaves
earlier pending deliberately intact. Root-error poisoning is RAM-candidate
evidence only; actual durable crash/reorg commits still require separate tests.
