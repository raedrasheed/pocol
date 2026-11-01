//! Immutable fixture chain. Headers become servable only after every one of them decodes strictly,
//! re-encodes from typed fields to the exact saved bytes, and links from the genesis parent through
//! items 2-9 with netid (`window::validate_chain`). An explicit empty chain (head 0) is also
//! available for the F3 / empty-chain cases.

use crate::hashes::{self, H256};
use crate::header::{self, Header};
use crate::rlp;
use crate::window::{self, NetConfig, Window};

#[derive(Debug)]
pub struct FixtureChain {
    head: u64,
    encoded: Vec<Vec<u8>>,
    block_hashes: Vec<H256>,
}

impl FixtureChain {
    pub fn empty() -> FixtureChain {
        FixtureChain { head: 0, encoded: Vec::new(), block_hashes: Vec::new() }
    }

    pub fn load(cfg: &NetConfig, raw: Vec<Vec<u8>>) -> Result<FixtureChain, String> {
        let mut hdrs: Vec<Header> = Vec::with_capacity(raw.len());
        for (i, b) in raw.iter().enumerate() {
            let hd = header::decode(b).map_err(|e| format!("H{}: rule 1 ({})", i + 1, e.detail))?;
            if hd.encode() != *b {
                return Err(format!("H{}: typed re-encoding differs from the saved bytes", i + 1));
            }
            if hd.h != i as u64 + 1 {
                return Err(format!("H{}: header height {}", i + 1, hd.h));
            }
            hdrs.push(hd);
        }
        window::validate_chain(cfg, &hdrs).map_err(|(h, e)| format!("H{h}: rule {} ({})", e.rule, e.detail))?;
        let mut w = Window::new(cfg, true);
        let mut block_hashes = Vec::with_capacity(hdrs.len());
        for i in 0..hdrs.len() {
            block_hashes.push(w.block_hash(&hdrs, i));
        }
        Ok(FixtureChain { head: hdrs.len() as u64, encoded: raw, block_hashes })
    }

    pub fn head(&self) -> u64 {
        self.head
    }

    pub fn block_hash(&self, h: u64) -> Option<H256> {
        if h == 0 || h > self.head {
            return None;
        }
        Some(self.block_hashes[(h - 1) as usize])
    }

    /// RLP list of the encodings of headers from..from+count-1 (caller has applied F0-F4).
    pub fn headers_rlp(&self, from: u64, count: u64) -> Option<Vec<u8>> {
        if from < 1 || count < 1 {
            return None;
        }
        let last = from as u128 + count as u128 - 1;
        if last > self.head as u128 {
            return None;
        }
        let mut payload = Vec::new();
        for h in from..=(last as u64) {
            payload.extend_from_slice(&self.encoded[(h - 1) as usize]);
        }
        let mut out = Vec::with_capacity(payload.len() + 9);
        rlp::encode_list_payload(&payload, &mut out);
        Some(out)
    }

    /// Digest over the whole served state; used to prove that no RPC call mutates it.
    pub fn state_digest(&self) -> H256 {
        let mut all = self.head.to_be_bytes().to_vec();
        for e in &self.encoded {
            all.extend_from_slice(&hashes::keccak256(e));
        }
        hashes::keccak256(&all)
    }
}
