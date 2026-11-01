# Claude author turn013 — new0.15 completeC26 logging boundary repair

Read coordination/review-001/REVIEW-0.14.md. 0.14 statuscollisionrepaired,butnext check(name,ok,**kw) callwithkeywordname crashes at e4.maxRecord;9 E4checks missing.347pass113recorded3FAIL exit1. Keep0.13/0.14failedproofs. Lastaccepted0.12.

Write ONLY NEWm1-draft-0.15; nooldfiles/GUI/ledger/reviews/checkouts edits; noexecution/testclaims, production/install/deploy/transactions/memory/subagents/shell. Filetools/defaultpermissions.

Repair WHOLE diagnosticboundary while importingolderrunnersreadonly: BOTHrecord andcheck label/core-status/condition arguments POSITIONAL-ONLY (/) orotherwise collision-free; move diagnostics name->diagName,check->diagCheck,status->proposalStatus withoutchangingcore check label/status. Core schema writtenlast,enumvalidated. Scanallcallers andnestedwrappers for reservedfieldcollisions (name/status/check/ok etc), includingmaxRecord(name=...). ReplaceR13.check in itsactualmoduleglobals soe4_checks usesit, notjustan unusedexport. Use safeR13.record anddoNOT letdiagnostics overwriteresultenum. No models/expectedvectors/codec/engine/schedules/scheduler/budgets/gates/ownerpolicies changed.

RerunENTIRE0.13E4/E5andallinherited0.12/0.11/0.10. Preserve0.14guards detectingabort/missing59E4checknames plusE5coverage. All9previouslyskipped maxRecord/disk/ticketchecks mustexecute. Reproduce0.13statusTypeError and0.14nameTypeError incontrolledpristinefunctions, assertionsnotweakened. Addtests injectingallreserveddiagnosticnames atonceandverifycore name/status/condition remaincorrect andallmetadataretained safely. NewresultsONLYpkg0.15/results/run-results-0.15.json. No oldmain() calls oroldresultswrite.

README/amendment/status honestinheritance/fullcoverageprovenance/inputSHA. Afterrootreview E4/E5mayPartial (36Partial5NotStarted0Complete). CR-E4-01/02 remainproposals, oldowners/fullgate/CONF_DEPTH unchanged, noChrome/TS/storageclaim. No E6/E7expansionyet. Rootindependentreview/test/sameGUI/PR2 thencontinueotherfeasiblework. Returnconcisechangedfiles andnotexecutedstatement.
