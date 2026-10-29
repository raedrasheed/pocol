# Claude author turn010 — C25 physical namespace accounting repair, new0.12

0.11 corrected C24:211pass48recorded0fail, original24 independent probes24/24pass. But Root's new namespace probe FAILS. Read coordination/review-001/REVIEW-0.11.md and namespace-independent-probe-0.11.py/.json. The actualGUI verdict is revise. Preserve0.10/0.11 andallfailedproofs. Lastacceptedbatch0.9.

Write ONLY newm1-draft-0.12. No oldfiles/ledger/GUI/review/checkouts modifications, tests/execution claims, production, installs, sockets/Chrome/storage/EVM, transactions, deployments, memory, subagents orshell. Filetools only,defaultpermissions. This mechanical accounting fix needs no owner judgment.

C25 exact baseline reference/browser.md:509-510 lists ALL pocol:epoch:* items withgetKeys, EN their count, then checksEN+1<=8 BEFORE issuing. In0.11 World.epochs ignoresmalformednames, and _attempt useslen(canonicalepochs), so F5 +7 distinctLF-tailed prefixnames (8physicalnames) ->newF6 ->9physicalnames, stateconfirmed andviolations[]. Neverhidebadnamesfromthephysicalbudget.

Separate:
1. RAW namespace element count (allstringnamesstartswithpocol:epoch:, whethercanonical ornot), used for EN gate, stats andname-budgetinvariants.
2. Canonicalepoch parsing andmax, generationwindow andrecordlookup (strictC24fullmatch/bool/ranges remain). Invalidnames mustNOT contributeanepochnumber orbenormalized.
3. Safe sweep: preserveunknown/malformeditems unlessanexplicitapprovedcleanup policyexists; doNOT addautomatic deletion. Sweep canonical lowernames only underexistingT_LATE rule. Largestvalidname remains, physicalEN isre-read aftersweep; ifstill8, noissue, persistentepochNamesfailure/action25. RetainexistingEN=8validname h2/h3/h4 behavior. Initialoverfullstates areobservedreported,notmade worse. NegativeepochNonceNames/epochNoGate mayviolate bydesign andmuststilldetected.

Implement a reference-only patch/adapter importing0.11/0.10 readonly andexporta usable sr_ref module with World runtimeactuallyusingrepairedgate/invariants. DoNOT changeclock/scheduler/retrycaps/checkpoints/seq/disk/siteadapters, invalidbootMs modelchoice, ownerpolicy orcanonicalnameranges. No E4-E7 expansion yet.

Add canonical+loneLF/CRLF/nonce/unknownprefix mixednamespace fixtures: exactly7prefixitems mayissueone (to8), exactly8cannotissue,9initial cannotworsen andmustbereported; prefixlookalikesoutsidepocol:epoch: don'tcount; physicalcount remainsafterfailedcleanup; malformednamesneverinfluencemax. ReproduceC25againstpristine0.11, correctmodelrejects. Gate mustblock BEFORE backendset/data/snapshot, notafterdetectingovershoot.

Rerunall0.10scopeandallC24/new0.11regressions. The0.11 specialwholeWorldfixture that expected7malformedLFnames+F5 toconfirm is ITSELF theincorrectexpectation C25changes: explicitlyamendthatone golden expectation to rawEN8 gateblocked/epochNamesfailure withoutdroppingtestcoverage. Documentexactold/newoutcomes; doNOT simplywhitewashallinheritedpasses. Allother tests/control cases keepbaselineexpectations. No writingpriorrunnerresults: callsteps/rebindinmemory fromnewrunner, resultsONLYm1-draft-0.12/results/run-results-0.12.json. Inputhashes/provenance, README/amendment/honeststatus; noChrome/TS/fullD104claim.

AfterRootreview successful,E1-E3Partial (34Partial7NotStarted0Complete), OwnerU01/U02/U10/U14/CR-M1-01unapproved,CONF_DEPTHrequired/fullgate. Rootindependentreview/testandrealGUIreview willfollow; thencontinueE4-E7/V2-V4,notafixedroundlimit.
