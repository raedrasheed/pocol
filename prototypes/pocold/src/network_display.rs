//! Operator-designated network names. These presentation fields do not change
//! genesis identity, admission scope, currency policy or launch qualification.
use serde_json::Value;

pub(crate) const MAINNET_CHAIN_ID: u64 = 2_656_089_850;
pub(crate) const MAINNET_GENESIS: &str = "664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e";

fn is_mainnet(info: &Value) -> bool {
    info["chainId"].as_u64() == Some(MAINNET_CHAIN_ID)
        && info["genesisHash"].as_str().is_some_and(|hash| {
            hash.strip_prefix("0x").unwrap_or(hash).eq_ignore_ascii_case(MAINNET_GENESIS)
        })
}

/// Add a display designation to information from an already validated chain.
/// Both identifiers must match; local fixtures and unrelated networks are kept intact.
pub fn annotate(mut info: Value) -> Value {
    if is_mainnet(&info) {
        info["networkName"] = Value::String("PoCol Mainnet".into());
        info["networkEnvironment"] = Value::String("mainnet".into());
    }
    info
}

/// Invitations have not yet booted. Do not trust their metadata alone to identify
/// the network: the enclosed GenesisSpec must carry the same committed identity.
pub fn annotate_genesis(info: Value, genesis_hex: &str) -> Value {
    if !is_mainnet(&info) { return info; }
    let valid = crate::types::unhex(genesis_hex.trim()).ok().is_some_and(|bytes| {
        bytes.len() <= crate::genesis_spec::MAX_SPEC_BYTES
            && crate::genesis_spec::decode(&bytes).ok().is_some_and(|spec| {
                spec.chain_id == MAINNET_CHAIN_ID
                    && crate::types::hex(&crate::genesis_spec::genesis_hash(&bytes))[2..]
                        .eq_ignore_ascii_case(MAINNET_GENESIS)
            })
    });
    if valid { annotate(info) } else { info }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn known() -> Value {
        json!({"chainId":MAINNET_CHAIN_ID,"genesisHash":format!("0x{MAINNET_GENESIS}"),
            "scope":"experimental-devnet","profileKind":"Simnet","publicLaunchReady":false,
            "specificationCompliant":false,"testCurrency":true,"forkSchedule":[[1,0],[2,12345]],
            "balanceWei":"1234567890123456789","networkFingerprint":"unchanged"})
    }

    #[test]
    fn designation_preserves_identity_balances_and_qualification() {
        let before = known();
        let mut after = annotate(before.clone());
        assert_eq!(after["networkName"], "PoCol Mainnet");
        assert_eq!(after["networkEnvironment"], "mainnet");
        after.as_object_mut().unwrap().remove("networkName");
        after.as_object_mut().unwrap().remove("networkEnvironment");
        assert_eq!(after, before);
    }

    #[test]
    fn another_chain_or_genesis_cannot_receive_the_designation() {
        let mut other_chain = known(); other_chain["chainId"] = json!(777915);
        let mut other_genesis = known(); other_genesis["genesisHash"] = json!(format!("0x{}", "00".repeat(32)));
        let mut missing = known(); missing.as_object_mut().unwrap().remove("genesisHash");
        let mut wrong_type = known(); wrong_type["chainId"] = json!(MAINNET_CHAIN_ID.to_string());
        for info in [other_chain, other_genesis, missing, wrong_type, Value::Null] {
            assert_eq!(annotate(info.clone()), info);
        }
    }

    #[test]
    fn invitation_metadata_does_not_override_its_genesis() {
        let before = known();
        for bytes in ["00", "not hex", "0xc0"] {
            assert_eq!(annotate_genesis(before.clone(), bytes), before);
        }
    }

    #[test]
    fn published_genesis_receives_the_designation_without_changing_its_bytes() {
        // Public, existing GenesisSpec; no keys or state are embedded here.
        let genesis = "0xf901bc02849e50b6faf8c4846b01fd289d331c14db73ff371e10c5b505558d8473ed9318cdd6f79aab68f276a6ae0a82025801025a08200282010004088203e88201f4883782dace9d9000008901158e460913d0000004822710018260001040102082016882016882017c04822000837a1200843b9aca00830200008303000082800084400000008401000000824e20833d0900830f4240830240008210008201008202008405b8d8008080808082010081800283c09cc08801bc16d674ec8000821388839896800482017c8209c4a03e8d8110695bcc118f6c63032c868154d596aa2651ed9a78bd1fd4bcaa8295fff8acea9415df38aaba05b83fb8c58d2dd9d804654278a96f9415df38aaba05b83fb8c58d2dd9d804654278a96fea944479d23cb5580b912a8402230071b8a7f0a701ec94bf335311d1a3ffc8c0efd1dc422f727f48e6de11ea94c319eeced588afe03515508bab5f27805534b3079456bd8b1a42a6a3b09c04ebdd91e6fa3cc944b6bdea94e45343ea8d6d54fb693a6ad424fac11d85624b6e944ce8b7625efc6898e59d7436c3a5a7569e91b3fca0a1d91673b4b4a2c88ff75d4828dc1932dbaa2831284a66f4b1ea77b4c5e666a2";
        let before = known();
        assert_eq!(annotate_genesis(before.clone(), genesis), annotate(before.clone()));
        let mut changed = crate::types::unhex(genesis).unwrap();
        let end = changed.len() - 1;
        changed[end] ^= 1;
        assert_eq!(annotate_genesis(before.clone(), &crate::types::hex(&changed)), before);
    }
}
