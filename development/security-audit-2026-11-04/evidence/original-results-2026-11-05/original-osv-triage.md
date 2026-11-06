# سجل OSV الأصلي

> سجل مراجعة بتاريخ 5 نوفمبر قبل دمج PR24. الأعداد هنا تخص التقارير الأصلية؛ [الحالة الأحدث](../../LATEST-STATUS-2026-11-05-AR.md) تبيّن ما تغير لاحقًا.

14 سجلًا خامًا؛ 12 مفتاحًا مميزًا (advisory,package,version,file). ليست عدد ثغرات مثبتة.

|السجل|الحزمة|التنبيه|الملف|الحالة|تكرار السجل|
|---|---|---|---|---|---|
|1|keccak@0.1.0|RUSTSEC-2026-0012|`prototypes/lp1-node/Cargo.lock`|precondition-not-enabled-in-reviewed-build|—|
|2|tmp@0.0.33|CVE-2025-54798|`prototypes/lp4-execution/tools/package-lock.json`|affected-version-retained-exploitability-not-established|—|
|3|rand@0.7.3|RUSTSEC-2026-0097|`prototypes/lp1-node/Cargo.lock`|precondition-not-established-in-reviewed-build|—|
|4|follow-redirects@1.16.1|CVE-2026-116689|`prototypes/lp4-execution/tools/package-lock.json`|patched-on-security-branch-not-main|—|
|5|tmp@0.0.33|CVE-2026-44705|`prototypes/lp4-execution/tools/package-lock.json`|affected-version-retained-exploitability-not-established|—|
|6|derivative@2.2.0|RUSTSEC-2024-0388|`prototypes/lp4-execution/Cargo.lock`|unmaintained-dependency-debt|—|
|7|derivative@2.2.0|RUSTSEC-2024-0388|`prototypes/pocold/Cargo.lock`|unmaintained-dependency-debt|—|
|8|paste@1.0.15|RUSTSEC-2024-0436|`prototypes/lp4-execution/Cargo.lock`|unmaintained-dependency-debt|—|
|9|paste@1.0.15|RUSTSEC-2024-0436|`prototypes/pocold/Cargo.lock`|unmaintained-dependency-debt|—|
|10|libsecp256k1@0.5.0|RUSTSEC-2025-0161|`prototypes/lp1-node/Cargo.lock`|unmaintained-dependency-debt|—|
|11|keccak@0.1.0|RUSTSEC-2026-0012|`prototypes/lp1-node/Cargo.lock`|precondition-not-enabled-in-reviewed-build|1|
|12|rand@0.7.3|RUSTSEC-2026-0097|`prototypes/lp1-node/Cargo.lock`|precondition-not-established-in-reviewed-build|3|
|13|crypto-bigint@0.5.5|RUSTSEC-2026-0387|`prototypes/lp4-execution/Cargo.lock`|open-timing-risk-signing-reachable|—|
|14|crypto-bigint@0.5.5|RUSTSEC-2026-0387|`prototypes/pocold/Cargo.lock`|open-timing-risk-signing-reachable|—|
