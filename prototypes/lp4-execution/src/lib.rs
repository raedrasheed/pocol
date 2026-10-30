//! Experimental Cancun control backend. This is not PoCol execution or boot authorization.
use revm::{inspector_handle_register, primitives::{Address, ExecutionResult, ResultAndState, SpecId, TxKind, U256}, Database, Evm, EvmContext, InMemoryDB, Inspector};
use revm::interpreter::{CallInputs, CallOutcome};

pub const CHAIN_ID: u64 = 777902;
pub fn pay_address() -> Address { Address::from_word(U256::from(0xc0c0ffu64).into()) }

#[derive(Default)]
struct PayGuard { seen: bool }
impl<DB: Database> Inspector<DB> for PayGuard {
    fn call(&mut self, _: &mut EvmContext<DB>, inputs: &mut CallInputs) -> Option<CallOutcome> {
        self.seen |= inputs.bytecode_address == pay_address() || inputs.target_address == pay_address();
        None
    }
}

/// A snapshot is cloned for each transaction; returned diffs are never committed.
#[derive(Clone, Default)]
pub struct Sandbox { pub snapshot: InMemoryDB }
#[derive(Debug)]
pub enum SandboxError { UnsupportedPayBoundary, InvalidTransaction(String) }
impl Sandbox {
    pub fn execute(&self, caller: Address, to: Address, gas_limit: u64, chain_id: u64) -> Result<ResultAndState, SandboxError> {
        let mut evm = Evm::builder()
            .with_db(self.snapshot.clone())
            .with_external_context(PayGuard::default())
            .with_spec_id(SpecId::CANCUN)
            .modify_cfg_env(|cfg| cfg.chain_id = CHAIN_ID)
            .modify_block_env(|block| { block.gas_limit = U256::from(30_000_000u64); block.basefee = U256::ZERO; })
            .modify_tx_env(|tx| { tx.caller = caller; tx.transact_to = TxKind::Call(to); tx.gas_limit = gas_limit; tx.gas_price = U256::ZERO; tx.chain_id = Some(chain_id); tx.nonce = Some(0); })
            .append_handler_register(inspector_handle_register)
            .build();
        let result = evm.transact();
        // Fail closed at the sandbox boundary. This is NOT a definition of consensus PAY.
        if evm.context.external.seen { return Err(SandboxError::UnsupportedPayBoundary); }
        result.map_err(|error| SandboxError::InvalidTransaction(format!("{error:?}")))
    }
}
pub fn output(result: &ExecutionResult) -> &[u8] { result.output().map(|bytes| bytes.as_ref()).unwrap_or_default() }

#[cfg(test)]
mod tests {
    use super::*;
    use revm::primitives::{AccountInfo, Bytecode, Bytes};
    fn fixture(code: &[u8]) -> (Sandbox, Address, Address) {
        let caller = Address::from([0x11; 20]); let target = Address::from([0x22; 20]);
        let mut sandbox = Sandbox::default();
        sandbox.snapshot.insert_account_info(caller, AccountInfo::from_balance(U256::from(1_000_000u64)));
        let bytecode = Bytecode::new_raw(Bytes::copy_from_slice(code));
        sandbox.snapshot.insert_account_info(target, AccountInfo::new(U256::ZERO, 0, bytecode.hash_slow(), bytecode));
        (sandbox, caller, target)
    }
    #[test] fn sstore_is_a_diff_and_snapshot_is_immutable() {
        let (sandbox, caller, target) = fixture(&[0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xf3]);
        for _ in 0..2 {
            let result = sandbox.execute(caller,target,100_000,CHAIN_ID).unwrap();
            assert!(result.result.is_success()); assert_eq!(result.result.gas_used(), 43_112);
            assert_eq!(result.state[&target].storage[&U256::ZERO].present_value(), U256::from(1));
        }
        assert!(sandbox.snapshot.accounts[&target].storage.is_empty());
        assert_eq!(sandbox.snapshot.accounts[&caller].info.nonce,0);
    }
    #[test] fn revert_discards_storage_diff() {
        let (sandbox, caller,target)=fixture(&[0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xfd]);
        let result=sandbox.execute(caller,target,100_000,CHAIN_ID).unwrap();
        assert!(matches!(result.result, ExecutionResult::Revert{..}));
        assert!(result.state.get(&target).map(|a|a.storage.values().all(|slot| slot.present_value() == U256::ZERO)).unwrap_or(true));
        assert!(sandbox.snapshot.accounts[&target].storage.is_empty());
    }
    #[test] fn out_of_gas_discards_storage_diff() {
        let (sandbox,caller,target)=fixture(&[0x60,1,0x60,0,0x55]);
        let result=sandbox.execute(caller,target,21_000,CHAIN_ID).unwrap();
        assert!(matches!(result.result,ExecutionResult::Halt{..})); assert_eq!(result.result.gas_used(),21_000);
        assert!(result.state.get(&target).map(|a|a.storage.values().all(|slot| slot.present_value() == U256::ZERO)).unwrap_or(true));
    }
    #[test] fn chain_id_opcode_and_invalid_chain() {
        let (sandbox,caller,target)=fixture(&[0x46,0x60,0,0x52,0x60,32,0x60,0,0xf3]);
        let attempted=sandbox.execute(caller,target,100_000,CHAIN_ID);
        assert_eq!(attempted.is_ok(), true, "valid chain must execute");
        let result=attempted.unwrap();
        assert_eq!(output(&result.result),U256::from(CHAIN_ID).to_be_bytes::<32>());
        assert_eq!(result.result.gas_used(),21_017);
        assert!(matches!(sandbox.execute(caller,target,100_000,CHAIN_ID+1),Err(SandboxError::InvalidTransaction(_))));
        assert!(matches!(sandbox.execute(caller,target,20_999,CHAIN_ID),Err(SandboxError::InvalidTransaction(_))));
    }
    #[test] fn direct_pay_is_not_accepted_as_empty_account_success() {
        let (sandbox,caller,_)=fixture(&[]);
        assert!(matches!(sandbox.execute(caller,pay_address(),100_000,CHAIN_ID),Err(SandboxError::UnsupportedPayBoundary)));
    }
    #[test] fn nested_pay_is_guarded_even_when_call_result_is_ignored() {
        // CALL(gas=65535,to=C0C0FF,value=0,in=0/0,out=0/0); POP; STOP.
        let code=[0x60,0,0x60,0,0x60,0,0x60,0,0x60,0,0x62,0xc0,0xc0,0xff,0x61,0xff,0xff,0xf1,0x50,0];
        let (sandbox,caller,target)=fixture(&code);
        assert!(matches!(sandbox.execute(caller,target,100_000,CHAIN_ID),Err(SandboxError::UnsupportedPayBoundary)));
    }
    #[test] fn exact_store_gas_boundary() {
        let (sandbox,caller,target)=fixture(&[0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xf3]);
        assert!(sandbox.execute(caller,target,43_112,CHAIN_ID).unwrap().result.is_success());
        assert!(matches!(sandbox.execute(caller,target,43_111,CHAIN_ID).unwrap().result, ExecutionResult::Halt{..}));
    }
    #[test] fn delegated_and_static_pay_calls_are_guarded() {
        for op in [0xf4,0xfa] {
            let code=[0x60,0,0x60,0,0x60,0,0x60,0,0x62,0xc0,0xc0,0xff,0x61,0xff,0xff,op,0x50,0];
            let (sandbox,caller,target)=fixture(&code);
            assert!(matches!(sandbox.execute(caller,target,100_000,CHAIN_ID),Err(SandboxError::UnsupportedPayBoundary)));
        }
    }

}

pub mod pay;
pub mod native;
pub mod state_view;

pub mod collision;
pub mod modexp;
pub mod wire;
pub mod receipts;
pub mod fees;
#[path="wire_rlp.rs"] #[allow(dead_code)] pub(crate) mod rlp;
#[path="p1_fixed.rs"] #[allow(dead_code)] pub mod fixed;
#[path="p1_trie.rs"] #[allow(dead_code)] pub(crate) mod trie;
#[path="p1_hex.rs"] #[allow(dead_code)] pub(crate) mod hex;
pub(crate) mod hashes {
    pub type H256=[u8;32];
    pub fn keccak256(bytes:&[u8])->H256{revm::primitives::keccak256(bytes).0}
    pub fn sha256(bytes:&[u8])->H256{use sha2::Digest;sha2::Sha256::digest(bytes).into()}
}

pub mod user_phase;

// Byte-identical accepted LP3 identity and JSON primitives; no boot authorization.
#[path="p1_profile.rs"] #[allow(dead_code)] pub mod profile;
#[path="p1_genesis_spec.rs"] #[allow(dead_code)] pub mod genesis_spec;
#[path="p1_genesis.rs"] #[allow(dead_code)] pub mod genesis;
#[path="p1_decimal.rs"] #[allow(dead_code)] pub mod decimal;
#[path="p1_json.rs"] #[allow(dead_code)] pub mod json;

pub mod code_caps;
#[cfg(test)] mod code_caps_engine_tests;

// No public SystemCall/block-admission API until validated phase integration.
#[allow(dead_code)] pub(crate) mod system;

// P3 local pool and loopback RPC (merged from codex-lp4-p3-wire).
pub mod pool;
pub mod rpc_pool;
pub mod rpc_http;

#[allow(dead_code)] pub(crate) mod genesis_builder;

#[allow(dead_code)] pub(crate) mod finality_core;

// Full block apply, devnet profiles and transaction signing (local devnet only).
pub mod apply;
// C-1 E1: the state root kept incrementally (world_root stays the reference).
pub mod state_trie;
pub mod node_trie;
// C-1 E2: the per-block whole-state checks from a state summary.
pub mod state_summary;
// C-1 E3: the executor's state, whole or as an overlay over a parent.
pub mod view;
pub mod devnet;
pub mod txsign;
pub mod resources;

// A02 boot admission: the accepted LP3 ParamGate evaluator and its arithmetic,
// shared byte-for-byte with lp1-node (not copied), and the fact assembly around it.
#[path="../../lp1-node/src/timing.rs"] #[allow(dead_code)] pub mod timing;
#[path="../../lp1-node/src/param_gate.rs"] #[allow(dead_code)] pub mod param_gate;
#[path="../../lp1-node/src/gate.rs"] #[allow(dead_code)] pub mod gate;
pub mod admission;
