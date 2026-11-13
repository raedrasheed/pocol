//! A generated continuous spec-1 network, as `pocold init-testnet` writes one with `"continuous": true`: the
//! builder's defaults (T_blk 10 s, D_att 60, dFbWait 6), the Continuous program set, no H_END, R_max 1, three miners
//! and one funded account. Tests that need a long-lived public-style chain build it here instead of reading any
//! published network's files; its consensus parameters are the builder's, so only g_ts, the chain id and the
//! members differ from any other default continuous network.
use std::path::Path;

#[allow(dead_code)]
pub const CHAIN_ID: u64 = 777917;

/// Writes the network's genesis.spec.hex, alloc.json and devnet.json into the new directory `out`.
pub fn continuous_network(out: &Path) {
    use pocol_lp4_execution::devnet;
    let miners: Vec<serde_json::Value> = ["a", "b", "c"].iter()
        .map(|l| { let m = devnet::miner_identity(&format!("continuous-{l}")); serde_json::json!({"id": m.id, "reward": m.reward}) })
        .collect();
    let config = serde_json::json!({"continuous": true, "chainId": CHAIN_ID, "genesisTime": 1_800_000_000u64,
        "registrationPerBlock": 1, "miners": miners,
        "accounts": [{"address": devnet::account_key("continuous-funder").address(), "wei": "1000000000000000000000000"}]});
    pocold::testnet::prepare(config.to_string().as_bytes(), out).expect("the generated continuous network");
}
