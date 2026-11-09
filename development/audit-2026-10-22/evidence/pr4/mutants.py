#!/usr/bin/env python3
"""PR4 mutation controls (plan §6). Each mutant: apply text edits to one or more sources (each old string must
occur exactly once), run the fixed test set, record KILLED/PASS/BUILD_FAIL and the failing tests, then restore
the sources with `git checkout`. Usage: run.py WT OUTDIR [ID ...]"""
import json, os, re, subprocess, sys, time

WT = sys.argv[1]; OUT = sys.argv[2]; ONLY = set(sys.argv[3:])
P = os.path.join(WT, "prototypes/pocold")
C, N = "src/chain.rs", "src/node.rs"

GATE = "        if pe.pruned||pe.slim{return Err(Reject::NeedsRebuild(x.parent_hash))}\n"
PE = "        let pe:&Entry=&self.blocks[&x.parent_hash];\n"
FUT = "        if let Some(c)=clock{ if crate::timing::is_future(x.ts,c,phi){return Err(Reject::Future)} }\n"
BIND = '        if !bound{return Err(Reject::Envelope("15 body does not match the header roots".into()))}\n'
FB = ("        // 12 (fallback): UT byte-equal to Build_fb (needs apply), for an owner-signed copy only.\n"
      "        let fb_out=if signed{None}else{\n"
      "            let out=self.execute(p,&ctx,x.ts,Proposer::Fallback,&b.body,vec![])?;\n"
      "            if self.fill_ut(p,&ctx,x.ts,&out,&[]).ut_raw!=x.ut_raw{return blk(\"12 fallback UT differs from Build_fb\")}\n"
      "            Some(out)};\n")
OWN = "        let owner=ctx.assignment.owner(x.nonce).ok_or_else(||Reject::Block(\"13 owner\".into()))?;\n"
CANON = "        let canon=self.chain.canonical_at(h.h).map(|c|c.drawn);\n"
DRAWN = (CANON +
         "        let drawn=canon.is_some_and(|d|h.proposer[..]==d[..])||\n"
         "            ((canon.is_none()||!self.chain.is_canonical(&h.parent_hash))&&pe.full&&!pe.pruned&&!pe.slim\n"
         "                &&self.chain.child_ctx(pe,0).is_ok_and(|c|h.proposer[..]==c.proposer[..]));\n")
EWIN = "        if h.h.saturating_add(u64::from(self.chain.spec.cp().E_win))<self.chain.head().height+1{return false}\n"
SEENKEY = "fn seen_key(h:&Header)->([u8;32],u64){let mut p=h.parent_hash.to_vec();p.extend_from_slice(&h.proposer);(hashes::keccak256(&p),h.h)}"

MUTANTS = [
    ("M1", "the state gate back above item 2", [(C, PE, PE + GATE)]),
    ("M2", "the materialization of a diff-held parent back above item 2",
     [(C, PE, PE + "        if !pe.pruned&&!pe.slim&&!pe.full{let _m=self.full_entry(&x.parent_hash).ok_or(Reject::NeedsRebuild(x.parent_hash))?;self.materializations.fetch_add(1,std::sync::atomic::Ordering::Relaxed);}\n")]),
    ("M3", "Future moved after the state gate", [(C, FUT, ""), (C, GATE, GATE + FUT)]),
    ("M4", "the body binding moved after the state gate", [(C, BIND, ""), (C, GATE, GATE + BIND)]),
    ("M5", "a fallback copy is executed before item 13", [(C, FB, ""), (C, OWN, FB + OWN)]),
    ("M6", "observe_template rebuilds the parent with full_entry again",
     [(N, DRAWN, "        let Some(par)=self.full_entry(&h.parent_hash) else{return false};\n"
                 "        let drawn=self.chain.child_ctx(&par,0).is_ok_and(|c|h.proposer[..]==c.proposer[..]);\n")]),
    ("M7", "the deep arm reads child_ctx on the parent, not canonical_at(h).drawn",
     [(N, CANON, "        let canon=self.chain.canonical_at(h.h).and_then(|_|self.chain.child_ctx(pe,0).ok()).map(|c|c.proposer);\n")]),
    ("M8", "the side-parent arm removed (drawn needs c.parent==h.parent_hash)",
     [(N, CANON, "        let canon=self.chain.canonical_at(h.h).filter(|c|c.parent==Some(h.parent_hash)).map(|c|c.drawn);\n")]),
    ("M9", "import_with calls observe_template again",
     [(N, "if entry.signed{self.record_template(&b.header);}", "self.observe_template(&b.header);")]),
    ("M10", "reprune removed (both calls)",
     [(N, "self.chain.reprune();return Err(e)}", "return Err(e)}"), (N, "        if tries>0&&res.is_err(){self.chain.reprune();}\n", "")]),
    ("M11", "4b keep_diff always true",
     [(C, "let keep_diff=!self.is_canonical(&h)||e.height>cut;", "let keep_diff=true;let _=cut;")]),
    ("M12", "the costly cache keyed by blockHash, not seal(TemplateID,nonce)",
     [(C, "match self.costly_of(&sk){", "match self.costly_of(&hash){"),
      (C, "{self.note_costly(sk,Costly::Invalid(", "{self.note_costly(hash,Costly::Invalid("),
      (C, "self.note_costly(sk,Costly::Owner(owner));", "self.note_costly(hash,Costly::Owner(owner));")]),
    ("M13", "no note_costly(Owner)", [(C, "self.note_costly(sk,Costly::Owner(owner));", "")]),
    ("M14", "no note_costly(Invalid)",
     [(C, "{self.note_costly(sk,Costly::Invalid(truncate(&w).into_boxed_str()));Err(Reject::Block(w))}", "{Err(Reject::Block(w))}")]),
    ("M15", "the Owner lookup refuses o==winner", [(C, "Some(Costly::Owner(o)) if o!=winner=>", "Some(Costly::Owner(o)) if o==winner=>")]),
    ("M16", "the costly lookup removed", [(C, "match self.costly_of(&sk){", "match None::<Costly>{")]),
    ("M17", "item 14 verdicts cached too (!w.starts_with(\"14 \") removed)",
     [(C, 'Err(Reject::Block(w)) if !w.starts_with("14 ")=>', "Err(Reject::Block(w))=>")]),
    ("M18", "child_pre uses p.height+2",
     [(C, "let h=p.height+1;\n        let window=AttemptWindow::new(p.ts,attempt", "let h=p.height+2;\n        let window=AttemptWindow::new(p.ts,attempt")]),
    ("M19", "rebuild_token always true", [(N, "fn rebuild_token(&mut self)->bool{", "fn rebuild_token(&mut self)->bool{if true{return true}")]),
    ("M20", "the session deep-rebuild cap removed", [(N, "if self.session.as_ref().is_some_and(|s|s.rebuilds>=SESSION_REBUILD_MAX){", "if false&&self.session.as_ref().is_some_and(|s|s.rebuilds>=SESSION_REBUILD_MAX){")]),
    ("M20b", "the session cap drops the block but end_session is not called",
     [(N, 'claim set aside");self.end_session(false);res=Err(Reject::Evicted);break}', 'claim set aside");res=Err(Reject::Evicted);break}')]),
    ("M21", "solicited blocks gated on the global bucket (no session allowance)",
     [(N, "            if on_branch{\n", "            if false&&on_branch{\n")]),
    ("M22", "the costly FIFO never trims (no COSTLY_MAX bound)",
     [(C, "while order.len()>=COSTLY_MAX{if let Some(old)=order.pop_front(){map.remove(&old);}}", "")]),
    # Review fixes after pass 2 (2060983, 34b533f, 36ef14b, 00fba86).
    ("M23", "observe_template's E_win age filter removed (any past height observed)", [(N, EWIN, "")]),
    ("M24", "an overflow of seen_templates clears the map whole again",
     [(N, "if self.seen_templates.len()>SEEN_MAX{self.evict_seen_templates();}", "if self.seen_templates.len()>SEEN_MAX{self.seen_templates.clear();}")]),
    ("M25", "the side parent's own Draw arm removed (child_ctx only for the tip)",
     [(N, "            ((canon.is_none()||!self.chain.is_canonical(&h.parent_hash))&&pe.full", "            (canon.is_none()&&pe.full")]),
    ("M26", "seen_templates keyed by (parent, h) again, not per signer",
     [(N, SEENKEY, "fn seen_key(h:&Header)->([u8;32],u64){(h.parent_hash,h.h)}")]),
    ("M27", "rebuild_token without the backward-clock clamp",
     [(N, "if self.rebuild_refill_ms==0||now<self.rebuild_refill_ms{self.rebuild_refill_ms=now;}", "if self.rebuild_refill_ms==0{self.rebuild_refill_ms=now;}")]),
    ("M28", "every block from the session's peer charged to the session allowance (on_session_branch bypassed)",
     [(N, "let on_branch=solicited&&self.session.as_ref().is_some_and(|s|self.on_session_branch(s,&b.header.parent_hash));",
          "let on_branch=solicited&&self.session.is_some();")]),
]

CONS = ["pr4_", "every_validity_item_rejects_its_mutation", "template_form_classes_follow_d129", "a01_",
        "a_valid_header_with_a_foreign_body", "k_hist_retention", "journal_replay", "store_mode_imports_on_the_head",
        "e4_store_node_rebuilds", "node_detects_template_equivocation", "an_equivocation_switch",
        "an_invalid_template_from_the_drawn_proposer", "d133_heal", "a13_bound_breakdown",
        "a_sync_session_refetches_an_evicted_branch"]
RUNS = [
    ["cargo", "test", "--release", "--locked", "--lib", "--", "f5f6_tests"],
    ["cargo", "test", "--release", "--locked", "--test", "consensus", "--", "--test-threads=2"] + CONS,
    ["cargo", "test", "--release", "--locked", "--test", "relay"],
]

def git(*a): return subprocess.run(["git", "-C", WT] + list(a), capture_output=True, text=True, check=True).stdout

def run_set(log):
    failed, build_fail, passed = [], False, 0
    for cmd in RUNS:
        r = subprocess.run(cmd, cwd=P, capture_output=True, text=True)
        out = r.stdout + r.stderr
        log.write("$ " + " ".join(cmd) + "\n" + "\n".join(l for l in out.splitlines() if l.startswith(("test ", "error", "test result")) or "FAIL-ON-ORIGINAL" in l or "panicked" in l) + "\n")
        if re.search(r"^error(\[E\d+\])?: ", out, re.M) and "test result" not in out:
            build_fail = True; break
        failed += re.findall(r"^test (\S+) \.\.\. FAILED", out, re.M)
        passed += sum(int(x) for x in re.findall(r"test result: \w+\. (\d+) passed", out))
    return build_fail, failed, passed

def main():
    assert git("status", "--porcelain", "--", "prototypes/pocold/src") == "", "sources must be clean"
    head = git("rev-parse", "--short", "HEAD").strip()
    os.makedirs(OUT, exist_ok=True)
    jl = open(os.path.join(OUT, "mutants.log"), "a"); full = open(os.path.join(OUT, "mutants-full.log"), "a")
    jl.write(f"# PR4 mutation controls (plan §6), tree {head}; each mutant's sources restored with git checkout after its run.\n")
    jl.write("# Test set: " + " ; ".join(" ".join(c) for c in RUNS) + "\n")
    for mid, desc, edits in MUTANTS:
        jl.write(f"# {mid} {desc}: " + " | ".join(f"{f}: replace {o.strip()!r} with {n.strip()!r}" for f, o, n in edits) + "\n")
    jl.write(f"## pass ({head})\n"); jl.flush()
    results = []
    todo = ([("BASE", "", [])] if not ONLY or "BASE" in ONLY else []) + [m for m in MUTANTS if not ONLY or m[0] in ONLY]
    for mid, desc, edits in todo:
        try:
            for f, o, n in edits:
                path = os.path.join(P, f); s = open(path).read()
                assert s.count(o) == 1, f"{mid}: {o!r} occurs {s.count(o)} times in {f}"
                open(path, "w").write(s.replace(o, n))
            t = time.time(); full.write(f"\n===== {mid} {desc}\n")
            bf, failed, passed = run_set(full)
            status = "BUILD_FAIL" if bf else ("KILLED" if failed else "PASS")
            rec = {"mutant": f"{mid} {desc}".strip() if mid != "BASE" else "BASE", "status": status, "failed": failed}
            results.append(rec); jl.write(json.dumps(rec) + "\n"); jl.flush(); full.flush()
            print(mid, status, failed, f"{passed} passed", f"{time.time()-t:.0f}s", flush=True)
        finally:
            git("checkout", "--", "prototypes/pocold/src")
    json.dump({f"pass_{head}": results}, open(os.path.join(OUT, f"mutants-{head}.json"), "w"), indent=1)

main()
