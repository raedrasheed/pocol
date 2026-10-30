//! LP1: local experimental PoCol M3-foundation slice over accepted M1 formats.
//!
//! Scope: fixed-width integers, bounded ASERT, strict RLP and V1 header codec, SHA-256 / Keccak
//! domain hashes with secp256k1 recovery (verification only), RP window items netid / future /
//! 2-9, an immutable validated fixture chain and a read-only loopback JSON-RPC node. Not full M3,
//! not consensus, no EVM, no P2P, no transactions, no keys. The V1NET profile is synthetic and
//! non-bootable.
//!
//! LP2 adds `store`: a durable, experimental journal of header *candidates* (scoped H-pre only,
//! awaiting H-full; never executed, canonical or live consensus) with recovery into new stores.
//!
//! LP3 S1 (first increment) adds `genesis_spec`: the full GenesisSpec v1 decoder / encoder with the
//! accepted error order and genesisHash binding. Identity only: not ParamGate, not bootable.
//! `genesis_batch` is the bounded line reader behind `lp1-node genesis-decode`; `decimal` renders the
//! exact gsVersion detail.

pub mod asert;
pub mod assignment;
pub mod chain;
pub mod decimal;
pub mod envelope;
pub mod finality_core;
pub mod fixed;
pub mod fixtures;
pub mod gate;
pub mod genesis;
pub mod genesis_batch;
pub mod genesis_spec;
pub mod hashes;
pub mod header;
pub mod hex;
pub mod http;
pub mod json;
pub mod param_gate;
pub mod profile;
pub mod rewards;
pub mod rlp;
pub mod rpc;
pub mod secp_backend;
pub mod store;
pub mod timing;
pub mod verify;
pub mod window;

/// LP4 canonical trie root prerequisite; no execution or boot acceptance.
pub mod trie;
