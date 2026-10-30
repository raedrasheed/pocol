# Codex independent review — M1 author turn011 / revision0.13

Preserved-copy execution with Python3.11.1:409 entries,309 pass,99 recorded,1FAIL,exit1,14.5s. E4 stops at run_checks_013.py:165: record(name,status,**kw) is called with positional 'recorded' and keyword status=sc['status'], raising TypeError. Later E4 parse/base64/value/maximum-size/disk checks therefore did not execute. C26: checker diagnostic field collision; must repair before acceptance without redefining result status or hiding omitted checks.

Independent probes30/30 pass,exit0: six native Node byte/base64 answers and full roundtrips; exact1442048 bound; inclusive disk/name boundaries; immediate ticketreservation prevents a second name allocation; refresh issued before settlement does not retire the ticket, later refresh does; two reads retain physicalslots after readTimeout; late read only releases its slot and cannot issue a checkpoint, and a waiting newgate takes the releasedslot.

Verdict: revise. Preserve0.13 results and successful scoped probes. New0.14 should change the diagnostic metadata name (proposalStatus, not status) and rerun every E4/E5 and inherited0.12 test; no source or policy changes needed for that repair. Last accepted batch0.12.

Codec claims are explicitly limited to Unicode scalar inputs and valueepochs representable as JS safe integers. Proposed CR-E4-01/02 are unresolved baseline/schema changes, not approvals. Native Node demonstrates a collision for distinct lone-surrogate keys and loss of exact numericprecision above2^53-1; Python agreement is not a Chrome proof. FullD104/AdminDelete/sites and realChrome measurements remain pending. No merge/deploy, transactions or owner policy change.
