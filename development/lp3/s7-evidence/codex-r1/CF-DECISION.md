# CF1–CF6: missing canonical test binding

Status: OPEN acceptance-design question. Not a network/credential or routine execution
permission issue. M1 and the approved resource amendment remain unchanged.

The accepted M3 list names CF1–CF6 in reference/implementation.md:35 and the identical
FINAL_DESIGN.md:4694. Searching all preserved reference sections, vectors and tools
finds no definition of the six CF inputs, expected outcomes or their mapping. The LP3
S7 criterion requires core T0/CF/SV evidence. Passing unrelated tests cannot establish
an undefined named gate. No original CF pass is claimed, and S7 must not merge yet.

The existing consensus closing/final rules are precise, so an explicit core mapping
can be implemented without changing their semantics. The following is a *proposal*,
not a statement that these were M1's original intended CF labels.

| Proposed label | Source rule | Concrete core acceptance examples |
|---|---|---|
| CF1 | consensus.md §8 closing trigger | With H_CLOSE=100,T_CLOSE=1000: h=99,ts=999,parentClosing=false -> false; h=100 or ts=1000 -> true |
| CF2 | §8 sticky closing | parentClosing=true remains true below both current trigger thresholds; first closeH is retained |
| CF3 | §8 cursor completion bound | closeH=40,RecLen=128,RET=128 -> latest done state h=40; RecLen=129 -> h=41; end/full 553,1024,8 -> h=680; reject RET=0 |
| CF4 | §8 gate2 | H_END=700,P2=2,T_END=10000,T_blk=10: h=697,ts=9979 -> false; h=698 or ts=9980 -> true; exact equality accepted |
| CF5 | §8 final predicate on supplied validated parent facts | parentClosing and parentCursorDone and h>=parentCloseH+D and (h>=H_END or ts>=T_END); independently falsify every conjunct, test equality and u64 endpoints |
| CF6 | §8/validity parent-final and height ceilings | A final parent has no valid child; h>H_END rejected; core arithmetic never emits an overflowing bound. Actual finalize, cursor mutation, pending[F] absence and zero final subsidy remain real-execution LP4 checks |

Resolution requested: provide the canonical CF1–CF6 definitions if they exist outside
the preserved material, or explicitly approve this as the LP3 **core test mapping**.
The approval would not change M1 consensus rules, accept full-execution tests, defer
additional criteria silently, or authorize boot. Each proposed case would then be
implemented in Rust/TS and checked by an independent integer/boolean oracle and
mutations before S7 acceptance. T0 and final S8/S9 audit remain separately tracked.

Numerical S7 work already implemented/reviewed independently of this question:
actual Rust AS1-AS19 traces, million-case Rust/TS/Python equivalence, Decimal-60 ASQ,
allocation counter, release/Node benchmarks, and supplied-fact SV arithmetic. This
draft checkpoint preserves those results without calling the undefined CF gates passed.
