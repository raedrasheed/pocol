//! Cutover genesis (development/decisions/EARN-IN-SPEC.md section 7): the new network's allocation from the
//! old chain's canonical state after block S. Deterministic: the same old state and config give the same
//! genesis bytes. The old data directory is opened the way a node start opens it (its lock; the open may
//! rewrite the node's derived files), and a state below the K_hist window is rebuilt, in store mode into its
//! state.redb. No block is added to it. The node is not started, mines nothing and has no peers; the old node
//! must be stopped first, its home lock enforces that.
use pocol_lp4_execution::apply;
use revm::{InMemoryDB,primitives::{Address,U256}};
use serde_json::{json,Value};

/// C2–C4: the new allocation and its report. `miners` are the new genesis members' reward addresses, each
/// funding a bond of `bond` from its own copied balance.
pub fn transform(old:&InMemoryDB,height:u64,d:u64,miners:&[Address],bond:U256)->Result<(InMemoryDB,Value),String>{
    let mut alloc=InMemoryDB::default();
    let (mut copied,mut excluded,mut accounts)=(U256::ZERO,Vec::new(),0u64);
    let mut addrs:Vec<_>=old.accounts.keys().copied().collect();addrs.sort();
    for a in &addrs{
        let acct=&old.accounts[a];
        let Some(info)=acct.info() else{continue};
        if !crate::testnet::allowed(*a){
            // System programs and reserved addresses are rebuilt by the new genesis (C2).
            if !info.balance.is_zero()||!acct.storage.values().all(|v|v.is_zero()){excluded.push(json!({"address":format!("{a:#x}"),"balance":info.balance.to_string()}));}
            continue;
        }
        let mut info=info;
        if info.code.is_none()&&info.code_hash!=revm::primitives::KECCAK_EMPTY{info.code=old.contracts.get(&info.code_hash).cloned();}
        if info.nonce==0&&info.balance.is_zero()&&info.code.as_ref().is_none_or(|c|c.is_empty())&&acct.storage.values().all(|v|v.is_zero()){continue}
        copied+=info.balance;accounts+=1;
        alloc.insert_account_info(*a,info);
        let mut slots:Vec<_>=acct.storage.iter().filter(|(_,v)|!v.is_zero()).map(|(k,v)|(*k,*v)).collect();slots.sort();
        for (k,v) in slots{alloc.insert_account_storage(*a,k,v).map_err(|_|"storage")?;}
    }
    // C3: every HELD, non-tomb old record's bond returns to its reward address.
    let mut credited=Vec::new();let mut credited_total=U256::ZERO;
    let mut ids=apply::record_list(old);ids.sort();
    for id in ids{
        let Some(r)=apply::miner_record(old,id) else{continue};
        if r.bond_state!=1||r.tomb||r.bond.is_zero(){continue}
        if !crate::testnet::allowed(r.reward){return Err(format!("old record {id:#x} has a reserved reward address"))}
        let mut info=alloc.accounts.get(&r.reward).and_then(|a|a.info()).unwrap_or_default();
        info.balance=info.balance.checked_add(r.bond).ok_or("balance overflow")?;
        alloc.insert_account_info(r.reward,info);
        credited_total+=r.bond;
        credited.push(json!({"id":format!("{id:#x}"),"reward":format!("{:#x}",r.reward),"bond":r.bond.to_string(),"status":r.status}));
    }
    // C4: each genesis miner funds its bond from its own (durable) reward balance.
    for reward in miners{
        let mut info=alloc.accounts.get(reward).and_then(|a|a.info()).ok_or(format!("genesis miner reward {reward:#x} has no balance on the old chain"))?;
        if info.balance<bond{return Err(format!("genesis miner reward {reward:#x} holds less than the bond"))}
        if info.nonce==0&&info.code.as_ref().is_none_or(|c|c.is_empty()){return Err(format!("genesis miner reward {reward:#x} is not durable (nonce 0, no code)"))}
        info.balance-=bond;alloc.insert_account_info(*reward,info);
    }
    // Reported, not reproduced: the unsettled pots of the last D blocks, burns, and the Escrow balance.
    let mut pots=Vec::new();let mut pots_total=U256::ZERO;
    for j in height.saturating_sub(d)+1..=height{let (amount,meta,_)=apply::pending(old,j);if !meta.is_zero(){pots_total+=amount;pots.push(json!({"height":j,"amount":amount.to_string()}));}}
    let facts=apply::chain_facts(old);
    let alloc_total=alloc.accounts.values().filter_map(|a|a.info()).fold(U256::ZERO,|t,i|t+i.balance);
    let bonds=bond*U256::from(miners.len());
    let report=json!({
        "accountsCopied":accounts,"copiedBalances":copied.to_string(),"excluded":excluded,
        "bondsCredited":credited,"bondsCreditedTotal":credited_total.to_string(),
        "minerBondsDeducted":bonds.to_string(),"potsDropped":pots,"potsDroppedTotal":pots_total.to_string(),
        "oldEscrow":{"heldTotal":facts.held_total.to_string(),"burnedTotal":facts.burned_total.to_string(),
            "balance":old.accounts.get(&apply::escrow()).and_then(|a|a.info()).map(|i|i.balance).unwrap_or_default().to_string()},
        "allocTotal":alloc_total.to_string(),"totalSupply":(alloc_total+bonds).to_string()});
    Ok((alloc,report))
}

/// The state of the canonical block `hash` (at `height`), rebuilt when R21 has pruned it: a node keeps states only
/// for the last K_hist blocks (32), and the runbook cuts S at least 60 below the head. The rebuild re-executes
/// through the node's own path (TrieRebuild) from genesis, the nearest state kept below S, and checks every
/// rebuilt stateRoot against its header. The evidence walk of a re-executed block reads the chain's own entries,
/// up to E_win heights back, through restored copies of the slimmed ones (PR7, `Chain::ensure_state_with`), so
/// one round suffices. A walk that still meets a slimmed entry (C-1 S4) ends the round with NeedsRebuild. That entry and the
/// slimmed canonical entries in the E_win heights below it are restored and the rebuild runs again, as import
/// does, so each such evidence block costs at most one more round. Every round but the last restores a slimmed
/// entry and nothing slims one again here, so there are at most that many rounds plus one; the bound only turns
/// a fault into an error. Returns the number of rebuild rounds (0 when the state is kept).
pub fn rebuild_at(node:&mut crate::node::Node,hash:&[u8;32],height:u64)->Result<u32,String>{
    if node.chain.view_of(hash).is_some(){return Ok(0)}
    let rounds=1+node.chain.blocks.values().filter(|e|e.slim).count() as u32;
    let e_win=u64::from(node.chain.spec.cp().E_win);
    for round in 1..=rounds{
        match node.rebuild_state(hash){
            Ok(())=>return Ok(round),
            Err(crate::chain::Reject::NeedsRebuild(p)) if node.chain.get(&p).is_some_and(|e|e.slim)=>{
                let ph=node.chain.get(&p).map_or(0,|e|e.height);
                let mut restore=vec![p];
                restore.extend((ph.saturating_sub(e_win)..ph).filter_map(|h|node.chain.canonical_at(h)).filter(|e|e.slim).map(|e|e.hash));
                for x in restore{node.unslim(&x).map_err(|e|format!("restore block {} for the rebuild at height {height}: {e:?}",crate::types::hex(&x)))?;}
            }
            Err(crate::chain::Reject::NeedsRebuild(p))=>return Err(format!("rebuild the state at height {height}: block {} needs a rebuild",crate::types::hex(&p))),
            Err(e)=>return Err(format!("rebuild the state at height {height}: {e:?}")),
        }
    }
    Err(format!("rebuild the state at height {height}: no state after {rounds} rounds"))
}

/// `pocold cutover --network OLD_DIR --data OLD_DATA --height S --config NEW.json --out NEW_DIR [NODE] [LOCAL]`
pub fn run(net:&std::path::Path,data:&std::path::Path,height:u64,config:&[u8],out:&std::path::Path,limits:crate::node::Limits,local:&pocol_lp4_execution::admission::NodeLocal)->Result<(),String>{
    if out.exists(){return Err("output must be a new directory; genesis is never overwritten".into())}
    let parsed:Value=serde_json::from_slice(config).map_err(|_|"invalid new genesis config")?;
    if parsed["earnin"]!=json!(true)||parsed["continuous"]!=json!(true){return Err("the new network config must set \"earnin\": true and \"continuous\": true".into())}
    let miners:Vec<Address>=parsed["miners"].as_array().ok_or("miners")?.iter().map(|m|m["reward"].as_str().unwrap_or("").parse().map_err(|_|"miner reward address".to_string())).collect::<Result<_,_>>()?;
    let spec=crate::load_spec_for(net,local,limits.state_store.then_some(data))?;
    let (chain_id,genesis)=(spec.chain_id(),crate::types::hex(&spec.genesis_hash()));
    let d=u64::from(spec.cp().D);
    let cfg=crate::node::Config{dir:data.to_path_buf(),name:"cutover".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits};
    let mut node=crate::node::Node::open(cfg,spec)?;
    let head=node.chain.head().height;
    if height>head{return Err(format!("height {height} is above the old head {head}"))}
    let hash=node.chain.canonical_at(height).ok_or("no canonical block at that height")?.hash;
    let block=crate::types::hex(&hash);
    rebuild_at(&mut node,&hash,height)?;
    let state=node.chain.view_of(&hash).ok_or("the state at that height is not available")?;
    let mut old=crate::chain::whole_state(state.v());
    // Code by hash, for accounts whose code the view returns separately.
    let hashes:Vec<_>=old.accounts.values().filter_map(|a|a.info()).filter(|i|i.code.is_none()&&i.code_hash!=revm::primitives::KECCAK_EMPTY).map(|i|i.code_hash).collect();
    for h in hashes{if let Some(c)=state.v().contract(&h){old.contracts.insert(h,c);}}
    drop(state);
    let new_bond:U256="20000000000000000000".parse().unwrap();
    let (alloc,mut report)=transform(&old,height,d,&miners,new_bond)?;
    report["old"]=json!({"chainId":chain_id,"genesisHash":genesis,"height":height,"blockHash":block,"head":head});
    drop(node);
    crate::testnet::prepare_cutover(config,alloc,report,out)
}
