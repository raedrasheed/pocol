# Independent ledger reconciliation ? C09 and C12

These two issue entries still said open even though preserved later revisions contain their remedies. This audit closes only the original specification/tooling defects, not implementation/owner gates.

C09:0.3 R3-05 explicitly specifies manifestversionu8 vswebsiteIDsuint32; saved review0.3 results335entries333pass2recorded0fail include allfour version-width cases. R3-06 T1-10 specifies each action's exact writablebit ranges andimmutablepublishedBlock. T1-12 explicitlystarts current1, changes to2 then removesA andrequires exactNotPublisher(A) whenAtargets1; AlreadyCurrent no longer masksmissingauth. T1-16 EVM/ABI experiment remains unexecuted (implementation gate), and ownership/publisher policy decisions remain unchanged. Closeoriginal prose/test ambiguity as specification-only.

C12:0.4 R4-03 withdraws426asviewerbound anddoesnot restrict acceptednoncanonicalsplit. Lowerbound54+23n derives max2847references/2850requests. Its2848fixture failed C14;0.5fixedlength,0.6fixedprefix andhash withpassedmachinechecks. Independent freshRLP-length encoder (nooldrunner/modelimport) now computes nestedlengths2847=[65484,65527,65530,65535] and2848=[65507,65551,65555,65561]. Thelatteroversize isnot65558 because nested lengthheadersgrow. ledger-bound-independent-probe.py/.json confirmsbothliteralboundaries. Closeoriginal request-bound defect as specification/tooling-only; historicalfailed0.4/0.5proofspreserved andstateproofCR-M1-01unapproved.

No draftpackage modifications, sourceauthority changes, ownerapprovals, implementation or merge/deploy.
