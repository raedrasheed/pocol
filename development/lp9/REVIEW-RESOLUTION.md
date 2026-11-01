# LP9 independent review — resolution

Reviewer: an independent agent with read-only access to `prototypes/pocol-extension`. It checked the
sources against `browser.md` and used Node probes for parser differentials, records simulation and
storage `apply`. No critical or high finding. Areas found sound: no all-layers sandbox network/realm
escape; no message reaches the node or a local effect without BridgeAuth; wallet keys are per netKey;
chainId is in the signed payload; `from` is checked; there is no path that signs without approval;
records ordering/durability/isolation hold; frame ids in the viewer relay cannot be forged; the RP
window and site verification match the spec.

| # | Severity | Finding | Resolution |
|---|---|---|---|
| 1 | medium | The transaction confirmation showed value/gas/nonce/data/hash/raw but not maxFeePerGas / maxPriorityFeePerGas, so a site could hide a large tip behind "value 0" | `sw.js` sends both fee fields and the worst-case cost `gas·maxFee + value`; `viewer.js` displays them. The signed bytes are unchanged and still equal the displayed `raw` |
| 2 | low | `walletKey()` refreshed the 15-minute lock on every call, so a site calling `eth_accounts` kept the seed decrypted | The lock is refreshed only by user actions (unlock, an approved grant, an approved signature) |
| 3 | low | `site_storageSet('__proto__', v)` returned success but was dropped (an ordinary object's prototype setter) | `apply()` and `records.js` use null-prototype dictionaries; unit test stores, counts and recovers the key with no prototype pollution |
| 4 | low | Disk/name admission awaited fresh measurements and ignored live reservations (D105 requires a synchronous check-and-add with `unres/unresN`) | `RecordStore.admitDisk` is synchronous over the cached measurement plus reservations; reservations are released after the settle is re-measured; unit test: two concurrent writes, the one over the cap is refused |

Note on signing order: the worker signs before showing the confirmation, because BR14 requires the
displayed txHash to equal keccak(raw). RFC 6979 makes the signature deterministic, so signing after the
click would give identical bytes. The signature never leaves the worker before approval, and submission
uses exactly the displayed `raw`.
