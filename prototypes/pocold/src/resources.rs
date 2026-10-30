//! A13: conservative resident-memory bound of a pocold node for an admitted profile and its
//! local limits, derived from the actual representation and the enforced caps. Every term
//! names the code that enforces its assumption; arithmetic is checked (an overflow is a
//! rejection, never a wrap). `Node::open` refuses to boot when the bound exceeds the
//! node's memory budget. Derivation and open obligations: development/audit-2026-10-22/
//! A13-DERIVATION.md.
use std::mem::size_of;
use pocol_lp4_execution::{profile::ConsensusParams,resources::memory,user_phase::caps};
use crate::{chain::{self,Entry},header::Header,net,node::{self,Limits},types::{Block,Body}};

/// Full working copies alive besides the retained ones (C-1 E3; 10 before E3). A block, production,
/// template selection, RPC simulation and the pool run on overlays (lp4 `view::Overlay`, `view::Layer`)
/// over a state they share; the engine reads through `StateRef` and PAY through the same view
/// (decision package C-2). What remains, one task at a time on the consensus thread:
/// - a block applied: its child state, merged once at the end (one copy);
/// - a rebuild's or restore's rolling re-execution: the state it holds and the next block's merged state
///   (two);
/// - after a new head, until `rebuild_pool`: the pool's base, the previous head's state, which the chain
///   no longer holds full (one; `restore_canonical` runs before `settle`, so not during a rolling
///   re-execution).
///
/// At most two.
pub const TRANSIENT_SNAPSHOTS:u128=2;
/// The cheapest load that leaves a slot behind: revm 19.7 inserts a slot into the journaled state before it
/// charges the load (revm-interpreter host.rs `sload`, journaled_state.rs `sload`), a frame that then runs out
/// of gas keeps it (a revert only marks it cold), and the frame costs at least a warm CALL (memory::classes,
/// "slot loaded unpaid").
pub const SLOT_LOAD_GAS_MIN:u128=100;
/// The cheapest creation (CREATE, 32,000 gas).
pub const CREATE_GAS:u128=32_000;
/// C-1 E3, D-7 (A13-DERIVATION §2d, the audit of what an execution keeps): the entries an overlay or the
/// pool's layer (`view::Layer`) holds after executions that spent at most `gas`, as (accounts, slots).
/// `Overlay::commit` takes only touched accounts, and a touch is paid: a call or a transfer to a cold account
/// (2,600), or to a warm one not loaded before (an access-list entry, 2,400 paid, the precompiles, the
/// coinbase), a creation, a self-destruct's beneficiary; a touch in a frame that reverts is undone
/// (journaled_state.rs, AccountTouched). So three per transaction (sender, recipient, fee), one per
/// ACCOUNT_TOUCH_GAS_MIN and the warm addresses. Slots: each changed slot (one first write each,
/// SLOT_WRITE_GAS_MIN; a write in a frame that reverts is undone), and every slot an account created in the
/// same transaction loaded, which the commit keeps whole, zero or not: one per SLOT_LOAD_GAS_MIN (confirmed by a
/// witness kept outside the repository; its log is in evidence/d7-bound).
pub fn layer_writes(gas:u128)->Option<(u128,u128)>{
    Some((add(&[mul(3,gas/TX_GAS_MIN)?,gas/ACCOUNT_TOUCH_GAS_MIN,WARM_ADDRESSES])?,add(&[gas/SLOT_WRITE_GAS_MIN,gas/SLOT_LOAD_GAS_MIN])?))
}
/// Bytes of a layer with these entries: each account entry, each slot entry (spread over the accounts'
/// storage tables, a table's own share for each), the cleared, deleted and retained address sets and the
/// layer's own tables, every element at `memory::entry_bytes` (hash-table growth at its worst).
pub fn layer_bytes(accounts:u128,slots:u128)->Option<u128>{
    use revm::{db::DbAccount,primitives::{Address,U256}};use memory::{entry_bytes,table_bytes};
    let (acct,slot,addr)=(size_of::<(Address,DbAccount)>() as u128,size_of::<(U256,U256)>() as u128,size_of::<Address>() as u128);
    add(&[mul(accounts,entry_bytes(acct))?,mul(slots,entry_bytes(slot))?,mul(accounts.min(slots),table_bytes(slot))?,
        mul(mul(accounts,3)?,entry_bytes(addr))?,mul(2,table_bytes(acct))?,mul(3,table_bytes(addr))?])
}
/// C-1 E3: the overlay of the block being applied (one at a time on the consensus thread, like a template's
/// selection, which runs at most GAS_LIMIT): the layer of the USER phase's gas (`block_writes`' 5/4 GAS_LIMIT)
/// with the SYSTEM phase's accounts and keys (SYS_ACCT_BLOCK_MAX, SYS_KEYS_BLOCK_MAX) and the escrow's
/// issuance. Before D-7's audit this counted the changed slots only (one per 2,900 gas) and each element at
/// twice its inline size.
pub fn overlay_max(cp:&ConsensusParams)->Option<u128>{
    let spent=u128::from(cp.GAS_LIMIT).checked_mul(5)?/4;let (a,s)=layer_writes(spent)?;
    layer_bytes(add(&[a,u128::from(cp.SYS_ACCT_BLOCK_MAX),1])?,add(&[s,u128::from(cp.SYS_KEYS_BLOCK_MAX)])?)
}
/// D-7: the gas the pool's layer can hold: its bound (`node::pool_gas_bound`), and at most POOL_MAX_TXS
/// transactions of GAS_LIMIT each (the count bound binds first above it).
pub fn pool_gas(cp:&ConsensusParams,limits:&Limits)->Option<u128>{
    let g=u128::from(node::pool_gas_bound(cp,limits).ok()?);Some(g.min(mul(node::POOL_MAX_TXS as u128,u128::from(cp.GAS_LIMIT))?))
}
/// Gas per byte of deployed runtime (EIP-170 code deposit, 200): an execution of g gas deploys at
/// most g/200 bytes.
pub const CODE_DEPOSIT:u128=200;
/// glibc keeps up to 40 MiB of exited threads' stacks for reuse (the nptl stack cache);
/// counted on every platform.
pub const STACK_CACHE:u128=40<<20;
/// Heap of one thread outside its stack (std's Thread, its packet and name).
pub const THREAD_HEAP:u128=4096;
/// The main thread only collects the arguments, spawns the consensus thread and joins it
/// (main.rs). Its stack is reserved by the platform (RLIMIT_STACK, the PE header), not by
/// pocold, so it is counted at 1 MiB by that code structure (A13-DERIVATION §4, residual).
pub const MAIN_STACK_RESIDENT:u128=1<<20;
/// EVM executions alive at once: blocks, pool re-execution, eth_call/eth_estimateGas and the
/// admission timing all run on the consensus thread, one at a time.
pub const EXECUTIONS:u128=1;
const _:()=assert!(crate::rpc::CALL_GAS_MAX<=pocol_lp4_execution::apply::PER_CALL_GAS,"eth_call gas exceeds the execution model");
/// Threads alive at once, every one created by `threads::spawn` with its reservation: per
/// inbound peer a reader and a writer (`net::spawn_peer`: the reader joins the writer and
/// releases the slot as its last action); per configured peer a dialer, an event forwarder,
/// a reader and a writer; conn_max RPC connection threads; the miner (joined before it is
/// replaced), the miner and listener forwarders, the timer, the P2P listener and the RPC
/// acceptor (6). Plus the consensus and main threads. Exiting threads are counted: the RPC
/// acceptor, the P2P listener and each dialer join a finished thread before its place is used
/// again (threads::reap), and a reader joins its writer.
pub fn io_threads(peers_max:usize,dials:usize,conn_max:usize)->Option<u128>{add(&[mul(2,peers_max as u128)?,mul(4,dials as u128)?,conn_max as u128,6])}
/// Smallest valid transaction encoding (a signed legacy transfer exceeds it).
pub const MIN_TX_BYTES:u128=85;
/// Intrinsic gas of every transaction. A block's transactions reserve their gas limits against
/// GAS_LIMIT and use at least this each (user_phase: the reservation check and `used <= limit`), so a
/// valid block holds at most GAS_LIMIT / TX_GAS_MIN transactions.
pub const TX_GAS_MIN:u128=21_000;
/// C-1 S3a: full states materialized from a full one through the diffs, alive at once: the parent of a
/// block being validated or an RPC tag's state (one at a time on the consensus thread), and one that
/// `ensure_state_with` keeps full until the next settle.
pub const MATERIALIZED:u128=2;
/// C-1 S3a: diffs held at once. Canonical blocks above the K_hist cut (K_hist), side states within
/// side_max, the block being imported, and during an adopting import the abandoned branch's diffs until
/// `settle` (K_hist more).
pub fn diffs_held(k_hist:u32,side_max:u32)->u128{2*u128::from(k_hist)+u128::from(side_max)+1}
/// The cheapest first write of a distinct storage slot in a transaction (EIP-2200/2929 SSTORE reset of a
/// warm slot: 2,900 gas); a slot changed between a parent's state and its child's had one in some
/// transaction of the block.
pub const SLOT_WRITE_GAS_MIN:u128=2_900;
/// The cheapest way for a transaction to add an account to the committed state: a warm touch costs 100
/// after an access-list entry of 2,400 (EIP-2930), counted at 2,400.
pub const ACCOUNT_TOUCH_GAS_MIN:u128=2_400;
/// Precompile and system addresses a block can touch at 100 gas each (warm by EIP-2929): counted at once.
pub const WARM_ADDRESSES:u128=32;
/// C-1 S3a: the largest diff of one block. USER gas is at most GAS_LIMIT and refunds return at most a fifth
/// of it (EIP-3529), so at most 5/4 GAS_LIMIT was spent. Changed slots: one first write each
/// (SLOT_WRITE_GAS_MIN) plus the SYSTEM phase's SYS_KEYS_BLOCK_MAX (apply.rs enforces it). Changed
/// accounts: three per transaction (sender, recipient, fee), one per ACCOUNT_TOUCH_GAS_MIN, the warm
/// addresses, the SYSTEM phase's SYS_ACCT_BLOCK_MAX and the escrow's issuance. A removed account keeps no
/// earlier storage in a diff: under Cancun only an account created in the same transaction is destroyed,
/// and an empty account cannot hold storage (condition `s3a-removal`). Contracts: at most one per changed
/// account. Block hashes: the 256-block window moves by one (257 entries). Each element is counted at its
/// inline size with Vec growth (×2).
pub fn diff_max(cp:&ConsensusParams)->Option<u128>{
    use std::mem::size_of;use crate::statediff::{AccountDiff,StateDiff};use revm::primitives::{Address,Bytecode,B256,U256};
    let (accounts,slots)=block_writes(cp)?;
    let vec2=|n:u128,sz:usize|mul(n,2*sz as u128);
    add(&[vec2(accounts,size_of::<(Address,AccountDiff)>())?,vec2(slots,size_of::<(U256,Option<U256>,Option<U256>)>())?,
        vec2(accounts,size_of::<(B256,Option<Bytecode>,Option<Bytecode>)>())?,vec2(257,size_of::<(U256,Option<B256>,Option<B256>)>())?,size_of::<StateDiff>() as u128+16])
}
/// The accounts and slots one block can change (diff_max's counts): (accounts, slots).
pub fn block_writes(cp:&ConsensusParams)->Option<(u128,u128)>{
    let spent=u128::from(cp.GAS_LIMIT).checked_mul(5)?/4;
    let slots=add(&[spent/SLOT_WRITE_GAS_MIN,u128::from(cp.SYS_KEYS_BLOCK_MAX)])?;
    let accounts=add(&[3*(spent/TX_GAS_MIN),spent/ACCOUNT_TOUCH_GAS_MIN,WARM_ADDRESSES,u128::from(cp.SYS_ACCT_BLOCK_MAX),1])?;
    Some((accounts,slots))
}
/// C-1 E1: one state trie (lp4 `state_trie`) of a state with these accounts and nonzero slots: a leaf per
/// account and per slot; every branch has at least two children, so a trie of n keys has at most n − 1
/// branches and 2n − 2 child pointers. A world node is a `Node<Acct>`, a storage node a `Node<U256>`, each
/// behind an Arc (`state_trie::node_bytes`); a child pointer is one word.
pub fn trie_bytes(accounts:u128,slots:u128)->Option<u128>{
    use pocol_lp4_execution::state_trie::node_bytes;
    add(&[mul(2*accounts,u128::from(node_bytes(true)))?,mul(2*slots,u128::from(node_bytes(false)))?,mul(2*(accounts+slots),size_of::<usize>() as u128)?])
}
/// C-1 E1: nodes on the path from a trie's root to a leaf, at most: a branch at each of the 64 nibbles of
/// a hashed key, and the leaf.
pub const TRIE_PATH_NODES:u128=65;
/// C-1 E1: the nodes a block's trie does not share with its parent's: the copied path of every changed
/// slot (in its account's storage trie) and of every changed account (in the world trie). An account whose
/// storage changes is one of the block's changed accounts (`block_writes`).
pub fn trie_paths(cp:&ConsensusParams)->Option<u128>{
    use pocol_lp4_execution::state_trie::node_bytes;let (accounts,slots)=block_writes(cp)?;let w=size_of::<usize>() as u128;
    add(&[mul(mul(slots,TRIE_PATH_NODES)?,u128::from(node_bytes(false))+w)?,mul(mul(accounts,TRIE_PATH_NODES)?,u128::from(node_bytes(true))+w)?])
}
/// C-1 E4 (store mode): a block's batch (`node_trie::Batch`, the new nodes of its child version until
/// `Pinned::new` commits them) is alive one at a time: validation commits it in `child_state` and a
/// rolling re-execution in `rolled` before the next block runs; production keeps no execution output
/// (`node::Production`). An inventory by inspection (CONDITIONS, `transient-snapshot-inventory`).
pub const BATCHES:u128=1;
/// C-1 E4: one batch at most (`node_trie::batch_nodes` for the block's changed accounts and slots, in a
/// child of at most `state_keys`): its branches and leaves, each decoded behind an `Arc` with its map slot
/// (a leaf holds its key's preimage, C-1 E5), every branch's children (`Node::resident_bytes`), and the
/// block's write set (`state_trie::Touched`). Its code is shared with the execution's.
pub fn batch_max(cp:&ConsensusParams)->Option<u128>{
    use pocol_lp4_execution::{node_trie::{batch_nodes,Id,Node},state_trie::Ref};use revm::primitives::{Address,U256};
    let (accounts,slots)=block_writes(cp)?;let (ka,ks)=state_keys(cp)?;let (branches,leaves,kids)=batch_nodes(accounts,slots,ka,ks)?;
    let slot=|n:usize|memory::entry_bytes(n as u128);let node=(2*size_of::<usize>()+size_of::<Node>()) as u128+slot(size_of::<(Id,std::sync::Arc<Node>)>());
    add(&[mul(branches+leaves,node)?,mul(kids,size_of::<(Id,Ref)>() as u128)?,
        mul(accounts,slot(size_of::<(Address,Option<Vec<U256>>)>()))?,mul(slots,memory::VEC*size_of::<U256>() as u128)?])
}
/// The keys of a state within the caps: (accounts, slots). Accounts: ACCT_CAP user accounts and the 5
/// programs; slots: SLOT_CAP and SYS_SLOT_MAX (as `derive`'s snapshot).
pub fn state_keys(cp:&ConsensusParams)->Option<(u128,u128)>{Some((u128::from(cp.ACCT_CAP).checked_add(5)?,u128::from(cp.SLOT_CAP).checked_add(u128::from(cp.SYS_SLOT_MAX))?))}
/// C-1 E4: the stored versions with a handle at once: every version `held_runs` lists, each with its own handle.
/// The pool's base and production share an entry's handle. (Review 2026-10-27: this count once listed the runs
/// apart from `held_versions_bytes` and left out the block being imported and the rolling re-execution's next
/// version; both now read `held_runs`.)
pub fn stored_versions(k_hist:u32,side_max:u32)->u128{held_runs(k_hist,side_max).iter().sum()}
/// One stored version's handle (`chain::Pinned` behind its `Arc`) and its BLOCKHASH window (257 entries).
pub fn pinned_bytes()->u128{size_of::<chain::Pinned>() as u128+16+257*memory::entry_bytes(64)+memory::table_bytes(64)}
/// C-1 E4.4: a checkpoint being written: its bytes (every entry at most `checkpoint::REC_MAX`, the
/// unslimmed list and the eviction FIFO), the index of entries it sorts, and redb's copy of the value in
/// its dirty pages.
pub fn checkpoint_bytes(cp:&ConsensusParams,limits:&Limits)->Option<u128>{
    let entries=add(&[u128::from(cp.H_END)+1,limits.side_cap as u128,mul(2,u128::from(limits.r_keep))?])?;
    let bytes=add(&[256,mul(entries,crate::checkpoint::REC_MAX as u128)?,mul(u128::from(cp.H_END)+1+chain::EVICTED_MAX as u128,32)?])?;
    add(&[mul(2,bytes)?,mul(entries,size_of::<usize>() as u128)?])
}
/// C-1 E4.5: redb 2.6.4's usable bytes per region (page_manager.rs MAX_USABLE_REGION_SPACE) and its pages
/// (4 KiB, the default page size).
pub const REDB_REGION_BYTES:u128=4<<30;
pub const REDB_REGION_PAGES:u128=REDB_REGION_BYTES/4096;
/// The allocator redb 2.6.4 keeps in memory for one region of `pages` pages (buddy_allocator.rs
/// `BuddyAllocator::new`, bitmap.rs): for every order 0..=min(20, log2 pages), with the page count halved
/// at each order, a free `BtreeBitmap` (64-way levels of u64 words down to one word, each level a
/// `U64GroupedBitmap`) and an allocated `U64GroupedBitmap` (one bit per page of the order), every bitmap
/// its words and its header, and the allocator's two Vecs.
pub fn redb_region_allocator(pages:u128)->Option<u128>{
    let words=|c:u128|c.div_ceil(64);let grouped=|c:u128|words(c).checked_mul(8).and_then(|w|w.checked_add(32));
    let max_order=(127-pages.max(1).leading_zeros()).min(20);let (mut c,mut b)=(pages,48u128);
    for _ in 0..=max_order{
        let mut cap=c;loop{b=b.checked_add(grouped(cap)?)?;if cap<=64{break}cap=cap.div_ceil(64);}
        b=b.checked_add(grouped(c)?)?;b=b.checked_add(24+24)?;c/=2;
    }
    Some(b)
}
/// redb's allocator over a file of `bytes`: every region it spans is allocated whole (a partly used last
/// region too), with its region-tracker entry (counted at 64 bytes).
pub fn redb_allocator_bytes(bytes:u128)->Option<u128>{mul(bytes.div_ceil(REDB_REGION_BYTES).max(1),redb_region_allocator(REDB_REGION_PAGES)?.checked_add(64)?)}
/// C-1 E4.5: the largest state.redb file, for its allocator term (A13 counts the file's allocator state
/// at one byte per 4 KiB page, as for history.redb; the file itself is disk, not memory). Nodes: the versions
/// the entries hold and those each kept checkpoint pins (`held_versions_bytes`). Code is kept for good
/// (storage.md): the code every block to H_END can deploy. (C-1 E5: a leaf keeps its key's preimage,
/// released with it; no preimage is kept for good.) The checkpoints' snapshots. All at STATE_FILE_FACTOR
/// times their bytes.
pub fn state_file_max(cp:&ConsensusParams,k_hist:u32,side_max:u32,limits:&Limits)->Option<u128>{let p=state_file_parts(cp,k_hist,side_max,limits)?;add(&p)}
/// `state_file_parts`' versions: the bytes state.redb keeps for `n` versions at consecutive heights of one
/// chain. A node is stored once however many versions name it (`state_store::commit` counts one more
/// reference to a stored node), so they keep the oldest version's nodes and what each later block's version
/// added (`node_trie::block_bytes`); and at most n whole versions (`node_trie::version_bytes`).
pub fn chain_versions_bytes(cp:&ConsensusParams,n:u128)->Option<u128>{
    use pocol_lp4_execution::node_trie::{block_bytes,version_bytes};
    if n==0{return Some(0)}
    let (ka,ks)=state_keys(cp)?;let (wa,ws)=block_writes(cp)?;let whole=version_bytes(ka,ks)?;
    Some(mul(n,whole)?.min(add(&[whole,mul(n-1,block_bytes(wa,ws,ka,ks)?)?])?))
}
/// The versions held at once (C-1 E4, the `stored_versions` inventory), as runs of consecutive versions:
/// genesis; the K_hist canonical window with the block being imported (K_hist + 1); during an adopting import,
/// until `settle`, the abandoned window (K_hist, on the old branch); side_max side states (one each); a rolling
/// re-execution's version with the next one it commits (2).
pub fn held_runs(k_hist:u32,side_max:u32)->Vec<u128>{
    let k=u128::from(k_hist);let mut v=vec![1,k+1,k];v.extend(std::iter::repeat_n(1,side_max as usize));v.push(2);v
}
/// The bytes of the versions `held_runs` lists, each run `chain_versions_bytes`. A kept checkpoint pins
/// versions its entries held, at most as many again.
pub fn held_versions_bytes(cp:&ConsensusParams,k_hist:u32,side_max:u32)->Option<u128>{
    held_runs(k_hist,side_max).into_iter().try_fold(0u128,|t,n|add(&[t,chain_versions_bytes(cp,n)?]))
}
/// `state_file_max`'s parts: (versions' nodes, key preimages, code, checkpoint snapshots), each at
/// STATE_FILE_FACTOR times its bytes. The entries' versions and the KEPT checkpoints' are counted apart,
/// and so is one dropped checkpoint whose nodes are still owed (D-6, 2026-10-27: `StateStore::release_step`
/// works them off between events; a checkpoint's write finishes what is left first, so at most one is owed),
/// with a `release` row (ROW bytes) for at most each of its nodes. Each node counted here is at least ROW +
/// BRANCH_ENC bytes (a branch without its children, the smallest node), so a held set has at most
/// bytes / (ROW + BRANCH_ENC) nodes.
pub fn state_file_parts(cp:&ConsensusParams,k_hist:u32,side_max:u32,limits:&Limits)->Option<[u128;4]>{
    use pocol_lp4_execution::node_trie::{BRANCH_ENC,ROW};
    let kept=crate::checkpoint::KEPT as u128;let held=held_versions_bytes(cp,k_hist,side_max)?;
    let owed_rows=mul(ROW,(held/(ROW+BRANCH_ENC)).checked_add(1)?)?;
    let nodes=add(&[mul(kept+2,held)?,owed_rows])?;
    let blocks=u128::from(cp.H_END)+1;
    let preimages=0;
    let code=mul(blocks,memory::execution_gas(cp.GAS_LIMIT)/CODE_DEPOSIT)?;
    let ckpts=mul(kept,checkpoint_bytes(cp,limits)?/2)?;
    Some([mul(STATE_FILE_FACTOR,nodes)?,mul(STATE_FILE_FACTOR,preimages)?,mul(STATE_FILE_FACTOR,code)?,mul(STATE_FILE_FACTOR,ckpts)?])
}
/// C-1 E5: state.redb's file size over the bytes it stores (every key and value), for its allocator term.
/// Measured (journal_measure::jm_store_mode_disk_impact, evidence/c1-e4) after live import, an open that
/// resumes from a checkpoint and an open that replays from genesis, and after each close: at most 6.07
/// with 14.9 MB stored, at most 11.31 with 1.5 to 2.8 MB stored (redb doubles a file below one region,
/// so a small file carries more slack; above one region it grows a region at a time); redb's allocated
/// pages at most 2.38 times the stored bytes. Counted at 12, above every measurement. Below one region
/// (REDB_REGION_BYTES) the term is one region's allocator whatever the factor.
pub const STATE_FILE_FACTOR:u128=12;
/// Receipt encoding without logs: status, cumulative gas, a 256-byte bloom, list headers.
pub const RECEIPT_BASE:u128=300;
/// C-1 S1: history.redb's file size over the bytes it stores (receipts records, index entries, keys),
/// for its allocator term. Measured at most 4.1 (journal_measure::history_integrity_check_cost: redb
/// grows the file by regions and rounds values to page orders); counted at 8.
pub const HISTORY_FILE_FACTOR:u128=8;
/// Longest stored rejection reason (chain::REASON_MAX) and status rejects kept (node).
pub const REASON_MAX:u128=chain::REASON_MAX as u128;

/// Largest encoded header for S_max shares: fixed fields <= 768 bytes, 9 per share nonce.
pub fn header_max(s_max:u16)->u128{768+9*u128::from(s_max)}

/// What the bound does not establish (A13-DERIVATION §4). The total is the sum of requested
/// heap bytes and reserved stacks under these conditions, not an unconditional bound on the
/// resident memory of the process.
pub const CONDITIONS:&[(&str,&str)]=&[
    ("allocator-overhead","allocator per-allocation overhead and fragmentation are not modelled"),
    ("transient-snapshot-inventory","TRANSIENT_SNAPSHOTS (2, C-1 E3) and EXECUTIONS (1) are inventories by code inspection, not enforced by a counter"),
    ("main-thread","the main thread's stack is reserved by the platform; counted at 1 MiB because it only spawns and joins"),
    ("revm-representation","the execution term relies on revm 19.7 behaviour checked by source inspection (iterative frames, SharedMemory truncation, fixed stack capacity) and by measured adversarial peaks"),
    ("cpu-and-timing","CPU costs (world-root rebuild, chain scans, pool re-execution, deep-fork rebuild) are not memory terms; worst-case timing is not bounded"),
    ("s3a-removal","a diff keeps no storage for a removed account: under Cancun (revm 19.7, EIP-6780) only an account created in the same transaction is destroyed, and an account without code, nonce and balance holds no storage; by inspection, not enforced by a counter"),
    ("redb-representation","the history.redb terms rely on redb 2.6.4 behaviour checked by source inspection (cache split and write-buffer flush, allocator bitmaps) and on a measured file-size factor (HISTORY_FILE_FACTOR)"),
    ("open-replay-transient","Node::open holds the whole journal's payloads (store::open, pre-existing) and a replay index of two entries per record while replaying; this open-time transient grows with the journal and is not part of the steady-state bound. C-1 E4.4 adds to it, at open only: every record's offset and rolling hash, a decoded checkpoint snapshot and the stored versions read for their summaries (streamed: the oldest whole, a node at a time with the DFS stack of node ids; each other through its diff from its parent's version, which holds the accounts and slots that differ, one block's writes at most, as an import's write set, the recursion's path through the two world tries and through the two storage tries of one account, and the storage walk's DFS stack of node ids)"),
    ("store-inventory","store mode (C-1 E4): BATCHES (1) and stored_versions are inventories by code inspection, not enforced by a counter; the node cache is bounded by its resident-byte count (state_store::CACHE_ENTRY, Node::resident_bytes), checked by a test"),
    ("revm-loading","the pool layer, overlay and store-mode code terms rely on revm 19.7 behaviour checked by source inspection (A13-DERIVATION §2d): a touch needs paid gas or a warm address, a touch or write in a reverted frame is undone, a loaded account or slot stays until its transaction ends; that a created account keeps the slots it loaded in out-of-gas frames is also tested"),
    ("store-file-factor","the state.redb allocator term takes the largest file as STATE_FILE_FACTOR times the stored bytes: a factor measured on stores of 1.5 to 15 MB (evidence/c1-e4 disk measurements), not established for the terabyte files of the adversarial case (every version a whole state on disk)"),
];
/// The model total and its parts. `items` itemizes every group term ("group/item", bytes); the
/// group fields are the sums of their items and `total` is the sum of all items.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Bound{
    /// Whole states counted (`snapshots` of `snapshot_bytes` each) and the snapshot group's total (with
    /// the table growth of a working copy). C-1 E4: `store` is the node store's terms (store mode).
    pub snapshots:u128,pub snapshot_bytes:u128,pub snapshot_total:u128,pub diffs:u128,pub trie:u128,pub code:u128,pub store:u128,pub history:u128,pub side_blocks:u128,
    pub queues:u128,pub rpc:u128,pub threads:u128,pub execution:u128,pub total:u128,
    pub items:Vec<(&'static str,u128)>,
}
fn add(v:&[u128])->Option<u128>{v.iter().try_fold(0u128,|a,b|a.checked_add(*b))}
fn mul(a:u128,b:u128)->Option<u128>{a.checked_mul(b)}
/// HashSet<[u8;32]> at a high-water live-key count, including tombstone-driven growth.
/// Rust 1.99.1 vendors hashbrown 0.17.1: reserve_rehash_inner rehashes in place only
/// at <= half full capacity. Otherwise it may double despite deleted slots. Thus
/// <32/7 buckets per high-water key, 33 bytes/bucket, rounded to 152 B/key;
/// allow two whole allocations simultaneously plus control/alignment slack.
/// Requested allocations, not a proof of allocator RSS or the complete A13 model.
pub fn hash32_churn_bytes(high:u128)->Option<u128>{add(&[mul(high,304)?,512])}
/// The old FIFO plus a retention batch coexist before trim. A batch is bounded by
/// side_cap + 2*r_keep protected + r_keep abandoned + one import. Capacities persist.
/// VecDeque: <64 B/key retained; allow two allocations during growth, plus slack.
pub fn eviction_bookkeeping_bytes(side_cap:u128,r_keep:u128)->Option<u128>{
    let high=add(&[chain::EVICTED_MAX as u128,side_cap,mul(3,r_keep)?,1])?;
    add(&[hash32_churn_bytes(high)?,mul(high,128)?,512])
}

/// The bound, or None on arithmetic overflow.
pub fn derive(cp:&ConsensusParams,k_hist:u32,side_max:u32,limits:&Limits,dials:usize)->Option<Bound>{
    // A continuous chain has no finite lifetime bound. Project from observed journal records
    // plus 2048 records of headroom, and recheck locally as the journal grows. Never substitute
    // zero as the history size or claim that this projection bounds an infinite history.
    let mut projected=cp.clone();
    let continuous=cp.H_END==0;
    if continuous{
        let records=limits.continuous_history?.checked_add(2048)?;
        projected.H_END=records;
        projected.TX_CAP=cp.TX_CAP.min(cp.GAS_LIMIT/21_000).checked_mul(records.checked_add(1)?)?;
        projected.TXBYTES_CAP=cp.TXBYTES_CAP.min(u64::from(cp.BODY_MAX)).checked_mul(records.checked_add(1)?)?;
    }
    let cp=&projected;
    let log_cap=if continuous{u128::from(caps::LOG_BLOCK_MAX).checked_mul(u128::from(cp.H_END)+1)?}else{u128::from(caps::LOG_CAP)};
    let s=u128::from(cp.S_max);let m=u128::from(cp.M_max);let hdr=header_max(cp.S_max);
    // Snapshot: accounts <= ACCT_CAP user accounts (genesis check_caps, USER halts AcctCap)
    // + 5 programs; slots <= SLOT_CAP (USER halts SlotCap) + SYS_SLOT_MAX (post-block check);
    // code entries <= accounts + 2 defaults; containers normalized after every commit.
    let accounts=u128::from(cp.ACCT_CAP).checked_add(5)?;let slots=u128::from(cp.SLOT_CAP).checked_add(u128::from(cp.SYS_SLOT_MAX))?;
    let snapshot_bytes=memory::snapshot(accounts,slots,accounts.checked_add(2)?)?;
    // C-1 S3a: full states are the head, genesis, the side states within side_max, and the materialized
    // ones (MATERIALIZED); every other retained state is a diff (below). Before S3a: 1 + K_hist + side_max.
    let (snapshots,mut items)=if limits.state_store{
    // C-1 E4 (store mode, `Limits::state_store`): every retained state is a version pinned in state.redb,
    // read in place through hash-checked nodes (chain::Pinned, state_store): no whole state, diff or E1
    // trie is held (the checkpoint encoder refuses an entry that holds one, checkpoint::rec_of). What is in
    // memory is the caches, the work in progress and the handles.
    let code_set=memory::code_bytes(u128::from(cp.CODE_CAP),0)?;
    let exec_code=memory::code_bytes(memory::execution_gas(cp.GAS_LIMIT)/CODE_DEPOSIT,0)?;
    // D-7: the pool's layer holds what its gas bound can write (no whole state is held in store mode). An
    // entry keeps its account's code hash, not the copy of its code the account was read with
    // (`Layer::drop_code_copies`, node::admit_tx); its own code is its creations (CODE_DEPOSIT, CREATE_GAS).
    // The copies a transaction reads while it is admitted are the execution's (the set below).
    let pg=pool_gas(cp,limits)?;let (pa,ps)=layer_writes(pg)?;
    let pool_code=memory::code_bytes(pg/CODE_DEPOSIT,pg/CREATE_GAS)?;
    let snapshots=0;
    let items:Vec<(&'static str,u128)>=vec![
        ("diffs/the pool's pending layer (D-7: what its gas bound can write)",layer_bytes(pa,ps)?),
        ("diffs/the overlay of the block being applied (C-1 E3)",overlay_max(cp)?),
        ("store/a block's new trie nodes before their commit (BATCHES batches)",mul(BATCHES,batch_max(cp)?)?),
        ("store/decoded-node cache (NODE_CACHE, resident bytes)",crate::state_store::NODE_CACHE as u128),
        ("store/state.redb page cache (STATE_PAGE_CACHE)",crate::state_store::STATE_PAGE_CACHE as u128),
        // redb 2.6.4 keeps every region's allocator in memory, sized for the region's whole capacity
        // (redb_allocator_bytes), over the largest file (state_file_max).
        ("store/state.redb allocator state (per-region bitmaps over the largest file)",redb_allocator_bytes(state_file_max(cp,k_hist,side_max,limits)?)?),
        ("store/stored versions' handles and BLOCKHASH windows",mul(stored_versions(k_hist,side_max),pinned_bytes())?),
        ("store/a checkpoint being written (its bytes and its entry index)",checkpoint_bytes(cp,limits)?),
        // Each account an execution loads brings its code from the store (TrieState::info), whether or not
        // the execution runs or reads that code: a copy per loaded account, held until the transaction
        // ends and, for a touched account, in the overlay of the block or template being built. revm 19.7
        // loads an account before it charges for it, so a load costs as little as a warm CALL (100 gas):
        // the gas does not bound these copies at the network profile (A13-DERIVATION §2d). They are at most
        // the logical code of every existing account (code_caps, CODE_CAP) with its analysis. One execution
        // and one overlay at a time (EXECUTIONS).
        ("code/read from the store by the execution in progress and its overlay (a set)",mul(EXECUTIONS,code_set)?),
        ("code/the pool's pending layer (D-7: its creations)",pool_code),
        ("code/new runtime of the execution in progress",mul(EXECUTIONS,exec_code)?)];
    (snapshots,items)}else{
    let snapshots=add(&[1,1,u128::from(side_max),MATERIALIZED,TRANSIENT_SNAPSHOTS])?;
    // Code (decision package C-1 S2). A runtime is held in refcounted buffers (revm Bytecode: Bytes and
    // an Arc jump table) that a state copy shares (journal_measure::retained_states_of_one_lineage_
    // share_their_code_buffers). Each block's state is its parent's copy plus that block's changes, and
    // under Cancun code leaves a state only within its creating transaction. So the states of one
    // lineage hold one set of buffers, at most CODE_CAP logical bytes with analysis (code_caps).
    // A separate execution of the same code makes separate buffers. Counted as a full set each:
    // - every side-branch state (unshared at worst: rebuilt from an older state);
    // - a rebuild or restore in progress, which re-creates the code it re-executes.
    // D-7: the pool's layer shares the head's buffers for the accounts it touched (a Bytecode clone shares
    // its Bytes and jump table) and holds its own creations, at most its gas / CODE_DEPOSIT bytes. The
    // per-copy containers (map entries, Bytecode headers) are in every snapshot, and the execution in
    // progress makes at most its gas / CODE_DEPOSIT bytes of new runtime.
    let code_set=memory::code_bytes(u128::from(cp.CODE_CAP),0)?;let code_entries=memory::code_bytes(0,accounts)?;
    let exec_code=memory::code_bytes(memory::execution_gas(cp.GAS_LIMIT)/CODE_DEPOSIT,0)?;
    let pg=pool_gas(cp,limits)?;let (pa,ps)=layer_writes(pg)?;let pool_code=memory::code_bytes(pg/CODE_DEPOSIT,pg/CREATE_GAS)?;
    let side_states=u128::from(side_max);
    let items:Vec<(&'static str,u128)>=vec![("snapshots/head state",snapshot_bytes),("snapshots/genesis state (counted as a full one)",snapshot_bytes),
        ("snapshots/side_max side-branch states",mul(side_states,snapshot_bytes)?),
        ("snapshots/materialized states (C-1 S3a: a parent or an RPC tag, and one kept until settle)",mul(MATERIALIZED,snapshot_bytes)?),
        ("snapshots/transient working copies (C-1 E3: a block's merged child, a rolling re-execution's two)",mul(TRANSIENT_SNAPSHOTS,snapshot_bytes)?),
        // A table that grows in a working copy keeps its old allocation until the new one is filled:
        // one table at a time (single consensus thread), at most a whole snapshot's tables.
        ("snapshots/table growth in a working copy",snapshot_bytes),
        ("diffs/retained and side states as diffs (C-1 S3a: 2·K_hist + side_max + 1)",mul(diffs_held(k_hist,side_max),diff_max(cp)?)?),
        ("diffs/the overlay of the block being applied (C-1 E3)",overlay_max(cp)?),
        ("diffs/the pool's pending layer (D-7: what its gas bound can write)",layer_bytes(pa,ps)?),
        // C-1 E1: only the head keeps its state's trie after settle (chain::Chain::settle). A parent that is
        // not the head has its trie derived from the head's or built from its state, one at a time on the
        // consensus thread (validation, production, a rebuild's rolling re-execution); its distinct nodes are
        // counted as a whole trie. The block executed on it shares all but its changed paths.
        ("trie/head state trie (C-1 E1)",trie_bytes(accounts,slots)?),("trie/a parent's trie derived or built (one at a time)",trie_bytes(accounts,slots)?),
        ("trie/the changed paths of the block executed on it",trie_paths(cp)?),
        ("code/one set for the canonical lineage (its states share the buffers)",code_set),("code/side_max side-branch states (a set each)",mul(side_states,code_set)?),
        ("code/a rebuild or restore in progress (a set)",code_set),("code/the pool's pending layer (D-7: its creations)",pool_code),
        ("code/containers in every snapshot",mul(snapshots,code_entries)?),("code/new runtime of the execution in progress",mul(EXECUTIONS,exec_code)?)];
    (snapshots,items)};
    // Per block kept in memory: the map slot (its key and the boxed Entry, with its control byte; a table
    // that grows holds its old buckets and twice as many new ones, 24/7 buckets per entry, counted at 4), the
    // Entry and its allocation header (counted at 32), the canonical index, header heap (encoded, ut_raw,
    // bloom, proposer, sig, share nonces), ParentResult shares, nonce ranges (members <= M_max) and evidence
    // (E_max items <= EVID_BYTES_MAX); an empty state is shared (C-1 E5).
    let entry=add(&[mul(size_of::<([u8;32],Box<Entry>)>() as u128+1,4)?,size_of::<Entry>() as u128,32])?;
    // C-1 E2: and its state summary (boxed; a slimmed entry drops it).
    let per_block=add(&[entry,64,size_of::<pocol_lp4_execution::state_summary::Summary>() as u128,mul(hdr,2)?,256+20+65+64,mul(8,s)?,mul(32,s+1)?,mul(64,m)?,
        mul(u128::from(cp.E_max),24)?])?;
    // A block's transactions are bounded by its body and by its gas.
    let side_txs=(u128::from(cp.BODY_MAX)/MIN_TX_BYTES).min(u128::from(cp.GAS_LIMIT)/TX_GAS_MIN);
    let per_tx=24+24+32+RECEIPT_BASE;
    // Canonical chain (C-1 S1): <= H_END+1 blocks. The last `window` (Limits::history_window, default
    // K_hist) are resident whole; every older journaled canonical block is offloaded by Chain::retain:
    // its encoded header, body (evidence included), receipts and tx hashes are dropped and read from the
    // journal and history.redb, so its entry is the per-block term less one header and the evidence.
    // The window's content is bounded per block (BODY_MAX, E_max evidence items, side_txs receipts,
    // LOG_BLOCK_MAX log bytes encoded at most 33/32) and in total by the chain caps (TXBYTES_CAP, the
    // evidence identities, TX_CAP, LOG_CAP), so a window of the whole chain is the model before S1.
    let blocks=u128::from(cp.H_END)+1;let window=u128::from(limits.history_window.unwrap_or(u64::from(k_hist))).min(blocks);
    let offloaded_entry=per_block.checked_sub(hdr)?.checked_sub(mul(u128::from(cp.E_max),24)?)?;
    // Evidence: valid evidence tombstones its identity, and evidence against a tombstone is a host error
    // (block invalid), so the chain holds at most one item per identity ever: M_max initial records plus
    // R_max registrations per block (D86 registry rotation).
    let identities=u128::from(cp.M_max).checked_add(mul(u128::from(cp.R_max),u128::from(cp.H_END))?)?;
    let w_bodies=mul(window,u128::from(cp.BODY_MAX))?.min(u128::from(cp.TXBYTES_CAP));
    let w_evidence=mul(window,mul(u128::from(cp.E_max),u128::from(cp.EVID_BYTES_MAX))?)?.min(mul(identities,u128::from(cp.EVID_BYTES_MAX)+24)?);
    let w_receipts=mul(mul(window,side_txs)?.min(u128::from(cp.TX_CAP)),per_tx)?;
    let w_logs=mul(window,u128::from(caps::LOG_BLOCK_MAX))?.min(log_cap)/32*33;
    // history.redb (crate::history): its page cache is HISTORY_CACHE (redb splits it 9:1 between reads
    // and the write buffer, and flushes the write buffer to the file past its share). Its allocator keeps
    // per page two buddy bitmaps over the page orders (about 4 bits a page, redb 2.6.4 buddy_allocator.rs),
    // counted at one byte per 4 KiB page of the file. The file holds per block a receipts record and per
    // transaction an index entry, at most HISTORY_FILE_FACTOR times their bytes (measured below 4.2,
    // evidence/c1-s1/).
    // C-1 S4: each block's nonce ranges too (owner 20 + start 16 + end 16 bytes per member, a seal).
    let payload=add(&[mul(blocks,32+32+64)?,mul(blocks,32+32+52*m)?,mul(u128::from(cp.TX_CAP),RECEIPT_BASE+4+64+4)?,log_cap/32*33])?;
    // C-1 S4: canonical journaled entries deeper than slim_depth (default 2·R_keep + E_win + 257) keep only
    // the entry, its map slot and canonical index, and an empty state; the header's heap fields, the
    // ParentResult and the nonce ranges are restored from the journal and history.redb on demand.
    let depth=u128::from(limits.slim_depth.unwrap_or(2*limits.r_keep+u64::from(cp.E_win)+257)).min(blocks);
    // C-1 E5: an entry without a state shares one empty state (chain::empty_state), counted once below.
    let slim_entry=add(&[entry,64])?;
    let (full_old,slim_old)=(depth.saturating_sub(window),blocks-depth.max(window));
    let redb_alloc=mul(payload,HISTORY_FILE_FACTOR)?/4096;
    let hist=[("history/offloaded canonical block metadata (slim_depth − window entries)",mul(full_old,offloaded_entry)?),
        ("history/slimmed canonical block metadata (H_END+1 − slim_depth entries, C-1 S4)",mul(slim_old,slim_entry)?),
        ("history/the one empty state entries share (C-1 E5)",memory::empty_snapshot()?),("history/resident window: block entries",mul(window,per_block)?),("history/resident window: transaction bytes",w_bodies),("history/resident window: evidence",w_evidence),
        ("history/resident window: transaction hashes and receipts",w_receipts),("history/resident window: logs",w_logs),
        ("history/history.redb page cache (HISTORY_CACHE)",crate::history::HISTORY_CACHE as u128),("history/history.redb allocator state",redb_alloc)];
    let history=add(&hist.map(|x|x.1))?;items.extend(hist);
    // Side blocks (D-REORG-REDB): every journaled non-canonical block is offloaded to metadata at the
    // retain() that follows its import (Chain::retain): its encoded header and body (evidence included),
    // receipts and tx hashes are dropped, so an entry is the per-block term less one header and the
    // evidence, plus its protect-set slot. Live: <= side_cap unprotected + protect_max (2·r_keep) held
    // by a sync session. Replay: all <= side_cap + protect_max (Chain::replaying). One full block is
    // resident between its insert and that retain (or while restore_canonical re-executes it): its
    // transactions are bounded by its body and by its gas.
    let side_block=add(&[per_block,mul(u128::from(cp.E_max),u128::from(cp.EVID_BYTES_MAX))?,u128::from(cp.BODY_MAX),mul(side_txs,per_tx)?,u128::from(caps::LOG_BLOCK_MAX)/32*33])?;
    let side_entries=add(&[limits.side_cap as u128,mul(2,u128::from(limits.r_keep))?])?;
    let side_entry=per_block.checked_sub(hdr)?.checked_sub(mul(u128::from(cp.E_max),24)?)?.checked_add(304+64)?;
    // Protect filtering holds old/new sets (including deletion churn) and a Vec.
    let side_meta=add(&[mul(side_entries,side_entry)?,1024])?;let side_full=side_block;
    let evicted=eviction_bookkeeping_bytes(limits.side_cap as u128,u128::from(limits.r_keep))?;
    // Side membership also suffers deletion churn. A reorg inserts its abandoned suffix
    // before retain trims; include one newly imported entry at that high-water boundary.
    let side_set=hash32_churn_bytes(add(&[side_entries,u128::from(limits.r_keep),1])?)?;
    let side_blocks=add(&[side_meta,side_full,evicted,side_set])?;
    items.extend([("side/offloaded side-block metadata (side_cap + 2·r_keep entries)",side_meta),("side/one full block before offload or during restore",side_full),("side/evicted hash set and FIFO (batch high-water and growth overlap)",evicted),("side/the chain's side set (churn high-water and growth overlap)",side_set)]);
    // Queues and caches (node.rs, net.rs constants).
    let frame=net::MAX_FRAME as u128;let block=size_of::<Block>() as u128;let l=&limits.net;
    let pool=add(&[node::POOL_MAX_BYTES as u128,mul(node::POOL_MAX_BYTES as u128/MIN_TX_BYTES,32+24+24)?])?;
    // D-7: revalidation expands one abandoned block's body at a time into the backlog, beside the pool's bytes
    // (`Node::pool_step_until`; `Node::pool_audit` checks the sum).
    let abandoned=add(&[u128::from(cp.BODY_MAX),mul(u128::from(cp.BODY_MAX)/MIN_TX_BYTES,32+24+24)?])?;
    let orphans=add(&[node::ORPHAN_BYTES_MAX as u128,mul((node::ORPHAN_PARENTS_MAX*node::ORPHANS_PER_PARENT) as u128,block+64)?])?;
    let future=mul(node::FUTURE_MAX as u128,frame+block)?;let templates=mul(node::TEMPLATES_MAX as u128+1,2*frame+block)?;
    let seen=add(&[mul(node::SEEN_MAX as u128+1,3*hdr+128)?,mul(node::SEEN_MAX as u128+1,192)?])?;
    let caches=add(&[mul(chain::INVALID_MAX as u128,32+REASON_MAX+64)?,mul(node::RECEIVED_MAX as u128+1,64)?,mul(node::REJECTS_MAX as u128+1,64+REASON_MAX+64)?,
        mul(node::EVIDENCE_POOL_MAX as u128,u128::from(cp.EVID_BYTES_MAX)+24)?,mul(node::FUTURE_MAX as u128,32)?])?;
    // Inbound peers <= peers_max; outbound <= one per dialer (net::dial).
    let peers=mul(l.peers_max as u128+dials as u128,add(&[l.queue_bytes_max as u128,2*frame,4096])?)?;
    // Each forwarder also holds the one event it is blocked sending (+1).
    let (main_events,net_events)=(mul(node::EVENT_QUEUE as u128,frame+256)?,mul((1+dials) as u128,mul(net::NET_EVENT_QUEUE as u128+1,frame+256)?)?);
    let q=[("queues/transaction pool",pool),("queues/an abandoned block's transactions being revalidated (one body, D-7)",abandoned),("queues/orphan blocks",orphans),("queues/future blocks",future),("queues/templates",templates),("queues/seen sets",seen),
        ("queues/rejection and evidence caches",caches),("queues/peer send queues and frames",peers),("queues/main event queue",main_events),("queues/network event forwarders",net_events)];
    let queues=add(&q.map(|x|x.1))?;items.extend(q);
    // RPC (rpc::conn): per connection its header buffer (HEAD_MAX+READ_CHUNK, and its lossy UTF-8
    // copy, up to 3 bytes per input byte) and a body of exactly Content-Length <= REQ_BUF bytes. Answers held
    // by connections are within the response pools (rpc::answer: light (conn_max+1) x
    // LIGHT_RESP_MAX, heavy HEAVY_RESP_POOL), except small busy answers and HTTP literals (each
    // <= SMALL_REPLY_MAX, built within SMALL_REPLY_PEAK of heap: ids, answer, header and one
    // transient id parse; the round-2 term counted the answer only). A heavy body can outlive its
    // connection in the main queue: at most queued_max of them (rpc::Held). The light lane holds <= conn_max bodies of <=
    // LIGHT_BODY_MAX. The consensus thread handles one request at a time: its parse tree
    // (rpc::parse_bytes_max, enforced by JSON_VALUES_MAX) and the answer being built: the batch
    // so far (a growing String: VEC x BATCH_BYTES_HARD) plus one member (its JSON tree and
    // string, a log result stopping at RESP_MAX), counted as 4 x RESP_MAX by inspection. RpcGuard:
    // queued heavy bodies are capped at HEAVY_QUEUE (not 2·conn_max), the light lane at LIGHT_QUEUE
    // (each body is shared with its connection through an Arc, never copied; the Arc and the
    // admission state fit the +256 per body); the per-connection token bucket keeps no state. Busy
    // and rate answers on connection threads scan the members' ids without parsing a tree
    // (rpc::scan_ids) and take SMALL_REPLY_PEAK each. The SendPacer writes the answer straight from its
    // reservation (only a small response header is allocated).
    let r=&limits.rpc;use crate::rpc::{REQ_BUF,HEAD_MAX,READ_CHUNK,RESP_MAX,LIGHT_BODY_MAX,HEAVY_RESP_POOL,BATCH_BYTES_HARD,SMALL_REPLY_PEAK};
    let rq=[("rpc/connection requests (conn_max)",mul(r.conn_max as u128,(4*(HEAD_MAX+READ_CHUNK)+REQ_BUF) as u128)?),
        ("rpc/heavy response pool",HEAVY_RESP_POOL as u128),("rpc/light response pool ((conn_max+1) x LIGHT_RESP_MAX)",crate::rpc::light_pool(r) as u128),
        ("rpc/small answers outside the pools (conn_max x SMALL_REPLY_PEAK)",mul(r.conn_max as u128,SMALL_REPLY_PEAK as u128)?),
        ("rpc/queued heavy bodies (HEAVY_QUEUE)",mul(crate::rpc::queued_max(r) as u128,REQ_BUF as u128+256)?),("rpc/light lane (LIGHT_QUEUE)",mul(r.light_queue as u128,LIGHT_BODY_MAX as u128+256)?),
        ("rpc/request parse tree",crate::rpc::parse_bytes_max(REQ_BUF)),("rpc/answer being built",memory::VEC*(BATCH_BYTES_HARD as u128)+4*(RESP_MAX as u128))];
    let rpc=add(&rq.map(|x|x.1))?;items.extend(rq);
    // Threads: reserved stacks (an upper bound on their resident stack) and per-thread heap.
    use crate::threads::{IO_STACK,CONSENSUS_STACK};
    let io=io_threads(l.peers_max,dials,r.conn_max)?;
    // Exiting-not-yet-joined threads (point 3): a reader/outbound-reader/RPC-conn thread can finish
    // between its reaper's visits. Bound E = peers_max + dials + conn_max (writers are joined by
    // their reader synchronously; singletons never respawn). Measured ~25.8 KiB resident each (the
    // bulk of the stack returns to the exited-stack cache counted below); counted CONSERVATIVELY at a
    // full IO_STACK each, so the term is an upper bound, not the measured figure (evidence/thread-
    // inventory + threadmem). 
    let exiting=(l.peers_max+dials+r.conn_max) as u128;
    let th=[("threads/I/O thread stacks and heap",mul(io,IO_STACK as u128+THREAD_HEAP)?),("threads/consensus stack",CONSENSUS_STACK as u128),
        ("threads/main stack and heap",MAIN_STACK_RESIDENT+2*THREAD_HEAP),("threads/exited-stack cache",STACK_CACHE),
        ("threads/exiting not-yet-joined (conservative)",mul(exiting,IO_STACK as u128)?)];
    let threads=add(&th.map(|x|x.1))?;items.extend(th);
    // EVM execution buffers, journal, transient storage and loaded state (A13 obligation 4).
    let (gas,code_max)=(memory::execution_gas(cp.GAS_LIMIT),u128::from(cp.B_code_max));
    let execution=mul(EXECUTIONS,memory::execution(gas,code_max)?)?;let fixed=mul(EXECUTIONS,memory::fixed_heap(gas,code_max)?)?;
    items.extend([("execution/frames, code analysis, preallocations, precompiles",fixed),("execution/memory and operation classes (worst gas split)",execution.checked_sub(fixed)?)]);
    let total=add(&items.iter().map(|x|x.1).collect::<Vec<_>>())?;
    // Each group is the sum of its items.
    let group=|g:&str|add(&items.iter().filter(|x|x.0.split('/').next()==Some(g)).map(|x|x.1).collect::<Vec<_>>());
    let (snapshot_total,diffs,trie,code,store)=(group("snapshots")?,group("diffs")?,group("trie")?,group("code")?,group("store")?);
    debug_assert_eq!(Some(total),add(&[snapshot_total,diffs,trie,code,store,history,side_blocks,queues,rpc,threads,execution]));
    let _=size_of::<Header>()+size_of::<Body>();
    Some(Bound{snapshots,snapshot_bytes,snapshot_total,diffs,trie,code,store,history,side_blocks,queues,rpc,threads,execution,total,items})
}
/// The share of the budget kept outside the model (D-A13-BUDGET): allocator headers, size-class
/// rounding and fragmentation are not modelled (CONDITIONS), so the model total must fit in
/// RESERVE_KEEP_EIGHTHS/8 of the budget. 7/8 keeps 12.5% (768 MiB of 6 GiB) in reserve.
pub const RESERVE_KEEP_EIGHTHS:u128=7;
/// The smallest budget that boots a bound of `total` bytes: ceil(8·total/7).
pub fn budget_required(total:u128)->Option<u128>{total.checked_mul(8).map(|x|x.div_ceil(RESERVE_KEEP_EIGHTHS))}
/// Boot check: the bound, with the reserve, must fit the node's memory budget.
pub fn check(cp:&ConsensusParams,k_hist:u32,side_max:u32,limits:&Limits,dials:usize)->Result<Bound,String>{
    node::pool_gas_bound(cp,limits)?;
    let b=derive(cp,k_hist,side_max,limits,dials).ok_or_else(||"A13 memory bound: arithmetic overflow".to_string())?;
    let need=budget_required(b.total).ok_or_else(||"A13 memory bound: arithmetic overflow".to_string())?;
    if need>u128::from(limits.mem_budget){return Err(format!("A13 memory bound {} B with its 1/8 reserve needs {need} B, which exceeds the node memory budget {} B ({b:?})",b.total,limits.mem_budget))}
    Ok(b)
}
