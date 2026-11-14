//! V3-20/24 on disposable fixtures, and FN-2a (owner decision 10d, 2026-11-13): no creation fee. New accounts,
//! nonzero slots and code pay ordinary gas only; ACCT_CAP and SLOT_CAP stay A03 halts that charge the declared gas
//! (owner answer 11), and CODE_CAP still fails the CREATE. No production profile or coefficients.
use pocol_lp4_execution::fees;
use revm::primitives::U256;

#[test]
fn fee_floor_has_no_legacy_shortcut(){
    let floor=U256::from(10_000_000);let mut old=U256::from(1_000_000_000);let mut new=old;
    for _ in 0..1000{old=fees::next_base_fee(old,0,8_000_000,U256::ZERO).unwrap();new=fees::next_base_fee(new,0,8_000_000,floor).unwrap();assert!(new>=floor)}
    assert_eq!(old,U256::from(7));assert_eq!(new,floor);
}

#[cfg(feature="v3-preview")]
mod preview{
    use super::*;
    use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},native::{self,NativeBlockContext},receipts,state_summary::Summary,txsign::Tx,user_phase::{self,Halt,PoolMeter},view::View,wire};
    use revm::{InMemoryDB,primitives::{AccountInfo,Address,B256,Bytecode,Bytes,alloy_primitives::U512}};

    fn net(v3:bool,tune:impl FnOnce(&mut DevnetConfig))->devnet::Devnet{
        let mut c=DevnetConfig::new(777938,1_800_000_000,&["execution"]);
        c.spec_version=if v3{2}else{1};c.earnin=true;c.nonce_mode=1;
        tune(&mut c);devnet::build(&c).unwrap()
    }
    fn ctx(n:&devnet::Devnet)->NativeBlockContext{NativeBlockContext{height:U256::from(1),timestamp:U256::from(1_800_000_004u64),gas_limit:U256::from(n.profile.cp().GAS_LIMIT),base_fee:U256::from(100),prevrandao:B256::ZERO}}
    fn tx(nonce:u64,to:Option<Address>,value:u64,data:Vec<u8>,gas:u64)->Vec<u8>{
        Tx{chain_id:777938,nonce,max_priority:U256::from(7),max_fee:U256::from(200),gas,to,value:U256::from(value),data:Bytes::from(data)}.sign_1559(&devnet::account_key("alice"))
    }
    fn run(n:&devnet::Devnet,db:&InMemoryDB,raw:&[Vec<u8>])->user_phase::UserCandidate{
        user_phase::execute_for_block(db,raw,&ctx(n),2_000_000,1000,&n.profile,user_phase::UserContext::default()).unwrap()
    }
    fn code(db:&mut InMemoryDB,a:Address,bytes:Vec<u8>){let b=Bytecode::new_raw(Bytes::from(bytes));db.insert_account_info(a,AccountInfo{nonce:1,code_hash:b.hash_slow(),code:Some(b),..Default::default()});}
    fn count(db:&InMemoryDB)->Summary{Summary::scan(db,32768)}
    fn sum(db:&InMemoryDB)->U512{db.accounts.values().filter_map(|a|a.info()).map(|a|U512::from(a.balance)).sum()}
    fn new_address(i:u8)->Address{Address::from([i;20])}
    fn init(runtime:&[u8])->Vec<u8>{assert!(runtime.len()<256);let len=runtime.len() as u8;let mut b=vec![0x60,len,0x60,12,0x60,0,0x39,0x60,len,0x60,0,0xf3];b.extend_from_slice(runtime);b}
    struct Temp(std::path::PathBuf);
    impl Temp{fn new()->Self{use std::sync::atomic::{AtomicU64,Ordering};static N:AtomicU64=AtomicU64::new(0);Self(std::env::temp_dir().join(format!("pocol-v3-execution-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed))))}}
    impl Drop for Temp{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    fn node_config(dir:&Temp,store:bool)->pocold::node::Config{pocold::node::Config{dir:dir.0.clone(),name:"execution-test".into(),miner:None,listen:None,rpc:None,peers:vec![],stop_at:None,log:false,
        limits:pocold::node::Limits{r_keep:4,side_cap:2,state_store:store,history_window:Some(2),slim_depth:Some(4),pool_sender_gas_max:Some(8_000_000),..Default::default()}}}
    fn node_cfg(tune:impl FnOnce(&mut DevnetConfig))->DevnetConfig{
        let mut c=DevnetConfig::new(777938,1_700_000_000,&["execution"]);c.spec_version=2;c.earnin=true;c.nonce_mode=1;c.hash_rate=16;c.c=8;c.d_att=91;c.r_max=1;
        tune(&mut c);c
    }
    fn node_spec(c:&DevnetConfig)->pocold::chain::Spec{pocold::chain::Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap().0)}
    fn node_tx(node:&pocold::node::Node,key:&pocol_lp4_execution::txsign::Key,nonce:u64,to:Address,gas:u64)->Vec<u8>{
        let head=node.chain.full_entry(&node.chain.head).unwrap();let base=node.chain.child_ctx(&head,0).unwrap().base_fee;
        Tx{chain_id:777938,nonce,max_priority:U256::from(7),max_fee:base*U256::from(2)+U256::from(7),gas,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(key)
    }
    fn rpc(node:&mut pocold::node::Node,method:&str,params:serde_json::Value)->serde_json::Value{
        serde_json::from_str(&pocold::rpc::handle(node,serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string().as_bytes())).unwrap()
    }
    fn solve(node:&pocold::node::Node,raw:Vec<Vec<u8>>,clock:u64)->pocold::types::Block{
        solve_chain(&node.chain,raw,clock)
    }
    fn solve_chain(chain:&pocold::chain::Chain,raw:Vec<Vec<u8>>,clock:u64)->pocold::types::Block{
        use pocold::types::*;let p=chain.full_entry(&chain.head).unwrap();let key=devnet::miner_identity("execution").key;let body=Body{txs:raw,evidence:vec![]};
        let (mut h,_,ctx)=chain.build_signed(&p,&key,clock,body.clone(),vec![]).unwrap();let tid=template_id(&h.ut_raw);
        for nonce in 0..1_000_000{if le_target(&pow_hash(&tid,nonce),&ctx.target){h.nonce=nonce;reseal(&mut h);h.winner_sig=key.sign_hash(&B256::from(pocold::hashes::win_msg(&tid,nonce,&share_root(&h))));reseal(&mut h);return Block{header:h,body}}}
        panic!("private fixture solution not found");
    }

    /// The A03 identity of a USER phase without a creation fee: the sender's balance falls by exactly
    /// gas_used·effective_price + value, Escrow (the coinbase) gains the tips, and the supply falls by exactly
    /// gas_used·base_fee, the EIP-1559 base burn. No receipt carries a log a transaction did not emit.
    fn pays_gas_only(db:&InMemoryDB,o:&user_phase::UserCandidate,value:u64){
        let gas=o.commitments().gas_used;let alice=devnet::account_key("alice").address();
        assert_eq!(sum(db)-sum(o.state()),U512::from(gas*100),"the supply falls by the base burn only");
        assert_eq!(db.accounts[&alice].info.balance-o.state().accounts[&alice].info.balance,U256::from(gas*107+value),"the sender pays gas and value only");
        assert_eq!(o.tips(),U256::from(gas*7));
    }

    #[test]
    fn account_creation_pays_ordinary_gas_only_in_both_domains(){
        for v3 in [false,true]{
            let n=net(v3,|c|c.acct_cap=8);let db=n.genesis.state();let before=count(db);
            let raw:Vec<_>=(0..3).map(|i|tx(i,Some(new_address(40+i as u8)),1,vec![],21000)).collect();let o=run(&n,db,&raw);
            assert!(o.halts().is_empty());assert_eq!(o.commitments().gas_used,63000);assert_eq!(o.log_bytes(),0);
            for i in 0..3{assert_eq!(o.receipts()[i as usize],receipts::encode(2,true,(i+1)*21000,&[]).unwrap(),"v3={v3}: no annotation log");}
            assert_eq!(o.commitments().logs_bloom,[0u8;256]);pays_gas_only(db,&o,3);
            assert_eq!(count(o.state()).user_accounts,before.user_accounts+3);
            for i in 0..3u8{assert_eq!(o.state().accounts[&new_address(40+i)].info.balance,U256::from(1));}
        }
    }

    /// The same transactions give the same receipts, gas and created units on a spec-1 and a v3 profile: without the
    /// creation fee, v3 adds nothing to a creation (the v3 log caps only raise limits these transactions stay under).
    #[test]
    fn creations_execute_identically_on_spec_1_and_v3(){
        let target=new_address(63);let runtime=vec![0;80];
        let raws=||vec![tx(0,Some(new_address(64)),5,vec![],21000),tx(1,None,0,init(&runtime),200000),tx(2,Some(target),0,vec![],100000)];
        let mut outs=Vec::new();
        for v3 in [false,true]{let n=net(v3,|_|{});let mut db=n.genesis.state().clone();code(&mut db,target,vec![0x60,1,0x60,0,0x55,0x60,2,0x60,1,0x55,0]);
            let o=run(&n,&db,&raws());assert!(o.halts().is_empty(),"v3={v3}");pays_gas_only(&db,&o,5);
            let (a,b)=(count(&db),count(o.state()));
            outs.push((o.receipts().to_vec(),o.commitments().gas_used,b.user_accounts-a.user_accounts,b.user_slots-a.user_slots,b.inventory()-a.inventory()));}
        assert_eq!(outs[0],outs[1]);
    }

    #[test]
    fn slot_replacement_at_a_full_cap_succeeds_and_is_independent_of_write_order(){
        let n=net(true,|c|c.slot_cap=1);let a=new_address(61);let mut db=n.genesis.state().clone();
        // Clear slot0 then insert slot1: the live count stays one, within the cap, and only gas is paid.
        code(&mut db,a,vec![0x60,0,0x60,0,0x55,0x60,9,0x60,1,0x55,0]);db.insert_account_storage(a,U256::ZERO,U256::from(5)).unwrap();
        let o=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);assert!(o.halts().is_empty());assert_eq!(count(o.state()).user_slots,1);pays_gas_only(&db,&o,0);
        assert_eq!(o.state().slot(&a,&U256::ZERO),U256::ZERO);assert_eq!(o.state().slot(&a,&U256::from(1)),U256::from(9));
        // Reverse the independent writes: the live-count accounting is a function of committed additions and removals,
        // never revm's map order or transient opcode order. Inserting first passes the cap only transiently.
        code(&mut db,a,vec![0x60,9,0x60,1,0x55,0x60,0,0x60,0,0x55,0]);
        let r=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);assert!(r.halts().is_empty());assert_eq!(r.state().slot(&a,&U256::from(1)),U256::from(9));
        assert_eq!(r.commitments().gas_used,o.commitments().gas_used);
    }

    #[test]
    fn native_reverts_and_temporary_slot_creations_leave_nothing(){
        for tail in [vec![0x60,0,0x60,0,0xfd],vec![0x60,0,0x60,0,0x55,0]]{
            let n=net(true,|_|{});let a=new_address(62);let mut db=n.genesis.state().clone();let mut runtime=vec![0x60,1,0x60,0,0x55];runtime.extend(tail);code(&mut db,a,runtime);
            let o=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),0);
            assert_eq!(o.state().slot(&a,&U256::ZERO),U256::ZERO);pays_gas_only(&db,&o,0);
        }
    }

    /// A deployment (its account and every logical code byte) and a chunk-sized deployment burn nothing beyond gas.
    #[test]
    fn a_deployment_burns_nothing_beyond_gas(){
        let n=net(true,|_|{});let db=n.genesis.state();let before=count(db);let runtime=vec![0;80];
        let o=run(&n,db,&[tx(0,None,0,init(&runtime),200000),tx(1,None,0,init(&runtime),200000)]);assert!(o.halts().is_empty());
        assert_eq!(count(o.state()).inventory(),before.inventory()+160,"identical code at two addresses counts twice toward CODE_CAP");
        assert_eq!(count(o.state()).user_accounts,before.user_accounts+2);assert_eq!(o.log_bytes(),0);pays_gas_only(db,&o,0);
        // The largest runtime (B_code_max bytes), deployed with the same init prefix: still gas only.
        let bmax=n.profile.cp().B_code_max as usize;let big=vec![0x5b;bmax];
        let mut initcode=vec![0x61,(bmax>>8) as u8,(bmax&255) as u8,0x60,14,0x60,0,0x39,0x61,(bmax>>8) as u8,(bmax&255) as u8,0x60,0,0xf3];initcode.extend(&big);
        let o=run(&n,db,&[tx(0,None,0,initcode,8_000_000)]);assert!(o.halts().is_empty(),"{:?}",o.halts());
        assert_eq!(count(o.state()).inventory(),before.inventory()+bmax as u64);pays_gas_only(db,&o,0);
    }

    #[test]
    fn pool_pending_state_and_block_execution_agree_on_creations(){
        let n=net(true,|c|c.acct_cap=10);let start=n.genesis.state();let mut db=start.clone();let mut meter=PoolMeter::of(&db,&n.profile);let context=ctx(&n);
        let raw:Vec<_>=(0..4).map(|i|tx(i,Some(new_address(70+i as u8)),1,vec![],21000)).collect();
        for r in &raw{
            let env=wire::decode_for_profile(r,&n.profile).unwrap().tx_env();let total=count(&db).inventory();
            let result=native::execute_on_validated(&db,total,env.clone(),&context,&n.profile).unwrap();assert!(result.result.logs().is_empty());
            meter.admit(&db,&result.state).unwrap();
            user_phase::commit_tx(&mut db,result.state,true);let actual=count(&db);assert_eq!(meter.live(),(actual.user_slots,actual.user_accounts,actual.sys_slots));
        }
        let block=run(&n,start,&raw);assert_eq!(block.commitments().logs_bloom,[0u8;256]);
        assert_eq!(block.state_root(100,100).unwrap(),apply::state_root(&db).unwrap());assert_eq!(sum(block.state()),sum(&db));
    }

    #[test]
    fn a_front_loaded_32k_log_does_not_halt_a_later_transfer_log_on_v3(){
        for v3 in [false,true]{
            let n=net(v3,|_|{});let mut db=n.genesis.state().clone();let front=new_address(81);let transfer=new_address(82);
            // LOG0 charges 36+32732=32768 bytes. A fixed-transfer token fixture
            // moves one unit between balances and emits ERC-20 Transfer.
            code(&mut db,front,vec![0x61,0x7f,0xdc,0x60,0,0xa0,0]);
            let mut token=vec![0x60,99,0x60,0,0x55,0x60,1,0x60,1,0x55,0x60,1,0x60,0,0x52,0x73];
            token.extend_from_slice(devnet::account_key("bob").address().as_slice());token.push(0x73);token.extend_from_slice(devnet::account_key("alice").address().as_slice());token.push(0x7f);
            token.extend_from_slice(revm::primitives::keccak256("Transfer(address,address,uint256)").as_slice());token.extend_from_slice(&[0x60,32,0x60,0,0xa3,0]);code(&mut db,transfer,token);
            db.insert_account_storage(transfer,U256::ZERO,U256::from(100)).unwrap();
            let o=run(&n,&db,&[tx(0,Some(front),0,vec![],1000000),tx(1,Some(transfer),0,vec![],100000)]);
            if v3{assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),32768+36+96+32,"no annotation log");
                assert_eq!(o.state().slot(&transfer,&U256::ZERO),U256::from(99));assert_eq!(o.state().slot(&transfer,&U256::from(1)),U256::from(1));
            }else{assert_eq!(o.halts(),&[(1,Halt::LogBytes)]);assert_eq!(o.log_bytes(),32768);assert_eq!(o.state().slot(&transfer,&U256::ZERO),U256::from(100));assert_eq!(o.state().slot(&transfer,&U256::from(1)),U256::ZERO)}
        }
    }


    /// ACCT_CAP and SLOT_CAP stay A03 halts (FN-2a keeps them): a creation past a full cap halts, charges
    /// all of its declared gas (owner answer 11 of 2026-11-13: the other A03 cap halts charge declared gas), keeps only
    /// the nonce and the fee, and creates nothing.
    #[test]
    fn full_live_caps_halt_and_charge_the_declared_gas(){
        let alice=devnet::account_key("alice").address();
        // ACCT_CAP: three genesis accounts fill a cap of 3.
        let n=net(true,|c|c.acct_cap=3);let db=n.genesis.state();assert_eq!(count(db).user_accounts,3);let to=new_address(91);
        let o=run(&n,db,&[tx(0,Some(to),1,vec![],50000)]);
        assert_eq!(o.halts(),&[(0,Halt::AcctCap)]);assert_eq!(o.commitments().gas_used,50000,"the declared gas, not the 21000 executed");
        assert_eq!(o.receipts()[0],receipts::encode(2,false,50000,&[]).unwrap());assert!(!o.state().accounts.contains_key(&to));
        assert_eq!(o.state().accounts[&alice].info.nonce,1);pays_gas_only(db,&o,0);
        // SLOT_CAP: one live slot fills a cap of 1; the clear that would earn a refund is discarded with the halt.
        let n=net(true,|c|c.slot_cap=1);let a=new_address(93);let mut db=n.genesis.state().clone();
        code(&mut db,a,vec![0x60,1,0x60,1,0x55,0]);db.insert_account_storage(a,U256::ZERO,U256::from(1)).unwrap();
        let o=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);assert_eq!(o.halts(),&[(0,Halt::SlotCap)]);assert_eq!(o.commitments().gas_used,100000);
        assert_eq!(count(o.state()).user_slots,1);assert_eq!(o.state().slot(&a,&U256::from(1)),U256::ZERO);pays_gas_only(&db,&o,0);
    }

    /// C3-8: both domains fill the SAME caps. A dust-filled cap halts v1 and v3 alike; the same raised cap admits
    /// both, and neither charges anything beyond gas.
    #[test]
    fn dust_filled_account_and_slot_caps_halt_in_both_domains_and_raised_caps_admit_both(){
        for v3 in [false,true]{
            for (cap,full) in [(3,true),(10,false)]{
                let n=net(v3,|c|c.acct_cap=cap);let db=n.genesis.state();
                let o=run(&n,db,&[tx(0,Some(new_address(92)),1,vec![],50000)]);
                if full{assert_eq!(o.halts(),&[(0,Halt::AcctCap)],"v3={v3}");pays_gas_only(db,&o,0);}
                else{assert!(o.halts().is_empty(),"v3={v3}");pays_gas_only(db,&o,1);}
            }
            for (cap,full) in [(1,true),(4,false)]{
                let n=net(v3,|c|c.slot_cap=cap);let mut db=n.genesis.state().clone();let a=new_address(93);
                code(&mut db,a,vec![0x60,1,0x60,1,0x55,0]);db.insert_account_storage(a,U256::ZERO,U256::from(1)).unwrap();
                let o=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);
                if full{assert_eq!(o.halts(),&[(0,Halt::SlotCap)],"v3={v3}");assert_eq!(count(o.state()).user_slots,1);}
                else{assert!(o.halts().is_empty(),"v3={v3}");assert_eq!(count(o.state()).user_slots,2);}
                pays_gas_only(&db,&o,0);
            }
        }
    }

    #[test]
    fn dust_filled_code_cap_halts_a_deployment_in_both_domains(){
        for v3 in [false,true]{
            let baseline=net(v3,|_|{});let sys=count(baseline.genesis.state()).inventory();
            // The ChainCaps appended words have a fixed width, so changing the
            // cap does not change its own logical runtime length.
            for (room,full) in [(100,true),(200,false)]{
                let n=net(v3,|c|c.code_cap=sys+room);let mut db=n.genesis.state().clone();
                assert_eq!(count(&db).inventory(),sys);code(&mut db,new_address(94),vec![0;100]);
                let o=run(&n,&db,&[tx(0,None,0,init(&[0;80]),200000)]);
                if full{assert_eq!(count(o.state()).inventory(),sys+100,"v3={v3}");assert_eq!(o.receipts()[0],receipts::encode(2,false,200000,&[]).unwrap());}
                else{assert!(o.halts().is_empty(),"v3={v3}");assert_eq!(count(o.state()).inventory(),sys+180);}
                pays_gas_only(&db,&o,0);
            }
        }
    }

    /// The supply invariant over real blocks with creations: initial balances plus issuance equal the balances plus
    /// the EIP-1559 base burn, nothing else, on the copy, incremental and stored (node trie) paths.
    #[test]
    fn creations_conserve_supply_and_escrow_on_copy_overlay_and_stored_views(){
        use pocol_lp4_execution::{node_trie::{Batch,MemStore,NodeTrie,TrieState},state_trie::StateTrie,view::Overlay};
        let n=net(true,|c|c.acct_cap=20);let mut db=n.genesis.state().clone();let initial=sum(&db);let mut issued=U512::ZERO;let mut external=U512::ZERO;let mut parent_result=None;
        for height in 1..=12{
            let raw=vec![tx(height-1,Some(new_address(100+((height-1)%6) as u8)),1,vec![],21000)];
            let input=apply::BlockInput{height,timestamp:1_800_000_000+height*4,base_fee:U256::from(100),gas_limit:n.profile.cp().GAS_LIMIT,prevrandao:B256::ZERO,
                proposer:apply::Proposer::Fallback,parent_result:parent_result.clone(),evidence:vec![],evidence_raw:&[],transactions:&raw,ancestors:vec![(0,n.genesis_hash)],cum_log_bytes:0};
            let before=count(&db);let trie=StateTrie::build(&db);
            let reference=apply::apply_block(&db,&n.genesis,&n.profile,&input).unwrap();
            let incremental=apply::apply_block_with(&db,Some(&trie),Some(&before),&n.genesis,&n.profile,&input).unwrap();
            let mut store=MemStore::default();let mut batch=Batch::default();let nt=NodeTrie::build(&db,&store,&mut batch);store.commit(&batch,nt.world);
            let block_hashes=Default::default();let stored=TrieState{store:&store,trie:nt,block_hashes:&block_hashes};
            let run=apply::apply_block_on(&stored,&before,&n.genesis,&n.profile,&input).unwrap();
            assert_eq!(run.facts,reference.facts);assert_eq!(run.receipts,reference.receipts);assert_eq!(run.receipts_root,reference.receipts_root);
            let merged=Overlay::from_layer(&db,run.layer).merge();
            assert_eq!(apply::state_root(&merged).unwrap(),reference.state_root);assert_eq!(incremental.state_root,reference.state_root);
            assert_eq!(incremental.receipts,reference.receipts);assert_eq!(incremental.facts,reference.facts);assert_eq!(incremental.summary,count(&reference.state));
            assert_eq!(sum(&merged),sum(&reference.state));
            assert_eq!(count(&reference.state).user_accounts,before.user_accounts+u64::from(height<=6),"height {height}: a new account at heights 1..6");
            issued+=U512::from(reference.issued);external+=U512::from(reference.gas_used*100);
            assert_eq!(sum(&reference.state)+external,initial+issued,"height {height}: only the base burn leaves the supply");
            apply::check_escrow_invariant(&reference.state,&n.profile,height,false).unwrap();
            assert_eq!(reference.facts,apply::chain_facts(&reference.state),"block facts stay pure state facts: no per-block burn field");
            db=reference.state;parent_result=Some(apply::ParentResult{block_hash:B256::from(U256::from(height)),winner:n.miners[0].id,shares:vec![]});
        }
    }

    #[test]
    fn node_pool_builder_rpc_receipts_and_restart_agree_on_a_creation(){
        for stored in [false,true]{let dir=Temp::new();let cfg=node_cfg(|c|c.base_fee_min=U256::from(2_000_000_000u64));
            let mut node=pocold::node::Node::open(node_config(&dir,stored),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");let to=new_address(121);
            let head=node.chain.full_entry(&node.chain.head).unwrap();let child=node.chain.child_ctx(&head,0).unwrap();assert_eq!(child.base_fee,U256::from(2_000_000_000u64));
            assert_eq!(rpc(&mut node,"eth_gasPrice",serde_json::json!([]))["result"],"0xb2d05e00");
            let estimate=rpc(&mut node,"eth_estimateGas",serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1"},"latest"]));
            assert!(estimate.get("error").is_none(),"{estimate}");let gas=u64::from_str_radix(estimate["result"].as_str().unwrap().trim_start_matches("0x"),16).unwrap();assert!((21000..=22000).contains(&gas));
            let raw=node_tx(&node,&alice,0,to,21000);let hash=node.submit_tx(raw.clone(),false).unwrap();
            let head=node.chain.full_entry(&node.chain.head).unwrap();let selection=node.template_selection(&head,cfg.g_ts+4);assert_eq!(selection.txs,vec![raw.clone()]);assert_eq!(selection.counted,21000);
            let b=solve(&node,selection.txs,cfg.g_ts+4);assert!(node.import(b,None).unwrap());
            let receipt=rpc(&mut node,"eth_getTransactionReceipt",serde_json::json!([hash]));assert!(receipt.get("error").is_none(),"{receipt}");assert_eq!(receipt["result"]["status"],"0x1");
            assert_eq!(receipt["result"]["gasUsed"],"0x5208");assert_eq!(receipt["result"]["logs"],serde_json::json!([]),"no creation-fee annotation");
            assert_eq!(balance(&node,to),U256::from(1));
            let head_hash=node.chain.head;let root=node.chain.head().header.state_root;if stored{node.checkpoint_now().unwrap();}drop(node);
            let mut reopened=pocold::node::Node::open(node_config(&dir,stored),node_spec(&cfg)).unwrap();assert_eq!(reopened.chain.head,head_hash);assert_eq!(reopened.chain.head().header.state_root,root);
            assert_eq!(rpc(&mut reopened,"eth_getTransactionReceipt",serde_json::json!([hash])),receipt);
        }
    }

    /// At a full ACCT_CAP the estimate and the pool refuse a creating transfer with the cap halt (AcctCap), while
    /// eth_call stays a query and a no-sender read is estimated.
    #[test]
    fn estimation_and_pool_refuse_a_creation_at_a_full_cap_but_an_unpriced_call_remains_a_query(){
        let dir=Temp::new();let cfg=node_cfg(|c|c.acct_cap=8);let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        let fills:Vec<_>=(0..5u8).map(|i|node_tx(&node,&alice,u64::from(i),new_address(130+i),21000)).collect();
        let b=solve(&node,fills,cfg.g_ts+4);assert!(node.import(b,None).unwrap());assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,8);
        let to=new_address(122);
        let raw=node_tx(&node,&alice,5,to,21000);assert_eq!(node.submit_tx(raw,false).unwrap_err(),"-32000 pending state would exceed a cap (AcctCap)");assert!(node.pool.txs.is_empty());
        let request=serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1"},"latest"]);
        let estimate=rpc(&mut node,"eth_estimateGas",request.clone());assert_eq!(estimate["error"]["code"],-32000);assert_eq!(estimate["error"]["data"]["reason"],"AcctCap","{estimate}");
        let call=rpc(&mut node,"eth_call",request);assert_eq!(call["result"],"0x");
        let read=rpc(&mut node,"eth_estimateGas",serde_json::json!([{"to":devnet::account_key("bob").address()},"latest"]));assert!(read.get("error").is_none(),"{read}");
    }

    /// An EIP-2930 (type 1) envelope with an empty access list: 0x01 || RLP([chainId, nonce, gasPrice,
    /// gas, to, value, data, accessList, yParity, r, s]).
    fn sign_2930(t:&Tx,key:&pocol_lp4_execution::txsign::Key)->Vec<u8>{
        use pocold::rlp::{encode_bytes,encode_list_payload};
        let uint=|v:U256,p:&mut Vec<u8>|{let b=v.to_be_bytes::<32>();let i=b.iter().position(|x|*x!=0).unwrap_or(32);encode_bytes(&b[i..],p)};
        let list=|p:&[u8]|{let mut o=Vec::new();encode_list_payload(p,&mut o);o};
        let mut p=Vec::new();uint(U256::from(t.chain_id),&mut p);uint(U256::from(t.nonce),&mut p);uint(t.max_fee,&mut p);uint(U256::from(t.gas),&mut p);
        match t.to{Some(a)=>encode_bytes(a.as_slice(),&mut p),None=>encode_bytes(&[],&mut p)}
        uint(t.value,&mut p);encode_bytes(&t.data,&mut p);p.extend(list(&[]));
        let sig=key.sign_hash(&revm::primitives::keccak256([&[1u8][..],&list(&p)].concat()));
        uint(U256::from(sig[64]),&mut p);uint(U256::from_be_slice(&sig[..32]),&mut p);uint(U256::from_be_slice(&sig[32..64]),&mut p);
        [&[1u8][..],&list(&p)].concat()
    }


    /// FN-2a for transaction types 0, 1 and 2: a transfer creating the last free account (ACCT_CAP 4, three genesis
    /// accounts), signed with a plain 21000-gas wallet maximum, is included and pays exactly 21000·effective_price plus
    /// its value: no burn, so no bound beyond the gas the sender signed. (Under D-V3-21 option A it halted with
    /// CreationFee: a 21000-gas maximum could not carry the last unit's burn.)
    #[test]
    fn the_last_free_account_costs_only_gas_for_every_type(){
        let alice=devnet::account_key("alice");let value=U256::from(10u128.pow(21));let (max_fee,base)=(200u64,100u64);
        for kind in [0u8,1,2]{
            let effective=if kind==2{base+7}else{max_fee};let tip=effective-base;
            let n=net(true,|c|c.acct_cap=4);let db=n.genesis.state();assert_eq!(count(db).user_accounts,3);
            let to=new_address(140+kind);let t=Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:U256::from(max_fee),gas:21000,to:Some(to),value,data:Default::default()};
            let raw=match kind{0=>t.sign_legacy(&alice),1=>sign_2930(&t,&alice),_=>t.sign_1559(&alice)};assert_eq!(wire::decode_for_profile(&raw,&n.profile).unwrap().kind(),kind);
            let o=run(&n,db,&[raw]);let lost=db.accounts[&alice.address()].info.balance-o.state().accounts[&alice.address()].info.balance;
            assert!(o.halts().is_empty(),"type {kind}");assert_eq!(o.commitments().gas_used,21000);assert_eq!(o.tips(),U256::from(21000*tip));
            assert_eq!(lost,U256::from(21000*effective)+value,"type {kind}: gas and value only");assert_eq!(o.state().accounts[&to].info.balance,value);
            assert_eq!(o.receipts()[0],receipts::encode(kind,true,21000,&[]).unwrap());assert_eq!(count(o.state()).user_accounts,4);
            assert_eq!(sum(db)-sum(o.state()),U512::from(21000*base));
        }
    }

    /// A 1559 transfer with explicit fee fields, signed by `key`.
    fn priced_tx(key:&pocol_lp4_execution::txsign::Key,nonce:u64,to:Address,value:U256,gas:u64,max_fee:U256)->Vec<u8>{
        Tx{chain_id:777938,nonce,max_priority:U256::from(7),max_fee,gas,to:Some(to),value,data:Default::default()}.sign_1559(key)
    }
    fn next_base(node:&pocold::node::Node)->U256{let head=node.chain.full_entry(&node.chain.head).unwrap();node.chain.child_ctx(&head,0).unwrap().base_fee}
    fn balance(node:&pocold::node::Node,a:Address)->U256{node.chain.view_of(&node.chain.head).unwrap().account(&a).map(|x|x.info.balance).unwrap_or_default()}
    fn quantity(v:&serde_json::Value)->U256{U256::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"),16).unwrap()}


    /// Without a creation fee eth_estimateGas needs only the gas: transfers to new accounts are estimated within the
    /// search's 1000-gas resolution of 21000 up to the last free account (ACCT_CAP 8), and a plain 21000-gas wallet
    /// transfer creating it is admitted, included and pays exactly 21000·price + value. The next one is AcctCap.
    #[test]
    fn estimated_creations_need_only_gas_up_to_the_last_free_account(){
        let dir=Temp::new();let cfg=node_cfg(|c|c.acct_cap=8);
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,3);
        let max_fee=U256::from(100_000_000_000u64);
        let request=|to:Address|serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1","maxFeePerGas":format!("{max_fee:#x}"),"maxPriorityFeePerGas":"0x7"},"latest"]);
        let estimate=|node:&mut pocold::node::Node,to:Address|{let e=rpc(node,"eth_estimateGas",request(to));assert!(e.get("error").is_none(),"{e}");quantity(&e["result"]).to::<u64>()};
        let mut fills=Vec::new();
        for i in 0..4u64{let to=new_address(150+i as u8);let gas=estimate(&mut node,to);assert!((21000..=22000).contains(&gas),"{gas}");
            let raw=priced_tx(&alice,i,to,U256::from(1),21000,max_fee);node.submit_tx(raw.clone(),false).unwrap();fills.push(raw);}
        let p=node.chain.full_entry(&node.chain.head).unwrap();let selection=node.template_selection(&p,cfg.g_ts+4);assert_eq!(selection.txs,fills);
        let b=solve(&node,selection.txs,cfg.g_ts+4);assert!(node.import(b,None).unwrap());assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,7);
        let price=next_base(&node)+U256::from(7);let to=new_address(160);
        let gas=estimate(&mut node,to);assert!((21000..=22000).contains(&gas),"the last free account: {gas}");
        let raw=priced_tx(&alice,4,to,U256::from(1),21000,max_fee);let hash=node.submit_tx(raw.clone(),false).unwrap();
        let before=balance(&node,alice.address());let p=node.chain.full_entry(&node.chain.head).unwrap();let selection=node.template_selection(&p,cfg.g_ts+8);assert_eq!(selection.txs,vec![raw]);
        let b=solve(&node,selection.txs,cfg.g_ts+8);assert!(node.import(b,None).unwrap());
        let receipt=rpc(&mut node,"eth_getTransactionReceipt",serde_json::json!([hash]));assert_eq!(receipt["result"]["status"],"0x1","{receipt}");assert_eq!(receipt["result"]["gasUsed"],"0x5208");
        assert_eq!(receipt["result"]["logs"],serde_json::json!([]));
        assert_eq!(before-balance(&node,alice.address()),price*U256::from(21000)+U256::from(1));
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,8);
        let full=rpc(&mut node,"eth_estimateGas",request(new_address(161)));assert_eq!(full["error"]["data"]["reason"],"AcctCap","{full}");
    }

    /// Slot and code creations are estimated at their gas alone too: SSTORE(CALLER, 1) and an 80-byte deployment.
    #[test]
    fn estimated_slot_and_code_creations_need_only_gas(){
        let dir=Temp::new();let cfg=node_cfg(|_|{});let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();
        let x=deploy(&mut node,&[vec![0x60,1,0x33,0x55,0x00]],cfg.g_ts+4)[0];let alice=devnet::account_key("alice");
        let price=next_base(&node)+U256::from(7);
        for (to,data) in [(Some(x),vec![]),(None,init(&[0;80]))]{
            let mut call=serde_json::json!({"from":alice.address(),"data":format!("0x{}",data.iter().map(|b|format!("{b:02x}")).collect::<String>()),"maxFeePerGas":format!("{price:#x}"),"maxPriorityFeePerGas":"0x7"});
            if let Some(to)=to{call["to"]=serde_json::json!(to);}
            let e=rpc(&mut node,"eth_estimateGas",serde_json::json!([call,"latest"]));assert!(e.get("error").is_none(),"{e}");let gas=quantity(&e["result"]).to::<u64>();
            // SSTORE of a fresh slot is about 43k gas, the 80-byte deployment about 70k: no burn allowance is added.
            assert!(gas<100_000,"{gas}");
        }
    }

    /// Deploys each runtime from bob (nonces 0, 1, ...) in one imported block at `clock`; their addresses.
    fn deploy(node:&mut pocold::node::Node,runtimes:&[Vec<u8>],clock:u64)->Vec<Address>{
        let bob=devnet::account_key("bob");
        let raw:Vec<_>=runtimes.iter().enumerate().map(|(i,rt)|Tx{chain_id:777938,nonce:i as u64,max_priority:U256::from(7),max_fee:U256::from(1_000_000_000_000u64),gas:500_000,to:None,value:U256::ZERO,data:Bytes::from(init(rt))}.sign_1559(&bob)).collect();
        let b=solve(node,raw,clock);assert!(node.import(b,None).unwrap());
        let out:Vec<_>=(0..runtimes.len() as u64).map(|i|bob.address().create(i)).collect();
        let view=node.chain.view_of(&node.chain.head).unwrap();assert!(out.iter().all(|a|view.account(a).is_some_and(|x|x.info.code_hash!=revm::primitives::KECCAK_EMPTY)),"deployed");out
    }


    /// The append round (B5) executes its candidates only in `build_signed`, which accepts halts. A user fills the
    /// block's log count with one included call and uses the selection's execution budget with four heavy one-log calls
    /// that B4 then leaves out (they stay pooled); a one-log call with a large gas limit is held for the append round.
    /// Appended after the filler it would halt with LogCount and pay its whole declared gas; under v3 the honest builder
    /// drops it instead, and it stays pooled.
    #[test]
    fn the_v3_append_round_drops_an_appended_transaction_that_would_halt(){
        let dir=Temp::new();let senders=["carol","h1","h2","h3","h4"];
        let cfg=node_cfg(|c|{for s in senders{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let mut config=node_config(&dir,false);let k=devnet::miner_identity("execution").key;config.miner=Some(pocol_lp4_execution::txsign::Key::from_secret(&k.secret()).unwrap());
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        // F: 4368 x LOG0. H: a 150000-iteration loop, then one LOG0. O: one LOG0.
        let filler=vec![0x61,0x11,0x10,0x5b,0x60,0,0x60,0,0xa0,0x60,1,0x90,0x03,0x80,0x60,3,0x57,0x00];
        let heavy=vec![0x62,0x02,0x49,0xf0,0x5b,0x60,1,0x90,0x03,0x80,0x60,4,0x57,0x60,0,0x60,0,0xa0,0x00];
        let one=vec![0x60,0,0x60,0,0xa0,0x00];
        let d=deploy(&mut node,&[filler,heavy,one],cfg.g_ts+4);let (fa,ha,oa)=(d[0],d[1],d[2]);
        let price=next_base(&node)+U256::from(7);
        let call=|k:&str,to:Address,gas:u64|Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:price*U256::from(2),gas,to:Some(to),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key(k));
        let f=call("carol",fa,2_000_000);node.submit_tx(f.clone(),false).unwrap();
        for h in ["h1","h2","h3","h4"]{node.submit_tx(call(h,ha,3_990_000),false).unwrap();}
        let v=Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:price*U256::from(2),gas:1_000_000,to:Some(oa),value:U256::ZERO,data:Default::default()}.sign_1559(&alice);
        node.submit_tx(v.clone(),false).unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();let clock=cfg.g_ts+8;let sel=node.template_selection(&p,clock);
        assert_eq!(sel.txs,vec![f.clone()],"only the filler is selected; B4 leaves the heavy calls out");
        assert!(sel.rest.iter().any(|c|c.raw==v),"the one-log call is held for the append round");
        let (_,body)=node.build_template_unrecorded(&p,sel,clock).expect("a template");
        assert_eq!(body.txs,vec![f.clone()],"the honest append round leaves out the call that would halt");
        assert!(node.pool.txs.iter().any(|(_,r)|*r==v),"it stays pooled");
        // The template is valid and charges alice nothing.
        let before=balance(&node,alice.address());let b=solve(&node,body.txs,clock);assert!(node.import(b,None).unwrap());
        assert_eq!(balance(&node,alice.address()),before);
    }

    /// FN-2a: pocol_quoteCreationBurn is gone. A v3 node and a spec-1 node answer it exactly as any unknown method.
    #[test]
    fn the_creation_burn_quote_is_unknown_to_every_node(){
        for spec in [2u8,1]{
            let mut c=node_cfg(|_|{});c.spec_version=spec;let dir=Temp::new();let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&c)).unwrap();
            assert_eq!(node.chain.spec.v3_active(u64::MAX),spec==2);
            let call=serde_json::json!([{"to":new_address(162)}]);
            let quote=rpc(&mut node,"pocol_quoteCreationBurn",call.clone());let unknown=rpc(&mut node,"pocol_quoteCreationBurx",call);
            assert_eq!(quote["error"]["code"],-32601,"spec {spec}: {quote}");assert_eq!(quote["error"]["message"],"method pocol_quoteCreationBurn not found");assert_eq!(unknown["error"]["code"],-32601);
        }
    }

    #[test]
    fn log_count_and_byte_boundaries_are_inclusive_in_both_domains(){
        for v3 in [false,true]{let n=net(v3,|_|{});let limits=user_phase::log_caps(v3);let target=new_address(125);
            for extra in [0,1]{let mut db=n.genesis.state().clone();let count=limits.count+extra;let mut runtime=Vec::new();
                for _ in 0..count{runtime.extend_from_slice(&[0x60,0,0x60,0,0xa0])}runtime.push(0);code(&mut db,target,runtime);
                let o=run(&n,&db,&[tx(0,Some(target),0,vec![],5_000_000)]);
                if extra==0{assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),36*count)}else{assert_eq!(o.halts(),&[(0,Halt::LogCount)]);assert_eq!(o.log_bytes(),0)}
                let bytes=limits.bytes-36+extra;let mut runtime=vec![0x62,((bytes>>16)&255) as u8,((bytes>>8)&255) as u8,(bytes&255) as u8,0x60,0,0xa0,0];code(&mut db,target,std::mem::take(&mut runtime));
                let o=run(&n,&db,&[tx(0,Some(target),0,vec![],5_000_000)]);
                if extra==0{assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),limits.bytes)}else{assert_eq!(o.halts(),&[(0,Halt::LogBytes)]);assert_eq!(o.log_bytes(),0)}
            }
        }
    }


    /// A slot creation adds no log: a call that creates a slot and emits a log of exactly the remaining v3 block bytes
    /// succeeds; one more byte halts with LogBytes, reverts the slot and charges the declared gas.
    #[test]
    fn a_creation_adds_no_log_to_the_log_caps(){
        let n=net(true,|_|{});let target=new_address(126);
        for extra in [0,1]{let mut db=n.genesis.state().clone();let bytes=256*1024-36+extra;
            let mut runtime=vec![0x60,1,0x60,0,0x55,0x62,((bytes>>16)&255) as u8,((bytes>>8)&255) as u8,(bytes&255) as u8,0x60,0,0xa0,0];
            code(&mut db,target,std::mem::take(&mut runtime));let o=run(&n,&db,&[tx(0,Some(target),0,vec![],5_000_000)]);
            if extra==0{assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),256*1024);assert_eq!(o.state().slot(&target,&U256::ZERO),U256::from(1))}else{
                assert_eq!(o.halts(),&[(0,Halt::LogBytes)]);assert_eq!(o.log_bytes(),0);assert_eq!(o.state().slot(&target,&U256::ZERO),U256::ZERO);assert_eq!(o.commitments().gas_used,5_000_000);
            }
            pays_gas_only(&db,&o,0);
        }
    }

    #[test]
    fn create_collision_preserves_copied_storage_and_counts_no_fictitious_creations(){
        let n=net(true,|c|c.slot_cap=1);let mut db=n.genesis.state().clone();let alice=devnet::account_key("alice").address();let contract=alice.create(0);
        db.insert_account_info(contract,AccountInfo{balance:U256::from(1),..Default::default()});db.insert_account_storage(contract,U256::from(99),U256::from(7)).unwrap();
        // LP4-CREATE-01 deliberately includes nonzero storage in the collision
        // check, even with nonce zero and empty code. V3 preserves this rule.
        let raw=vec![tx(0,None,0,init(&[0x60,1,0x60,0,0x55,0]),200000),tx(1,Some(contract),0,vec![],100000)];
        let o=run(&n,&db,&raw);assert!(o.halts().is_empty(),"{:?}",o.halts());
        assert_eq!(o.state().slot(&contract,&U256::from(99)),U256::from(7));assert_eq!(o.state().slot(&contract,&U256::ZERO),U256::ZERO);assert_eq!(count(o.state()).user_slots,1);
        assert_eq!(o.state().accounts[&contract].info.nonce,0);assert_eq!(o.receipts()[0],receipts::encode(2,false,200000,&[]).unwrap());assert_eq!(o.receipts()[1],receipts::encode(2,true,221000,&[]).unwrap());
        let mut state=db.clone();let mut meter=PoolMeter::of(&state,&n.profile);let context=ctx(&n);
        for raw in raw{let env=wire::decode_for_profile(&raw,&n.profile).unwrap().tx_env();let total=count(&state).inventory();let r=native::execute_on_validated(&state,total,env.clone(),&context,&n.profile).unwrap();
            meter.admit(&state,&r.state).unwrap();user_phase::commit_tx(&mut state,r.state,true);let summary=count(&state);assert_eq!(meter.live(),(summary.user_slots,summary.user_accounts,summary.sys_slots));}
        assert_eq!(apply::state_root(&state).unwrap(),o.state_root(100,100).unwrap());
    }


    #[test]
    fn estimation_needs_only_affordable_transaction_gas_and_accepts_no_sender_reads_at_a_full_cap(){
        // No creation fee: at 1 gwei a transfer to a new account needs exactly 21000 gas. The balance covers 21000 gas
        // plus the value and nothing more: far below a block's gas, which the search must not require.
        let dir=Temp::new();let cfg=node_cfg(|c|c.accounts[0].1=U256::from(21_000_000_000_001u64));
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");let to=new_address(128);
        let params=serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1","gasPrice":"0x3b9aca00"},"latest"]);
        let estimate=rpc(&mut node,"eth_estimateGas",params);assert_eq!(estimate["result"],"0x5208","{estimate}");
        let signed=|gas|Tx{chain_id:777938,nonce:0,max_priority:U256::ZERO,max_fee:U256::from(1_000_000_000u64),gas,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(&alice);
        node.submit_tx(signed(21000),false).unwrap();
        // Boot with calibration headroom, then fill the cap through actual
        // paid transactions. A cap below the calibration fixture cannot boot.
        let dir=Temp::new();let cfg=node_cfg(|c|c.acct_cap=8);let mut full=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();
        let mut raw=Vec::new();for i in 0..5{let tx=node_tx(&full,&alice,i,new_address(130+i as u8),21000);full.submit_tx(tx.clone(),false).unwrap();raw.push(tx);}
        let block=solve(&full,raw,cfg.g_ts+4);assert!(full.import(block,None).unwrap());
        for from in [None,Some(serde_json::Value::Null)]{let mut request=serde_json::json!({"to":devnet::account_key("bob").address()});if let Some(from)=from{request["from"]=from;}
            let result=rpc(&mut full,"eth_estimateGas",serde_json::json!([request,"latest"]));assert!(result.get("error").is_none(),"a virtual query caller uses no live account unit: {result}");}
        assert_eq!(full.chain.head().summary.as_ref().unwrap().user_accounts,8);
    }

    /// N4 (V3-24): a v3 estimate models the v3 log caps (256 KiB / 4,368 per block). A call whose one LOG0 carries
    /// 32 KiB of data (past the spec-1 block cap, which the spec-1 estimate refuses: fee_pool.rs) is estimated; one whose
    /// LOG0 charges past 256 KiB is refused (LogBytes), and so is one that emits 4,369 logs (LogCount).
    #[test]
    fn v3_estimate_models_the_v3_log_caps(){
        let dir=Temp::new();let cfg=node_cfg(|_|{});let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        let caps=user_phase::log_caps(true);
        // PUSH3 size; PUSH1 0; LOG0; STOP.
        let one=|size:u64|vec![0x62,(size>>16) as u8,(size>>8) as u8,size as u8,0x60,0,0xa0,0];
        // PUSH2 count; loop: LOG0(0, 0); count-1; JUMPI loop while nonzero; STOP.
        let count=caps.count+1;let many=vec![0x61,(count>>8) as u8,(count&255) as u8,0x5b,0x60,0,0x60,0,0xa0,0x60,1,0x90,0x03,0x80,0x60,3,0x57,0];
        let head=node.chain.full_entry(&node.chain.head).unwrap();let fee=node.chain.child_ctx(&head,0).unwrap().base_fee*U256::from(2)+U256::from(7);
        let deploy=|nonce:u64,r:&[u8]|Tx{chain_id:777938,nonce,max_priority:U256::from(7),max_fee:fee,gas:300_000,to:None,value:U256::ZERO,data:Bytes::from(init(r))}.sign_1559(&alice);
        let b=solve(&node,vec![deploy(0,&one(32768)),deploy(1,&one(caps.bytes)),deploy(2,&many)],cfg.g_ts+4);assert!(node.import(b,None).unwrap());
        let est=|node:&mut pocold::node::Node,i:u64|rpc(node,"eth_estimateGas",serde_json::json!([{"from":alice.address(),"to":alice.address().create(i)},"latest"]));
        let r=est(&mut node,0);assert!(r.get("error").is_none(),"32 KiB of logs fit a v3 block: {r}");
        let r=est(&mut node,1);assert_eq!(r["error"]["code"],-32000,"{r}");assert_eq!(r["error"]["data"]["reason"],"LogBytes","{r}");
        let r=est(&mut node,2);assert_eq!(r["error"]["code"],-32000,"{r}");assert_eq!(r["error"]["data"]["reason"],"LogCount","{r}");
    }

    #[test]
    fn a_thousand_validated_empty_blocks_keep_the_v3_floor_and_legacy_london_decay(){
        for v3 in [false,true]{let mut cfg=node_cfg(|_|{});cfg.spec_version=if v3{2}else{1};let mut chain=pocold::chain::Chain::new(node_spec(&cfg));
            for height in 1..=1000{let clock=cfg.g_ts+height*u64::from(cfg.t_blk);let block=solve_chain(&chain,vec![],clock);
                let (entry,_)=chain.validate(&block,None,0).unwrap();assert_eq!(entry.gas_used,0);if v3{assert!(entry.base_fee>=cfg.base_fee_min,"height {height}");}
                assert!(chain.insert(entry));assert_eq!(chain.head().height,height);
            }
            assert_eq!(chain.head().base_fee,if v3{cfg.base_fee_min}else{U256::from(7)});
        }
    }

    /// C3-2: one v3 transaction may emit every block log (here 4,368 LOG0 of 24 bytes, 262,080 charged
    /// bytes). Its JSON receipt exceeds LIGHT_RESP_MAX, so a v3 node serves eth_getTransactionReceipt on
    /// the heavy lane (RESP_MAX); a spec-1 node keeps the light row.
    #[test]
    fn a_maximal_log_v3_receipt_is_served_on_the_heavy_lane(){
        let dir=Temp::new();let cfg=node_cfg(|_|{});let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        assert!(node.cfg.limits.rpc.heavy_receipts,"the v3 domain serves receipts heavy");
        let caps=user_phase::log_caps(true);let (count,data)=(caps.count,24u64);assert!(count*(36+data)<=caps.bytes&&count*(36+data+1)>caps.bytes);
        // PUSH2 count; loop: LOG0(0, 24); count-1; JUMPI loop while nonzero; STOP.
        let runtime=[0x61,(count>>8) as u8,(count&255) as u8,0x5b,0x60,data as u8,0x60,0,0xa0,0x60,1,0x90,0x03,0x80,0x60,3,0x57,0];
        let fee=|n:&pocold::node::Node|{let head=n.chain.full_entry(&n.chain.head).unwrap();n.chain.child_ctx(&head,0).unwrap().base_fee*U256::from(2)+U256::from(7)};
        let deploy=Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:fee(&node),gas:300_000,to:None,value:U256::ZERO,data:Bytes::from(init(&runtime))}.sign_1559(&alice);
        node.submit_tx(deploy.clone(),false).unwrap();let b=solve(&node,vec![deploy],cfg.g_ts+4);assert!(node.import(b,None).unwrap());
        let call=Tx{chain_id:777938,nonce:1,max_priority:U256::from(7),max_fee:fee(&node),gas:3_000_000,to:Some(alice.address().create(0)),value:U256::ZERO,data:Default::default()}.sign_1559(&alice);
        let before=node.chain.head().cum_log_bytes;let hash=node.submit_tx(call.clone(),false).unwrap();let b=solve(&node,vec![call],cfg.g_ts+8);assert!(node.import(b,None).unwrap());
        assert_eq!(node.chain.head().cum_log_bytes-before,count*(36+data),"the receipt holds every block log");
        let body=serde_json::json!({"jsonrpc":"2.0","id":1,"method":"eth_getTransactionReceipt","params":[hash]}).to_string();
        assert!(pocold::rpc::is_light(body.as_bytes()),"the legacy row is light");
        let light=pocold::rpc::is_light_for(body.as_bytes(),node.cfg.limits.rpc.heavy_receipts);assert!(!light,"the v3 connection queues it heavy");
        let ask=|node:&mut pocold::node::Node,light:bool|{let (t,_r)=std::sync::mpsc::channel();
            pocold::rpc::answer(node,&pocold::rpc::Request::local(body.clone().into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(60),light)).body};
        let served=ask(&mut node,light);assert!(served.len()>pocold::rpc::LIGHT_RESP_MAX&&served.len()<=pocold::rpc::RESP_MAX,"{}",served.len());
        assert!(served.len() as u64<=r26_receipt_bound(),"the v3 R26 receipt bound covers it: {}",served.len());
        let v:serde_json::Value=serde_json::from_str(&served).unwrap();assert_eq!(v["result"]["status"],"0x1","{}",&served[..200]);
        let logs=v["result"]["logs"].as_array().unwrap();assert_eq!(logs.len() as u64,count);assert_eq!(logs[0]["data"].as_str().unwrap().len(),2+2*data as usize);
        assert_eq!(logs[count as usize-1]["logIndex"],format!("{:#x}",count-1));
        // The light reservation cannot hold it: the reason the v3 row is heavy.
        let forced=ask(&mut node,true);assert!(forced.contains("resultBytes")&&!forced.contains("\"result\""),"{forced}");
        // The in-process handler applies the same v3 answer limit.
        assert_eq!(rpc(&mut node,"eth_getTransactionReceipt",serde_json::json!([hash]))["result"]["logs"].as_array().unwrap().len() as u64,count);
        let mut legacy=node_cfg(|_|{});legacy.spec_version=1;let dir=Temp::new();let old=pocold::node::Node::open(node_config(&dir,false),node_spec(&legacy)).unwrap();
        assert!(!old.cfg.limits.rpc.heavy_receipts,"spec-1 keeps the light receipt row");
    }

    /// The v3 R26 receipt row (param_gate.rs satisfies_r26_for, network.md (B)):
    /// 2·V3_LOG_BLOCK_MAX + V3_LOG_COUNT_MAX·512 B + 2 KiB.
    fn r26_receipt_bound()->u64{let c=user_phase::log_caps(true);2*c.bytes+c.count*512+2048}

    /// C3-2, topics and data: a v3 receipt whose logs carry four topics and data each (720 LOG4 of
    /// 200 bytes, 262,080 charged bytes) exercises the 2·charged-bytes term of the R26 receipt bound.
    #[test]
    fn a_full_topic_and_data_v3_receipt_stays_within_the_r26_receipt_bound(){
        let dir=Temp::new();let cfg=node_cfg(|_|{});let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        let caps=user_phase::log_caps(true);let data=200u64;let per=36+4*32+data;let count=caps.bytes/per;assert!(count<=caps.count&&count*per+per>caps.bytes);
        // PUSH2 count; loop: LOG4(0, data, n, n, n, n); count-1; JUMPI loop while nonzero; STOP.
        let runtime=[0x61,(count>>8) as u8,(count&255) as u8,0x5b,0x80,0x80,0x80,0x80,0x60,data as u8,0x60,0,0xa4,0x60,1,0x90,0x03,0x80,0x60,3,0x57,0];
        let fee=|n:&pocold::node::Node|{let head=n.chain.full_entry(&n.chain.head).unwrap();n.chain.child_ctx(&head,0).unwrap().base_fee*U256::from(2)+U256::from(7)};
        let deploy=Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:fee(&node),gas:300_000,to:None,value:U256::ZERO,data:Bytes::from(init(&runtime))}.sign_1559(&alice);
        node.submit_tx(deploy.clone(),false).unwrap();let b=solve(&node,vec![deploy],cfg.g_ts+4);assert!(node.import(b,None).unwrap());
        let call=Tx{chain_id:777938,nonce:1,max_priority:U256::from(7),max_fee:fee(&node),gas:3_000_000,to:Some(alice.address().create(0)),value:U256::ZERO,data:Default::default()}.sign_1559(&alice);
        let before=node.chain.head().cum_log_bytes;let hash=node.submit_tx(call.clone(),false).unwrap();let b=solve(&node,vec![call],cfg.g_ts+8);assert!(node.import(b,None).unwrap());
        assert_eq!(node.chain.head().cum_log_bytes-before,count*per,"the receipt holds every block log");
        let body=serde_json::json!({"jsonrpc":"2.0","id":1,"method":"eth_getTransactionReceipt","params":[hash]}).to_string();
        let light=pocold::rpc::is_light_for(body.as_bytes(),node.cfg.limits.rpc.heavy_receipts);assert!(!light);
        let (t,_r)=std::sync::mpsc::channel();
        let served=pocold::rpc::answer(&mut node,&pocold::rpc::Request::local(body.into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(60),light)).body;
        assert!(served.len()>pocold::rpc::LIGHT_RESP_MAX&&served.len() as u64<=r26_receipt_bound(),"{}",served.len());
        let v:serde_json::Value=serde_json::from_str(&served).unwrap();assert_eq!(v["result"]["status"],"0x1","{}",&served[..200]);
        let logs=v["result"]["logs"].as_array().unwrap();assert_eq!(logs.len() as u64,count);
        assert_eq!(logs[0]["topics"].as_array().unwrap().len(),4);assert_eq!(logs[0]["data"].as_str().unwrap().len(),2+2*data as usize);
    }


    /// An estimate observes the next block's real GASLIMIT (8M), not the larger eth_call ceiling: the runtime below
    /// writes a slot only when GASLIMIT differs from 8M. SLOT_CAP is full (the init code wrote slot 5), so at the
    /// 30M ceiling the estimate would answer SlotCap; at the real limit it is a read.
    #[test]
    fn estimation_uses_the_actual_block_gaslimit_opcode(){
        let dir=Temp::new();let cfg=node_cfg(|c|c.slot_cap=1);let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_slots,0);
        let runtime=[0x45,0x62,0x7a,0x12,0x00,0x14,0x60,15,0x57,0x60,1,0x60,0,0x55,0,0x5b,0];
        // SSTORE(5, 1), then the init of `init`, with its code offset moved past the 5-byte prefix.
        let len=runtime.len() as u8;let mut initcode=vec![0x60,1,0x60,5,0x55,0x60,len,0x60,17,0x60,0,0x39,0x60,len,0x60,0,0xf3];initcode.extend_from_slice(&runtime);
        let raw=Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:U256::from(2_000_000_007u64),gas:200000,to:None,value:U256::ZERO,data:Bytes::from(initcode)}.sign_1559(&alice);
        node.submit_tx(raw.clone(),false).unwrap();let block=solve(&node,vec![raw],cfg.g_ts+4);assert!(node.import(block,None).unwrap());
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_slots,1,"the cap is full");
        let result=rpc(&mut node,"eth_estimateGas",serde_json::json!([{"from":alice.address(),"to":alice.address().create(0)},"latest"]));
        assert!(result.get("error").is_none(),"the next block has an 8M GASLIMIT and creates no slot: {result}");
    }

    /// A 1559 transfer of 1 wei with an explicit tip and gas limit.
    fn tipped(key:&pocol_lp4_execution::txsign::Key,nonce:u64,tip:u64,max_fee:U256,to:Address,gas:u64)->Vec<u8>{
        Tx{chain_id:777938,nonce,max_priority:U256::from(tip),max_fee,gas,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(key)
    }

    /// N4 x D-V3-21 review, documented (network.md, Replace-by-fee): a pooled replacement is checked at the pending
    /// state's utilisation, which counts what the old transaction creates, so near a cap it counts as if one more unit
    /// were taken. bob's four creations bring the pending state to 7 of 8 accounts; alice's transfer creating the 8th
    /// is pooled and selected, but her fee bump of it (same gas limit and recipient) is refused (AcctCap) and the
    /// original stays pooled and selected.
    #[test]
    fn a_replacement_creating_the_last_free_account_is_counted_with_its_predecessor(){
        let dir=Temp::new();let cfg=node_cfg(|c|c.acct_cap=8);
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,3);
        let max_fee=U256::from(100_000_000_000u64);let to=new_address(180);
        let bob=devnet::account_key("bob");for i in 0..4u8{node.submit_tx(priced_tx(&bob,u64::from(i),new_address(170+i),U256::from(1),100_000,max_fee),false).unwrap();}
        let first=priced_tx(&alice,0,to,U256::from(1),100_000,max_fee);node.submit_tx(first.clone(),false).unwrap();
        let bumped=Tx{chain_id:777938,nonce:0,max_priority:U256::from(100),max_fee:max_fee*U256::from(2),gas:100_000,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(&alice);
        assert_eq!(node.submit_tx(bumped,false).unwrap_err(),"-32000 pending state would exceed a cap (AcctCap)");
        assert_eq!((node.pool.txs.len(),node.stats.pool_replaced),(5,0));
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_selection(&p,cfg.g_ts+4).txs.contains(&first));
    }

    /// N4 x D-V3-21 re-review: neither the head nor the pending state has a parked transaction's effects, so under v3 a
    /// transaction behind its sender's parked ones evicts nothing, whatever it would pay: alice's parked creation of X
    /// and her follow-up transfer to X (valid in sequence; with free room it is parked) at a full parked set would evict
    /// `low` by fee, and so would her plain transfer to a funded account. Both are refused "pool full (gas)", counted in
    /// evictRefused, and low stays.
    #[test]
    fn a_parked_run_member_evicts_nothing_under_v3(){
        let cfg=node_cfg(|c|{c.acct_cap=64;for s in ["filler","low"]{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let open=|dir:&Temp|{let mut config=node_config(dir,false);config.limits.pool_gas_max=Some(8_000_000);pocold::node::Node::open(config,node_spec(&cfg)).unwrap()};
        let max_fee=U256::from(100_000_000_000u64);let sink=devnet::account_key("bob").address();let key=|s:&str|devnet::account_key(s);
        let x=new_address(232);let alice=key("alice");
        let (d1,d2)=(Temp::new(),Temp::new());let (mut node,mut free)=(open(&d1),open(&d2));
        let gl=node.chain.spec.cp().GAS_LIMIT;
        let g=100_000u64;
        let create=tipped(&alice,0,5_000_000_000,max_fee,x,g);let follow=tipped(&alice,1,3_000_000_000,max_fee,x,21000);
        let low=tipped(&key("low"),0,1_000_000_000,max_fee,sink,21000);
        for n in [&mut node,&mut free]{
            n.submit_tx(tipped(&key("filler"),0,1_000_000_000,max_fee,sink,gl),false).unwrap();
            assert_eq!(n.submit_tx(low.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
            assert_eq!(n.submit_tx(create.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        }
        free.set_pool_parked_max(3);assert_eq!(free.submit_tx(follow.clone(),false).unwrap_err(),pocold::node::POOL_PARKED,"with room it is parked");
        node.set_pool_parked_max(2);
        let refused=node.stats.evict_refused;
        assert_eq!(node.submit_tx(follow.clone(),false).unwrap_err(),pocold::node::POOL_FULL_GAS);
        assert_eq!(node.submit_tx(tipped(&alice,1,3_000_000_000,max_fee,sink,21000),false).unwrap_err(),pocold::node::POOL_FULL_GAS);
        assert_eq!((node.stats.parked_fee_evicted,node.stats.parked_evicted,node.stats.evict_refused),(0,0,refused+2));
        assert_eq!(node.pool_parked().len(),2);assert!(node.pool_parked().contains(&low));node.pool_audit().unwrap();
    }

    /// A node fixture, with `funded` accounts and "cap" added, whose ACCT_CAP leaves CAP_FREE accounts free on the
    /// genesis head (the fewest the admission calibration, which adds its sender and spin contract, accepts).
    const CAP_FREE:u64=2;
    fn capped_cfg(funded:&[String])->DevnetConfig{
        let tune=|c:&mut DevnetConfig|{for s in funded.iter().map(|s|s.as_str()).chain(["cap"]){c.accounts.push((s.into(),U256::from(10u128.pow(24))));}};
        let mut cfg=node_cfg(|c|{c.acct_cap=64;tune(c)});
        let used=count(devnet::build(&cfg).unwrap().genesis.state()).user_accounts;cfg.acct_cap=used+CAP_FREE;cfg
    }
    /// Pools `n` transfers from "cap" (5 gwei) that each create an account, so the pending state leaves
    /// CAP_FREE - n accounts free under ACCT_CAP; returns them.
    fn take_free(node:&mut pocold::node::Node,n:u64)->Vec<Vec<u8>>{
        let cap=node.chain.spec.cp().ACCT_CAP;assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts+CAP_FREE,cap);
        let key=devnet::account_key("cap");let max_fee=U256::from(100_000_000_000u64);
        (0..n).map(|i|{let raw=tipped(&key,i,5_000_000_000,max_fee,new_address(150+i as u8),21000);node.submit_tx(raw.clone(),false).unwrap();raw}).collect()
    }
    fn names(v:&[&str])->Vec<String>{v.iter().map(|s|(*s).to_string()).collect()}
    const AT_CAP:&str="-32000 pending state would exceed a cap (AcctCap)";

    /// FN-2a conversion of the D-V3-21 count-bound test, on a live cap: the pool's checks run before the victim leaves,
    /// so a newcomer that would pass ACCT_CAP evicts nothing. Pooled creations fill ACCT_CAP in the pending state and the
    /// pool is full by count (the rest transfers to an existing account at 1 gwei); zed outbids them at 3 gwei with a 21000-gas transfer
    /// to a new account: refused AcctCap by the head checks (counted in evictRefused), and the pool, its ghost gas and
    /// the eviction count are unchanged. The same bid to an existing account evicts one.
    #[test]
    fn an_over_cap_newcomer_at_the_count_bound_evicts_nothing(){
        use pocold::node::POOL_MAX_TXS;
        let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let mut funded=senders.clone();funded.push("zed".into());
        let cfg=capped_cfg(&funded);
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(POOL_MAX_TXS as u64*21000+2*21000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();take_free(&mut node,CAP_FREE);
        let max_fee=U256::from(100_000_000_000u64);assert!(next_base(&node)+U256::from(3_000_000_000u64)<max_fee);let sink=devnet::account_key("alice").address();
        let mut nonce=vec![0u64;senders.len()];let mut k=0;
        while node.pool.txs.len()<POOL_MAX_TXS{let i=k%senders.len();node.submit_tx(tipped(&devnet::account_key(&senders[i]),nonce[i],1_000_000_000,max_fee,sink,21000),false).unwrap();nonce[i]+=1;k+=1;}
        node.pool_audit().unwrap();
        let zed=devnet::account_key("zed");let refused=node.stats.evict_refused;
        assert_eq!(node.submit_tx(tipped(&zed,0,3_000_000_000,max_fee,new_address(190),21000),false).unwrap_err(),AT_CAP);
        assert_eq!((node.pool.txs.len(),node.pool.ghost_gas,node.stats.pool_evicted,node.stats.evict_refused),(POOL_MAX_TXS,0,0,refused+1),"nothing evicted");
        node.pool_audit().unwrap();
        node.submit_tx(tipped(&zed,0,3_000_000_000,max_fee,sink,21000),false).unwrap();
        assert_eq!((node.pool.txs.len(),node.pool.ghost_gas,node.stats.pool_evicted),(POOL_MAX_TXS,21000,1),"the same bid within the cap evicts one");node.pool_audit().unwrap();
    }

    /// FN-2a conversion of the D-V3-21 ghost-write test, on a live cap: a replaced (or evicted) transaction's writes
    /// stay in the pending state until the next head, so there a creation can look done. ACCT_CAP has one free account in
    /// the pending state;
    /// alice pools a transfer creating account A (the pending state is then at the cap) and replaces it with a plain
    /// transfer; A stays in the pending state and in its utilisation. At the count bound a newcomer that would evict
    /// also passes the pool's checks on the head state: 21000-gas transfers to A, which create nothing on the pending
    /// state but create A on the head, are refused AcctCap and evict nothing (counted in evictRefused); so is one to
    /// a new account. The same bid to a funded account evicts one.
    #[test]
    fn a_ghost_write_lets_no_over_cap_newcomer_evict(){
        use pocold::node::POOL_MAX_TXS;
        let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let attackers:Vec<String>=(0..8).map(|i|format!("a{i}")).collect();
        let funded:Vec<String>=senders.iter().chain(attackers.iter()).cloned().chain(["zed".to_string()]).collect();
        let cfg=capped_cfg(&funded);let max_fee=U256::from(100_000_000_000u64);
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(POOL_MAX_TXS as u64*21000+10_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();take_free(&mut node,CAP_FREE-1);
        assert!(next_base(&node)+U256::from(6_000_000_000u64)<max_fee);
        let sink=devnet::account_key("alice").address();let a_new=new_address(220);let alice=devnet::account_key("alice");
        node.submit_tx(tipped(&alice,0,5_000_000_000,max_fee,a_new,21000),false).unwrap();
        let plain=tipped(&alice,0,6_000_000_000,max_fee*U256::from(2),sink,21000);node.submit_tx(plain.clone(),false).unwrap();assert_eq!(node.stats.pool_replaced,1);
        let mut nonce=vec![0u64;senders.len()];let mut k=0;
        while node.pool.txs.len()<POOL_MAX_TXS{let i=k%senders.len();node.submit_tx(tipped(&devnet::account_key(&senders[i]),nonce[i],1_000_000_000,max_fee,sink,21000),false).unwrap();nonce[i]+=1;k+=1;}
        node.pool_audit().unwrap();let refused=node.stats.evict_refused;
        assert_eq!(node.submit_tx(tipped(&devnet::account_key("zed"),0,3_000_000_000,max_fee,new_address(221),21000),false).unwrap_err(),AT_CAP);
        for a in &attackers{
            assert_eq!(node.submit_tx(tipped(&devnet::account_key(a),0,3_000_000_000,max_fee,a_new,21000),false).unwrap_err(),AT_CAP,"on the head a transfer to A creates it");
        }
        assert_eq!((node.pool.txs.len(),node.stats.pool_evicted,node.pool.ghost_gas,node.stats.evict_refused),(POOL_MAX_TXS,0,0,refused+1+attackers.len() as u64),"nothing evicted");
        node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_txs(&p).contains(&plain));
        // The same bid to a funded account evicts one, as before.
        node.submit_tx(tipped(&devnet::account_key(&attackers[0]),0,3_000_000_000,max_fee,sink,21000),false).unwrap();
        assert_eq!((node.pool.txs.len(),node.stats.pool_evicted,node.pool.ghost_gas),(POOL_MAX_TXS,1,21000));node.pool_audit().unwrap();
    }

    /// FN-2a conversion, on a live cap: a parked transaction is not executed, so before one evicts a parked one by fee
    /// it passes the pool's checks. Pooled creations fill ACCT_CAP; a 21000-gas transfer to a new account at a full parked set is
    /// refused AcctCap (counted in evictRefused) and the valid parked transaction stays and is selected. The same bid
    /// to a funded account evicts it. With free parked room the over-cap one is still parked (it is not executed;
    /// network.md, Parked set), and the selection skips it.
    #[test]
    fn a_parked_over_cap_newcomer_evicts_nothing_by_fee(){
        let cfg=capped_cfg(&names(&["filler","low","hi"]));
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(8_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        let gl=node.chain.spec.cp().GAS_LIMIT;assert_eq!(gl,8_000_000);take_free(&mut node,CAP_FREE);
        let max_fee=U256::from(100_000_000_000u64);let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        node.submit_tx(tipped(&key("filler"),0,1_000_000_000,max_fee,sink,gl-CAP_FREE*21000),false).unwrap();
        assert_eq!(node.pool.gas,node.pool.gas_max,"the gas bound is held");
        node.set_pool_parked_max(1);
        let low=tipped(&key("low"),0,1_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(low.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let hi=tipped(&key("hi"),0,3_000_000_000,max_fee,new_address(199),21000);let refused=node.stats.evict_refused;
        assert_eq!(node.submit_tx(hi.clone(),false).unwrap_err(),AT_CAP);
        assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted,node.stats.parked_evicted,node.stats.evict_refused),(vec![low.clone()],0,0,refused+1),"nothing evicted");
        node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_selection(&p,cfg.g_ts+4).txs.contains(&low));
        // The same bid within the cap evicts it, as before.
        let hi2=tipped(&key("bob"),0,3_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(hi2.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted),(vec![hi2.clone()],1));node.pool_audit().unwrap();
        // Free parked room: the over-cap one is held (two reads, no execution) but never selected.
        node.set_pool_parked_max(2);
        assert_eq!(node.submit_tx(hi.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let sel=node.template_selection(&p,cfg.g_ts+4);assert!(sel.txs.contains(&hi2)&&!sel.txs.contains(&hi));
    }

    /// FN-2a conversion, on a live cap: the same for PR6's declared-gas eviction, which comes first. Honest parked
    /// transactions: one declaring the most gas (PR6's first victim) and four cheap transfers, all to funded accounts,
    /// with ACCT_CAP filled by pooled creations. 21000-gas transfers to new accounts at a higher tip are refused AcctCap and evict none of
    /// them; the template is unchanged. A valid newcomer still evicts the large one by PR6's rule.
    #[test]
    fn parked_over_cap_newcomers_evict_no_honest_parked_ones(){
        let honest=["h0","h1","h2","h3"];let spam=["z0","z1","z2","z3","z4"];
        let cfg=capped_cfg(&names(&[&honest[..],&spam[..],&["filler","maker","valid"][..]].concat()));
        let spec=node_spec(&cfg);let gl=spec.cp().GAS_LIMIT;
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(gl);config.limits.pool_sender_gas_max=Some(gl);
        let mut node=pocold::node::Node::open(config,spec).unwrap();node.set_pool_parked_max(5);take_free(&mut node,CAP_FREE);
        let max_fee=U256::from(100_000_000_000u64);assert!(next_base(&node)+U256::from(3_000_000_000u64)<max_fee);
        let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        let filler=tipped(&key("filler"),0,2_000_000_000,max_fee,sink,gl-CAP_FREE*21000);node.submit_tx(filler.clone(),false).unwrap();
        assert_eq!(node.pool.gas,gl,"the gas bound is held");
        let made=tipped(&key("maker"),0,2_000_000_000,max_fee,sink,200_000);
        assert_eq!(node.submit_tx(made.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        for h in honest{assert_eq!(node.submit_tx(tipped(&key(h),0,1_000_000_000,max_fee,sink,21000),false).unwrap_err(),pocold::node::POOL_PARKED);}
        let parked=node.pool_parked();assert_eq!(parked.len(),5);node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();let before=node.template_txs(&p);
        assert_eq!(before.len(),8,"the two creations, the filler and the 5 honest parked ones");
        let refused=node.stats.evict_refused;
        for (i,z) in spam.iter().enumerate(){
            assert_eq!(node.submit_tx(tipped(&key(z),0,3_000_000_000,max_fee,new_address(210+i as u8),21000),false).unwrap_err(),AT_CAP);
        }
        assert_eq!((node.pool_parked(),node.stats.parked_evicted,node.stats.parked_fee_evicted,node.stats.evict_refused),(parked.clone(),0,0,refused+spam.len() as u64),"no honest parked one was evicted");
        node.pool_audit().unwrap();assert_eq!(node.template_txs(&p),before);
        // A valid newcomer declaring less gas evicts the large one by PR6's rule, as before.
        let valid=tipped(&key("valid"),0,3_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(valid.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let now=node.pool_parked();assert!(now.contains(&valid)&&!now.contains(&made));assert_eq!(node.stats.parked_evicted,1);node.pool_audit().unwrap();
    }

    /// FN-2a conversion, on a live cap: a parked transaction whose cap use depends on a pooled transaction a block
    /// takes before it. tx1 (pooled, 3 gwei) sends 1 wei to X; tx2 (3 gwei) is a creation whose init code writes two
    /// slots only when the address it reads has a balance, with SLOT_CAP 1. On the head X is empty, so the head checks
    /// alone pass it; every block takes tx1 first, so tx2 would pass SLOT_CAP. At a full parked set it would evict
    /// `low` by fee: it is also checked on the pending state (which has tx1's effects), so it is refused SlotCap, low
    /// stays and is selected, and the refusal counts in evictRefused. The same holds when tx1 is another sender's.
    /// The control (the init code reads an address no one funds) evicts low and is selected.
    fn predecessor_case(watch:u8,same_sender:bool)->Result<(),String>{
        let cfg=node_cfg(|c|{c.slot_cap=1;for s in ["filler","low","ann"]{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(8_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_slots,0);
        let max_fee=U256::from(100_000_000_000u64);assert!(next_base(&node)+U256::from(3_000_000_000u64)<max_fee);
        let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        let ann=key("ann");let first=if same_sender{key("ann")}else{key("bob")};
        let tx1=tipped(&first,0,3_000_000_000,max_fee,new_address(230),100_000);node.submit_tx(tx1.clone(),false).unwrap();
        let filler=tipped(&key("filler"),0,3_000_000_000,max_fee,sink,8_000_000-150_000);node.submit_tx(filler.clone(),false).unwrap();
        node.set_pool_parked_max(1);
        let low=tipped(&key("low"),0,1_000_000_000,max_fee,sink,60_000);
        assert_eq!(node.submit_tx(low.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        // PUSH20 watch; BALANCE; JUMPI 26; STOP; 26: JUMPDEST; SSTORE(0, 1); SSTORE(1, 1); STOP.
        let mut code=vec![0x73];code.extend_from_slice(new_address(watch).as_slice());code.extend_from_slice(&[0x31,0x60,26,0x57,0x00,0x5b,0x60,1,0x60,0,0x55,0x60,1,0x60,1,0x55,0x00]);
        assert_eq!(code[26],0x5b);
        let tx2=Tx{chain_id:777938,nonce:u64::from(same_sender),max_priority:U256::from(3_000_000_000u64),max_fee,gas:200_000,to:None,value:U256::ZERO,data:Bytes::from(code)}.sign_1559(&ann);
        let refused=node.stats.evict_refused;
        let e=node.submit_tx(tx2.clone(),false).unwrap_err();
        let p=node.chain.full_entry(&node.chain.head).unwrap();let sel=node.template_selection(&p,cfg.g_ts+4).txs;
        assert!(sel.contains(&tx1)&&sel.contains(&filler));node.pool_audit().unwrap();
        if e==pocold::node::POOL_PARKED{
            assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted,node.stats.evict_refused),(vec![tx2.clone()],1,refused));
            assert!(sel.contains(&tx2),"the control is selected");
            return Ok(())
        }
        assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted,node.stats.evict_refused),(vec![low.clone()],0,refused+1),"nothing evicted");
        assert!(sel.contains(&low));
        Err(e)
    }
    #[test]
    fn a_parked_newcomer_whose_cap_use_depends_on_a_pooled_predecessor_evicts_nothing(){
        for same in [true,false]{
            assert_eq!(predecessor_case(231,same),Ok(()),"control: the init code reads an address no one funds");
            assert_eq!(predecessor_case(230,same).unwrap_err(),"-32000 pending state would exceed a cap (SlotCap)","same sender: {same}");
        }
    }

    /// FN-2a conversion, on a live cap: a replacement of a sender's first parked transaction passes the checks an
    /// evicting one passes, so a transaction cannot evict by fee and then be swapped for one that could not have
    /// evicted. Pooled creations fill ACCT_CAP. hi's valid transfer at 3 gwei evicts `low`; its bumped replacement (4 gwei, 2x
    /// max_fee, 21000 gas) to a new account is refused AcctCap, and the parked set stays [good], which is selected. A
    /// bumped replacement within the cap replaces it.
    #[test]
    fn a_parked_replacement_passes_the_eviction_checks(){
        let cfg=capped_cfg(&names(&["filler","low","hi"]));
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(8_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        let gl=node.chain.spec.cp().GAS_LIMIT;take_free(&mut node,CAP_FREE);
        let max_fee=U256::from(100_000_000_000u64);let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        node.submit_tx(tipped(&key("filler"),0,1_000_000_000,max_fee,sink,gl-CAP_FREE*21000),false).unwrap();
        node.set_pool_parked_max(1);
        let low=tipped(&key("low"),0,1_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(low.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let good=tipped(&key("hi"),0,3_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(good.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted),(vec![good.clone()],1),"low evicted by a valid one");
        let bad=tipped(&key("hi"),0,4_000_000_000,max_fee*U256::from(2),new_address(231),21000);
        assert_eq!(node.submit_tx(bad.clone(),false).unwrap_err(),AT_CAP);
        assert_eq!((node.pool_parked(),node.stats.pool_replaced),(vec![good.clone()],0),"the parked one stays");node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_selection(&p,cfg.g_ts+4).txs.contains(&good));
        let ok=tipped(&key("hi"),0,4_000_000_000,max_fee*U256::from(2),sink,21000);
        node.submit_tx(ok.clone(),false).unwrap();
        assert_eq!((node.pool_parked(),node.stats.pool_replaced),(vec![ok.clone()],1));node.pool_audit().unwrap();
    }

    /// FN-2a conversion, documented (network.md, Lowest-fee eviction): the eviction checks run a newcomer on the head
    /// state too, where its sender's own earlier pooled transactions have not run. ACCT_CAP has one free account in the
    /// pending state, which
    /// alice's pooled transfer creating X takes in the pending state. Her follow-up (a 21000-gas transfer to X) is
    /// valid in sequence and, below the bounds, admitted and selected after it; at the count bound, with cheaper
    /// victims, it is a creation on the head past the pending utilisation and refused (AcctCap) until the next head,
    /// and nothing is evicted. The same bid to a funded account evicts one.
    #[test]
    fn a_follow_up_to_its_senders_pooled_creation_is_checked_on_the_head_at_the_bound(){
        use pocold::node::POOL_MAX_TXS;
        let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();
        let cfg=capped_cfg(&senders);let max_fee=U256::from(100_000_000_000u64);
        let open=|dir:&Temp|{let mut config=node_config(dir,false);config.limits.pool_gas_max=Some(POOL_MAX_TXS as u64*21000+10_000_000);pocold::node::Node::open(config,node_spec(&cfg)).unwrap()};
        let (d1,d2)=(Temp::new(),Temp::new());let (mut node,mut below)=(open(&d1),open(&d2));
        for n in [&mut node,&mut below]{take_free(n,CAP_FREE-1);}
        let sink=devnet::account_key("bob").address();let x=new_address(230);let alice=devnet::account_key("alice");
        let create=tipped(&alice,0,5_000_000_000,max_fee,x,21000);let follow=tipped(&alice,1,3_000_000_000,max_fee,x,21000);
        below.submit_tx(create.clone(),false).unwrap();below.submit_tx(follow.clone(),false).unwrap();
        let p=below.chain.full_entry(&below.chain.head).unwrap();let sel=below.template_txs(&p);
        assert!(sel.contains(&create)&&sel.contains(&follow),"valid in sequence");
        node.submit_tx(create.clone(),false).unwrap();
        let mut nonce=vec![0u64;senders.len()];let mut k=0;
        while node.pool.txs.len()<POOL_MAX_TXS{let i=k%senders.len();node.submit_tx(tipped(&devnet::account_key(&senders[i]),nonce[i],1_000_000_000,max_fee,sink,21000),false).unwrap();nonce[i]+=1;k+=1;}
        let refused=node.stats.evict_refused;
        assert_eq!(node.submit_tx(follow.clone(),false).unwrap_err(),AT_CAP);
        assert_eq!((node.stats.pool_evicted,node.stats.evict_refused,node.pool.txs.len()),(0,refused+1,POOL_MAX_TXS),"nothing evicted");node.pool_audit().unwrap();
        node.submit_tx(tipped(&alice,1,3_000_000_000,max_fee,sink,21000),false).unwrap();assert_eq!(node.stats.pool_evicted,1);node.pool_audit().unwrap();
    }
}
