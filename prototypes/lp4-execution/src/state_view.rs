//! The state an engine reads, borrowed instead of cloned (decision package C-2, item 3).
//! Complete local state backend only (InMemoryDB over EmptyDB), as collision and code_caps require.
use std::convert::Infallible;
use revm::{Database, DatabaseRef, InMemoryDB};
use revm::primitives::{AccountInfo, Address, Bytecode, B256, U256};
use crate::view::View;

/// A pre-transaction (or pre-call) state, read through `DatabaseRef`. revm's `transact()` and the
/// SYSTEM frames write only to their journaled state, so the engine never needs its own copy. The reads
/// are those of a `CacheDB` over `EmptyDB` (an absent or NotExisting account is `None`, a missing slot
/// reads zero, missing code is empty, BLOCKHASH falls back to EmptyDB's), through `view::View`, so the
/// state may be a whole `InMemoryDB` or an overlay over one (C-1 E3).
///
/// `code_total` is the state's logical code total (`code_caps::inventory`) when the caller already
/// knows it, so the CODE_CAP check of a create does not rescan the whole state.
pub struct StateRef<'a> { pub(crate) db: &'a dyn View, pub(crate) code_total: Option<u64> }
impl<'a> StateRef<'a> {
    pub(crate) fn new(db: &'a dyn View, code_total: Option<u64>) -> Self { Self { db, code_total } }
}
impl DatabaseRef for StateRef<'_> {
    type Error = Infallible;
    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Infallible> { Ok(self.db.account(&address).and_then(|a| a.info())) }
    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Infallible> { Ok(self.db.contract(&code_hash).unwrap_or_default()) }
    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Infallible> { Ok(self.db.slot(&address, &index)) }
    fn block_hash_ref(&self, number: u64) -> Result<B256, Infallible> { Ok(self.db.block_hash(number)) }
}
impl Database for StateRef<'_> {
    type Error = Infallible;
    fn basic(&mut self, address: Address) -> Result<Option<AccountInfo>, Infallible> { self.basic_ref(address) }
    fn code_by_hash(&mut self, code_hash: B256) -> Result<Bytecode, Infallible> { self.code_by_hash_ref(code_hash) }
    fn storage(&mut self, address: Address, index: U256) -> Result<U256, Infallible> { self.storage_ref(address, index) }
    fn block_hash(&mut self, number: u64) -> Result<B256, Infallible> { self.block_hash_ref(number) }
}

/// An engine database whose complete base state can be inspected (collision, code_caps, PAY).
pub trait LocalState {
    fn local(&self) -> &dyn View;
    /// The base state's logical code total, if known.
    fn code_total(&self) -> Option<u64> { None }
}
impl LocalState for InMemoryDB { fn local(&self) -> &dyn View { self } }
impl LocalState for StateRef<'_> {
    fn local(&self) -> &dyn View { self.db }
    fn code_total(&self) -> Option<u64> { self.code_total }
}
