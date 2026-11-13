//! V3-20/21/24 on disposable fixtures. No production profile or coefficients.
use pocol_lp4_execution::{creation_fees,fees};
use revm::primitives::U256;

#[test]
fn fee_floor_and_exact_creation_prices_have_no_legacy_shortcut(){
    let floor=U256::from(10_000_000);let mut old=U256::from(1_000_000_000);let mut new=old;
    for _ in 0..1000{old=fees::next_base_fee(old,0,8_000_000,U256::ZERO).unwrap();new=fees::next_base_fee(new,0,8_000_000,floor).unwrap();assert!(new>=floor)}
    assert_eq!(old,U256::from(7));assert_eq!(new,floor);
    assert_eq!(creation_fees::price(U256::from(100),10,7,3),Some(U256::from(333+500+1000)));
}

#[cfg(feature="v3-preview")]
mod preview{
    use super::*;
    use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},native::{self,NativeBlockContext},receipts,state_summary::Summary,txsign::Tx,user_phase::{self,Halt,PoolMeter},view::View,wire};
    use revm::{InMemoryDB,primitives::{AccountInfo,Address,B256,Bytecode,Bytes,Log,alloy_primitives::U512}};

    fn net(v3:bool,tune:impl FnOnce(&mut DevnetConfig))->devnet::Devnet{
        let mut c=DevnetConfig::new(777938,1_800_000_000,&["execution"]);
        c.spec_version=if v3{2}else{1};c.earnin=true;c.nonce_mode=1;
        c.f0_acct=U256::from(1000);c.f0_slot=U256::from(2000);c.f0_code=U256::from(3);
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
    /// The per-block creation burn as the receipts carry it: the sum of every CreationFeeBurn
    /// annotation from ChainCaps (no block report or ChainFacts field holds it).
    fn receipt_burn(receipts:&[Vec<u8>])->creation_fees::CreationBurn{
        let topic=revm::primitives::keccak256("CreationFeeBurn(address,uint256,uint256,uint256)");let mut total=creation_fees::CreationBurn::default();
        for r in receipts{
            let body=if r.first().is_some_and(|b|*b<0x7f){&r[1..]}else{&r[..]};let it=pocold::rlp::decode(body).unwrap();let f=it.list().unwrap();
            for l in f[3].list().unwrap(){let lf=l.list().unwrap();
                if lf[0].bytes().unwrap()!=apply::caps().as_slice()||lf[1].list().unwrap().first().and_then(|t|t.bytes())!=Some(topic.as_slice()){continue}
                let d=lf[2].bytes().unwrap();assert_eq!(d.len(),96);let w=|i:usize|U256::from_be_slice(&d[32*i..32*i+32]);
                total=total.checked_add(creation_fees::CreationBurn{accounts:w(0),slots:w(1),code:w(2)}).unwrap();
            }
        }
        total
    }
    fn init(runtime:&[u8])->Vec<u8>{assert!(runtime.len()<256);let len=runtime.len() as u8;let mut b=vec![0x60,len,0x60,12,0x60,0,0x39,0x60,len,0x60,0,0xf3];b.extend_from_slice(runtime);b}
    struct Temp(std::path::PathBuf);
    impl Temp{fn new()->Self{use std::sync::atomic::{AtomicU64,Ordering};static N:AtomicU64=AtomicU64::new(0);Self(std::env::temp_dir().join(format!("pocol-v3-execution-{}-{}",std::process::id(),N.fetch_add(1,Ordering::Relaxed))))}}
    impl Drop for Temp{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.0);}}
    fn node_config(dir:&Temp,store:bool)->pocold::node::Config{pocold::node::Config{dir:dir.0.clone(),name:"execution-test".into(),miner:None,listen:None,rpc:None,peers:vec![],stop_at:None,log:false,
        limits:pocold::node::Limits{r_keep:4,side_cap:2,state_store:store,history_window:Some(2),slim_depth:Some(4),pool_sender_gas_max:Some(8_000_000),..Default::default()}}}
    fn node_cfg(tune:impl FnOnce(&mut DevnetConfig))->DevnetConfig{
        let mut c=DevnetConfig::new(777938,1_700_000_000,&["execution"]);c.spec_version=2;c.earnin=true;c.nonce_mode=1;c.hash_rate=16;c.c=8;c.d_att=91;c.r_max=1;
        c.f0_acct=U256::from(1000);c.f0_slot=U256::from(2000);c.f0_code=U256::from(3);tune(&mut c);c
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

    #[test]
    fn account_creation_uses_running_counts_receipts_and_sender_burns(){
        for v3 in [false,true]{
            let n=net(v3,|c|c.acct_cap=8);let db=n.genesis.state();let before=count(db);let mut expected=U256::ZERO;
            let raw:Vec<_>=(0..3).map(|i|tx(i,Some(new_address(40+i as u8)),1,vec![],21000)).collect();let o=run(&n,db,&raw);
            assert!(o.halts().is_empty());assert_eq!(o.commitments().gas_used,63000);
            let mut all_logs=Vec::new();
            for i in 0..3{
                let fee=if v3{creation_fees::price(U256::from(1000),8,before.user_accounts+i,1).unwrap()}else{U256::ZERO};expected+=fee;
                let burn=creation_fees::CreationBurn{accounts:fee,..Default::default()};let logs:Vec<Log>=burn.log(devnet::account_key("alice").address()).into_iter().collect();
                assert_eq!(o.receipts()[i as usize],receipts::encode(2,true,(i+1)*21000,&logs).unwrap());all_logs.extend(logs);
            }
            assert_eq!(o.creation_burn().accounts,expected);assert_eq!(o.creation_burn().slots,U256::ZERO);assert_eq!(o.creation_burn().code,U256::ZERO);
            assert_eq!(sum(db)-sum(o.state()),U512::from(63000*100)+U512::from(expected));
            assert_eq!(o.tips(),U256::from(63000*7));assert_eq!(o.commitments().logs_bloom,receipts::bloom(&all_logs));
            assert_eq!(o.log_bytes(),user_phase::log_charge(&all_logs));
            let alice=devnet::account_key("alice").address();assert_eq!(db.accounts[&alice].info.balance-o.state().accounts[&alice].info.balance,U256::from(63000*107+3)+expected);
            assert_eq!(count(o.state()).user_accounts,before.user_accounts+3);
        }
    }

    /// D-V3-21 option A: an unpaid or overflowing charge (here above max_fee*gas_limit = 10^7) halts
    /// with CreationFee and charges the executed gas (21000 of the declared 50000), not the declared gas.
    #[test]
    fn unpaid_or_overflowing_fee_is_an_executed_gas_halt_and_commits_no_creation(){
        for f0 in [U256::from(1_000_000_000_000u64),U256::MAX]{
            let n=net(true,|c|{c.f0_acct=f0;c.accounts[0].1=U256::from(20_000_000)});
            let db=n.genesis.state();let to=new_address(51);let o=run(&n,db,&[tx(0,Some(to),1,vec![],50000)]);
            assert_eq!(o.halts(),&[(0,Halt::CreationFee)]);assert_eq!(o.commitments().gas_used,21000);assert_eq!(o.log_bytes(),0);
            assert_eq!(o.creation_burn(),Default::default());assert!(!o.state().accounts.contains_key(&to));
            let a=devnet::account_key("alice").address();assert_eq!(o.state().accounts[&a].info.nonce,1);
            assert_eq!(db.accounts[&a].info.balance-o.state().accounts[&a].info.balance,U256::from(21000*107));
            assert_eq!(sum(db)-sum(o.state()),U512::from(21000*100));assert_eq!(o.tips(),U256::from(21000*7));
            assert_eq!(o.receipts()[0],receipts::encode(2,false,21000,&[]).unwrap());
        }
    }

    #[test]
    fn slot_replacement_at_a_full_cap_pays_for_a_gross_creation_and_never_hash_order(){
        let n=net(true,|c|{c.slot_cap=1;c.f0_slot=U256::from(101)});let a=new_address(61);let mut db=n.genesis.state().clone();
        // Clear slot0 then insert slot1. The final live count remains one, but a
        // different persistent slot was created and must pay the last-unit fee.
        code(&mut db,a,vec![0x60,0,0x60,0,0x55,0x60,9,0x60,1,0x55,0]);db.insert_account_storage(a,U256::ZERO,U256::from(5)).unwrap();
        let o=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);assert!(o.halts().is_empty());
        assert_eq!(o.creation_burn().slots,U256::from(101));assert_eq!(count(o.state()).user_slots,1);
        assert_eq!(o.state().slot(&a,&U256::ZERO),U256::ZERO);assert_eq!(o.state().slot(&a,&U256::from(1)),U256::from(9));
        // Reverse the independent writes: pricing is a function of committed
        // additions/removals, never revm's map order or transient opcode order.
        code(&mut db,a,vec![0x60,9,0x60,1,0x55,0x60,0,0x60,0,0x55,0]);
        let r=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);assert_eq!(r.creation_burn(),o.creation_burn());
        assert_eq!(r.state().slot(&a,&U256::from(1)),U256::from(9));
    }

    #[test]
    fn native_reverts_and_temporary_slot_creations_are_not_charged(){
        for tail in [vec![0x60,0,0x60,0,0xfd],vec![0x60,0,0x60,0,0x55,0]]{
            let n=net(true,|_|{});let a=new_address(62);let mut db=n.genesis.state().clone();let mut runtime=vec![0x60,1,0x60,0,0x55];runtime.extend(tail);code(&mut db,a,runtime);
            let o=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);assert!(o.halts().is_empty());assert_eq!(o.creation_burn(),Default::default());assert_eq!(o.log_bytes(),0);
            assert_eq!(o.state().slot(&a,&U256::ZERO),U256::ZERO);
        }
    }

    #[test]
    fn deployment_charges_each_logical_code_byte_in_addition_to_its_account(){
        let n=net(true,|_|{});let db=n.genesis.state();let before=count(db);let runtime=vec![0;80];
        let o=run(&n,db,&[tx(0,None,0,init(&runtime),200000),tx(1,None,0,init(&runtime),200000)]);assert!(o.halts().is_empty());
        let cp=n.profile.cp();let expected=creation_fees::price(U256::from(3),cp.CODE_CAP,before.inventory(),160).unwrap();
        assert_eq!(o.creation_burn().code,expected);assert_eq!(count(o.state()).inventory(),before.inventory()+160,"identical code at two addresses is charged twice, like the logical cap");
        assert!(o.creation_burn().accounts>U256::ZERO);assert_eq!(o.creation_burn().slots,U256::ZERO);
        assert_eq!(sum(db)-sum(o.state()),U512::from(o.commitments().gas_used*100)+U512::from(o.creation_burn().total().unwrap()));
    }

    #[test]
    fn pool_pending_state_and_block_execution_deduct_identical_running_fees(){
        let n=net(true,|c|c.acct_cap=10);let start=n.genesis.state();let mut db=start.clone();let mut meter=PoolMeter::of(&db,&n.profile);let context=ctx(&n);
        let raw:Vec<_>=(0..4).map(|i|tx(i,Some(new_address(70+i as u8)),1,vec![],21000)).collect();let mut burns=creation_fees::CreationBurn::default();let mut logs=Vec::new();
        for r in &raw{
            let env=wire::decode_for_profile(r,&n.profile).unwrap().tx_env();let total=count(&db).inventory();
            let mut result=native::execute_on_validated(&db,total,env.clone(),&context,&n.profile).unwrap();
            let b=meter.admit_result(&db,&mut result,env.caller,user_phase::SignedFee::of(&env),&context,&n.profile,total).unwrap();burns=burns.checked_add(b).unwrap();logs.extend_from_slice(result.result.logs());
            user_phase::commit_tx(&mut db,result.state,true);let actual=count(&db);assert_eq!(meter.live(),(actual.user_slots,actual.user_accounts,actual.sys_slots));
        }
        let block=run(&n,start,&raw);assert_eq!(block.creation_burn(),burns);assert_eq!(receipt_burn(block.receipts()),burns,"the receipts carry the whole burn");assert_eq!(block.commitments().logs_bloom,receipts::bloom(&logs));
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
            if v3{assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),32768+36+96+32+196);assert!(o.creation_burn().slots>U256::ZERO);
                assert_eq!(o.state().slot(&transfer,&U256::ZERO),U256::from(99));assert_eq!(o.state().slot(&transfer,&U256::from(1)),U256::from(1));
            }else{assert_eq!(o.halts(),&[(1,Halt::LogBytes)]);assert_eq!(o.log_bytes(),32768);assert_eq!(o.state().slot(&transfer,&U256::ZERO),U256::from(100));assert_eq!(o.state().slot(&transfer,&U256::from(1)),U256::ZERO)}
        }
    }

    #[test]
    fn zero_coefficients_do_not_waive_live_caps(){
        let n=net(true,|c|{c.acct_cap=3;c.f0_acct=U256::ZERO;c.f0_slot=U256::ZERO;c.f0_code=U256::ZERO});
        assert_eq!(count(n.genesis.state()).user_accounts,3);
        let o=run(&n,n.genesis.state(),&[tx(0,Some(new_address(91)),1,vec![],50000)]);
        assert_eq!(o.halts(),&[(0,Halt::AcctCap)]);assert_eq!(o.creation_burn(),Default::default());
        let free=net(true,|c|{c.f0_acct=U256::ZERO;c.f0_slot=U256::ZERO;c.f0_code=U256::ZERO});
        let o=run(&free,free.genesis.state(),&[tx(0,Some(new_address(91)),1,vec![],21000)]);
        assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),0);assert_eq!(o.creation_burn(),Default::default());
    }

    /// C3-8: both domains fill the SAME caps. A dust-filled cap halts v1 and v3 alike (v3 never waives a
    /// cap); the same raised cap admits both, and only v3 charges the creation burn.
    #[test]
    fn dust_filled_legacy_account_and_slot_caps_halt_while_raised_v3_caps_charge(){
        for v3 in [false,true]{
            for (cap,full) in [(3,true),(10,false)]{
                let n=net(v3,|c|c.acct_cap=cap);
                let o=run(&n,n.genesis.state(),&[tx(0,Some(new_address(92)),1,vec![],50000)]);
                if full{assert_eq!(o.halts(),&[(0,Halt::AcctCap)],"v3={v3}");assert_eq!(o.creation_burn(),Default::default());}
                else{assert!(o.halts().is_empty(),"v3={v3}");assert_eq!(o.creation_burn().accounts>U256::ZERO,v3);}
            }
            for (cap,full) in [(1,true),(4,false)]{
                let n=net(v3,|c|c.slot_cap=cap);let mut db=n.genesis.state().clone();let a=new_address(93);
                code(&mut db,a,vec![0x60,1,0x60,1,0x55,0]);db.insert_account_storage(a,U256::ZERO,U256::from(1)).unwrap();
                let o=run(&n,&db,&[tx(0,Some(a),0,vec![],100000)]);
                if full{assert_eq!(o.halts(),&[(0,Halt::SlotCap)],"v3={v3}");assert_eq!(count(o.state()).user_slots,1);assert_eq!(o.creation_burn(),Default::default());}
                else{assert!(o.halts().is_empty(),"v3={v3}");assert_eq!(count(o.state()).user_slots,2);assert_eq!(o.creation_burn().slots,if v3{U256::from(8000/3)}else{U256::ZERO});}
            }
        }
    }

    #[test]
    fn dust_filled_legacy_code_cap_halts_a_deployment_while_v3_pays(){
        for v3 in [false,true]{
            let baseline=net(v3,|_|{});let sys=count(baseline.genesis.state()).inventory();
            // The ChainCaps appended words have a fixed width, so changing the
            // cap does not change its own logical runtime length.
            for (room,full) in [(100,true),(200,false)]{
                let n=net(v3,|c|c.code_cap=sys+room);let mut db=n.genesis.state().clone();
                assert_eq!(count(&db).inventory(),sys);code(&mut db,new_address(94),vec![0;100]);
                let o=run(&n,&db,&[tx(0,None,0,init(&[0;80]),200000)]);
                if full{assert_eq!(count(o.state()).inventory(),sys+100,"v3={v3}");assert_eq!(o.receipts()[0],receipts::encode(2,false,200000,&[]).unwrap());assert_eq!(o.creation_burn(),Default::default());}
                else{assert!(o.halts().is_empty(),"v3={v3}");assert_eq!(count(o.state()).inventory(),sys+180);
                    if v3{assert!(o.creation_burn().code>U256::from(80*3))}else{assert_eq!(o.creation_burn(),Default::default())}}
            }
        }
    }

    #[test]
    fn classified_burns_conserve_supply_and_escrow_on_copy_overlay_and_stored_views(){
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
            let burn=receipt_burn(&reference.receipts);assert_eq!(receipt_burn(&run.receipts),burn);assert_eq!(burn.slots,U256::ZERO);assert_eq!(burn.code,U256::ZERO);
            assert_eq!(burn.accounts>U256::ZERO,height<=6);
            issued+=U512::from(reference.issued);external+=U512::from(reference.gas_used*100)+U512::from(burn.total().unwrap());
            assert_eq!(sum(&reference.state)+external,initial+issued,"height {height}: every creation/base burn is external to Escrow's tracked need");
            apply::check_escrow_invariant(&reference.state,&n.profile,height,false).unwrap();
            assert_eq!(reference.facts,apply::chain_facts(&reference.state),"block facts stay pure state facts: no per-block burn field");
            db=reference.state;parent_result=Some(apply::ParentResult{block_hash:B256::from(U256::from(height)),winner:n.miners[0].id,shares:vec![]});
        }
    }

    #[test]
    fn node_pool_builder_rpc_receipts_and_restart_preserve_the_paid_fee(){
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
            let logs=receipt["result"]["logs"].as_array().unwrap();assert_eq!(logs.len(),1);assert_eq!(logs[0]["address"],serde_json::json!(apply::caps()));assert_eq!(logs[0]["data"].as_str().unwrap().len(),2+96*2);
            let head_hash=node.chain.head;let root=node.chain.head().header.state_root;if stored{node.checkpoint_now().unwrap();}drop(node);
            let mut reopened=pocold::node::Node::open(node_config(&dir,stored),node_spec(&cfg)).unwrap();assert_eq!(reopened.chain.head,head_hash);assert_eq!(reopened.chain.head().header.state_root,root);
            assert_eq!(rpc(&mut reopened,"eth_getTransactionReceipt",serde_json::json!([hash])),receipt);
        }
    }

    #[test]
    fn estimation_and_pool_refuse_unpaid_creation_but_unpriced_call_remains_a_query(){
        let dir=Temp::new();let cfg=node_cfg(|c|c.f0_acct=U256::MAX);let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");let to=new_address(122);
        let raw=node_tx(&node,&alice,0,to,21000);assert!(node.submit_tx(raw,false).unwrap_err().contains("CreationFee"));assert!(node.pool.txs.is_empty());
        let request=serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1"},"latest"]);
        let estimate=rpc(&mut node,"eth_estimateGas",request.clone());assert_eq!(estimate["error"]["code"],-32000);assert!(estimate["error"]["message"].as_str().unwrap().contains("CreationFee"),"{estimate}");
        let call=rpc(&mut node,"eth_call",request);assert_eq!(call["result"],"0x");
        // No-from estimation of a read must not charge creation of a synthetic
        // default caller; its virtual balance/cap unit is never committed.
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

    /// D-V3-21 option A at the consensus boundary, for types 0, 1 and 2. A transfer creates the last
    /// free account (ACCT_CAP 4, three genesis accounts), priced F0·CAP. The burn is charged iff
    /// gas_used·effective_price + burn <= max_fee·gas_limit (types 0/1: the gas price); the value is
    /// outside that sum, as in the Ethereum balance pre-check. At equality the sender pays exactly the
    /// signed maximum plus the value; one gas unit less, or one wei more of burn, halts with
    /// CreationFee and charges only the executed gas, with no transfer and no account.
    #[test]
    fn the_burn_is_charged_only_within_the_signed_maximum_fee_for_every_type(){
        let alice=devnet::account_key("alice");let value=U256::from(10u128.pow(21));let (max_fee,base,gas_limit)=(200u64,100u64,50_000u64);
        for kind in [0u8,1,2]{
            // Types 0/1 pay the whole gas price (its tip is gas price − base fee); type 2 pays base + 7.
            let effective=if kind==2{base+7}else{max_fee};let tip=effective-base;
            let burn=max_fee*gas_limit-21000*effective;assert_eq!(burn%4,0);let f0=burn/4;
            for (f0,gas,charged) in [(f0,gas_limit,true),(f0,gas_limit-1,false),(f0+1,gas_limit,false)]{
                let n=net(true,|c|{c.acct_cap=4;c.f0_acct=U256::from(f0)});let db=n.genesis.state();assert_eq!(count(db).user_accounts,3);
                let to=new_address(140+kind);let t=Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:U256::from(max_fee),gas,to:Some(to),value,data:Default::default()};
                let raw=match kind{0=>t.sign_legacy(&alice),1=>sign_2930(&t,&alice),_=>t.sign_1559(&alice)};assert_eq!(wire::decode_for_profile(&raw,&n.profile).unwrap().kind(),kind);
                let o=run(&n,db,&[raw]);let lost=db.accounts[&alice.address()].info.balance-o.state().accounts[&alice.address()].info.balance;
                let signed=U256::from(max_fee)*U256::from(gas);
                assert_eq!(o.commitments().gas_used,21000);assert_eq!(o.tips(),U256::from(21000*tip));assert_eq!(o.state().accounts[&alice.address()].info.nonce,1);
                if charged{
                    assert!(o.halts().is_empty(),"type {kind}");assert_eq!(o.creation_burn().accounts,U256::from(4*f0));
                    assert_eq!(lost,signed+value,"type {kind}: at equality the sender pays exactly max_fee*gas_limit, plus the separate value");
                    assert_eq!(o.state().accounts[&to].info.balance,value);
                }else{
                    assert_eq!(o.halts(),&[(0,Halt::CreationFee)],"type {kind} gas {gas} f0 {f0}");assert_eq!(o.creation_burn(),Default::default());
                    assert_eq!(lost,U256::from(21000*effective),"type {kind}: only the executed gas");assert!(lost<=signed);
                    assert!(!o.state().accounts.contains_key(&to));assert_eq!(o.receipts()[0],receipts::encode(kind,false,21000,&[]).unwrap());
                }
            }
        }
    }

    /// Fails on the original C3 rule: near a full cap a plain transfer to a fresh address, signed with
    /// an ordinary wallet maximum (21000 gas at 200 wei), was charged the whole last-unit burn
    /// (F0·CAP = 4·10^9 wei, about 950 times its signed maximum) because only the balance limited it. Under
    /// option A it halts with CreationFee and the sender loses its executed gas, at most max_fee·gas_limit.
    /// A spec-1 profile charges no burn for the same transfer.
    #[test]
    fn a_near_full_cap_transfer_with_a_wallet_gas_limit_cannot_burn_beyond_its_signed_maximum(){
        let alice=devnet::account_key("alice").address();let to=new_address(144);
        for v3 in [true,false]{
            let n=net(v3,|c|{c.acct_cap=4;c.f0_acct=U256::from(1_000_000_000u64)});let db=n.genesis.state();assert_eq!(count(db).user_accounts,3);
            let o=run(&n,db,&[tx(0,Some(to),1,vec![],21000)]);let lost=db.accounts[&alice].info.balance-o.state().accounts[&alice].info.balance;
            if v3{
                assert!(db.accounts[&alice].info.balance>U256::from(4_000_000_000u64),"the balance alone would allow the burn");
                assert_eq!(o.halts(),&[(0,Halt::CreationFee)]);assert_eq!(o.creation_burn(),Default::default());
                assert_eq!(lost,U256::from(21000*107));assert!(lost<=U256::from(200*21000));assert!(!o.state().accounts.contains_key(&to));
            }else{assert!(o.halts().is_empty());assert_eq!(o.creation_burn(),Default::default());assert_eq!(lost,U256::from(21000*107+1));}
        }
    }

    /// The bound is checked before the balance; the balance check stays as a defence. Revm's
    /// pre-check (balance >= max_fee·gas_limit + value) makes it unreachable for an engine result, so
    /// it is exercised on a result whose post-execution balance was lowered by hand.
    #[test]
    fn the_balance_check_still_refuses_an_unpaid_burn_within_the_signed_maximum(){
        let n=net(true,|_|{});let db=n.genesis.state();let raw=tx(0,Some(new_address(145)),1,vec![],21000);
        let env=wire::decode_for_profile(&raw,&n.profile).unwrap().tx_env();let total=count(db).inventory();let live=(count(db).user_slots,count(db).user_accounts);
        let fresh=||native::execute_on_validated(db,total,env.clone(),&ctx(&n),&n.profile).unwrap();
        let mut paid=fresh();let burn=user_phase::charge_creation(db,&mut paid,env.caller,user_phase::SignedFee::of(&env),&ctx(&n),&n.profile,live,total).unwrap();
        assert!(burn.accounts>U256::ZERO);
        let mut poor=fresh();poor.state.get_mut(&env.caller).unwrap().info.balance=burn.accounts-U256::from(1);let kept=poor.state[&env.caller].info.balance;
        assert_eq!(user_phase::charge_creation(db,&mut poor,env.caller,user_phase::SignedFee::of(&env),&ctx(&n),&n.profile,live,total),Err(Halt::CreationFee));
        assert_eq!(poor.state[&env.caller].info.balance,kept);assert!(poor.result.logs().is_empty());
        let quoted=user_phase::quote_creation(db,&fresh(),&ctx(&n),&n.profile,live,total,[0;3]).unwrap();assert_eq!(quoted,burn);
    }

    /// The estimate's margin is per class (accounts, slots, code bytes): each drift moves only its own
    /// class, and the code drift counts bytes, so a margin of whole deployments is d·B_code_max bytes.
    #[test]
    fn the_quote_drift_prices_each_class_at_its_own_margin(){
        let sys=count(net(true,|_|{}).genesis.state()).inventory();let bmax=u64::from(net(true,|_|{}).profile.cp().B_code_max);
        let n=net(true,|c|{c.code_cap=sys+8*bmax;c.f0_code=U256::from(1_000_000u64)});let db=n.genesis.state();let raw=tx(0,None,0,init(&[0;80]),400000);
        let env=wire::decode_for_profile(&raw,&n.profile).unwrap().tx_env();let s=count(db);let live=(s.user_slots,s.user_accounts);let total=s.inventory();
        let r=native::execute_on_validated(db,total,env,&ctx(&n),&n.profile).unwrap();let cp=n.profile.cp();
        let q=|drift|user_phase::quote_creation(db,&r,&ctx(&n),&n.profile,live,total,drift).unwrap();
        let base=q([0;3]);assert!(base.accounts>U256::ZERO&&base.code>U256::ZERO,"{base:?}");assert_eq!(base.slots,U256::ZERO);
        let f0=|v:u128|U256::from(v);
        let accts=q([4,0,0]);assert_eq!(accts.accounts,creation_fees::price(f0(1000),cp.ACCT_CAP,s.user_accounts+4,1).unwrap());assert_eq!(accts.code,base.code);
        let bytes=4*u64::from(cp.B_code_max);let code=q([0,0,bytes]);
        assert_eq!(code.code,creation_fees::price(f0(1_000_000),cp.CODE_CAP,total+bytes,80).unwrap());assert_eq!(code.accounts,base.accounts);
        assert!(code.code>q([0,0,4]).code,"4 deployments of the largest size cost more than 4 bytes");
        assert_eq!(q([0,0,u64::MAX]).code,creation_fees::price(f0(1_000_000),cp.CODE_CAP,cp.CODE_CAP-80,80).unwrap(),"clamped so the deployment still fits");
    }

    /// A 1559 transfer with explicit fee fields, signed by `key`.
    fn priced_tx(key:&pocol_lp4_execution::txsign::Key,nonce:u64,to:Address,value:U256,gas:u64,max_fee:U256)->Vec<u8>{
        Tx{chain_id:777938,nonce,max_priority:U256::from(7),max_fee,gas,to:Some(to),value,data:Default::default()}.sign_1559(key)
    }
    fn next_base(node:&pocold::node::Node)->U256{let head=node.chain.full_entry(&node.chain.head).unwrap();node.chain.child_ctx(&head,0).unwrap().base_fee}
    fn balance(node:&pocold::node::Node,a:Address)->U256{node.chain.view_of(&node.chain.head).unwrap().account(&a).map(|x|x.info.balance).unwrap_or_default()}
    fn quantity(v:&serde_json::Value)->U256{U256::from_str_radix(v.as_str().unwrap().trim_start_matches("0x"),16).unwrap()}

    /// D-V3-21 option A for wallets: eth_estimateGas returns a gas limit whose max_fee·gas_limit also
    /// covers the burn (with ESTIMATE_DRIFT_UNITS more occupied units), and pocol_quoteCreationBurn
    /// reports that burn, split from the gas, at the current occupancy. Four estimated transfers fill
    /// ACCT_CAP 8 to 7 in one block, all covered by the head estimate's margin; then the last free
    /// account: an ordinary 21000-gas wallet transaction is refused by the pool, while the estimated
    /// one is included, pays exactly gas + burn (the quoted burn) and stays within its signed maximum.
    #[test]
    fn estimated_gas_covers_the_quoted_burn_and_the_transfer_pays_exactly_gas_plus_burn(){
        let dir=Temp::new();let f0=U256::from(1_000_000_000_000_000u64);let cfg=node_cfg(|c|{c.acct_cap=8;c.f0_acct=f0});
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,3);
        let max_fee=U256::from(100_000_000_000u64);let price=next_base(&node)+U256::from(7);assert!(price<max_fee);
        let request=|to:Address|serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1","maxFeePerGas":format!("{max_fee:#x}"),"maxPriorityFeePerGas":"0x7"},"latest"]);
        let estimate=|node:&mut pocold::node::Node,to:Address|{let e=rpc(node,"eth_estimateGas",request(to));assert!(e.get("error").is_none(),"{e}");quantity(&e["result"]).to::<u64>()};
        let mut fills=Vec::new();
        for i in 0..4u64{let to=new_address(150+i as u8);let gas=estimate(&mut node,to);
            // The head is at 3 of 8: the margin prices the account at min(3+4, 7) = 7 occupied units.
            let margin=creation_fees::price(f0,8,7,1).unwrap();let need=price*U256::from(21000)+margin;
            // The search's resolution is 1000 gas above the least covering limit.
            assert!(max_fee*U256::from(gas)>=need&&max_fee*U256::from(gas)<need+max_fee*U256::from(1001),"{gas}");
            let raw=priced_tx(&alice,i,to,U256::from(1),gas,max_fee);node.submit_tx(raw.clone(),false).unwrap();fills.push(raw);}
        let p=node.chain.full_entry(&node.chain.head).unwrap();let selection=node.template_selection(&p,cfg.g_ts+4);assert_eq!(selection.txs,fills,"every estimated fill is included");
        let b=solve(&node,selection.txs,cfg.g_ts+4);assert!(node.import(b,None).unwrap());assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,7);
        let price=next_base(&node)+U256::from(7);
        let to=new_address(160);let burn=creation_fees::price(f0,8,7,1).unwrap();assert_eq!(burn,f0*U256::from(8));
        let wallet=priced_tx(&alice,4,to,U256::from(1),21000,max_fee);assert!(node.submit_tx(wallet,false).unwrap_err().contains("CreationFee"),"a 21000-gas maximum cannot carry the burn");
        let quote=rpc(&mut node,"pocol_quoteCreationBurn",serde_json::json!([request(to)[0]]));let q=&quote["result"];
        assert_eq!(quantity(&q["accounts"]),burn,"{quote}");assert_eq!(quantity(&q["total"]),burn);assert_eq!(quantity(&q["slots"]),U256::ZERO);assert_eq!(quantity(&q["code"]),U256::ZERO);
        assert_eq!(q["gasUsed"],"0x5208");assert_eq!(q["occupancy"]["accounts"],"0x7");assert_eq!(q["v3Active"],true);assert_eq!(quantity(&q["baseFee"]),price-U256::from(7));
        let gas=estimate(&mut node,to);let signed=max_fee*U256::from(gas);assert!(signed>=price*U256::from(21000)+burn&&gas<=cfg_gas_limit(&node));
        let raw=priced_tx(&alice,4,to,U256::from(1),gas,max_fee);let hash=node.submit_tx(raw.clone(),false).unwrap();
        let before=balance(&node,alice.address());let p=node.chain.full_entry(&node.chain.head).unwrap();let selection=node.template_selection(&p,cfg.g_ts+8);assert_eq!(selection.txs,vec![raw]);
        let b=solve(&node,selection.txs,cfg.g_ts+8);assert!(node.import(b,None).unwrap());
        let receipt=rpc(&mut node,"eth_getTransactionReceipt",serde_json::json!([hash]));assert_eq!(receipt["result"]["status"],"0x1","{receipt}");assert_eq!(receipt["result"]["gasUsed"],"0x5208");
        let data=receipt["result"]["logs"][0]["data"].as_str().unwrap();assert_eq!(U256::from_str_radix(&data[2..66],16).unwrap(),burn,"the receipt carries the quoted burn");
        let lost=before-balance(&node,alice.address());assert_eq!(lost,price*U256::from(21000)+burn+U256::from(1));assert!(lost<=signed+U256::from(1));
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,8);
        // At a full cap the quote reports the cap halt, as the estimate does.
        let full=rpc(&mut node,"pocol_quoteCreationBurn",serde_json::json!([request(new_address(161))[0]]));assert!(full["error"]["message"].as_str().unwrap().contains("AcctCap"),"{full}");
    }
    fn cfg_gas_limit(node:&pocold::node::Node)->u64{node.chain.spec.cp().GAS_LIMIT}

    /// Deploys each runtime from bob (nonces 0, 1, ...) in one imported block at `clock`; their addresses.
    fn deploy(node:&mut pocold::node::Node,runtimes:&[Vec<u8>],clock:u64)->Vec<Address>{
        let bob=devnet::account_key("bob");
        let raw:Vec<_>=runtimes.iter().enumerate().map(|(i,rt)|Tx{chain_id:777938,nonce:i as u64,max_priority:U256::from(7),max_fee:U256::from(1_000_000_000_000u64),gas:500_000,to:None,value:U256::ZERO,data:Bytes::from(init(rt))}.sign_1559(&bob)).collect();
        let b=solve(node,raw,clock);assert!(node.import(b,None).unwrap());
        let out:Vec<_>=(0..runtimes.len() as u64).map(|i|bob.address().create(i)).collect();
        let view=node.chain.view_of(&node.chain.head).unwrap();assert!(out.iter().all(|a|view.account(a).is_some_and(|x|x.info.code_hash!=revm::primitives::KECCAK_EMPTY)),"deployed");out
    }
    /// eth_estimateGas and pocol_quoteCreationBurn for one call object (alice at `max_fee`, tip 7).
    fn estimate_and_quote(node:&mut pocold::node::Node,to:Option<Address>,data:&[u8],max_fee:U256)->(u64,serde_json::Value){
        let mut call=serde_json::json!({"from":devnet::account_key("alice").address(),"data":format!("0x{}",data.iter().map(|b|format!("{b:02x}")).collect::<String>()),"maxFeePerGas":format!("{max_fee:#x}"),"maxPriorityFeePerGas":"0x7"});
        if let Some(to)=to{call["to"]=serde_json::json!(to);}
        let e=rpc(node,"eth_estimateGas",serde_json::json!([call.clone(),"latest"]));assert!(e.get("error").is_none(),"{e}");
        let q=rpc(node,"pocol_quoteCreationBurn",serde_json::json!([call]));assert!(q.get("error").is_none(),"{q}");
        (quantity(&e["result"]).to::<u64>(),q["result"].clone())
    }

    /// The slot class of eth_estimateGas's margin (`rpc::ESTIMATE_DRIFT_UNITS` slots), at the RPC: a call
    /// that creates one slot (SSTORE(CALLER, 1)) near a full SLOT_CAP 8 gets a gas limit whose
    /// max_fee·gas_limit covers the slot priced ESTIMATE_DRIFT_UNITS units further in, within the search's
    /// 1000-gas resolution, and not only at the current occupancy the quote reports.
    #[test]
    fn estimated_gas_covers_a_new_slot_at_the_slot_margin(){
        let dir=Temp::new();let f0=U256::from(1_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.slot_cap=8;c.f0_slot=f0;c.f0_acct=U256::ZERO;c.f0_code=U256::ZERO});
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();
        let x=deploy(&mut node,&[vec![0x60,1,0x33,0x55,0x00]],cfg.g_ts+4)[0];
        // Signed at the effective price, so the burn, not the execution's gas, sets the gas limit.
        let price=next_base(&node)+U256::from(7);let max_fee=price;let cap=node.chain.spec.cp().SLOT_CAP;assert_eq!(cap,8);
        let (gas,q)=estimate_and_quote(&mut node,Some(x),&[],max_fee);
        let slots=quantity(&q["occupancy"]["slots"]).to::<u64>();assert!(slots+4<cap,"{q}");let used=quantity(&q["gasUsed"]);
        assert_eq!(quantity(&q["slots"]),creation_fees::price(f0,cap,slots,1).unwrap(),"{q}");assert_eq!(quantity(&q["total"]),quantity(&q["slots"]));
        let margin=creation_fees::price(f0,cap,slots+pocold::rpc::ESTIMATE_DRIFT_UNITS,1).unwrap();
        // The margin is worth more than the search's resolution, so a search without it is caught.
        assert!(margin-quantity(&q["slots"])>max_fee*U256::from(1001));
        let need=price*used+margin;let signed=max_fee*U256::from(gas);
        assert!(signed>=need&&signed<need+max_fee*U256::from(1001),"gas {gas}: {signed} for {need}");
    }

    /// The code class of eth_estimateGas's margin, at the RPC: a deployment of 80 bytes near a full
    /// CODE_CAP gets a gas limit that covers its code priced ESTIMATE_DRIFT_UNITS·B_code_max bytes further
    /// in (4 deployments of the largest size), within the search's resolution, and not merely 4 bytes in.
    #[test]
    fn estimated_gas_covers_a_deployment_at_the_code_margin_of_whole_deployments(){
        let tune=|c:&mut DevnetConfig|{c.f0_code=U256::from(10_000_000_000_000u64);c.f0_acct=U256::ZERO;c.f0_slot=U256::ZERO};
        let probe=node_cfg(tune);let built=devnet::build(&probe).unwrap();let bmax=u64::from(built.profile.cp().B_code_max);
        let sys=Summary::scan(built.genesis.state(),built.profile.cp().B_code_max).inventory();
        let cfg=node_cfg(|c|{tune(c);c.code_cap=sys+8*bmax});let dir=Temp::new();
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();
        let price=next_base(&node)+U256::from(7);let max_fee=price;let cp=node.chain.spec.cp();let (cap,f0)=(cp.CODE_CAP,U256::from(10_000_000_000_000u64));
        let (gas,q)=estimate_and_quote(&mut node,None,&init(&[0;80]),max_fee);
        let code=quantity(&q["occupancy"]["code"]).to::<u64>();assert_eq!(code,sys,"{q}");let used=quantity(&q["gasUsed"]);
        assert_eq!(quantity(&q["code"]),creation_fees::price(f0,cap,code,80).unwrap(),"{q}");assert_eq!(quantity(&q["accounts"]),U256::ZERO);
        let drift=pocold::rpc::ESTIMATE_DRIFT_UNITS*bmax;assert!(code+drift<=cap-80);
        let margin=creation_fees::price(f0,cap,code+drift,80).unwrap();let bytes4=creation_fees::price(f0,cap,code+pocold::rpc::ESTIMATE_DRIFT_UNITS,80).unwrap();
        assert!(margin-bytes4>max_fee*U256::from(1001),"whole deployments, not bytes, are worth more than the resolution");
        let need=price*used+margin;let signed=max_fee*U256::from(gas);
        assert!(signed>=need&&signed<need+max_fee*U256::from(1001),"gas {gas}: {signed} for {need}");
        assert!(signed>=price*used+bytes4+max_fee*U256::from(1001),"more than a 4-byte margin");
    }

    /// pocol_quoteCreationBurn reports the cap halts eth_estimateGas reports, with the same reason: a call
    /// that creates a slot and emits V3_LOG_COUNT_MAX logs passes the log count only with the CreationFeeBurn
    /// log its burn adds, so both answer LogCount; the same logs without a creation are quoted.
    #[test]
    fn the_burn_quote_reports_the_estimate_s_own_log_halt(){
        let dir=Temp::new();let cfg=node_cfg(|_|{});let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();
        // 4368 x LOG0(0, 0), after SSTORE(CALLER, 1) or not.
        let logs=|from:u8|vec![0x61,0x11,0x10,0x5b,0x60,0,0x60,0,0xa0,0x60,1,0x90,0x03,0x80,0x60,from,0x57,0x00];
        let mut slot=vec![0x60,1,0x33,0x55];slot.extend(logs(7));
        let a=deploy(&mut node,&[slot,logs(3)],cfg.g_ts+4);assert_eq!(user_phase::caps::V3_LOG_COUNT_MAX,4368);
        let call=|to:Address|serde_json::json!({"from":devnet::account_key("alice").address(),"to":to,"maxFeePerGas":"0xe8d4a51000","maxPriorityFeePerGas":"0x7"});
        let e=rpc(&mut node,"eth_estimateGas",serde_json::json!([call(a[0]),"latest"]));let q=rpc(&mut node,"pocol_quoteCreationBurn",serde_json::json!([call(a[0])]));
        assert_eq!(e["error"]["message"],"execution would halt (LogCount)","{e}");assert_eq!(q["error"]["code"],-32000,"{q}");assert_eq!(q["error"]["message"],e["error"]["message"]);assert_eq!(q["error"],e["error"],"the same error object, reason included");assert_eq!(e["error"]["data"]["reason"],"LogCount");
        let (_,ok)=estimate_and_quote(&mut node,Some(a[1]),&[],U256::from(1_000_000_000_000u64));assert_eq!(quantity(&ok["total"]),U256::ZERO);
    }

    /// The append round (B5) executes its candidates only in `build_signed`, which accepts halts. A user fills
    /// the block's log count with one included call and uses the selection's execution budget with four heavy
    /// one-log calls that B4 then leaves out (they stay pooled); an estimated creating transfer (its gas limit
    /// carries the burn) is held for the append round. Appended after the filler it would halt with LogCount and
    /// pay its whole declared gas (review reproduction); under v3 the honest builder drops it instead, and it
    /// stays pooled.
    #[test]
    fn the_v3_append_round_drops_an_appended_transaction_that_would_halt(){
        let dir=Temp::new();let f0=U256::from(1_000_000_000_000_000u64);let senders=["carol","h1","h2","h3","h4"];
        let cfg=node_cfg(|c|{c.f0_acct=f0;for s in senders{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let mut config=node_config(&dir,false);let k=devnet::miner_identity("execution").key;config.miner=Some(pocol_lp4_execution::txsign::Key::from_secret(&k.secret()).unwrap());
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        // F: 4368 x LOG0. H: a 150000-iteration loop, then one LOG0.
        let filler=vec![0x61,0x11,0x10,0x5b,0x60,0,0x60,0,0xa0,0x60,1,0x90,0x03,0x80,0x60,3,0x57,0x00];
        let heavy=vec![0x62,0x02,0x49,0xf0,0x5b,0x60,1,0x90,0x03,0x80,0x60,4,0x57,0x60,0,0x60,0,0xa0,0x00];
        let d=deploy(&mut node,&[filler,heavy],cfg.g_ts+4);let (fa,ha)=(d[0],d[1]);
        let price=next_base(&node)+U256::from(7);
        let call=|k:&str,to:Address,gas:u64|Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:price*U256::from(2),gas,to:Some(to),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key(k));
        let f=call("carol",fa,2_000_000);node.submit_tx(f.clone(),false).unwrap();
        for h in ["h1","h2","h3","h4"]{node.submit_tx(call(h,ha,3_990_000),false).unwrap();}
        let to=new_address(170);
        let est=rpc(&mut node,"eth_estimateGas",serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1","maxFeePerGas":format!("{price:#x}"),"maxPriorityFeePerGas":"0x7"},"latest"]));
        let gas=quantity(&est["result"]).to::<u64>();assert!(gas>900_000,"the gas limit carries the burn: {gas}");
        let v=priced_tx(&alice,0,to,U256::from(1),gas,price);node.submit_tx(v.clone(),false).unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();let clock=cfg.g_ts+8;let sel=node.template_selection(&p,clock);
        assert_eq!(sel.txs,vec![f.clone()],"only the filler is selected; B4 leaves the heavy calls out");
        assert!(sel.rest.iter().any(|c|c.raw==v),"the transfer is held for the append round");
        let (_,body)=node.build_template_unrecorded(&p,sel,clock).expect("a template");
        assert_eq!(body.txs,vec![f.clone()],"the honest append round leaves out the transfer that would halt");
        assert!(node.pool.txs.iter().any(|(_,r)|*r==v),"it stays pooled");
        // The template is valid and charges alice nothing.
        let before=balance(&node,alice.address());let b=solve(&node,body.txs,clock);assert!(node.import(b,None).unwrap());
        assert_eq!(balance(&node,alice.address()),before);
    }

    /// pocol_quoteCreationBurn is node-local and v3 only: a spec-1 node answers it exactly as any unknown
    /// method, so its RPC surface is byte-identical.
    #[test]
    fn the_creation_burn_quote_is_unknown_to_a_spec_1_node(){
        let mut legacy=node_cfg(|_|{});legacy.spec_version=1;let dir=Temp::new();let mut old=pocold::node::Node::open(node_config(&dir,false),node_spec(&legacy)).unwrap();
        let call=serde_json::json!([{"to":new_address(162)}]);
        let quote=rpc(&mut old,"pocol_quoteCreationBurn",call.clone());let unknown=rpc(&mut old,"pocol_quoteCreationBurx",call);
        assert_eq!(quote["error"]["code"],-32601);assert_eq!(quote["error"]["message"],"method pocol_quoteCreationBurn not found");assert_eq!(unknown["error"]["code"],-32601);
    }

    /// D-V3-21 option A, front-running: the builder's big-first order raises alice's burn from the
    /// 4th to the 5th of 8 account units (1.6·10^17 to 2·10^17 wei), past her signed
    /// max_fee·gas_limit = 1.7·10^17 although her balance covers it. The honest builder reprices in
    /// its own order and skips her; a hostile proposer that includes her anyway gets a valid block in
    /// which her transaction halts with CreationFee and she pays only its executed gas.
    #[test]
    fn builder_order_cannot_push_a_sender_past_the_signed_maximum(){
        let dir=Temp::new();let f0=U256::from(100_000_000_000_000_000u64);let (first,second)=(creation_fees::price(f0,8,3,1).unwrap(),creation_fees::price(f0,8,4,1).unwrap());
        let cfg=node_cfg(|c|{c.acct_cap=8;c.f0_acct=f0});
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");let bob=devnet::account_key("bob");
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,3);
        let max_fee=U256::from(100_000_000_000u64);let base=next_base(&node);assert!(base+U256::from(7)<max_fee);let price=base+U256::from(7);
        let gas=1_700_000u64;let signed=max_fee*U256::from(gas);
        assert!(price*U256::from(21000)+first<=signed&&price*U256::from(21000)+second>signed&&balance(&node,alice.address())>signed+second,"the bound, not the balance, decides");
        let small=priced_tx(&alice,0,new_address(123),U256::from(1),gas,max_fee);let big=priced_tx(&bob,0,new_address(124),U256::from(1),5_000_000,max_fee);
        node.submit_tx(small.clone(),false).unwrap();node.submit_tx(big.clone(),false).unwrap();assert_eq!(node.pool.txs.len(),2);
        let p=node.chain.full_entry(&node.chain.head).unwrap();let selection=node.template_selection(&p,cfg.g_ts+4);assert_eq!(selection.txs,vec![big.clone()]);assert_eq!(selection.counted,21000);
        // The hostile order, included anyway: valid, and alice's loss is her executed gas only.
        let before=balance(&node,alice.address());let b=solve(&node,vec![big,small.clone()],cfg.g_ts+4);assert!(node.import(b,None).unwrap());
        let view=node.chain.view_of(&node.chain.head).unwrap();assert!(view.account(&new_address(123)).is_none());assert!(view.account(&new_address(124)).is_some());
        assert_eq!(view.account(&alice.address()).unwrap().info.nonce,1);let lost=before-balance(&node,alice.address());
        assert_eq!(lost,price*U256::from(21000));assert!(lost<=signed);
        let hash=revm::primitives::keccak256(&small);let receipt=rpc(&mut node,"eth_getTransactionReceipt",serde_json::json!([hash]));
        assert_eq!(receipt["result"]["status"],"0x0","{receipt}");assert_eq!(receipt["result"]["gasUsed"],"0x5208");assert!(receipt["result"]["logs"].as_array().unwrap().is_empty());
        assert_eq!(node.chain.head().gas_used,42000,"the halt counts its executed gas in the block");
    }

    /// D-V3-21 with N4's replace-by-fee: a replacement's burn is bounded by its own signed fee, as a new submission's
    /// is. alice pools a creating transfer whose 1.7M gas limit carries the burn of the 4th of 8 accounts. A replacement
    /// raising both caps by the bump but declaring 21000 gas would burn past its max_fee*gas_limit: it is refused with
    /// CreationFee, the answer the same transfer gets as a new submission (bob's), and the pooled one stays. A bumped
    /// replacement keeping the gas limit replaces it.
    #[test]
    fn a_replacement_whose_burn_would_pass_its_signed_maximum_is_refused(){
        let dir=Temp::new();let f0=U256::from(100_000_000_000_000_000u64);let cfg=node_cfg(|c|{c.acct_cap=8;c.f0_acct=f0});
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,3);
        let max_fee=U256::from(100_000_000_000u64);let price=next_base(&node)+U256::from(7);assert!(price<max_fee);
        let gas=1_700_000u64;assert!(price*U256::from(21000)+creation_fees::price(f0,8,3,1).unwrap()<=max_fee*U256::from(gas));
        let to=new_address(180);let first=priced_tx(&alice,0,to,U256::from(1),gas,max_fee);node.submit_tx(first.clone(),false).unwrap();
        let bumped=|key:&pocol_lp4_execution::txsign::Key,to:Address,gas:u64|Tx{chain_id:777938,nonce:0,max_priority:U256::from(8),max_fee:max_fee*U256::from(2),gas,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(key);
        // Priced as a newcomer is, at the pending state's 4 of 8 accounts: far past 2*max_fee*21000.
        assert!(max_fee*U256::from(2*21000)<creation_fees::price(f0,8,4,1).unwrap());
        let e=node.submit_tx(bumped(&alice,to,21000),false).unwrap_err();assert!(e.contains("CreationFee"),"{e}");
        let pooled=|node:&pocold::node::Node|node.pool.txs.iter().map(|(_,r)|r.clone()).collect::<Vec<_>>();
        assert_eq!(pooled(&node),vec![first.clone()],"the pooled one stays");assert_eq!(node.stats.pool_replaced,0);node.pool_audit().unwrap();
        let fresh=node.submit_tx(bumped(&bob,new_address(181),21000),false).unwrap_err();assert_eq!(e,fresh,"refused as a new submission is");
        let ok=bumped(&alice,to,gas);node.submit_tx(ok.clone(),false).unwrap();assert_eq!(pooled(&node),vec![ok]);assert_eq!(node.stats.pool_replaced,1);node.pool_audit().unwrap();
    }

    /// D-V3-21 with N4's lowest-fee eviction: the burn bound is checked before the victim leaves, so a newcomer whose
    /// burn would pass its signed maximum evicts nothing. The pool is full by count (POOL_MAX_TXS transfers to an
    /// existing account at 1 gwei); zed outbids them at 3 gwei with a 21000-gas transfer to a new account: refused with
    /// CreationFee, and the pool, its ghost gas and the eviction count are unchanged. The same bid to an existing
    /// account evicts one.
    #[test]
    fn an_over_burn_newcomer_at_the_count_bound_evicts_nothing(){
        use pocold::node::POOL_MAX_TXS;
        let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let f0=U256::from(100_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.acct_cap=64;c.f0_acct=f0;for s in senders.iter().map(|s|s.as_str()).chain(["zed"]){c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(POOL_MAX_TXS as u64*21000+2*21000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        let max_fee=U256::from(100_000_000_000u64);assert!(next_base(&node)+U256::from(3_000_000_000u64)<max_fee);let sink=devnet::account_key("alice").address();
        let pay=|key:&pocol_lp4_execution::txsign::Key,nonce:u64,tip:u64,to:Address|Tx{chain_id:777938,nonce,max_priority:U256::from(tip),max_fee,gas:21000,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(key);
        let per=POOL_MAX_TXS/senders.len();
        for s in &senders{let k=devnet::account_key(s);for j in 0..per{node.submit_tx(pay(&k,j as u64,1_000_000_000,sink),false).unwrap();}}
        assert_eq!(node.pool.txs.len(),POOL_MAX_TXS);node.pool_audit().unwrap();
        let accounts=node.chain.head().summary.as_ref().unwrap().user_accounts;
        assert!(max_fee*U256::from(21000)<creation_fees::price(f0,64,accounts,1).unwrap(),"the new account's burn passes the signed maximum");
        let zed=devnet::account_key("zed");
        let e=node.submit_tx(pay(&zed,0,3_000_000_000,new_address(190)),false).unwrap_err();assert!(e.contains("CreationFee"),"{e}");
        assert_eq!((node.pool.txs.len(),node.pool.ghost_gas,node.stats.pool_evicted),(POOL_MAX_TXS,0,0),"nothing evicted");node.pool_audit().unwrap();
        node.submit_tx(pay(&zed,0,3_000_000_000,sink),false).unwrap();
        assert_eq!((node.pool.txs.len(),node.pool.ghost_gas,node.stats.pool_evicted),(POOL_MAX_TXS,21000,1),"the same bid without a burn evicts one");node.pool_audit().unwrap();
    }

    /// D-V3-21 wallets raise the gas limit to carry the burn (eth_estimateGas); N4's pool charges that signed limit: the
    /// gas bound and the sender quota count it (`Pool::gas`), and the overdraft rule prices it at gas_limit*max_fee +
    /// value. dave holds the estimated creation's maximum plus one 21000-gas transfer's maximum, less one wei: the
    /// creation is pooled and his next transfer is refused by the overdraft rule, though both would fit his balance at
    /// 21000 gas each.
    #[test]
    fn the_pool_charges_the_gas_limit_a_wallet_raises_for_the_burn(){
        let f0=U256::from(1_000_000_000_000_000u64);let max_fee=U256::from(100_000_000_000u64);let to=new_address(185);
        let tune=|c:&mut DevnetConfig,balance:U256|{c.acct_cap=8;c.f0_acct=f0;c.accounts.push(("dave".into(),balance));};
        let dave=devnet::account_key("dave");
        let request=serde_json::json!([{"from":dave.address(),"to":to,"value":"0x1","maxFeePerGas":format!("{max_fee:#x}"),"maxPriorityFeePerGas":"0x7"},"latest"]);
        // Estimated on the same chain with a large balance, so the funds cap does not bind the search.
        let gas={let dir=Temp::new();let cfg=node_cfg(|c|tune(c,U256::from(10u128.pow(24))));let mut probe=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();
            let e=rpc(&mut probe,"eth_estimateGas",request);assert!(e.get("error").is_none(),"{e}");quantity(&e["result"]).to::<u64>()};
        assert!(gas>2*21000,"the gas limit carries the burn: {gas}");
        let one=max_fee*U256::from(21000)+U256::from(1);let balance=max_fee*U256::from(gas)+one;
        let dir=Temp::new();let cfg=node_cfg(|c|tune(c,balance));let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();
        let before=node.pool.gas;node.submit_tx(priced_tx(&dave,0,to,U256::from(1),gas,max_fee),false).unwrap();
        assert_eq!(node.pool.gas-before,gas,"the pool counts the raised gas limit");
        assert!(one+one<=balance,"at 21000 gas each both would be covered");
        let next=priced_tx(&dave,1,devnet::account_key("alice").address(),U256::from(1),21000,max_fee);
        assert_eq!(node.submit_tx(next,false).unwrap_err(),pocold::node::POOL_OVERDRAFT);assert_eq!(node.pool.txs.len(),1);
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

    #[test]
    fn the_creation_burn_annotation_is_included_in_log_caps_and_reverted_on_halt(){
        let n=net(true,|_|{});let target=new_address(126);
        for extra in [0,1]{let mut db=n.genesis.state().clone();let bytes=256*1024-36-196+extra;
            let mut runtime=vec![0x60,1,0x60,0,0x55,0x62,((bytes>>16)&255) as u8,((bytes>>8)&255) as u8,(bytes&255) as u8,0x60,0,0xa0,0];
            code(&mut db,target,std::mem::take(&mut runtime));let o=run(&n,&db,&[tx(0,Some(target),0,vec![],5_000_000)]);
            if extra==0{assert!(o.halts().is_empty());assert_eq!(o.log_bytes(),256*1024);assert!(o.creation_burn().slots>U256::ZERO)}else{
                assert_eq!(o.halts(),&[(0,Halt::LogBytes)]);assert_eq!(o.log_bytes(),0);assert_eq!(o.creation_burn(),Default::default());assert_eq!(o.state().slot(&target,&U256::ZERO),U256::ZERO);assert_eq!(o.commitments().gas_used,5_000_000);
            }
        }
    }

    #[test]
    fn create_collision_preserves_copied_storage_without_charging_fictitious_creations(){
        let n=net(true,|c|c.slot_cap=1);let mut db=n.genesis.state().clone();let alice=devnet::account_key("alice").address();let contract=alice.create(0);
        db.insert_account_info(contract,AccountInfo{balance:U256::from(1),..Default::default()});db.insert_account_storage(contract,U256::from(99),U256::from(7)).unwrap();
        // LP4-CREATE-01 deliberately includes nonzero storage in the collision
        // check, even with nonce zero and empty code. V3 preserves this rule.
        let raw=vec![tx(0,None,0,init(&[0x60,1,0x60,0,0x55,0]),200000),tx(1,Some(contract),0,vec![],100000)];
        let o=run(&n,&db,&raw);assert!(o.halts().is_empty(),"{:?}",o.halts());assert_eq!(o.creation_burn().accounts,U256::ZERO);
        assert_eq!(o.creation_burn(),Default::default());assert_eq!(o.state().slot(&contract,&U256::from(99)),U256::from(7));assert_eq!(o.state().slot(&contract,&U256::ZERO),U256::ZERO);assert_eq!(count(o.state()).user_slots,1);
        assert_eq!(o.state().accounts[&contract].info.nonce,0);assert_eq!(o.receipts()[0],receipts::encode(2,false,200000,&[]).unwrap());assert_eq!(o.receipts()[1],receipts::encode(2,true,221000,&[]).unwrap());
        let mut state=db.clone();let mut meter=PoolMeter::of(&state,&n.profile);let context=ctx(&n);
        for raw in raw{let env=wire::decode_for_profile(&raw,&n.profile).unwrap().tx_env();let total=count(&state).inventory();let mut r=native::execute_on_validated(&state,total,env.clone(),&context,&n.profile).unwrap();
            meter.admit_result(&state,&mut r,env.caller,user_phase::SignedFee::of(&env),&context,&n.profile,total).unwrap();user_phase::commit_tx(&mut state,r.state,true);let summary=count(&state);assert_eq!(meter.live(),(summary.user_slots,summary.user_accounts,summary.sys_slots));}
        assert_eq!(apply::state_root(&state).unwrap(),o.state_root(100,100).unwrap());
    }

    #[test]
    fn unpaid_slot_and_code_charges_revert_all_native_effects_and_burn_only_gas(){
        for deployment in [false,true]{let n=net(true,|c|{c.f0_acct=U256::ZERO;if deployment{c.f0_code=U256::from(10u128.pow(30))}else{c.f0_slot=U256::from(10u128.pow(30))}});
            let mut db=n.genesis.state().clone();let target=new_address(127);code(&mut db,target,vec![0x60,1,0x60,0,0x55,0]);let before=count(&db);
            let raw=if deployment{tx(0,None,0,init(&[0;80]),200000)}else{tx(0,Some(target),0,vec![],200000)};
            // The gas the same execution uses when its creations are free: a CreationFee halt charges it (D-V3-21).
            let free=net(true,|c|{c.f0_acct=U256::ZERO;c.f0_slot=U256::ZERO;c.f0_code=U256::ZERO});let executed=run(&free,&db,&[raw.clone()]).commitments().gas_used;assert!(executed<200000);
            let o=run(&n,&db,&[raw]);assert_eq!(o.halts(),&[(0,Halt::CreationFee)]);assert_eq!(o.creation_burn(),Default::default());assert_eq!(o.log_bytes(),0);assert_eq!(o.commitments().gas_used,executed);
            let after=count(o.state());assert_eq!((after.user_accounts,after.user_slots,after.code_total),(before.user_accounts,before.user_slots,before.code_total));
            assert_eq!(sum(&db)-sum(o.state()),U512::from(executed*100));
        }
    }

    /// A CreationFee halt discards every effect, including the storage clear that earned an EIP-3529
    /// refund, so it charges the gas spent before the refund (gas_used + gas_refunded), as a revert of
    /// the same work would, never less. Here slot 0 (5) is cleared and slot 1 is created: the new slot's
    /// burn is unpayable, the transaction halts, the sender pays the spent gas and slot 0 keeps its value.
    #[test]
    fn a_creation_fee_halt_charges_the_gas_spent_before_the_refund(){
        let n=net(true,|c|{c.f0_acct=U256::ZERO;c.f0_slot=U256::from(10u128.pow(30))});let a=new_address(62);let mut db=n.genesis.state().clone();
        code(&mut db,a,vec![0x60,0,0x60,0,0x55,0x60,9,0x60,1,0x55,0]);db.insert_account_storage(a,U256::ZERO,U256::from(5)).unwrap();
        let raw=tx(0,Some(a),0,vec![],100000);let env=wire::decode_for_profile(&raw,&n.profile).unwrap().tx_env();
        let (gas_used,gas_refunded)=match native::execute_on_validated(&db,count(&db).inventory(),env,&ctx(&n),&n.profile).unwrap().result{
            revm::primitives::ExecutionResult::Success{gas_used,gas_refunded,..}=>(gas_used,gas_refunded),other=>panic!("{other:?}")};
        assert_eq!(gas_refunded,4800,"the slot clear earns the EIP-3529 refund");let spent=gas_used+gas_refunded;assert!(spent<100000);
        let free=net(true,|c|{c.f0_acct=U256::ZERO;c.f0_slot=U256::ZERO});assert_eq!(run(&free,&db,&[raw.clone()]).commitments().gas_used,gas_used,"a success pays after the refund");
        let o=run(&n,&db,&[raw]);assert_eq!(o.halts(),&[(0,Halt::CreationFee)]);assert_eq!(o.creation_burn(),Default::default());
        assert_eq!(o.commitments().gas_used,spent);assert_eq!(o.receipts()[0],receipts::encode(2,false,spent,&[]).unwrap());
        let alice=devnet::account_key("alice").address();assert_eq!(db.accounts[&alice].info.balance-o.state().accounts[&alice].info.balance,U256::from(spent*107));
        assert_eq!(o.tips(),U256::from(spent*7));assert_eq!(o.state().slot(&a,&U256::ZERO),U256::from(5));assert_eq!(o.state().slot(&a,&U256::from(1)),U256::ZERO);
    }

    #[test]
    fn estimation_needs_only_affordable_transaction_gas_and_accepts_no_sender_reads_at_a_full_cap(){
        // D-V3-21 option A: the burn (about 1000 wei) must fit in max_fee*gas_limit with the gas, so at
        // 1 gwei the transfer needs 21001 gas. The balance covers 21001 gas plus the value and nothing
        // more: far below a block's gas, which the search must not require.
        let dir=Temp::new();let cfg=node_cfg(|c|c.accounts[0].1=U256::from(21_001_000_000_001u64));
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");let to=new_address(128);
        let params=serde_json::json!([{"from":alice.address(),"to":to,"value":"0x1","gasPrice":"0x3b9aca00"},"latest"]);
        let estimate=rpc(&mut node,"eth_estimateGas",params);assert_eq!(estimate["result"],"0x5209","{estimate}");
        let signed=|gas|Tx{chain_id:777938,nonce:0,max_priority:U256::ZERO,max_fee:U256::from(1_000_000_000u64),gas,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(&alice);
        assert!(node.submit_tx(signed(21000),false).unwrap_err().contains("CreationFee"),"21000 gas at 1 gwei leaves no room for the burn");
        node.submit_tx(signed(21001),false).unwrap();
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

    #[test]
    fn creation_fee_estimation_uses_the_actual_block_gaslimit_opcode(){
        let dir=Temp::new();let cfg=node_cfg(|c|c.f0_slot=U256::MAX);let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        // With the real 8M block limit this is a read. At the unrelated 30M
        // eth_call ceiling it instead creates a slot whose fee is unpayable.
        let runtime=[0x45,0x62,0x7a,0x12,0x00,0x14,0x60,15,0x57,0x60,1,0x60,0,0x55,0,0x5b,0];
        let raw=Tx{chain_id:777938,nonce:0,max_priority:U256::from(7),max_fee:U256::from(2_000_000_007u64),gas:200000,to:None,value:U256::ZERO,data:Bytes::from(init(&runtime))}.sign_1559(&alice);
        node.submit_tx(raw.clone(),false).unwrap();let block=solve(&node,vec![raw],cfg.g_ts+4);assert!(node.import(block,None).unwrap());
        let result=rpc(&mut node,"eth_estimateGas",serde_json::json!([{"from":alice.address(),"to":alice.address().create(0)},"latest"]));
        assert!(result.get("error").is_none(),"the next block has an 8M GASLIMIT and creates no slot: {result}");
    }

    /// A 1559 transfer of 1 wei with an explicit tip and gas limit.
    fn tipped(key:&pocol_lp4_execution::txsign::Key,nonce:u64,tip:u64,max_fee:U256,to:Address,gas:u64)->Vec<u8>{
        Tx{chain_id:777938,nonce,max_priority:U256::from(tip),max_fee,gas,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(key)
    }

    /// N4 x D-V3-21 review: a parked transaction is not executed, so before one evicts a parked one by fee it passes the
    /// pool's checks on the head state. A 21000-gas transfer to a new account (a common wallet default) whose burn
    /// passes its signed max_fee*gas_limit can never be included; at a full parked set it is refused with the creation
    /// charge's answer and the valid parked transaction stays. The same bid without a burn evicts it. With free parked
    /// room the over-burn one is still parked (it is not executed; network.md, Parked set), and the selection skips it.
    #[test]
    fn a_parked_over_burn_newcomer_evicts_nothing_by_fee(){
        let f0=U256::from(100_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.acct_cap=8;c.f0_acct=f0;for s in ["filler","low","hi"]{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(8_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        let gl=node.chain.spec.cp().GAS_LIMIT;assert_eq!(gl,8_000_000);
        let max_fee=U256::from(100_000_000_000u64);let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        node.submit_tx(tipped(&key("filler"),0,1_000_000_000,max_fee,sink,gl),false).unwrap();
        assert_eq!(node.pool.gas,node.pool.gas_max,"the gas bound is held");
        node.set_pool_parked_max(1);
        let low=tipped(&key("low"),0,1_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(low.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let accounts=node.chain.head().summary.as_ref().unwrap().user_accounts;
        assert!(max_fee*U256::from(21000)<creation_fees::price(f0,8,accounts,1).unwrap(),"the burn passes the signed maximum");
        let hi=tipped(&key("hi"),0,3_000_000_000,max_fee,new_address(199),21000);
        let e=node.submit_tx(hi.clone(),false).unwrap_err();assert_eq!(e,"-32000 creation charge would halt (CreationFee)");
        assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted,node.stats.parked_evicted),(vec![low.clone()],0,0),"nothing evicted");
        node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_selection(&p,cfg.g_ts+4).txs.contains(&low));
        // The same bid without a burn evicts it, as before.
        let hi2=tipped(&key("bob"),0,3_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(hi2.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted),(vec![hi2.clone()],1));node.pool_audit().unwrap();
        // Free parked room: the over-burn one is held (two reads, no execution) but never selected.
        node.set_pool_parked_max(2);
        assert_eq!(node.submit_tx(hi.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let sel=node.template_selection(&p,cfg.g_ts+4);assert!(sel.txs.contains(&hi2)&&!sel.txs.contains(&hi));
    }

    /// N4 x D-V3-21 review: the same for PR6's declared-gas eviction, which comes first. Honest parked transactions: a
    /// creation whose wallet raised its gas limit for the burn (the most declared gas, PR6's first victim) and four
    /// cheap transfers. Over-burn 21000-gas transfers to new accounts at a higher tip are refused with the creation
    /// charge's answer and evict none of them; the template is unchanged. A valid newcomer still evicts the creation by
    /// PR6's rule.
    #[test]
    fn parked_over_burn_newcomers_evict_no_honest_parked_ones(){
        let f0=U256::from(10_000_000_000_000_000u64);
        let honest=["h0","h1","h2","h3"];let spam=["z0","z1","z2","z3","z4"];
        let cfg=node_cfg(|c|{c.acct_cap=64;c.f0_acct=f0;for s in honest.iter().chain(spam.iter()).chain(["filler","maker","valid"].iter()){c.accounts.push(((*s).into(),U256::from(10u128.pow(24))));}});
        let spec=node_spec(&cfg);let gl=spec.cp().GAS_LIMIT;
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(gl);config.limits.pool_sender_gas_max=Some(gl);
        let mut node=pocold::node::Node::open(config,spec).unwrap();node.set_pool_parked_max(5);
        let max_fee=U256::from(100_000_000_000u64);assert!(next_base(&node)+U256::from(3_000_000_000u64)<max_fee);
        let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        let filler=tipped(&key("filler"),0,2_000_000_000,max_fee,sink,gl);node.submit_tx(filler.clone(),false).unwrap();
        assert_eq!(node.pool.gas,gl,"the gas bound is held");
        let accounts=node.chain.head().summary.as_ref().unwrap().user_accounts;let burn=creation_fees::price(f0,64,accounts,1).unwrap();
        let g=u64::try_from(burn/max_fee).unwrap()*2+21000;assert!(max_fee*U256::from(g)>=burn+max_fee*U256::from(21000u64));
        let made=tipped(&key("maker"),0,2_000_000_000,max_fee,new_address(201),g);
        assert_eq!(node.submit_tx(made.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        for h in honest{assert_eq!(node.submit_tx(tipped(&key(h),0,1_000_000_000,max_fee,sink,21000),false).unwrap_err(),pocold::node::POOL_PARKED);}
        let parked=node.pool_parked();assert_eq!(parked.len(),5);node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();let before=node.template_txs(&p);
        assert_eq!(before.len(),6,"the filler and the 5 honest parked ones");
        assert!(max_fee*U256::from(21000u64)<burn,"the spam's burn alone is past its signed maximum");
        for (i,z) in spam.iter().enumerate(){
            let e=node.submit_tx(tipped(&key(z),0,3_000_000_000,max_fee,new_address(210+i as u8),21000),false).unwrap_err();
            assert_eq!(e,"-32000 creation charge would halt (CreationFee)");
        }
        assert_eq!((node.pool_parked(),node.stats.parked_evicted,node.stats.parked_fee_evicted),(parked.clone(),0,0),"no honest parked one was evicted");
        node.pool_audit().unwrap();assert_eq!(node.template_txs(&p),before);
        // A valid newcomer declaring less gas evicts the creation by PR6's rule, as before.
        let valid=tipped(&key("valid"),0,3_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(valid.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let now=node.pool_parked();assert!(now.contains(&valid)&&!now.contains(&made));assert_eq!(node.stats.parked_evicted,1);node.pool_audit().unwrap();
    }

    /// N4 x D-V3-21 review: a replaced (or evicted) transaction's writes stay in the pending state until the next head,
    /// so there a creation can look done. alice pools a transfer creating account A (gas raised for the burn) and
    /// replaces it with a plain transfer; A stays in the pending state. At the count bound a newcomer that would evict
    /// also passes the pool's checks on the head state: 21000-gas transfers to A, whose burn on the head is past their
    /// signed maximum, are refused with the creation charge's answer and evict nothing; the same bid to a funded
    /// account evicts one.
    #[test]
    fn a_ghost_write_lets_no_over_burn_newcomer_evict(){
        use pocold::node::POOL_MAX_TXS;
        let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();let attackers:Vec<String>=(0..8).map(|i|format!("a{i}")).collect();
        let f0=U256::from(10_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.acct_cap=64;c.f0_acct=f0;for s in senders.iter().chain(attackers.iter()).map(|s|s.as_str()).chain(["zed"]){c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let max_fee=U256::from(100_000_000_000u64);
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(POOL_MAX_TXS as u64*21000+10_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        let accounts=node.chain.head().summary.as_ref().unwrap().user_accounts;let burn=creation_fees::price(f0,64,accounts,1).unwrap();
        assert!(next_base(&node)+U256::from(6_000_000_000u64)<max_fee);
        let sink=devnet::account_key("alice").address();let a_new=new_address(220);let alice=devnet::account_key("alice");
        let g=u64::try_from(burn/max_fee).unwrap()*2+21000;
        node.submit_tx(tipped(&alice,0,5_000_000_000,max_fee,a_new,g),false).unwrap();
        let plain=tipped(&alice,0,6_000_000_000,max_fee*U256::from(2),sink,21000);node.submit_tx(plain.clone(),false).unwrap();assert_eq!(node.stats.pool_replaced,1);
        let mut nonce=vec![0u64;senders.len()];let mut k=0;
        while node.pool.txs.len()<POOL_MAX_TXS{let i=k%senders.len();node.submit_tx(tipped(&devnet::account_key(&senders[i]),nonce[i],1_000_000_000,max_fee,sink,21000),false).unwrap();nonce[i]+=1;k+=1;}
        node.pool_audit().unwrap();
        let e=node.submit_tx(tipped(&devnet::account_key("zed"),0,3_000_000_000,max_fee,new_address(221),21000),false).unwrap_err();assert!(e.contains("CreationFee"),"{e}");
        assert!(max_fee*U256::from(21000u64)<burn,"on the head a 21000-gas transfer to A burns past its signed maximum");
        for a in &attackers{
            let e=node.submit_tx(tipped(&devnet::account_key(a),0,3_000_000_000,max_fee,a_new,21000),false).unwrap_err();
            assert_eq!(e,"-32000 creation charge would halt (CreationFee)");
        }
        assert_eq!((node.pool.txs.len(),node.stats.pool_evicted,node.pool.ghost_gas),(POOL_MAX_TXS,0,0),"nothing evicted");node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_txs(&p).contains(&plain));
        // The same bid to a funded account evicts one, as before.
        node.submit_tx(tipped(&devnet::account_key(&attackers[0]),0,3_000_000_000,max_fee,sink,21000),false).unwrap();
        assert_eq!((node.pool.txs.len(),node.stats.pool_evicted,node.pool.ghost_gas),(POOL_MAX_TXS,1,21000));node.pool_audit().unwrap();
    }

    /// N4 x D-V3-21 review, documented (network.md, Replace-by-fee): a pooled replacement is priced at the pending
    /// state's utilisation, which counts what the old transaction creates, so near a cap it pays as if one more unit
    /// were taken. bob's four creations bring the pending state to 7 of 8 accounts; alice's transfer creating the 8th
    /// is pooled and selected, but her fee bump of it (same gas limit and recipient) is refused (AcctCap) and the
    /// original stays pooled and selected.
    #[test]
    fn a_replacement_creating_the_last_free_account_is_priced_with_its_predecessor(){
        let dir=Temp::new();let f0=U256::from(1000u64);let cfg=node_cfg(|c|{c.acct_cap=8;c.f0_acct=f0});
        let mut node=pocold::node::Node::open(node_config(&dir,false),node_spec(&cfg)).unwrap();let alice=devnet::account_key("alice");
        assert_eq!(node.chain.head().summary.as_ref().unwrap().user_accounts,3);
        let max_fee=U256::from(100_000_000_000u64);let to=new_address(180);
        let bob=devnet::account_key("bob");for i in 0..4u8{node.submit_tx(priced_tx(&bob,u64::from(i),new_address(170+i),U256::from(1),100_000,max_fee),false).unwrap();}
        let first=priced_tx(&alice,0,to,U256::from(1),100_000,max_fee);node.submit_tx(first.clone(),false).unwrap();
        let bumped=Tx{chain_id:777938,nonce:0,max_priority:U256::from(100),max_fee:max_fee*U256::from(2),gas:100_000,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(&alice);
        assert_eq!(node.submit_tx(bumped,false).unwrap_err(),"-32000 creation charge would halt (AcctCap)");
        assert_eq!((node.pool.txs.len(),node.stats.pool_replaced),(5,0));
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_selection(&p,cfg.g_ts+4).txs.contains(&first));
    }

    /// N4 x D-V3-21 re-review: a parked transaction whose burn depends on a pooled transaction a block takes before it.
    /// tx1 (pooled, 3 gwei) sends 1 wei to X; tx2 (3 gwei, 100k gas, so its signed maximum is 1e16) is a creation whose
    /// init code writes a slot only when the address it reads has a balance. On the head X is empty, so the head checks
    /// alone pass it; every block takes tx1 first, so tx2 creates the slot and burns about 1e17, past its maximum. At a
    /// full parked set it would evict `low` by fee: it is also checked on the pending state (which has tx1's effects),
    /// so it is refused with the creation charge's answer, low stays and is selected, and the refusal counts in
    /// evictRefused. The same holds when tx1 is another sender's. The control (the init code reads an address no one
    /// funds) evicts low and is selected.
    fn predecessor_case(watch:u8,same_sender:bool)->Result<(),String>{
        let f0s=U256::from(100_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.f0_slot=f0s;for s in ["filler","low","ann"]{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(8_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        let max_fee=U256::from(100_000_000_000u64);assert!(next_base(&node)+U256::from(3_000_000_000u64)<max_fee);
        let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        let ann=key("ann");let first=if same_sender{key("ann")}else{key("bob")};
        let tx1=tipped(&first,0,3_000_000_000,max_fee,new_address(230),100_000);node.submit_tx(tx1.clone(),false).unwrap();
        let filler=tipped(&key("filler"),0,3_000_000_000,max_fee,sink,8_000_000-150_000);node.submit_tx(filler.clone(),false).unwrap();
        node.set_pool_parked_max(1);
        let low=tipped(&key("low"),0,1_000_000_000,max_fee,sink,60_000);
        assert_eq!(node.submit_tx(low.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let mut code=vec![0x73];code.extend_from_slice(new_address(watch).as_slice());code.extend_from_slice(&[0x31,0x60,26,0x57,0x00,0x5b,0x60,1,0x60,0,0x55,0x00]);
        assert_eq!(code[26],0x5b);
        let tx2=Tx{chain_id:777938,nonce:u64::from(same_sender),max_priority:U256::from(3_000_000_000u64),max_fee,gas:100_000,to:None,value:U256::ZERO,data:Bytes::from(code)}.sign_1559(&ann);
        assert!(max_fee*U256::from(100_000u64)<f0s,"one slot's burn passes tx2's signed maximum");
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
    fn a_parked_newcomer_whose_burn_depends_on_a_pooled_predecessor_evicts_nothing(){
        for same in [true,false]{
            assert_eq!(predecessor_case(231,same),Ok(()),"control: the init code reads an address no one funds");
            assert_eq!(predecessor_case(230,same).unwrap_err(),"-32000 creation charge would halt (CreationFee)","same sender: {same}");
        }
    }

    /// N4 x D-V3-21 re-review: a replacement of a sender's first parked transaction passes the checks an evicting one
    /// passes, so a transaction cannot evict by fee and then be swapped for one that could not have evicted. hi's valid
    /// transfer at 3 gwei evicts `low`; its bumped replacement (4 gwei, 2x max_fee, 21000 gas) to a new account, whose
    /// burn passes its signed maximum, is refused with the creation charge's answer, and the parked set stays [good],
    /// which is selected. A bumped replacement without a burn replaces it.
    #[test]
    fn a_parked_replacement_passes_the_eviction_checks(){
        let f0=U256::from(100_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.acct_cap=8;c.f0_acct=f0;for s in ["filler","low","hi"]{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let dir=Temp::new();let mut config=node_config(&dir,false);config.limits.pool_gas_max=Some(8_000_000);
        let mut node=pocold::node::Node::open(config,node_spec(&cfg)).unwrap();
        let gl=node.chain.spec.cp().GAS_LIMIT;
        let max_fee=U256::from(100_000_000_000u64);let sink=devnet::account_key("alice").address();let key=|s:&str|devnet::account_key(s);
        node.submit_tx(tipped(&key("filler"),0,1_000_000_000,max_fee,sink,gl),false).unwrap();
        node.set_pool_parked_max(1);
        let low=tipped(&key("low"),0,1_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(low.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        let good=tipped(&key("hi"),0,3_000_000_000,max_fee,sink,21000);
        assert_eq!(node.submit_tx(good.clone(),false).unwrap_err(),pocold::node::POOL_PARKED);
        assert_eq!((node.pool_parked(),node.stats.parked_fee_evicted),(vec![good.clone()],1),"low evicted by a valid one");
        let accounts=node.chain.head().summary.as_ref().unwrap().user_accounts;
        let bad=tipped(&key("hi"),0,4_000_000_000,max_fee*U256::from(2),new_address(231),21000);
        assert!(max_fee*U256::from(2*21000u64)<creation_fees::price(f0,8,accounts,1).unwrap());
        assert_eq!(node.submit_tx(bad.clone(),false).unwrap_err(),"-32000 creation charge would halt (CreationFee)");
        assert_eq!((node.pool_parked(),node.stats.pool_replaced),(vec![good.clone()],0),"the parked one stays");node.pool_audit().unwrap();
        let p=node.chain.full_entry(&node.chain.head).unwrap();assert!(node.template_selection(&p,cfg.g_ts+4).txs.contains(&good));
        let ok=tipped(&key("hi"),0,4_000_000_000,max_fee*U256::from(2),sink,21000);
        node.submit_tx(ok.clone(),false).unwrap();
        assert_eq!((node.pool_parked(),node.stats.pool_replaced),(vec![ok.clone()],1));node.pool_audit().unwrap();
    }

    /// N4 x D-V3-21 re-review, documented (network.md, Lowest-fee eviction): the eviction checks price a newcomer on
    /// the head state too, where its sender's own earlier pooled transactions have not run. alice's follow-up to her
    /// pooled creation of X (a 21000-gas transfer to X) is valid in sequence and, below the bounds, admitted and
    /// selected after it; at the count bound, with cheaper victims, it is priced on the head as a creation and refused
    /// (CreationFee) until the next head, and nothing is evicted. The same bid to a funded account evicts one.
    #[test]
    fn a_follow_up_to_its_senders_pooled_creation_is_priced_on_the_head_at_the_bound(){
        use pocold::node::POOL_MAX_TXS;
        let senders:Vec<String>=(0..16).map(|i|format!("s{i}")).collect();
        let f0=U256::from(10_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.acct_cap=64;c.f0_acct=f0;for s in senders.iter().map(|s|s.as_str()){c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let max_fee=U256::from(100_000_000_000u64);
        let open=|dir:&Temp|{let mut config=node_config(dir,false);config.limits.pool_gas_max=Some(POOL_MAX_TXS as u64*21000+10_000_000);pocold::node::Node::open(config,node_spec(&cfg)).unwrap()};
        let (d1,d2)=(Temp::new(),Temp::new());let (mut node,mut below)=(open(&d1),open(&d2));
        let accounts=node.chain.head().summary.as_ref().unwrap().user_accounts;let burn=creation_fees::price(f0,64,accounts,1).unwrap();
        let sink=devnet::account_key("bob").address();let x=new_address(230);let alice=devnet::account_key("alice");
        let g=u64::try_from(burn/max_fee).unwrap()*2+21000;
        let create=tipped(&alice,0,5_000_000_000,max_fee,x,g);let follow=tipped(&alice,1,3_000_000_000,max_fee,x,21000);
        assert!(max_fee*U256::from(21000u64)<burn,"on the bare head the follow-up would burn past its maximum");
        below.submit_tx(create.clone(),false).unwrap();below.submit_tx(follow.clone(),false).unwrap();
        let p=below.chain.full_entry(&below.chain.head).unwrap();let sel=below.template_txs(&p);
        assert!(sel.contains(&create)&&sel.contains(&follow),"valid in sequence");
        node.submit_tx(create.clone(),false).unwrap();
        let mut nonce=vec![0u64;senders.len()];let mut k=0;
        while node.pool.txs.len()<POOL_MAX_TXS{let i=k%senders.len();node.submit_tx(tipped(&devnet::account_key(&senders[i]),nonce[i],1_000_000_000,max_fee,sink,21000),false).unwrap();nonce[i]+=1;k+=1;}
        let refused=node.stats.evict_refused;
        assert_eq!(node.submit_tx(follow.clone(),false).unwrap_err(),"-32000 creation charge would halt (CreationFee)");
        assert_eq!((node.stats.pool_evicted,node.stats.evict_refused,node.pool.txs.len()),(0,refused+1,POOL_MAX_TXS),"nothing evicted");node.pool_audit().unwrap();
        node.submit_tx(tipped(&alice,1,3_000_000_000,max_fee,sink,21000),false).unwrap();assert_eq!(node.stats.pool_evicted,1);node.pool_audit().unwrap();
    }

    /// N4 x D-V3-21 re-review: neither the head nor the pending state has a parked transaction's effects, so under v3 a
    /// transaction behind its sender's parked ones evicts nothing, whatever it would pay: alice's parked creation of X
    /// and her follow-up transfer to X (valid in sequence; with free room it is parked) at a full parked set would evict
    /// `low` by fee, and so would her plain transfer to a funded account. Both are refused "pool full (gas)", counted in
    /// evictRefused, and low stays.
    #[test]
    fn a_parked_run_member_evicts_nothing_under_v3(){
        let f0=U256::from(10_000_000_000_000_000u64);
        let cfg=node_cfg(|c|{c.acct_cap=64;c.f0_acct=f0;for s in ["filler","low"]{c.accounts.push((s.into(),U256::from(10u128.pow(24))));}});
        let open=|dir:&Temp|{let mut config=node_config(dir,false);config.limits.pool_gas_max=Some(8_000_000);pocold::node::Node::open(config,node_spec(&cfg)).unwrap()};
        let max_fee=U256::from(100_000_000_000u64);let sink=devnet::account_key("bob").address();let key=|s:&str|devnet::account_key(s);
        let x=new_address(232);let alice=key("alice");
        let (d1,d2)=(Temp::new(),Temp::new());let (mut node,mut free)=(open(&d1),open(&d2));
        let gl=node.chain.spec.cp().GAS_LIMIT;
        let accounts=node.chain.head().summary.as_ref().unwrap().user_accounts;let burn=creation_fees::price(f0,64,accounts,1).unwrap();
        let g=u64::try_from(burn/max_fee).unwrap()*2+21000;assert!(max_fee*U256::from(21000u64)<burn);
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
}
