//! LP3 S1, first increment: GenesisSpec v1 against the accepted M1 vectors, read in place from
//! development/m1 and pinned by SHA-256 (nothing is copied or regenerated):
//!   m1-draft-0.21/vectors/v3-gsv1.json        GSV1 literal, K1-K3, L4n N1-N10 and 22 supplements
//!   m1-draft-0.22/vectors/c30-depth.json      13 deep (1500) / shallow framing and structure twins
//!   m1-draft-0.22/vectors/v3-e05-supplement.json  three-library H_GSV1 and GSV1 SHA-256
//! Expected codes come from those files or, for the schema tables below, from consensus.md:7-34.
//! Fixture bytes are built here by the same literal / edit rules the M1 runners use.

mod common;

use std::path::PathBuf;
use std::thread;

use serde_json::Value;

use common::Rng;
use lp1_node::fixed::U256;
use lp1_node::genesis_spec::{self as gs, GenesisSpec, GsError, Upper, CP_SCHEMA};
use lp1_node::hashes::{keccak256, sha256};
use lp1_node::{hex, json};

const PINNED: [(&str, &str); 3] = [
    ("m1-draft-0.21/vectors/v3-gsv1.json", "88097ee293aeab70c2089c2b61da8ce624503a59dfafd34657e9c1a465056f32"),
    ("m1-draft-0.22/vectors/c30-depth.json", "728d8b4ac88c0212d8b1cb6224c5f834f923619e1c32320dcf1eae120fb72464"),
    ("m1-draft-0.22/vectors/v3-e05-supplement.json", "cd8ead8d8d0293bf5a48ef6113fafe09c8babd5e4516d5176905325ac8ce0cdb"),
];

/// The parser must not need more than this; a recursive parser would overflow at these depths.
const SMALL_STACK: usize = 128 * 1024;

fn m1_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../development/m1")
}

fn read_m1(rel: &str) -> Vec<u8> {
    let p = m1_dir().join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("accepted M1 input {} is required: {e}", p.display()))
}

fn m1_json(rel: &str) -> Value {
    serde_json::from_slice(&read_m1(rel)).unwrap()
}

fn gsv1_doc() -> Value {
    m1_json(PINNED[0].0)
}

// ------------------------------------------------------------------ literal construction (M1 runner rules)

fn leaf(v: &Value) -> Vec<u8> {
    match v {
        Value::String(s) => hex::decode(s).unwrap(),
        Value::Object(o) if o.contains_key("repeat") => {
            let b = hex::decode(o["repeat"].as_str().unwrap()).unwrap();
            b.repeat(o["count"].as_u64().unwrap() as usize)
        }
        Value::Object(o) if o.contains_key("concat") => o["concat"].as_array().unwrap().iter().flat_map(leaf).collect(),
        other => panic!("leaf {other}"),
    }
}

fn list_header(n: usize) -> Vec<u8> {
    if n < 56 {
        return vec![0xc0 + n as u8];
    }
    let be = (n as u64).to_be_bytes();
    let first = be.iter().position(|x| *x != 0).unwrap();
    let mut h = vec![0xf7 + (8 - first) as u8];
    h.extend_from_slice(&be[first..]);
    h
}

/// The JSON fixture trees are a few levels deep; deep inputs are built by `wrap`, never here.
fn serialize(t: &Value) -> Vec<u8> {
    match t {
        Value::Array(items) => {
            let body: Vec<u8> = items.iter().flat_map(serialize).collect();
            let mut out = list_header(body.len());
            out.extend(body);
            out
        }
        other => leaf(other),
    }
}

/// `depth` list headers around `inner`, built by a loop in linear time.
fn wrap(inner: &[u8], depth: usize) -> Vec<u8> {
    let mut heads = Vec::with_capacity(depth);
    let mut len = inner.len();
    for _ in 0..depth {
        let h = list_header(len);
        len += h.len();
        heads.push(h);
    }
    let mut out = Vec::with_capacity(len);
    for h in heads.iter().rev() {
        out.extend_from_slice(h);
    }
    out.extend_from_slice(inner);
    out
}

fn path_of(v: &Value) -> Vec<usize> {
    v.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as usize).collect()
}

fn node_mut<'a>(t: &'a mut Value, path: &[usize]) -> &'a mut Vec<Value> {
    let mut n = t;
    for i in &path[..path.len() - 1] {
        n = &mut n[*i];
    }
    n.as_array_mut().unwrap()
}

fn get(t: &Value, path: &[usize]) -> Value {
    let mut n = t;
    for i in path {
        n = &n[*i];
    }
    n.clone()
}

fn apply_edits(tree: &Value, edits: &[Value]) -> Vec<u8> {
    let mut t = tree.clone();
    let mut post = Vec::new();
    for e in edits {
        match e["op"].as_str().unwrap() {
            "delete" => {
                let p = path_of(&e["path"]);
                node_mut(&mut t, &p).remove(p[p.len() - 1]);
            }
            "set" => {
                let p = path_of(&e["path"]);
                node_mut(&mut t, &p)[p[p.len() - 1]] = e["value"].clone();
            }
            "swap" => {
                let (a, b) = (path_of(&e["a"]), path_of(&e["b"]));
                let (va, vb) = (get(&t, &a), get(&t, &b));
                node_mut(&mut t, &a)[a[a.len() - 1]] = vb;
                node_mut(&mut t, &b)[b[b.len() - 1]] = va;
            }
            "copy" => {
                let (from, to) = (path_of(&e["from"]), path_of(&e["to"]));
                let v = get(&t, &from);
                node_mut(&mut t, &to)[to[to.len() - 1]] = v;
            }
            _ => post.push(e.clone()),
        }
    }
    let mut b = serialize(&t);
    for e in post {
        let h = hex::decode(e["hex"].as_str().unwrap()).unwrap();
        match e["op"].as_str().unwrap() {
            "appendBytes" => b.extend(h),
            "setOuterHeader" => b[..h.len()].copy_from_slice(&h),
            other => panic!("edit {other}"),
        }
    }
    b
}

fn gsv1() -> Vec<u8> {
    serialize(&gsv1_doc()["tree"])
}

fn with_cp(k: usize, item_hex: &str) -> Vec<u8> {
    let edit = serde_json::json!([{ "op": "set", "path": [2, k], "value": item_hex }]);
    apply_edits(&gsv1_doc()["tree"], edit.as_array().unwrap())
}

/// As `with_cp`, but alpha_bp is varied with gamma_bp = 0 so that its own bound is what is tested.
fn with_cp_alone(k: usize, item_hex: &str) -> Vec<u8> {
    if CP_SCHEMA[k].name != "alpha_bp" {
        return with_cp(k, item_hex);
    }
    let edit = serde_json::json!([{ "op": "set", "path": [2, 14], "value": "80" }, { "op": "set", "path": [2, k], "value": item_hex }]);
    apply_edits(&gsv1_doc()["tree"], edit.as_array().unwrap())
}

fn outcome(b: &[u8]) -> (String, String) {
    match gs::decode(b) {
        Ok(_) => ("ok".into(), String::new()),
        Err(GsError { code, detail }) => (code.into(), detail),
    }
}

fn code(b: &[u8]) -> String {
    outcome(b).0
}

fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    thread::Builder::new().stack_size(SMALL_STACK).spawn(f).unwrap().join().expect("no stack overflow or panic")
}

/// Canonical RLP integer item for `v` (hex).
fn int_item(v: &U256) -> String {
    let mut out = Vec::new();
    lp1_node::rlp::encode_bytes(&v.to_min_be(), &mut out);
    hex::encode(&out)
}

// ------------------------------------------------------------------ provenance and identity

#[test]
fn accepted_inputs_are_pinned_and_unchanged() {
    for (rel, want) in PINNED {
        assert_eq!(hex::encode(&sha256(&read_m1(rel))), want, "{rel}");
    }
}

#[test]
fn k1_k3_and_three_library_gsv1_hash() {
    let doc = gsv1_doc();
    let e05 = m1_json(PINNED[2].0);
    for k in ["K1", "K2", "K3"] {
        let row = doc["keccakVectors"][k].as_array().unwrap();
        let got = hex::encode(&keccak256(&hex::decode(row[0].as_str().unwrap()).unwrap()));
        assert_eq!(got, row[1].as_str().unwrap(), "{k}");
        assert_eq!(got, e05["K"][k].as_str().unwrap(), "{k} three-library");
    }
    let flat: Vec<u8> = doc["flatSegments"].as_array().unwrap().iter().flat_map(leaf).collect();
    let b = gsv1();
    assert_eq!(flat, b, "validation.md literal segments equal the item tree");
    let ex = &doc["expected"];
    assert_eq!(b.len() as u64, ex["length"].as_u64().unwrap());
    assert_eq!(serialize(&doc["tree"][2]).len() as u64, ex["cpEncodedLength"].as_u64().unwrap());
    assert_eq!(serialize(&doc["tree"][4]).len() as u64, ex["m0EncodedLength"].as_u64().unwrap());
    assert_eq!(b.len() as u64 - 3, ex["payloadLength"].as_u64().unwrap());
    assert_eq!((b.len(), &b[..3]), (341, &[0xf9u8, 0x01, 0x52][..]));
    assert_eq!(hex::encode(&sha256(&b)), e05["GSV1"]["inputSha256"].as_str().unwrap());
    assert_eq!(hex::encode(&gs::genesis_hash(&b)), e05["GSV1"]["keccak256"].as_str().unwrap());
    assert_eq!(hex::encode(&gs::genesis_hash(&b)), "de518e30a5e333ac3ddf127b6201f1a66f80dce2b56566524df4037bcdeb2277");
}

fn expected_cp() -> Vec<U256> {
    // Number lexemes are kept exactly (B_reg is 10^20, beyond u64).
    let raw = read_m1(PINNED[0].0);
    let doc = json::parse(&raw).unwrap();
    let cp = match doc.get("expected").and_then(|e| e.get("cp")) {
        Some(json::JVal::Arr(a)) => a.clone(),
        other => panic!("expected.cp {other:?}"),
    };
    cp.iter()
        .map(|v| match v {
            json::JVal::Num(lex) => U256::from_dec_str(lex).unwrap(),
            json::JVal::Obj(_) => match v.get("pow2") {
                Some(json::JVal::Num(n)) => U256::pow2(n.parse().unwrap()).unwrap(),
                other => panic!("cp {other:?}"),
            },
            other => panic!("cp {other:?}"),
        })
        .collect()
}

#[test]
fn gsv1_decodes_to_every_expected_field_and_reencodes() {
    let doc = gsv1_doc();
    let ex = &doc["expected"];
    let b = gsv1();
    let s = gs::decode(&b).unwrap();
    assert_eq!(ex["specVersion"].as_u64(), Some(1));
    assert_eq!(s.chain_id, ex["chainId"].as_u64().unwrap());
    assert_eq!(s.chain_id, 777_902);
    let want = expected_cp();
    assert_eq!(want.len(), 52);
    for (k, w) in want.iter().enumerate() {
        assert_eq!(s.cp[k], *w, "CP {}", CP_SCHEMA[k].name);
    }
    assert_eq!(s.cp_value("subsidy").unwrap().to_dec_string(), "80000");
    assert_eq!(s.cp_value("B_reg").unwrap().to_dec_string(), "100000000000000000000");
    assert_eq!(s.alloc_root.to_vec(), leaf(&ex["allocRoot"]));
    assert_eq!(s.sys_code_hash.to_vec(), leaf(&ex["sysCodeHash"]));
    let m0 = ex["m0"].as_array().unwrap();
    assert_eq!(s.m0_list.len(), m0.len());
    for (got, w) in s.m0_list.iter().zip(m0) {
        assert_eq!(got.id.to_vec(), leaf(&w[0]));
        assert_eq!(got.reward_addr.to_vec(), leaf(&w[1]));
    }
    assert_eq!(gs::encode(&s), b, "decode then encode returns the same 341 bytes");
    assert_eq!(gs::verify_hash(&b, &keccak256(&b)).unwrap(), s);
}

// ------------------------------------------------------------------ L4n and supplements

#[test]
fn l4n_and_all_supplement_negatives() {
    let doc = gsv1_doc();
    let negs = doc["negatives"].as_array().unwrap();
    assert_eq!(negs.len(), 32);
    let ids: Vec<&str> = negs.iter().map(|n| n["id"].as_str().unwrap()).collect();
    for n in 1..=10 {
        assert!(ids.contains(&format!("N{n}").as_str()), "N{n}");
    }
    let mut seen_ok = 0;
    for n in negs {
        let id = n["id"].as_str().unwrap();
        let b = apply_edits(&doc["tree"], n["edits"].as_array().unwrap());
        let want = n["expect"].as_str().unwrap();
        assert_eq!(code(&b), want, "{id}");
        if let Some(l) = n.get("lengths") {
            assert_eq!(b.len() as u64, l["length"].as_u64().unwrap(), "{id} length");
        }
        if want == "ok" {
            seen_ok += 1;
            assert_eq!(gs::encode(&gs::decode(&b).unwrap()), b, "{id} re-encodes");
        }
    }
    assert_eq!(seen_ok, 3, "W6b, S4 and S5 are the positive edges");
}

#[test]
fn n11_different_genesis_hash_is_rejected() {
    let b = gsv1();
    let mut wrong = keccak256(&b);
    wrong[31] ^= 1;
    assert_eq!(gs::verify_hash(&b, &wrong).unwrap_err(), GsError { code: "genesisHash", detail: "R24(a)".into() });
    // A changed spec has a different identity even when it decodes.
    let other = with_cp(0, "846553f101");
    assert!(gs::decode(&other).is_ok());
    assert_eq!(gs::verify_hash(&other, &keccak256(&b)).unwrap_err().code, "genesisHash");
    // Decode errors come first.
    assert_eq!(gs::verify_hash(&[0xc0], &keccak256(&[0xc0])).unwrap_err().code, "gsCount");
}

// ------------------------------------------------------------------ C30 depth twins

fn depth_case(doc: &Value, case: &Value, depth: usize) -> Vec<u8> {
    let inner = hex::decode(case["inner"].as_str().unwrap()).unwrap();
    let mut b = match case["place"].as_str().unwrap() {
        "top" => wrap(&inner, depth),
        place => {
            let mut t = doc["tree"].clone();
            let nested = Value::String(hex::encode(&wrap(&inner, depth)));
            if place == "gsv1Append" {
                t.as_array_mut().unwrap().push(nested);
            } else {
                let p = path_of(&case["path"]);
                node_mut(&mut t, &p)[p[p.len() - 1]] = nested;
            }
            let none = Vec::new();
            apply_edits(&t, case.get("edits").and_then(|e| e.as_array()).unwrap_or(&none))
        }
    };
    if case.get("dropLast").and_then(|x| x.as_bool()) == Some(true) {
        b.pop();
    }
    if let Some(s) = case.get("suffix") {
        b.extend(hex::decode(s.as_str().unwrap()).unwrap());
    }
    b
}

#[test]
fn c30_deep_and_shallow_twins_on_a_small_stack() {
    let cdoc = m1_json(PINNED[1].0);
    let gdoc = gsv1_doc();
    let cases = cdoc["cases"].as_array().unwrap().clone();
    assert_eq!(cases.len(), 13);
    for case in cases {
        let id = case["id"].as_str().unwrap().to_string();
        let want = (case["expect"]["code"].as_str().unwrap().to_string(), case["expect"]["detail"].as_str().unwrap().to_string());
        for twin in ["deep", "shallow"] {
            let depth = case[twin]["depth"].as_u64().unwrap() as usize;
            let b = depth_case(&gdoc, &case, depth);
            assert_eq!(b.len() as u64, case[twin]["length"].as_u64().unwrap(), "{id} {twin} length");
            let got = on_small_stack(move || outcome(&b));
            assert_eq!(got, want, "{id} {twin}");
        }
    }
}

#[test]
fn extreme_depths_keep_the_shallow_result() {
    let cdoc = m1_json(PINNED[1].0);
    let gdoc = gsv1_doc();
    for case in cdoc["cases"].as_array().unwrap() {
        let shallow = outcome(&depth_case(&gdoc, case, case["shallow"]["depth"].as_u64().unwrap() as usize));
        for depth in [5_000usize, 100_000] {
            let b = depth_case(&gdoc, case, depth);
            let got = on_small_stack(move || outcome(&b));
            assert_eq!(got, shallow, "{} depth {depth}", case["id"]);
        }
    }
    // 1500 levels of valid framing with a wrapped leaf inside a CP item: structure, not gsInt.
    let b = with_cp(2, &hex::encode(&wrap(&[0x81, 0x0a], 1500)));
    assert_eq!(on_small_stack(move || outcome(&b)), ("gsStructure".to_string(), "CP item is a list".to_string()));
}

// ------------------------------------------------------------------ schema tables (consensus.md:18-34)

fn upper(k: usize, alpha: u64) -> Option<u64> {
    match CP_SCHEMA[k].hi {
        Upper::None => None,
        Upper::Value(h) => Some(h),
        Upper::Gamma => Some(10_000 - alpha),
    }
}

#[test]
fn every_cp_width_minimum_enum_and_gamma_bound() {
    let alpha = 1000; // GSV1 alpha_bp
    let mut checked = 0;
    for (k, field) in CP_SCHEMA.iter().enumerate() {
        let max_in_width = if field.width == 256 { U256::MAX } else { U256::pow2(field.width).unwrap().checked_sub(&U256::ONE).unwrap() };
        let hi = upper(k, alpha);
        let in_bounds = |v: u64| v >= field.lo && hi.map_or(true, |h| v <= h);
        // Largest value of the width.
        let want = if hi.map_or(true, |h| U256::from_u64(h) >= max_in_width) { "ok" } else { "gsRange" };
        assert_eq!(code(&with_cp_alone(k, &int_item(&max_in_width))), want, "{} max of width", field.name);
        // One above the width: one more byte, or 2^256 as 33 bytes.
        let mut over = vec![0x01];
        over.extend(vec![0u8; field.width as usize / 8]);
        let mut item = Vec::new();
        lp1_node::rlp::encode_bytes(&over, &mut item);
        assert_eq!(outcome(&with_cp_alone(k, &hex::encode(&item))), ("gsRange".into(), field.name.into()), "{} 2^width", field.name);
        // Minimum, one below it, upper bound and one above it.
        for v in [0u64, 1, field.lo] {
            let want = if in_bounds(v) { "ok" } else { "gsRange" };
            assert_eq!(code(&with_cp_alone(k, &int_item(&U256::from_u64(v)))), want, "{} = {v}", field.name);
        }
        if let Some(h) = hi {
            assert_eq!(code(&with_cp_alone(k, &int_item(&U256::from_u64(h)))), "ok", "{} = hi", field.name);
            assert_eq!(outcome(&with_cp_alone(k, &int_item(&U256::from_u64(h + 1)))), ("gsRange".into(), field.name.into()), "{} = hi+1", field.name);
        }
        // Integer form: wrapped single byte, leading zero, zero as a byte.
        for bad in ["8105", "820005", "00"] {
            assert_eq!(outcome(&with_cp_alone(k, bad)), ("gsInt".into(), field.name.into()), "{} {bad}", field.name);
        }
        checked += 1;
    }
    assert_eq!(checked, 52);
    // gamma follows alpha: alpha 10^4 leaves gamma 0 only; alpha above 10^4 is alpha's own range error.
    let tree = &gsv1_doc()["tree"];
    let pair = |a: u64, g: u64| {
        let e = serde_json::json!([
            { "op": "set", "path": [2, 13], "value": int_item(&U256::from_u64(a)) },
            { "op": "set", "path": [2, 14], "value": int_item(&U256::from_u64(g)) }
        ]);
        outcome(&apply_edits(tree, e.as_array().unwrap()))
    };
    assert_eq!(pair(10_000, 0).0, "ok");
    assert_eq!(pair(10_000, 1), ("gsRange".into(), "gamma_bp".into()));
    assert_eq!(pair(0, 10_000).0, "ok");
    assert_eq!(pair(10_001, 0), ("gsRange".into(), "alpha_bp".into()));
}

fn set_top(k: usize, item: &str) -> Vec<u8> {
    let e = serde_json::json!([{ "op": "set", "path": [k], "value": item }]);
    apply_edits(&gsv1_doc()["tree"], e.as_array().unwrap())
}

#[test]
fn version_chain_id_and_root_fields() {
    for (item, want) in
        [("01", ("ok", "")), ("02", ("gsVersion", "2")), ("80", ("gsVersion", "0")), ("8101", ("gsInt", "specVersion")), ("820001", ("gsInt", "specVersion"))]
    {
        assert_eq!(outcome(&set_top(0, item)), (want.0.to_string(), want.1.to_string()), "version {item}");
    }
    let chain = [
        ("01", "ok"),
        ("88ffffffffffffffff", "ok"),
        ("80", "gsRange"),
        ("820001", "gsInt"),
        ("89010000000000000000", "gsRange"),
        ("8105", "gsInt"),
        ("00", "gsInt"),
    ];
    for (item, want) in chain {
        assert_eq!(code(&set_top(1, item)), want, "chainId {item}");
    }
    let ok = gs::decode(&set_top(1, "88ffffffffffffffff")).unwrap();
    assert_eq!(ok.chain_id, u64::MAX);
    for k in [3, 5] {
        for len in [0usize, 1, 31, 33] {
            let mut item = Vec::new();
            lp1_node::rlp::encode_bytes(&vec![0x22; len], &mut item);
            assert_eq!(outcome(&set_top(k, &hex::encode(&item))), ("gsLen".into(), "root".into()), "top[{k}] {len} bytes");
        }
        assert_eq!(outcome(&set_top(k, "c0")), ("gsStructure".into(), format!("top[{k}]")));
    }
}

fn m0_with(entries: &[(&[u8], &[u8])]) -> Vec<u8> {
    let list: Vec<Value> = entries
        .iter()
        .map(|(id, r)| {
            let mut a = Vec::new();
            let mut b = Vec::new();
            lp1_node::rlp::encode_bytes(id, &mut a);
            lp1_node::rlp::encode_bytes(r, &mut b);
            serde_json::json!([hex::encode(&a), hex::encode(&b)])
        })
        .collect();
    let e = serde_json::json!([{ "op": "set", "path": [4], "value": list }]);
    apply_edits(&gsv1_doc()["tree"], e.as_array().unwrap())
}

fn id_low(prefix: u8, low: u32) -> [u8; 20] {
    let mut id = [0u8; 20];
    id[0] = prefix;
    id[17] = (low >> 16) as u8;
    id[18] = (low >> 8) as u8;
    id[19] = low as u8;
    id
}

#[test]
fn member_list_order_reserved_ids_and_shapes() {
    let r = [0xb1u8; 20];
    let (a, b) = ([0xa1u8; 20], [0xa2u8; 20]);
    assert_eq!(code(&m0_with(&[(&a, &r)])), "ok", "one member");
    assert_eq!(code(&m0_with(&[(&a, &r), (&b, &r)])), "ok");
    assert_eq!(outcome(&m0_with(&[(&b, &r), (&a, &r)])), ("gsOrder".into(), "ids".into()));
    assert_eq!(outcome(&m0_with(&[(&a, &r), (&a, &r)])), ("gsOrder".into(), "ids".into()));
    assert_eq!(code(&m0_with(&[(&a, &a), (&b, &a)])), "ok", "rewardAddr is not ordered or unique");
    for (low, want) in [(0xC0_C000, "ok"), (0xC0_C001, "gsSys"), (0xC0_C080, "gsSys"), (0xC0_C0FF, "gsSys"), (0xC0_C100, "ok")] {
        let id = id_low(0, low);
        assert_eq!(code(&m0_with(&[(&id, &r)])), want, "id low {low:06x}");
    }
    let prefixed = id_low(1, 0xC0_C001);
    assert_eq!(code(&m0_with(&[(&prefixed, &r)])), "ok", "reserved range needs 17 zero bytes");
    let sys = gs::SYSTEM_ADDRESS;
    assert_eq!(outcome(&m0_with(&[(&a, &r), (&sys, &r)])), ("gsSys".into(), format!("0x{}", hex::encode(&sys))));
    assert_eq!(code(&m0_with(&[(&[0u8; 20], &r)])), "gsSys");
    // gsOrder is judged over the whole list before gsSys.
    let reserved = id_low(0, 0xC0_C001);
    assert_eq!(code(&m0_with(&[(&reserved, &r), (&reserved, &r)])), "gsOrder");
    // Lengths and shapes.
    assert_eq!(outcome(&m0_with(&[(&a[..19], &r)])), ("gsLen".into(), "M_0List address".into()));
    assert_eq!(outcome(&m0_with(&[(&a, &r[..0])])), ("gsLen".into(), "M_0List address".into()));
    assert_eq!(outcome(&m0_with(&[])), ("gsCount".into(), "M_0List empty".into()));
    let three = serde_json::json!([{ "op": "set", "path": [4, 0], "value": [hex::encode(&[0x94]) + &hex::encode(&a), "80", "80"] }]);
    assert_eq!(outcome(&apply_edits(&gsv1_doc()["tree"], three.as_array().unwrap())), ("gsCount".into(), "M_0List entry 3".into()));
    let flat = serde_json::json!([{ "op": "set", "path": [4, 0], "value": "80" }]);
    assert_eq!(outcome(&apply_edits(&gsv1_doc()["tree"], flat.as_array().unwrap())), ("gsStructure".into(), "M_0List entry".into()));
}

#[test]
fn member_count_relations_are_param_gate_not_decode() {
    // M_min <= |M_0List| <= M_max is ParamGate R12 (DG-V3-5); decode accepts and keeps the values.
    let e = serde_json::json!([{ "op": "set", "path": [2, 19], "value": "05" }]);
    let b = apply_edits(&gsv1_doc()["tree"], e.as_array().unwrap());
    let s = gs::decode(&b).unwrap();
    assert_eq!((s.m0_list.len(), s.cp_value("M_min").unwrap().to_u64()), (2, Some(5)));
}

#[test]
fn stage_order_across_fields() {
    let tree = &gsv1_doc()["tree"];
    let run = |edits: Value| outcome(&apply_edits(tree, edits.as_array().unwrap())).0;
    // structure before version, count before int, int before len, len before range, range before order.
    assert_eq!(run(serde_json::json!([{ "op": "set", "path": [0], "value": "02" }, { "op": "set", "path": [3], "value": [] }])), "gsStructure");
    assert_eq!(run(serde_json::json!([{ "op": "set", "path": [1], "value": "8105" }, { "op": "set", "path": [3], "value": "80" }])), "gsInt");
    assert_eq!(run(serde_json::json!([{ "op": "set", "path": [3], "value": "80" }, { "op": "set", "path": [1], "value": "80" }])), "gsLen");
    assert_eq!(run(serde_json::json!([{ "op": "set", "path": [2, 4], "value": "02" }, { "op": "swap", "a": [4, 0], "b": [4, 1] }])), "gsRange");
    // chainId is ranged before CP; within CP the first field in order wins.
    assert_eq!(
        outcome(&apply_edits(
            tree,
            serde_json::json!([{ "op": "set", "path": [1], "value": "80" }, { "op": "set", "path": [2, 0], "value": "89010000000000000000" }])
                .as_array()
                .unwrap()
        ))
        .1,
        "chainId"
    );
    assert_eq!(
        outcome(&apply_edits(
            tree,
            serde_json::json!([{ "op": "set", "path": [2, 9], "value": "80" }, { "op": "set", "path": [2, 4], "value": "02" }]).as_array().unwrap()
        ))
        .1,
        "nonceMode"
    );
    // gsLen judges both roots before member addresses.
    let b = apply_edits(
        tree,
        serde_json::json!([{ "op": "set", "path": [5], "value": "80" }, { "op": "set", "path": [4, 0, 0], "value": "80" }]).as_array().unwrap(),
    );
    assert_eq!(outcome(&b), ("gsLen".into(), "root".into()));
}

#[test]
fn encode_round_trips_valid_specs() {
    let base = gs::decode(&gsv1()).unwrap();
    let mut s: GenesisSpec = base.clone();
    s.chain_id = 1;
    s.cp[1] = U256::MAX;
    s.cp[15] = U256::ZERO;
    s.m0_list.truncate(1);
    let b = gs::encode(&s);
    assert_eq!(gs::decode(&b).unwrap(), s);
    // Encode does not validate: an invalid struct encodes, and decode rejects it.
    let mut bad = base;
    bad.chain_id = 0;
    assert_eq!(code(&gs::encode(&bad)), "gsRange");
}

// ------------------------------------------------------------------ seeded corpus

fn mutate(rng: &mut Rng, base: &[u8]) -> Vec<u8> {
    let mut v = base.to_vec();
    for _ in 0..1 + rng.below(4) {
        let len = v.len() as u64;
        match rng.below(6) {
            0 if len > 0 => {
                let i = rng.below(len) as usize;
                v[i] ^= 1 << rng.below(8);
            }
            1 if len > 0 => {
                let i = rng.below(len) as usize;
                v[i] = rng.next() as u8;
            }
            2 => v.truncate(rng.below(len + 1) as usize),
            3 => {
                let i = rng.below(len + 1) as usize;
                v.insert(i, rng.next() as u8);
            }
            4 if len > 0 => {
                let i = rng.below(len) as usize;
                v.remove(i);
            }
            _ => {
                let i = rng.below(len + 1) as usize;
                let n = rng.below(4) as usize;
                let chunk: Vec<u8> = v[i..(i + n).min(v.len())].to_vec();
                v.splice(i..i, chunk);
            }
        }
    }
    v
}

#[test]
fn seeded_corpus_never_panics_and_accepts_only_canonical_bytes() {
    let base = gsv1();
    let mut rng = Rng(0x4c50_3301_0001_0001);
    let mut counts: std::collections::BTreeMap<String, u64> = std::collections::BTreeMap::new();
    let trials = 20_000;
    for _ in 0..trials {
        let m = mutate(&mut rng, &base);
        let c = match gs::decode(&m) {
            Ok(s) => {
                assert_eq!(gs::encode(&s), m, "accepted mutant re-encodes exactly");
                "ok".to_string()
            }
            Err(e) => e.code.to_string(),
        };
        *counts.entry(c).or_insert(0) += 1;
    }
    let total: u64 = counts.values().sum();
    assert_eq!(total, trials);
    assert!(counts.get("L0").copied().unwrap_or(0) > 0 && counts.len() >= 5, "{counts:?}");
    let parts: Vec<String> = counts.iter().map(|(k, v)| format!("\"{k}\":{v}")).collect();
    println!("{{\"corpus\":\"lp3-genesis\",\"seed\":\"0x4c50330100010001\",\"trials\":{trials},\"codes\":{{{}}}}}", parts.join(","));
}

// ------------------------------------------------------------------ round 2: exact gsVersion detail, size bound

/// Independent of `lp1_node::decimal`: base-10 long division, one decimal digit per pass.
fn slow_decimal(be: &[u8]) -> String {
    let mut v: Vec<u8> = be.iter().copied().skip_while(|x| *x == 0).collect();
    if v.is_empty() {
        return "0".into();
    }
    let mut digits = Vec::new();
    while !v.is_empty() {
        let mut rem = 0u32;
        for x in v.iter_mut() {
            let cur = (rem << 8) | *x as u32;
            *x = (cur / 10) as u8;
            rem = cur % 10;
        }
        digits.push(b'0' + rem as u8);
        let lead = v.iter().position(|x| *x != 0).unwrap_or(v.len());
        v.drain(..lead);
    }
    digits.reverse();
    String::from_utf8(digits).unwrap()
}

fn version_item(raw: &[u8]) -> String {
    let mut out = Vec::new();
    lp1_node::rlp::encode_bytes(raw, &mut out);
    hex::encode(&out)
}

#[test]
fn gs_version_detail_is_the_exact_decimal_at_every_length() {
    // 2^128 and above were reported as a fixed string in round 1; the reference reports the value.
    let mut rng = Rng(0x4c50_3302_0002_0001);
    for len in [2usize, 16, 17, 20, 32, 33, 64, 255, 256, 257, 1024, 4096] {
        for fill in 0..3 {
            let raw: Vec<u8> = (0..len)
                .map(|i| match fill {
                    0 => 0xff,
                    1 => {
                        if i == 0 {
                            1
                        } else {
                            0
                        }
                    }
                    _ => rng.next() as u8 | if i == 0 { 1 } else { 0 },
                })
                .collect();
            let b = set_top(0, &version_item(&raw));
            assert_eq!(outcome(&b), ("gsVersion".to_string(), slow_decimal(&raw)), "len {len} fill {fill}");
        }
    }
    // Leading zero bytes do not change the value; 00..01 is version 1 and is judged as gsInt.
    assert_eq!(outcome(&set_top(0, &version_item(&[0, 0, 2]))), ("gsVersion".into(), "2".into()));
    assert_eq!(outcome(&set_top(0, &version_item(&[0, 0, 1]))), ("gsInt".into(), "specVersion".into()));
    assert_eq!(slow_decimal(&[0xff; 32]), "115792089237316195423570985008687907853269984665640564039457584007913129639935");
}

/// The largest spec that can pass decode and R12: every field at its widest and 65535 members.
fn max_spec() -> Vec<u8> {
    let mut s = gs::decode(&gsv1()).unwrap();
    s.chain_id = u64::MAX;
    for (k, f) in CP_SCHEMA.iter().enumerate() {
        s.cp[k] = match f.hi {
            Upper::None if f.width == 256 => U256::MAX,
            Upper::None => U256::pow2(f.width).unwrap().checked_sub(&U256::ONE).unwrap(),
            Upper::Value(h) => U256::from_u64(h),
            Upper::Gamma => U256::from_u64(5000),
        };
    }
    s.cp[13] = U256::from_u64(5000); // alpha_bp: 3 bytes, leaves gamma_bp 5000 in range
    s.m0_list = (1..=gs::MAX_MEMBERS as u32)
        .map(|i| {
            let mut id = [0x10u8; 20];
            id[16..].copy_from_slice(&i.to_be_bytes());
            gs::Member { id, reward_addr: [0xb1; 20] }
        })
        .collect();
    gs::encode(&s)
}

#[test]
fn max_spec_bytes_is_the_largest_valid_encoding() {
    assert_eq!(gs::MAX_MEMBERS, 65_535);
    assert_eq!(gs::MAX_SPEC_BYTES, 2_818_474);
    let b = max_spec();
    assert_eq!(b.len(), gs::MAX_SPEC_BYTES);
    let s = gs::decode(&b).unwrap();
    assert_eq!(s.m0_list.len(), gs::MAX_MEMBERS);
    assert_eq!(gs::encode(&s), b);
    // The bound is not a decode rule: one more member still decodes (R12 is ParamGate).
    let mut more = s.clone();
    let mut id = [0x10u8; 20];
    id[16..].copy_from_slice(&(gs::MAX_MEMBERS as u32 + 1).to_be_bytes());
    more.m0_list.push(gs::Member { id, reward_addr: [0xb1; 20] });
    let b2 = gs::encode(&more);
    assert_eq!(b2.len(), gs::MAX_SPEC_BYTES + 43);
    assert!(gs::decode(&b2).is_ok());
}
