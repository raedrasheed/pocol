# Codex independent review — M1 author turn008 / revision0.10

Claude authored E1-E3 specifications and fake-backend fixtures only; no execution claim. Independent preserved-copy run with Python3.11.1:113 entries,86 pass,27 recorded,0 fail,11.9s,exit0.

The independent E1 oracle and counterexamples disagree:24 checks,17 pass,7 FAIL,exit1. Native Node BigInt confirms9 canonical name/range examples, but malformed names ending with LF are accepted by parse_epoch_name and parse_record_name. Python regex `$` matches before a final newline, so the declared strict inverse is not implemented.

Boolean values are also accepted as epoch and sequence by isinstance(x,int), as bootMs by epoch_value_ok, and as epoch/seq record fields by equality (True==1). These are not numeric fields in the declared schema. This is C24, a specification-fixture validation defect; no owner judgment is needed to correct it.

Verdict: revise. Do not advance latest independently accepted revision past0.9. Preserve0.10 and its86-pass author-suite evidence plus7 independent failures. Request a new0.11 reference-only repair that rejects trailing characters and bool numeric fields, restores regressions and reruns the full0.10 schedules and negative controls. E1-E3 remain proposedPartial pending the repair; M1 acceptance and all owner decisions remain open.

The default runner reports enumeration invariant success. No actual Chrome/CDP/A15c/A15d measurement, fmt2 codec or full D104 bounds were executed. Those limitations remain explicit. This result demonstrates why author golden agreement is insufficient by itself.
