//! LP4 prerequisite: canonical Ethereum hexary MPT roots, not execution or persistence.
//! Raw keys are at most 64 bytes (secure keys are exactly 32; ordered indices <=9).
//! Recursion consumes at least one nibble, hence depth <=129. Entry budget is explicit
//! local API input, never an implicit consensus cap. Empty values denote deletion in
//! Ethereum; raw construction rejects them instead of silently retaining a ghost leaf.
use crate::{
    fixed::U256,
    hashes::{keccak256, H256},
    rlp,
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrieError {
    EntryBudget,
    KeyTooLong,
    DuplicateKey,
    EmptyValue,
}
struct Row<'a> {
    key: Vec<u8>,
    value: &'a [u8],
}
fn bytes(b: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    rlp::encode_bytes(b, &mut out);
    out
}
fn list(parts: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = parts.iter().flatten().copied().collect();
    let mut out = Vec::new();
    rlp::encode_list_payload(&payload, &mut out);
    out
}
fn compact(path: &[u8], leaf: bool) -> Vec<u8> {
    let odd = path.len() % 2 == 1;
    let flag = if leaf { 2 } else { 0 };
    let mut out = vec![((flag + odd as u8) << 4) | if odd { path[0] } else { 0 }];
    let start = if odd { 1 } else { 0 };
    for p in path[start..].chunks(2) {
        out.push(p[0] << 4 | p[1]);
    }
    out
}
fn child(encoded: Vec<u8>) -> Vec<u8> {
    if encoded.len() < 32 {
        encoded
    } else {
        bytes(&keccak256(&encoded))
    }
}
fn node(rows: &[Row<'_>], depth: usize) -> Vec<u8> {
    if rows.len() == 1 {
        return list(&[bytes(&compact(&rows[0].key[depth..], true)), bytes(rows[0].value)]);
    }
    let first = &rows[0].key;
    let last = &rows[rows.len() - 1].key;
    let mut end = depth;
    while end < first.len().min(last.len()) && first[end] == last[end] {
        end += 1;
    }
    if end > depth {
        return list(&[bytes(&compact(&first[depth..end], false)), child(node(rows, end))]);
    }
    let mut parts = vec![bytes(&[]); 17];
    let mut i = 0;
    if rows[0].key.len() == depth {
        parts[16] = bytes(rows[0].value);
        i = 1;
    }
    for nibble in 0..16 {
        let start = i;
        while i < rows.len() && rows[i].key[depth] == nibble as u8 {
            i += 1;
        }
        if i > start {
            parts[nibble] = child(node(&rows[start..i], depth + 1));
        }
    }
    list(&parts)
}
/// Root is always hashed, including roots whose node encoding is shorter than 32 bytes.
pub fn root(entries: &[(Vec<u8>, Vec<u8>)], entry_budget: usize) -> Result<H256, TrieError> {
    if entries.len() > entry_budget {
        return Err(TrieError::EntryBudget);
    }
    let mut rows = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        if key.len() > 64 {
            return Err(TrieError::KeyTooLong);
        }
        if value.is_empty() {
            return Err(TrieError::EmptyValue);
        }
        rows.push(Row { key: key.iter().flat_map(|b| [b >> 4, b & 15]).collect(), value });
    }
    rows.sort_by(|a, b| a.key.cmp(&b.key));
    if rows.windows(2).any(|w| w[0].key == w[1].key) {
        return Err(TrieError::DuplicateKey);
    }
    Ok(keccak256(&if rows.is_empty() { bytes(&[]) } else { node(&rows, 0) }))
}
fn uint(n: &U256) -> Vec<u8> {
    let b = n.to_be_bytes();
    bytes(&b[b.iter().position(|v| *v != 0).unwrap_or(32)..])
}
/// Zero slots are omitted; values are RLP(minimal integer), keys Keccak(be256(slot)).
pub fn storage_root(slots: &[(U256, U256)], entry_budget: usize) -> Result<H256, TrieError> {
    if slots.len() > entry_budget {
        return Err(TrieError::EntryBudget);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut entries = Vec::new();
    for (slot, value) in slots {
        if !seen.insert(slot.to_be_bytes()) {
            return Err(TrieError::DuplicateKey);
        }
        if !value.is_zero() {
            entries.push((keccak256(&slot.to_be_bytes()).to_vec(), uint(value)));
        }
    }
    root(&entries, entry_budget)
}
#[derive(Clone, Debug)]
pub struct Account {
    pub address: [u8; 20],
    pub nonce: u64,
    pub balance: U256,
    pub storage_root: H256,
    pub code_hash: H256,
}
/// Every supplied account is explicit. Account deletion/EIP-161 belongs to execution.
pub fn state_root(accounts: &[Account], entry_budget: usize) -> Result<H256, TrieError> {
    if accounts.len() > entry_budget {
        return Err(TrieError::EntryBudget);
    }
    let entries = accounts
        .iter()
        .map(|a| (keccak256(&a.address).to_vec(), list(&[uint(&U256::from_u64(a.nonce)), uint(&a.balance), bytes(&a.storage_root), bytes(&a.code_hash)])))
        .collect::<Vec<_>>();
    root(&entries, entry_budget)
}
/// Ordered body/receipt envelope bytes are opaque here; execution validates their type/content.
pub fn ordered_root(values: &[Vec<u8>], entry_budget: usize) -> Result<H256, TrieError> {
    if values.len() > entry_budget {
        return Err(TrieError::EntryBudget);
    }
    let entries = values.iter().enumerate().map(|(i, v)| (uint(&U256::from_u64(i as u64)), v.clone())).collect::<Vec<_>>();
    root(&entries, entry_budget)
}
