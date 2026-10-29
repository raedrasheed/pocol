# Separate sequential review — hosted receipts and notice preparation

Same author, separate pass after source-head CI and source-notice generation.
No external human/legal review or full LP4 acceptance is claimed.

- Verify native Linux and Windows logs contain40 actual successes each; do not add
  the legacy8 platform-blocked or2 unrun checks to passes. Source/run/head/tree are
  explicit. Raw Cargo/Windows terminal bytes remain unchanged.
- Verify all three downloaded ZIP SHA256s against GitHub metadata. Hosted63-case
  oracle bytes equal the existing local R8 oracle. Current20661-case log bytes equal
  R4's approved-rule log; reference that immutable evidence with exact SHA256 rather
  than relabelling an old unapproved diagnostic as an accepted result.
- Initial inventory checked only top-level package notices. Extend discovery through
  nested COPYRIGHT/COPYING/NOTICE/LICENSE files, including embedded C library notices.
  Default all-target107 external packages differ from Linux-only105 and all locked266.
- Resolve missing licenses at the exact .cargo_vcs_info SHA. EIP2930 and7702 refs have
  identical MIT blobs but each revision is separately preserved. ruint-macro's old
  recmo/uint repository redirects; exact original SHA is resolved in alloy-rs/ruint.
  Aurora CC0 is its normalized crate and checksum-linked VCS workspace declaration;
  retain unchanged standard legal text separately without inventing author attribution.
- A review found that checking only .crate SHA while reading extracted cache files
  would not bind the actual notices to the archive. Fix: compare every used license,
  Cargo.toml and VCS metadata byte with the verified tar member. Reject escapes,
  oversize notice members, missing/mismatched grants and unknown SPDX expressions.
- Archive output is deterministic (sorted names, fixed modes/owner/time, gzipmtime0).
  Source-notice helper has no credential reads, remote writes or runtime flag and
  cannot pass execution/resource gates. No protocol, dependency version or limit change.

Further acceptance: full P2 build/test-tool/source-distribution provenance review,
precise GST/PoCol deviation scope and every existing general resource/real execution
obligation remain open. New audit workflow changes require their own new-head checks.
This is a checkpoint for continuation, not a claim that LP4 or P2 is finished.
