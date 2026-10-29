//! LP4 P7/P8 resource derivations for the v1 system programs (engineering
//! derivation bound to this layout; see development/lp4/p7-resources/DERIVATION.md).
//!
//! Notation: S=S_max, D, K=max(D,W_w), RRM=REG_RECORDS_MAX, M=M_max, E=E_max,
//! RET, SW=min(SWEEP_MAX,16) deletes per beginBlock, O=min(S+1,M) owners per height.
//!
//! SYS slots (state footprint, R18 SYS_SLOT_MAX):
//!   Registry 5 + 5·RRM + M   (fixed, RecArr, Rec 4 words, ActList)
//!   Ledger   K·(2·O + 2)       (K retained heights: count+owner per owner, len, total)
//!   Escrow   2 + 3·(D+1)       (burnedTotal, final marker, pending)
//!   Caps     52
//! SYS keys changed per block (R18 SYS_KEYS_BLOCK_MAX):
//!   evidence 9·E; markClose 1 (+RET+1 closing); prune 2·O+2; recordShares 3·O+2;
//!   recordResult 1; settle 4; beginBlock 2+8·SW+1 (1 when closing); closeBlock 2;
//!   final: finalize 2 + 4·(D-1)
//! SYS accounts touched per block (R18 SYS_ACCT_BLOCK_MAX):
//!   8 fixed (5 programs, PAY, SYSTEM, origin) + payees
//!   normal O+1+SW; closing O+1+RET; final min(D·(O+1), M+R_max·D+1)+RET.
use crate::profile::ConsensusParams;

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct SysBounds{pub slot_min:u64,pub keys_normal:u64,pub keys_closing:u64,pub keys_final:u64,pub accts_normal:u64,pub accts_closing:u64,pub accts_final:u64}
impl SysBounds{pub fn keys_max(&self)->u64{self.keys_normal.max(self.keys_closing).max(self.keys_final)}pub fn accts_max(&self)->u64{self.accts_normal.max(self.accts_closing).max(self.accts_final)}}

pub fn derive(cp:&ConsensusParams)->SysBounds{
    let s=u64::from(cp.S_max);let d=u64::from(cp.D);let k=d.max(u64::from(cp.W_w));let rrm=u64::from(cp.REG_RECORDS_MAX);let m=u64::from(cp.M_max);
    let e=u64::from(cp.E_max);let ret=u64::from(cp.RET);let sw=u64::from(cp.SWEEP_MAX).min(16);let o=(s+1).min(m);let rmax=u64::from(cp.R_max);
    let slot_min=5+5*rrm+m + k*(2*o+2) + 2+3*(d+1) + 52;
    let common=9*e+(2*o+2)+(3*o+2)+1+4;
    let keys_normal=common+1+(2+8*sw+1)+2;
    let keys_closing=common+(1+ret+1)+1+2;
    let keys_final=common+(1+ret+1)+1+2+4*d.saturating_sub(1);
    let accts_normal=8+o+1+sw;let accts_closing=8+o+1+ret;let accts_final=8+(d*(o+1)).min(m+rmax*d+1)+ret;
    SysBounds{slot_min,keys_normal,keys_closing,keys_final,accts_normal,accts_closing,accts_final}
}
/// R18 (general, v1 layout): SYS_SLOT_MAX >= slot_min, per-block key/account caps
/// cover every phase class, and G_SYS can hold at least the per-call cap.
pub fn r18(cp:&ConsensusParams)->Result<SysBounds,Vec<String>>{
    let b=derive(cp);let mut v=Vec::new();
    if u64::from(cp.SYS_SLOT_MAX)<b.slot_min{v.push(format!("SYS_SLOT_MAX {} < {}",cp.SYS_SLOT_MAX,b.slot_min))}
    if u64::from(cp.SYS_KEYS_BLOCK_MAX)<b.keys_max(){v.push(format!("SYS_KEYS_BLOCK_MAX {} < {}",cp.SYS_KEYS_BLOCK_MAX,b.keys_max()))}
    if u64::from(cp.SYS_ACCT_BLOCK_MAX)<b.accts_max(){v.push(format!("SYS_ACCT_BLOCK_MAX {} < {}",cp.SYS_ACCT_BLOCK_MAX,b.accts_max()))}
    if cp.G_SYS<crate::apply::PER_CALL_GAS{v.push("G_SYS below per-call cap".into())}
    if v.is_empty(){Ok(b)}else{Err(v)}
}
/// Counts the actual SYS storage slots present (nonzero) in a state.
pub fn sys_slots(db:&revm::InMemoryDB)->u64{
    (0..5).map(|i|db.accounts.get(&crate::apply::program_address(i)).map(|a|a.storage.values().filter(|v|**v!=revm::primitives::U256::ZERO).count() as u64).unwrap_or(0)).sum()
}
/// R21 for snapshot-retaining nodes: retained state bytes <= (K_hist + side) x
/// bytes(state). With S_acct accounts and S_slot slots, bytes(state) <=
/// S_acct*ACCOUNT_BYTES + S_slot*SLOT_BYTES (in-memory InMemoryDB model).
pub const ACCOUNT_BYTES:u64=200;pub const SLOT_BYTES:u64=96;
pub fn r21_retained_bytes(k_hist:u64,side:u64,accounts:u64,slots:u64)->u64{(k_hist+side)*(accounts*ACCOUNT_BYTES+slots*SLOT_BYTES)}

#[cfg(test)] mod tests;

/// A13: resident bytes of one state snapshot (revm `InMemoryDB` as the node keeps it), from
/// the actual container layout. A hashbrown table of n >= 1 entries has at most
/// max(4, next_pow2(⌈8n/7⌉)) <= 6 + 16n/7 buckets (tables never shrink, so n is the peak
/// count; containers are normalized after every commit so the peak follows the live state),
/// each holding one entry and one control byte, plus a 16-byte trailing group and alignment.
/// The bound holds for every distribution of slots over accounts: each account's storage
/// table costs at most (6 + 16·s/7) buckets. Code bytes are reference-counted and shared by
/// clones, so `code_bytes` counts them separately. Checked u128 arithmetic throughout.
pub mod memory{
    use revm::{db::DbAccount,primitives::{Address,B256,Bytecode,U256}};
    pub const ACCOUNT_ENTRY:u128=std::mem::size_of::<(Address,DbAccount)>() as u128;
    pub const SLOT_ENTRY:u128=std::mem::size_of::<(U256,U256)>() as u128;
    pub const CODE_ENTRY:u128=std::mem::size_of::<(B256,Bytecode)>() as u128;
    pub const HASH_ENTRY:u128=std::mem::size_of::<(U256,B256)>() as u128;
    pub const DB:u128=std::mem::size_of::<revm::InMemoryDB>() as u128;
    /// BLOCKHASH window kept in a block's state (apply_block ancestors).
    pub const ANCESTORS:u128=256;
    /// Bytes of hashbrown tables holding `n` entries in total spread over `tables` tables.
    pub fn tables(n:u128,tables:u128,entry:u128)->Option<u128>{
        if n==0{return Some(0)}
        let buckets=n.checked_mul(16)?.div_ceil(7).checked_add(tables.checked_mul(6)?)?;
        buckets.checked_mul(entry.checked_add(1)?)?.checked_add(tables.checked_mul(32)?)
    }
    /// One snapshot with at most `accounts` accounts, `slots` storage slots and `codes`
    /// code entries; None on overflow (the caller rejects).
    pub fn snapshot(accounts:u128,slots:u128,codes:u128)->Option<u128>{
        let a=tables(accounts,1,ACCOUNT_ENTRY)?;let s=tables(slots,accounts.min(slots),SLOT_ENTRY)?;
        let c=tables(codes,1,CODE_ENTRY)?;let h=tables(ANCESTORS,1,HASH_ENTRY)?;
        a.checked_add(s)?.checked_add(c)?.checked_add(h)?.checked_add(DB)
    }
    /// A pruned (default) state: the default code entries and the empty containers.
    pub fn empty_snapshot()->Option<u128>{tables(2,1,CODE_ENTRY)?.checked_add(DB)}
    /// Code payload with its analysis (jump table of len/8 bytes plus padding) and the
    /// shared allocation headers, for `logical` code bytes in `codes` code entries.
    pub fn code_bytes(logical:u128,codes:u128)->Option<u128>{
        logical.checked_add(logical/8)?.checked_add(codes.checked_mul(33+64)?)
    }

    /// Largest gas any single EVM execution can consume: a SYSTEM call is capped at
    /// apply::PER_CALL_GAS, a transaction at GAS_LIMIT, pocold's eth_call/eth_estimateGas at
    /// the same 30M (pocold::rpc::CALL_GAS_MAX).
    pub fn execution_gas(gas_limit:u64)->u128{u128::from(gas_limit.max(crate::apply::PER_CALL_GAS))}
    /// Call frames alive at once (revm `CALL_STACK_LIMIT` + the top frame) and their sizes: each
    /// frame is a boxed CallFrame/CreateFrame (<= FRAME_BYTES, checked by a test) with a stack
    /// preallocated at STACK_LIMIT words.
    pub const FRAMES:u128=revm::CALL_STACK_LIMIT as u128+1;
    pub const FRAME_BYTES:u128=4096;
    pub const STACK_BYTES:u128=(revm::interpreter::STACK_LIMIT*32) as u128;
    pub const JOURNAL_ENTRY:u128=std::mem::size_of::<revm::JournalEntry>() as u128;
    pub const TRANSIENT_ENTRY:u128=std::mem::size_of::<((Address,U256),U256)>() as u128;
    pub const STATE_ACCOUNT:u128=std::mem::size_of::<(Address,revm::primitives::Account)>() as u128;
    pub const STATE_SLOT:u128=std::mem::size_of::<(U256,revm::primitives::EvmStorageSlot)>() as u128;
    pub const LOG_ENTRY:u128=std::mem::size_of::<revm::primitives::Log>() as u128;
    /// Process-lifetime precompile tables (the KZG trusted setup, loaded once).
    pub const PRECOMPILE_STATIC:u128=8<<20;
    /// Largest init code (EIP-3860: twice MAX_CODE_SIZE; LP4 leaves revm's limit unset while
    /// code runs, code_caps.rs sets it only around the deposit check).
    pub const INITCODE_MAX:u128=revm::primitives::MAX_INITCODE_SIZE as u128;
    /// Accounts loaded whatever the code does: sender, recipient, coinbase, the precompiles and
    /// system contracts touched, and one unpaid load by the top frame.
    pub const FIXED_ACCOUNTS:u128=33;
    /// Memory words purchasable with `g` gas in one frame: the largest w with 3w + w²/512 <= g.
    pub fn words(g:u128)->u128{let (mut lo,mut hi)=(0u128,1u128<<40);while lo<hi{let m=(lo+hi+1)/2;if 3*m+m*m/512<=g{lo=m}else{hi=m-1}}lo}
    /// Bytes of one hashbrown entry of `e` bytes at the table's worst: fewer than 16/7 buckets
    /// per entry (load factor 7/8, power-of-two growth), each its entry and a control byte, and
    /// the old table alive beside the new one while it grows (x2). Rounded up.
    pub const fn entry_bytes(e:u128)->u128{(32*(e+1)).div_ceil(7)}
    /// A table's own share beyond its entries (6 buckets, the control tail, alignment), x2.
    pub const fn table_bytes(e:u128)->u128{2*(6*(e+1)+32)}
    /// Vec growth: a Vec's capacity is at most twice its length, and while it reallocates the old
    /// buffer is alive beside the new one, so a Vec is counted at three times its length.
    pub const VEC:u128=3;
    /// A class of operations whose heap outlives the operation within one execution: `bytes`
    /// per operation (growth peaks included) and the least gas one such operation costs.
    #[derive(Debug,Clone,Copy,PartialEq,Eq)]
    pub struct Class{pub name:&'static str,pub bytes:u128,pub gas:u128}
    /// Journal of one operation: at most 2 entries in a growing Vec; a frame's own journal Vec
    /// (revm pushes one per checkpoint and keeps it after a commit; a revert truncates it), its
    /// first allocation 4 entries, and its slot in the outer Vec.
    const OP_JOURNAL:u128=2*VEC*JOURNAL_ENTRY;
    const FRAME_JOURNAL:u128=VEC*24+4*JOURNAL_ENTRY;
    /// A loaded account (both maps' entries, its journal, the frame that loaded it), a loaded
    /// slot likewise, and the two storage maps' own share that an account's first slot creates.
    const ACCOUNT_BYTES:u128=entry_bytes(STATE_ACCOUNT)+entry_bytes(ACCOUNT_ENTRY)+OP_JOURNAL+FRAME_JOURNAL;
    const SLOT_BYTES:u128=entry_bytes(STATE_SLOT)+entry_bytes(SLOT_ENTRY)+OP_JOURNAL+FRAME_JOURNAL;
    const MAP_BYTES:u128=table_bytes(STATE_SLOT)+table_bytes(SLOT_ENTRY);
    /// Every operation that leaves heap behind for the rest of the execution, at its cheapest.
    /// revm 19.7 inserts an account or slot into the journaled state and the engine database
    /// before it charges the cold cost (revm-interpreter host.rs sload/balance/extcode*,
    /// contract.rs call loading; journaled_state.rs: a revert only marks the entry cold). A
    /// frame that then runs out of gas keeps that one unpaid entry and cannot load another; the
    /// frame costs at least a warm CALL (100 gas). Paid loads (cold 2600/2100, access lists
    /// 2400/1900) add the same entries for more gas, so these classes cover them.
    /// A loaded account's storage maps (journaled state and engine database) start empty, and an
    /// empty hashbrown map allocates nothing (checked by a test); a map's own share
    /// (`table_bytes`) appears with the account's first slot. So with a accounts, s slots and
    /// m <= min(a, s) non-empty maps, for at least 100(a + s) gas, the bytes are at most
    /// A·a + S·s + B·m; over that region the largest bytes per gas is at a = s (with the base)
    /// or at s = 0 or a = 0, which are the three classes below.
    pub fn classes()->[Class;11]{[
        Class{name:"account loaded unpaid",bytes:ACCOUNT_BYTES,gas:100},
        Class{name:"slot loaded unpaid",bytes:SLOT_BYTES,gas:100},
        Class{name:"account with its first slot",bytes:ACCOUNT_BYTES+SLOT_BYTES+MAP_BYTES,gas:200},
        Class{name:"transient slot (TSTORE)",bytes:entry_bytes(TRANSIENT_ENTRY)+OP_JOURNAL,gas:100},
        Class{name:"journaled write (SSTORE, transfer, nonce)",bytes:OP_JOURNAL,gas:100},
        Class{name:"committed empty frame (warm CALL)",bytes:FRAME_JOURNAL,gas:100},
        // LOG: 375 gas per log and per topic, 8 per data byte; logs form one growing Vec; a
        // log's topics and data are exact allocations (a topic 32 B / 375 < 1 B / 8).
        Class{name:"log",bytes:VEC*LOG_ENTRY,gas:375},
        Class{name:"log data and topics",bytes:1,gas:8},
        // Code deposit: 200 gas per byte; the deposited code, its padding and jump table.
        Class{name:"created code",bytes:9+1,gas:1600},
        Class{name:"creation",bytes:entry_bytes(STATE_ACCOUNT)+entry_bytes(ACCOUNT_ENTRY)+entry_bytes(CODE_ENTRY)+table_bytes(STATE_SLOT)+table_bytes(SLOT_ENTRY)+97+OP_JOURNAL+FRAME_JOURNAL,gas:32000},
        // modexp returns mod_len bytes for at least max(200, ceil(mod_len/8)^2/3) gas (EIP-2565),
        // at most 192 B per 200 gas; held as return data. Other precompile outputs are <= 64 B.
        Class{name:"modexp output",bytes:192,gas:200},
    ]}
    /// Heap the classes can add with `gas`: the largest rate times the gas (rounded up). Mixing
    /// classes adds no more, since every class is linear in its own gas.
    pub fn class_heap(gas:u128)->Option<u128>{
        let mut m=0u128;for c in classes(){m=m.max(gas.checked_mul(c.bytes)?.div_ceil(c.gas));}Some(m)
    }
    /// Memory bought with `g` gas, held at once (A13 obligation 4). At most N = 2·FRAMES regions
    /// are paid for at once (the active frames' memory and one returned child per frame whose
    /// output is held), from disjoint gas, at 3w + ⌊w²/512⌋ ≥ 3w + w²/512 − 1 each; by
    /// convexity their words total at most N·(words(g/N + 2) + 1) = W. Bytes: the shared buffer
    /// (VEC × its length), the frames' inputs (copies of ranges of their callers' memory) and
    /// the returned outputs held (copies of the children's memory): (3 + 1 + 1) × 32 × W.
    pub fn memory_heap(g:u128)->Option<u128>{
        let n=2*FRAMES;n.checked_mul(words(g/n+2)+1)?.checked_mul((VEC+2)*32)
    }
    /// The parts that do not depend on how the gas is spent: frames and their stacks, each
    /// frame's analysed code (revm analyses raw code per frame: a padded copy and a jump table,
    /// one frame also holding the raw copy while it analyses), the call-stack and shared-memory
    /// preallocations, small precompile outputs, the fixed accounts, the BLOCKHASH entries the
    /// engine database caches (revm asks it only for the last 256 numbers), precompile working
    /// memory (<= 8 × the largest single-frame memory) and the static precompile tables.
    pub fn fixed_heap(gas:u128,code_max:u128)->Option<u128>{
        let code=code_max.max(INITCODE_MAX);let per_code=code.checked_add(code/8+33+64)?;
        let frames=FRAMES.checked_mul(STACK_BYTES+FRAME_BYTES+per_code+64+16)?.checked_add(code)?;
        let pre=(FRAMES*16).checked_add(4096)?.checked_add(1025*std::mem::size_of::<revm::Frame>() as u128)?
            .checked_add(ANCESTORS*entry_bytes(HASH_ENTRY)+table_bytes(HASH_ENTRY))?;
        // Fixed accounts load without gas; each may also hold storage maps.
        let accounts=FIXED_ACCOUNTS.checked_mul(ACCOUNT_BYTES+MAP_BYTES)?;
        let precompile=words(gas).checked_mul(32*8)?.checked_add(PRECOMPILE_STATIC)?;
        frames.checked_add(pre)?.checked_add(accounts)?.checked_add(precompile)
    }
    /// Upper bound on the heap of one EVM execution (revm 19.7, CANCUN) consuming at most `gas`
    /// with code at most `code_max` bytes: the gas is split between memory (g) and the classes
    /// (gas − g). memory_heap is increasing and class_heap is linear, so on each step
    /// [g_k, g_k+1] of a grid the sum is at most memory_heap(g_k+1) + class_heap(gas − g_k); the
    /// bound is the largest of these plus fixed_heap.
    pub fn execution(gas:u128,code_max:u128)->Option<u128>{
        const STEPS:u128=4096;let mut worst=0u128;
        for k in 0..STEPS{
            let (lo,hi)=(gas*k/STEPS,gas*(k+1)/STEPS);
            worst=worst.max(memory_heap(hi)?.checked_add(class_heap(gas-lo)?)?);
        }
        worst.checked_add(fixed_heap(gas,code_max)?)
    }
}
