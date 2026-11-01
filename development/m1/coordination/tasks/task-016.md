# Claude turn016 ? new0.18 repair C28 observation snapshot alias

Read REVIEW-0.17.md and real independentresults. Write ONLY NEWm1-draft-0.18. No oldfiles/results/ledger/UI edits, shell/execution/installs/production/subagents/ownerapproval. Accepted0.15;0.17 failed1.

R16.admin_cells appends live e.violations and othernestedactualreferences into cells then runsfutureevents. Past5000 [] is mutated by6000 violation, giving a falsefailure. Install a capture wrapper in actualR16 moduleglobals so EVERYrow/final actual and expected is deep-copied at observationtime. Audit other 0.17capture callsites for samealias. No merely deep-copying after wholefunction returns: alreadytoo late. Preserve row5000 zero violations androw6000 exactviolation, allsource/derived assertions, no filters/deletion/weakening.

Regression adversarial nestedmutable data dict/list: captureearlier cells thenmodifyengineinplace later, provepast snapshots stay byteidentical andlater cells reflectchange. Cover actual ANDexpected isolation, histories per row. Whole0.17scope/0.16suite and0.15readonlyreplay with allcoverage and inputSHAguards; nooldmain/resultswrite. Models/faults/expectedgoldens unchanged. Newrunner run_checks_018.py onlynewresults. C27remainclosed; RF-E6-1actualsourceconflict/proposals unapproved; ownersfullgate/CONF_DEPTHunchanged. RootindependentreviewthenV2-V4.
