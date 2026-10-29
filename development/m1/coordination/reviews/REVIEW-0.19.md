# Codex independent review — revision0.19 / turn017 V2 LogClient

Preserved-copy Python3.11.1:516entries,493pass,23recorded,0FAIL,1.1s,exit0. CompleteLC1-LC18 andvariants, literal totals5/5/7/4/11/12(variant8)/10, actualrequestlogs/canonicalfullresults/abortedconsumercontents, nineclientcontrols and15sinkcontrols executed. Root29 independent P1/RES/retry/Complete boundaryprobes allpass.

C29 independentcounterexample: SinkChecker accepts unknownTerminal afterbegin as validterminal andreturnsno violations. browser.md:125 S-a requires exactly one terminal abort orcommit. logclient_ref.py elsebranch marks every nonpiece/nonbegin eventterminal, hidingmissingrealterminal. Actual minimal timeline begin1 + unknownTerminal1 ->[]; expectedS-a. Rootprobe exit1. Repair checker withclosed eventgrammar, preserve everyoldcontrol andfullgolden.

Audit malformedevent/counterboundary so unknownorincorrectshapes returnexplicitS-a/S-e violations ratherthancrash or silentlypass. Counters/attemptIDs/pieceindices are integers, not Pythonbool/equivalentfloat. Only validabort/commit settles an attempt. CoreclientFSM/sourcegoldens remainunchanged. Add minimalunknownterminal,no-realterminal,unknownthenrealterminal/afterterminal, shapeandcounter controls plus existing15exactrules.

P-V2-1 openingheadbudgetexhaustion andP-V2-2..8 conventions remainexplicitproposals, notownerapproval. Scope note must distinguish absentproductionimplementation frommissingexperimentdefinition: M1spec requires specification/literalinputs/passcriteria, not actualTS/Chrome/serverexecution. VerdictreviseC29; lastaccepted0.18. Owners/fullgate/CONF_DEPTH/representationproposalsunchanged; V3/V4next afterrepair.
