//! Operator-designated network names. These presentation fields do not change
//! genesis identity, admission scope, currency policy or launch qualification.
use serde_json::Value;

const MAINNET_CHAIN_ID: u64 = 1_599_846_715;
const MAINNET_GENESIS: &str = "be137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca";

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
        let genesis = "0xf9016b01845f5bb93bf89e846ae3a7029e400040004000400040004000400040004000400040004000400040003fff0a82025880023c06200282010004088203e88201f4881bc16d674ec8000089056bc75e2d631000000140018220001040102040102804822000837a1200843b9aca008302000083030000828000832000008401000000824e2082c350824e20830100008208008201008202008405b8d8008080808081a0818002a00952692453fda50223e12b5eb117946dc5402053712d56d5be8b9b338fed9dacf881ea94854e2a71a9d84dc63290804fed3f51e79e1f5d9394854e2a71a9d84dc63290804fed3f51e79e1f5d93ea948f8fff3e07669f41cb2c2e458a0282eeef858b7e948f8fff3e07669f41cb2c2e458a0282eeef858b7eea94a28f9bd53bffdc2651832b238eaa96917f33adca94a28f9bd53bffdc2651832b238eaa96917f33adcaa0a877a16e8e236b8e505e7a752f8e95e26ee59e8ace13156ce8b2307d1ef14c4f";
        let before = known();
        assert_eq!(annotate_genesis(before.clone(), genesis), annotate(before.clone()));
        let mut changed = crate::types::unhex(genesis).unwrap();
        let end = changed.len() - 1;
        changed[end] ^= 1;
        assert_eq!(annotate_genesis(before.clone(), &crate::types::hex(&changed)), before);
    }
}
