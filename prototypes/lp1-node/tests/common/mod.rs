#![allow(dead_code)]

use std::path::PathBuf;

use serde_json::Value;

use lp1_node::fixtures;
use lp1_node::verify::{self, Node};

pub fn dir() -> PathBuf {
    fixtures::default_dir()
}

pub fn node() -> Node {
    verify::load_node(&dir(), false).expect("fixture node loads")
}

pub fn json(name: &str) -> Value {
    fixtures::read_json(&dir(), name).expect("fixture json")
}

pub fn case<'a>(cases: &'a Value, id: &str) -> &'a Value {
    cases["cases"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("case {id}"))
}

pub fn case_bytes(cases: &Value, id: &str) -> Vec<u8> {
    lp1_node::hex::decode(case(cases, id)["headers_rlp_hex"].as_str().unwrap()).unwrap()
}

/// Wraps already-encoded header items in an outer RLP list.
pub fn outer(items: &[Vec<u8>]) -> Vec<u8> {
    let mut payload = Vec::new();
    for i in items {
        payload.extend_from_slice(i);
    }
    let mut out = Vec::new();
    lp1_node::rlp::encode_list_payload(&payload, &mut out);
    out
}

/// Deterministic generator for corpora (xorshift64*).
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
}
