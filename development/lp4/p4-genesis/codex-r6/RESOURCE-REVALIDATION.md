# D27 measurements; full R18/R21/caps64 NOT PASSED

Pinned actual factory runtime762 bytes. The three independent first/repeat gas
pairs are57941/25752 (1 data byte),64315/26121 (31),5395010/448822 (24575).
These include USER transaction execution under an explicit synthetic context,
not SYSTEM raw-call allocation bounds or proofs for all admitted CPs/states.

First creation adds exactly1+len(data) logical code bytes for a new account and
increments the factory nonce. Repetition does neither. All six executions trace
zero SSTORE; no storage/log output is produced. Actual account/code/trie/history/
snapshot memory work and factory/child account dirtiness remain nonzero.

Native CODE-01 limits apply separately; data limit24575 is unchanged even when
runtime CP is32768. Complete ACCT/SLOT/dirty/log/native operation cap enforcement,
host physical code retention and all R21 clones/scans/persistence costs remain
mandatory. The isolated component does not establish their bounds, nor general
SYS allocation sufficiency, actual full-five execution measurements or caps64.
All complete resource gates remain NOT PASSED and boot remains disabled.
