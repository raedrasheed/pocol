# Separate review of the new decision and diagnostic

Confirmed the accepted phase order, parent-state assignment window and settlement
height definitions. No concrete Ledger slot map or prune-height equality appears
in the accepted reference/recorded decisions. The proposed definitions are labeled
new, not restored M1 rules. SYS/PAY/CREATE/CODE/Registry/issuance approvals remain
closed and no original criterion/cap is removed or counted as passing.

Checked the counterexample at the FIRST differentiating height: h5,D2,W_w3,
K3, common parent counts1..3. The current assignment has already read1..3 from
the parent; settle reads3 and the next assignment requires2..4. Both policies
retain2/3; adding the same future4 cannot erase their difference in old count1.
The same compiled code and initial root avoid a trivial changed-code-hash
counterexample. Actual EVM execution yields different storage/roots/gas, no LOG
and no SYSTEM account creation. Compiler/result repeat byte-for-byte.

This is a synthetic one-owner diagnostic, not the proposed Ledger implementation,
full block/header admission or an acceptance gate. Its boolean only chooses the
two diagnostic hypotheses and is explicitly forbidden in the proposed protocol
API. No canonical profile or runtime module includes this code.

The proposed Registry entry point keeps CALLER=SYSTEM on the mandatory message,
uses a fixed ordinary nested Registry->Ledger call, and updates Registry storage
inside Registry. It avoids silently granting Ledger arbitrary Registry writes or
adding an extra SYSTEM call. Missing-record handling is an explicit new proposal,
not an assumption about supported unbond delays. All malformed/partial/future
facts fail closed. Existing count/reward/burn/finalization equations stay binding.

The retention count2k+2 and maximumK recorded heights are only layout counts;
they do not prove gas, physical memory or cap sufficiency. All general bounds,
default/end/full/caps64, compiled controls and durable/finalization tests remain
mandatory. Any real cap contradiction must return to the owner, not be waived.
No implementation or acceptance of either new option occurs before approval.
