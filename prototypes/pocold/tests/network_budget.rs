//! A13 at the network profile: the model the 6 GiB gate checks, itemized, at the fresh network's own committed profile
//! (deploy/mainnet/v3-profile.json) and on the v3 devnet pocold boots. (The retired spec-1 network profile of
//! governance.md, the spec-1 devnet and the generated spec-1 continuous network these tests also modelled went with the
//! spec-1 rule set in Phase 6a; each property that exists on v3 is checked on it below.)
use pocol_lp4_execution::devnet::{self,DevnetConfig};

fn print(tag:&str,b:&pocold::resources::Bound){
    let gib=|x:u128|x as f64/(1u128<<30) as f64;
    let need=pocold::resources::budget_required(b.total).unwrap();
    println!("{tag} {}",serde_json::json!({"total":b.total.to_string(),"totalGiB":gib(b.total),"budgetRequiredGiB":gib(need),"budgetGiB":6,
        "snapshotGiB":gib(b.snapshot_bytes),"snapshots":b.snapshots.to_string(),
        "groupsGiB":{"snapshots":gib(b.snapshot_total),"diffs":gib(b.diffs),"trie":gib(b.trie),"code":gib(b.code),"store":gib(b.store),"history":gib(b.history),"side":gib(b.side_blocks),"queues":gib(b.queues),"rpc":gib(b.rpc),"threads":gib(b.threads),"execution":gib(b.execution)},
        "items":b.items.iter().map(|(k,v)|serde_json::json!([k,v.to_string(),gib(*v)])).collect::<Vec<_>>()}));
}
/// D-7 (A13-DERIVATION §2d): the pool's terms follow its gas bound. The layer holds what that gas can write
/// (`layer_writes`); its code is its creations in both modes (in store mode its entries keep no copy of the code
/// they were read with, `Layer::drop_code_copies`). A bound past POOL_MAX_TXS ×
/// GAS_LIMIT counts as that (the count bound binds first). The block overlay is the layer of 5/4 GAS_LIMIT with
/// the SYSTEM phase's accounts and keys.
#[test] fn d7_a13_pool_terms_follow_the_gas_bound(){
    use pocold::{node::{Limits,POOL_MAX_TXS},resources::{self as r,derive}};use pocol_lp4_execution::resources::memory::code_bytes;
    let l=pocold::single_host_local();
    // The v3 rules (the fresh network's profile and the v3 devnet), as a new node opens them (`at_open`).
    let v3=v3_network("d7");let (v3net,v3dev)=(v3.spec.cp().clone(),v3_devnet());
    let item=|b:&r::Bound,k:&str|b.items.iter().find(|x|x.0.starts_with(k)).unwrap_or_else(||panic!("{k}")).1;
    assert_eq!(r::layer_writes(32_000_000),Some((3*1523+13333+32,11034+320000)),"the counts at 4 × GAS_LIMIT");
    for cp in [&v3net,&v3dev]{
        let gl=u128::from(cp.GAS_LIMIT);let g=4*gl;let (pa,ps)=r::layer_writes(g).unwrap();
        for store in [false,true]{
            let at=|gmax:Option<u64>|derive(cp,32,l.side_max,&at_open(cp,Limits{state_store:store,pool_gas_max:gmax,..Default::default()}),2).unwrap();
            let b=at(None);
            assert_eq!(item(&b,"diffs/the pool's pending layer"),r::layer_bytes(pa,ps).unwrap());
            let spent=gl*5/4;let (oa,os)=r::layer_writes(spent).unwrap();
            // PR6: and its BLOCKHASH window (257 entries), which the template selection's overlay also carries.
            let window=257*pocol_lp4_execution::resources::memory::entry_bytes(64)+pocol_lp4_execution::resources::memory::table_bytes(64);
            assert_eq!(item(&b,"diffs/the overlay of the block being applied"),r::layer_bytes(oa+u128::from(cp.SYS_ACCT_BLOCK_MAX)+1,os+u128::from(cp.SYS_KEYS_BLOCK_MAX)).unwrap()+window);
            assert_eq!(item(&b,"code/the pool's pending layer"),code_bytes(g/r::CODE_DEPOSIT,g/r::CREATE_GAS).unwrap(),"store {store}");
            let b2=at(Some(8*cp.GAS_LIMIT));assert!(item(&b2,"diffs/the pool's pending layer")>item(&b,"diffs/the pool's pending layer")&&b2.total>b.total,"a larger bound counts more");
            assert_eq!(at(Some(u64::MAX)).total,at(Some(POOL_MAX_TXS as u64*cp.GAS_LIMIT)).total,"capped by the count bound");
            let body=u128::from(cp.BODY_MAX);assert_eq!(item(&b,"queues/an abandoned block's transactions"),body+body/r::MIN_TX_BYTES*80,"one abandoned body in revalidation");
            let e=r::check(cp,32,l.side_max,&at_open(cp,Limits{state_store:store,pool_gas_max:Some(cp.GAS_LIMIT-1),..Default::default()}),2).unwrap_err();assert!(e.contains("below GAS_LIMIT"),"{e}");
            // PR6: the pool's meta, the parked entries and the template selection's candidates, each at its bound; the
            // sender quota is checked with the gas bound.
            use pocold::node::{META_ENTRY_BYTES,PARKED_ENTRY_BYTES,POOL_PARKED_MAX};
            assert_eq!(item(&b,"queues/pool sender and declared gas"),3*POOL_MAX_TXS as u128*META_ENTRY_BYTES as u128);
            assert_eq!(item(&b,"queues/parked transactions' entries"),3*POOL_PARKED_MAX as u128*PARKED_ENTRY_BYTES as u128);
            // N4 (V3-20): the entries keep their PR6 sizes (the fee fields are read from the raw bytes), and the fee order's
            // scratch (FEE_ORDER_ENTRY_BYTES, a u32 and a u64 per pooled and parked candidate) takes the place of the
            // selection's first-seen sender set, which it replaces and does not exceed: no new term, no pin moves.
            assert_eq!((META_ENTRY_BYTES,PARKED_ENTRY_BYTES,pocold::node::FEE_ORDER_ENTRY_BYTES),(32,160,12),"the entry sizes the pinned totals were derived with");
            assert!(b.items.iter().all(|(k,_)|!k.contains("fee order")),"no separate fee-order term");
            {use pocol_lp4_execution::resources::memory::{entry_bytes,table_bytes};
             let set=(POOL_MAX_TXS+POOL_PARKED_MAX+pocold::node::BUILD_VISIT_MAX) as u128*entry_bytes(20)+table_bytes(20);
             assert_eq!(r::fee_order_bytes().unwrap(),(POOL_MAX_TXS+POOL_PARKED_MAX) as u128*12);assert!(r::fee_order_bytes().unwrap()<=set,"the fee order fits the set's place");}
            assert_eq!(item(&b,"queues/template selection"),r::selection_bytes(cp).unwrap());assert!(r::selection_bytes(cp).unwrap()>2*body);
            // PR6 review: after a gas failure the selection's tail (at most GAS_LIMIT/TX_GAS_MIN transactions, BODY_MAX
            // bytes) joins the append round's candidates, so the term counts it beside their own bound.
            {use pocol_lp4_execution::resources::memory::{entry_bytes,table_bytes};
            let entries=body/r::MIN_TX_BYTES+1+u128::from(cp.GAS_LIMIT)/r::TX_GAS_MIN;let per=3*pocold::node::CAND_ENTRY_BYTES as u128+3*32+3*24+entry_bytes(28);
            let senders=(POOL_MAX_TXS+POOL_PARKED_MAX+pocold::node::BUILD_VISIT_MAX) as u128;
            assert_eq!(r::selection_bytes(cp).unwrap(),2*body+u128::from(cp.TX_MAX)+entries*per+3*(senders*entry_bytes(20)+table_bytes(20)),"the candidates after a gas recovery");}
            let e=r::check(cp,32,l.side_max,&at_open(cp,Limits{state_store:store,pool_sender_gas_max:Some(cp.GAS_LIMIT-1),..Default::default()}),2).unwrap_err();assert!(e.contains("pool_sender_gas_max"),"{e}");
        }
    }
}

/// C-1 E4.5: the devnet profile pocold boots, whole-state and store mode with the default limits: both pass
/// the boot gate. Store mode is not the smaller bound here: the devnet's states are small, and its fixed
/// caches and worst-case batch and file terms outweigh them.
#[test] fn a13_store_model_at_the_devnet_profile(){
    let l=pocold::single_host_local();
    // The v3 devnet pocold boots (DevnetConfig::new), as a new node opens it.
    for (tag,cp) in [("V3-DEVNET",v3_devnet())]{
        let (m,s)=(at_open(&cp,pocold::node::Limits::default()),at_open(&cp,pocold::node::Limits{state_store:true,..Default::default()}));
        let bm=pocold::resources::check(&cp,l.k_hist,l.side_max,&m,0).unwrap_or_else(|e|panic!("{tag}: whole-state mode boots: {e}"));let bs=pocold::resources::check(&cp,l.k_hist,l.side_max,&s,0).unwrap_or_else(|e|panic!("{tag}: store mode boots: {e}"));
        print(&format!("A13-{tag}-MEM"),&bm);print(&format!("A13-{tag}-STORE"),&bs);
    }
}

/// Review 2026-10-27: the stored versions' handles and their bytes are counted from one inventory, and it holds
/// every version open keeps at once: genesis, the window of K_hist with the block being imported, the abandoned
/// window, the side states, and a rolling re-execution with the version it commits next (2·K_hist + side_max + 4).
/// C-1 S5 (PR7 §6.1): and the store-mode rebuild anchors, one version each (4 at the defaults, R_keep 2048 and
/// K_hist 32; none with eviction off or in whole-state mode).
#[test] fn a13_counts_a_handle_for_every_held_version(){
    use pocold::{node::Limits,resources as r};
    for (k,side,a) in [(32u32,2u32,0u128),(32,8,4),(1,0,0),(4096,64,3)]{
        let want=2*u128::from(k)+u128::from(side)+4+a;
        assert_eq!(r::held_runs(k,side,a).iter().sum::<u128>(),want,"the runs at K_hist {k}, side_max {side}, {a} anchors");
        assert_eq!(r::stored_versions(k,side,a),want,"a handle per held version at K_hist {k}, side_max {side}, {a} anchors");
    }
    let store=Limits{state_store:true,..Default::default()};
    // The versions' bytes are the same runs, and the anchors, at the fresh network's profile (E_win 360) and on the v3
    // devnet, as a new node opens them.
    let v3=v3_network("handles");
    for cp in [v3.spec.cp().clone(),v3_devnet()]{
        let one=r::chain_versions_bytes(&cp,1).unwrap();
        assert_eq!(r::held_versions_bytes(&cp,32,2,4).unwrap(),one+r::chain_versions_bytes(&cp,33).unwrap()+r::chain_versions_bytes(&cp,32).unwrap()+2*one+r::chain_versions_bytes(&cp,2).unwrap()+4*one,
            "the versions' bytes are the same runs");
        assert_eq!(r::anchors_held(&cp,32,&at_open(&cp,store.clone())),4,"⌈(2·2048 + 2 − 32) / 2048⌉ + 2 at the defaults (E_win {})",cp.E_win);
        assert_eq!((r::anchors_held(&cp,32,&at_open(&cp,Limits{evict_depth:Some(0),..store.clone()})),r::anchors_held(&cp,32,&at_open(&cp,Limits::default()))),(0,0),"none with eviction off or whole states");
    }
}

/// C-1 E4.5 (2026-10-27): state.redb's versions and a block's batch, re-derived from node_trie's bounds. The file
/// counts the versions held at once (genesis; the K_hist window and the block being imported; the abandoned window
/// until settle; side_max side states; a rolling re-execution and the version it commits next), once for the
/// entries and once for each kept checkpoint. A run of n consecutive versions is the oldest whole and n − 1 blocks'
/// additions, at most n whole versions.
#[test] fn a13_store_file_and_batch_terms_follow_the_node_trie_bounds(){
    use pocol_lp4_execution::node_trie::{batch_nodes,block_bytes,version_bytes,Id,Node,BRANCH_ENC,ROW};use pocold::resources as r;
    let l=pocold::single_host_local();
    // The v3 rules (the fresh network's profile and the v3 devnet), as a new node opens them.
    let v3=v3_network("file");let (v3net,v3dev)=(v3.spec.cp().clone(),v3_devnet());
    for cp in [&v3net,&v3dev]{
        let (ka,ks)=(u128::from(cp.ACCT_CAP)+5,u128::from(cp.SLOT_CAP)+u128::from(cp.SYS_SLOT_MAX));assert_eq!(r::state_keys(cp),Some((ka,ks)));
        let (wa,ws)=r::block_writes(cp).unwrap();let (whole,add)=(version_bytes(ka,ks).unwrap(),block_bytes(wa,ws,ka,ks).unwrap());
        let run=|n:u128|(n*whole).min(whole+(n-1)*add);
        for n in [1u128,2,5,33]{assert_eq!(r::chain_versions_bytes(cp,n),Some(run(n)),"a run of {n}");}
        let k=32u128;let sm=u128::from(l.side_max);
        let lim=at_open(cp,pocold::node::Limits{state_store:true,..Default::default()});let anchors=r::anchors_held(cp,32,&lim);assert_eq!(anchors,4);
        let held=whole+run(k+1)+run(k)+sm*whole+run(2)+anchors*whole;assert_eq!(r::held_versions_bytes(cp,32,l.side_max,anchors),Some(held));
        let p=r::state_file_parts(cp,32,l.side_max,&lim).unwrap();
        // D-6 (2026-10-27): and one dropped checkpoint whose nodes are owed, with a release row for at most each of its
        // nodes (each counted at ROW + BRANCH_ENC bytes or more).
        let owed_rows=ROW*(held/(ROW+BRANCH_ENC)+1);
        assert_eq!(p[0],r::STATE_FILE_FACTOR*((pocold::checkpoint::KEPT as u128+2)*held+owed_rows),"the entries' versions, each kept checkpoint's and one owed");
        // The batch: its branches and leaves behind their Arcs and map slots, every branch's children, the write set.
        let (b,lv,kids)=batch_nodes(wa,ws,ka,ks).unwrap();let slot=|n:usize|pocol_lp4_execution::resources::memory::entry_bytes(n as u128);
        let node=(2*std::mem::size_of::<usize>()+std::mem::size_of::<Node>()) as u128+slot(std::mem::size_of::<(Id,std::sync::Arc<Node>)>());
        let ws_bytes=wa*slot(std::mem::size_of::<(revm::primitives::Address,Option<Vec<revm::primitives::U256>>)>())+ws*pocol_lp4_execution::resources::memory::VEC*32;
        assert_eq!(r::batch_max(cp),Some((b+lv)*node+kids*std::mem::size_of::<(Id,pocol_lp4_execution::state_trie::Ref)>() as u128+ws_bytes));
        assert!(b<=64*(wa+ws)&&lv<=2*(wa+ws)&&kids<=16*b,"within the paths");
    }
    // At the network profile the window's run is the oldest version and its blocks' additions, not K_hist + 1 whole versions.
    assert!(r::chain_versions_bytes(&v3net,33).unwrap()<33*r::chain_versions_bytes(&v3net,1).unwrap(),"at the fresh network's");
}


/// PR2a (plan §9): peer discovery's queue terms are counted only with discovery on, each at its formula: the book's
/// BOOK_MAX entries reserved once; the links table at its churn high-water (the live peers, PR2b's S automatic slot
/// places among them, and one stale link per Closed still queued, the slots' forwarder's queue included); the
/// transient scratch; the file's read and save buffers; PR2b's outbound state with any slot. Off, no term is counted,
/// and without automatic outbound (`Count(0)`) the total is the on-total less exactly their sum. Local mode changes
/// nothing.
#[test] fn a13_discovery_terms_are_counted_only_when_enabled(){
    use pocold::{addrbook::{self as ab,BOOK_MAX,CACHE_MAX,SELF_MAX,ANCHORS_MAX,FILE_LINE_MAX},node::{self,Discovery,Limits,Outbound},net,resources::derive};use std::mem::size_of;
    // On the v3 devnet, as a new node opens it.
    for cp in [v3_devnet()]{let l=pocold::single_host_local();
    let disc=|b:&pocold::resources::Bound|b.items.iter().filter(|x|x.0.starts_with("queues/discovery")).cloned().collect::<Vec<_>>();
    let link=((node::LINK_BYTES as u128+1)*32).div_ceil(7);
    for (peers_max,dials) in [(net::Limits::default().peers_max,2usize),(16,5),(1,0)]{for outbound in [Discovery::default().outbound,Outbound::Count(0),Outbound::Count(8)]{
        let at=|d:Discovery|derive(&cp,32,l.side_max,&at_open(&cp,Limits{net:net::Limits{peers_max,..Default::default()},discovery:d,..Default::default()}),dials).unwrap();
        let d=Discovery{outbound,..Default::default()};let (s,f)=(d.slots(),usize::from(d.slots()>0));
        let (on,off,local)=(at(d.clone()),at(Discovery{enabled:false,outbound,..Default::default()}),at(Discovery{allow_local:true,..d.clone()}));
        assert!(disc(&off).is_empty(),"off: {:?}",disc(&off));
        let high=(net::Limits{peers_max,..Default::default()}.inbound_max()+dials+s+node::EVENT_QUEUE+(1+dials+f)*(net::NET_EVENT_QUEUE+1)) as u128;
        let want=[("queues/discovery: address book (BOOK_MAX, reserved once)",BOOK_MAX as u128*size_of::<ab::Entry>() as u128+512),
            ("queues/discovery: per-connection links (churn)",high*2*link+512),
            ("queues/discovery: selection, answer and self scratch",(2*BOOK_MAX*(size_of::<ab::Source>()+size_of::<ab::Key>()+8)+2*CACHE_MAX*size_of::<std::net::SocketAddr>()+(SELF_MAX+ANCHORS_MAX+7)*64+1024) as u128),
            ("queues/discovery: address-book file buffers",(pocold::launcher::MAX_INVITATION+1+FILE_LINE_MAX*(BOOK_MAX+ANCHORS_MAX+3)) as u128),
            ("queues/discovery: outbound state",(f*node::OUT_STATE_BYTES) as u128)];
        assert_eq!(disc(&on),want,"peers_max {peers_max}, dials {dials}, {outbound:?}");
        let sum:u128=want.iter().map(|x|x.1).sum();
        if s==0{assert_eq!((on.total-sum,on.queues-sum),(off.total,off.queues),"off is on less exactly the discovery terms");}
        assert_eq!(local,on,"local mode costs nothing more");
    }}}
    assert_eq!(BOOK_MAX,1024);
    if cfg!(all(target_os="linux",target_arch="x86_64")){assert_eq!((size_of::<ab::Entry>(),node::LINK_BYTES),(152,144),"the type sizes the pinned total was derived with");}
}


/// PR3: A13 counts every physical inbound place, S = inbound_max = peers_max + handshaking_max (the established ones and
/// those waiting for a Hello; an evicted peer keeps its place until its reader releases it), in each per-place term: the
/// send queues and frames, the group entries, the connection high-water (`high`, with the node's maps at 2,718 B an
/// entry and each connection's activity record), the I/O threads and the exiting ones; and the eviction registry (a
/// slot and a selection candidate per place, reserved once, and the ring). At the defaults one more established place
/// costs 3,424,620 B of model (PR2b: 3,424,474 before the link's automatic-outbound fields, 2·73 B a link).
#[test] fn a13_inbound_terms_count_every_inbound_place_and_the_eviction_registry(){
    use pocold::{net,node::{self,Limits},resources::derive,threads::IO_STACK};
    // On the v3 devnet, as a new node opens it.
    for cp in [v3_devnet()]{let l=pocold::single_host_local();
    let item=|b:&pocold::resources::Bound,k:&str|b.items.iter().find(|x|x.0.starts_with(k)).unwrap_or_else(||panic!("{k}")).1;
    let at=|peers_max:usize,handshaking_max:usize,dials:usize|derive(&cp,32,l.side_max,&at_open(&cp,Limits{net:net::Limits{peers_max,handshaking_max,..Default::default()},..Default::default()}),dials).unwrap();
    let conn_max=Limits::default().rpc.conn_max as u128;assert_eq!(conn_max,32,"the RPC connections the io and exiting terms were derived with");
    let (q,f)=(net::Limits::default().queue_bytes_max as u128,net::MAX_FRAME as u128);
    for (p,h,d) in [(8usize,8usize,2usize),(32,8,2),(64,8,7),(1,1,0)]{
        // PR2b: the default automatic outbound (S = 5 slot places with the spare, F = 1 forwarder).
        let (sl,fw)=(node::Discovery::default().slots() as u128,1u128);
        let b=at(p,h,d);let (s,d)=((p+h) as u128,d as u128);
        let high=s+d+sl+node::EVENT_QUEUE as u128+(1+d+fw)*(net::NET_EVENT_QUEUE as u128+1);
        assert_eq!(item(&b,"queues/inbound eviction"),s*192+high*64+3072,"the registry at ({p}, {h}, {d})");
        assert_eq!(item(&b,"queues/node peer and status maps"),high*2718+1024,"the maps at ({p}, {h}, {d})");
        assert_eq!(item(&b,"queues/peer send queues and frames"),(s+d+sl)*(q+2*f+4096),"the queues at ({p}, {h}, {d})");
        assert_eq!(item(&b,"threads/I/O thread stacks and heap"),(2*s+4*d+3*sl+fw+conn_max+6)*(IO_STACK as u128+4096),"the io threads at ({p}, {h}, {d})");
        assert_eq!(item(&b,"threads/exiting not-yet-joined"),(s+d+sl+conn_max)*IO_STACK as u128,"the exiting threads at ({p}, {h}, {d})");
    }
    let d=net::Limits::default();assert_eq!(at(d.peers_max+1,d.handshaking_max,2).total-at(d.peers_max,d.handshaking_max,2).total,3_424_620,"one more established place");
    }
    assert_eq!((net::SLOT_BYTES,net::CAND_BYTES,net::ACTIVITY_BYTES,net::EVICT_RING,net::EVICT_ENTRY_BYTES),(96,96,64,64,32),"the constants the pinned totals were derived with");
}
/// PR2b (plan §4.2, VA-SPEC §1a.7): automatic outbound costs exactly its terms. Each slot place (S = K + 1, the spare's
/// included) is a peer send queue with its frames, three threads (the slot's, its connection's reader and writer), an
/// exiting reader, and one more connection the node can know at once (its map entries, activity record and two
/// links); with any slot, one forwarder (its queue, its thread, and the connections its queue can hold) and the
/// outbound state. Discovery off counts none of it, whatever the setting.
#[test] fn a13_outbound_slots_cost_exactly_their_terms(){
    use pocold::{net,node::{self,Limits,Discovery,Outbound},resources::derive,threads::IO_STACK};
    // On the v3 devnet, as a new node opens it.
    for cp in [v3_devnet()]{let l=pocold::single_host_local();
    let at=|o:Outbound,on:bool|derive(&cp,32,l.side_max,&at_open(&cp,Limits{discovery:Discovery{enabled:on,outbound:o,..Default::default()},..Default::default()}),2).unwrap();
    let item=|b:&pocold::resources::Bound,k:&str|b.items.iter().find(|x|x.0.starts_with(k)).unwrap_or_else(||panic!("{k}")).1;
    let link=((node::LINK_BYTES as u128+1)*32).div_ceil(7);let (q,fr)=(net::Limits::default().queue_bytes_max as u128,net::MAX_FRAME as u128);
    let (io,stack,nq)=(IO_STACK as u128+4096,IO_STACK as u128,net::NET_EVENT_QUEUE as u128+1);
    let per_high=2718+64+2*link;
    let p_s=(q+2*fr+4096)+3*io+stack+per_high;
    let p_f=nq*(fr+256)+io+nq*per_high+node::OUT_STATE_BYTES as u128;
    let t=|o:Outbound|at(o,true).total;
    assert_eq!(t(Outbound::Auto)-t(Outbound::Count(0)),5*p_s+p_f,"auto counts S = 5 slot places and the forwarder");
    assert_eq!(t(Outbound::Count(8))-t(Outbound::Count(4)),4*p_s,"four more slots");
    assert_eq!(t(Outbound::Count(1))-t(Outbound::Count(0)),2*p_s+p_f,"one slot and the spare");
    assert_eq!(at(Outbound::Auto,false).total,at(Outbound::Count(0),false).total,"discovery off: no slot terms");
    println!("A13-OUTBOUND {}",serde_json::json!({"slotPlace":p_s.to_string(),"forwarder":p_f.to_string(),"auto":(5*p_s+p_f).to_string()}));
    let (a,z)=(at(Outbound::Auto,true),at(Outbound::Count(0),true));
    for (k,want) in [("queues/peer send queues and frames",5*(q+2*fr+4096)),("queues/network event forwarders",nq*(fr+256)),("threads/I/O thread stacks and heap",16*io),
        ("threads/exiting not-yet-joined",5*stack),("queues/node peer and status maps",(5+nq)*2718),("queues/inbound eviction",(5+nq)*64),
        ("queues/discovery: per-connection links",(5+nq)*2*link),("queues/discovery: outbound state",node::OUT_STATE_BYTES as u128)]{
        assert_eq!(item(&a,k)-item(&z,k),want,"{k}");
    }
    assert_eq!(item(&z,"queues/discovery: outbound state"),0,"no outbound state without slots");
    }
}
/// PR3 A2: the devnet profile pocold boots passes the A13 gate at a seed's settings (64 established, 8 waiting for a
/// Hello, 7 configured peers) and at the largest established count (128), in both modes.
#[test] fn a13_admits_seed_inbound_settings_at_the_devnet_profile(){
    // On the v3 devnet, as a new node opens it.
    for (tag,cp) in [("v3",v3_devnet())]{let l=pocold::single_host_local();
    for peers_max in [64usize,128]{for state_store in [false,true]{
        let lim=at_open(&cp,pocold::node::Limits{state_store,net:pocold::net::Limits{peers_max,handshaking_max:8,..Default::default()},..Default::default()});
        let b=pocold::resources::check(&cp,l.k_hist,l.side_max,&lim,7).unwrap_or_else(|e|panic!("{tag}: peers_max {peers_max}, store {state_store}: {e}"));
        println!("A13-DEVNET-INBOUND {}",serde_json::json!({"devnet":tag,"peersMax":peers_max,"stateStore":state_store,"total":b.total.to_string()}));
    }}}
}
/// V3-30 S1 (D-V3-30): the miner's threads are counted in A13, each at a full IO_STACK and its heap, and in the task
/// inventory; the default (one thread) is the bound and inventory of before V3-30, so the pinned totals are unchanged.
#[test] fn a13_counts_every_miner_thread(){
    use pocold::{node::Limits,resources::{derive,tasks_needed},threads::IO_STACK};
    // On the v3 devnet, as a new node opens it.
    for cp in [v3_devnet()]{let l=pocold::single_host_local();
    let at=|n:usize|derive(&cp,32,l.side_max,&at_open(&cp,Limits{mine_threads:n,..Default::default()}),2).unwrap();
    let item=|b:&pocold::resources::Bound,k:&str|b.items.iter().find(|x|x.0==k).unwrap_or_else(||panic!("{k}")).1;
    let io=IO_STACK as u128+4096;
    assert_eq!(at(1).total,derive(&cp,32,l.side_max,&at_open(&cp,Limits::default()),2).unwrap().total,"one thread is the default");
    assert_eq!(at(8).total-at(1).total,7*io,"seven more miner threads");
    assert_eq!(item(&at(64),"threads/I/O thread stacks and heap")-item(&at(1),"threads/I/O thread stacks and heap"),63*io);
    assert_eq!(item(&at(8),"threads/exiting not-yet-joined (conservative)"),item(&at(1),"threads/exiting not-yet-joined (conservative)"),
        "a miner's threads are joined before the next miner starts: none is exiting");
    let n=pocold::net::Limits::default();
    assert_eq!(tasks_needed(&n,2,5,32,8,false).unwrap()-tasks_needed(&n,2,5,32,1,false).unwrap(),7);
    assert_eq!(tasks_needed(&n,2,5,32,1,false),Some(144),"the defaults' inventory is unchanged");
    // The V3-30 work port: its acceptor only (its connections are among the conn_max, rpc::serve_all).
    let work=derive(&cp,32,l.side_max,&at_open(&cp,Limits{work_rpc:Some("127.0.0.1:1".parse().unwrap()),..Default::default()}),2).unwrap();
    assert_eq!(work.total-at(1).total,io,"one more thread");
    assert_eq!(tasks_needed(&n,2,5,32,1,true),Some(145));
    }
}

// ============================ Phase 5c: A13 on the v3 rules the fresh network runs ============================
// The properties at the network level on the v3 rules: the fresh network's own committed profile (deploy/mainnet/v3-profile.json, FRESH-NETWORK-CP.md) where
// the test models the network, and the v3 devnet (`DevnetConfig::new`) where it models the devnet pocold boots. A v3
// chain is continuous (H_END = 0) and runs the first-import index (`Spec::v3_active(MAX)`), so its model counts what
// Node::open observes before its first check: the journal records (`Limits::continuous_history`) and the files
// (`Limits::disk`), with the eviction depth Node::open derives (`booted`).

const MAINNET_PROFILE:&str=include_str!("../../../deploy/mainnet/v3-profile.json");
/// The fresh network's genesis: deploy/mainnet/v3-profile.json through the genesis builder with the test flag
/// (`testnet::prepare_with`: the public test founders, the local test chain id and a synthetic H_cal; no A13 term reads
/// any of them), loaded by the launcher at the profile's declared local timing (1, 1, 1), Δ_rel 7, Γ 5.
struct V3Net{dir:std::path::PathBuf,spec:pocold::chain::Spec}
impl Drop for V3Net{fn drop(&mut self){let _=std::fs::remove_dir_all(&self.dir);}}
fn v3_network(tag:&str)->V3Net{
    let dir=std::env::temp_dir().join(format!("pocold-nb-v3-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    pocold::testnet::prepare_with(MAINNET_PROFILE.as_bytes(),&dir,Some(&pocold::testnet::TestPlaceholders{genesis_time:1_800_000_000,hash_rate:16})).expect("the mainnet profile builds");
    let local=pocold::testnet::local_for_network(&dir).unwrap();
    let spec=pocold::load_spec_for(&dir,&local,None).expect("the mainnet profile is admitted");
    let c=spec.cp();
    assert_eq!((c.schema_version(),c.nonceMode,c.H_END,c.M_max,c.ACCT_CAP,c.SLOT_CAP,c.CODE_CAP,c.T_blk),(2,1,0,10_000,1_000_000,4_000_000,1<<30,10),"the FRESH-NETWORK-CP values");
    assert!(spec.v3_active(u64::MAX),"the V3 programs from genesis");
    V3Net{dir,spec}
}
/// The v3 devnet pocold boots (DevnetConfig::new: CP schema v2, the V3 programs, nonceMode 1, continuous).
fn v3_devnet()->pocol_lp4_execution::profile::ConsensusParams{
    let cp=devnet::build(&DevnetConfig::new(777910,1_700_000_000,&["a","b","c"])).unwrap().profile.cp().clone();
    assert_eq!((cp.schema_version(),cp.nonceMode,cp.H_END),(2,1,0));cp
}
/// The limits as Node::open sets them before its first A13 check (node.rs open; every admitted profile keeps the
/// first-import index): the eviction depth from R_keep (an explicit Some(0) keeps eviction off), the first-import
/// index, the continuous projection from `records` observed journal records, and the observed files `disk`.
fn booted(cp:&pocol_lp4_execution::profile::ConsensusParams,k_hist:u32,mut l:pocold::node::Limits,records:u64,disk:pocold::resources::DiskBytes)->pocold::node::Limits{
    l.evict_depth=Some(pocold::resources::evict_depth_of(cp,k_hist,&l,l.r_keep).unwrap().unwrap_or(0));
    l.first_imports=true;l.continuous_history=Some(records);l.disk=Some(disk);l
}
/// `l` as a new node (no records, empty files) opens it on the v3 `cp`: the booted limits.
fn at_open(cp:&pocol_lp4_execution::profile::ConsensusParams,l:pocold::node::Limits)->pocold::node::Limits{
    assert!(cp.schema_version()>=2,"a v3 CP");booted(cp,32,l,0,Default::default())
}

/// The fresh network's store-mode model as a new founder
/// node opens it (eviction on, the first-import index, empty files), itemized, with state.redb's largest file by part.
#[test] fn v3_a13_store_model_at_the_mainnet_profile(){
    let n=v3_network("store");let (cp,local)=(n.spec.cp(),n.spec.local());
    let lim=booted(cp,local.k_hist,pocold::node::Limits{state_store:true,..Default::default()},0,Default::default());
    let b=pocold::resources::derive(cp,local.k_hist,local.side_max,&lim,2).expect("no overflow");
    print("A13-V3-MAINNET-STORE",&b);
    let need=pocold::resources::budget_required(b.total).unwrap();
    let p=pocold::resources::state_file_parts(cp,local.k_hist,local.side_max,&lim).unwrap();let f:u128=p.iter().sum();
    println!("A13-V3-MAINNET-STATE-FILE {}",serde_json::json!({"fileTiB":f as f64/(1u128<<40) as f64,"partsTiB":{"versionsNodes":p[0] as f64/(1u128<<40) as f64,"preimages":p[1] as f64/(1u128<<40) as f64,"code":p[2] as f64/(1u128<<40) as f64,"checkpoints":p[3] as f64/(1u128<<40) as f64}}));
    let r=pocold::resources::check(cp,local.k_hist,local.side_max,&lim,2);
    assert_eq!(r.is_ok(),need<=u128::from(lim.mem_budget));
    assert!(r.is_ok(),"a new founder node opens the fresh network in store mode within the 6 GiB budget: {:?}",r.err());
    for k in ["history/first-import index cache and allocator","history/first-import journal lookup scratch","history/resident window: logs"]{
        assert!(b.items.iter().any(|x|x.0==k),"the v3 term {k}");
    }
    if cfg!(all(target_os="linux",target_arch="x86_64")){assert_eq!(b.total,V3_MAINNET_STORE_TOTAL,"the fresh network's store-mode model moved (record it in the PR)");}
}
/// Whole-state mode at the fresh network's caps, which the
/// launch does not run (FRESH-NETWORK-CP.md §1: the caps hold subject to A13 store mode). Printed, and its verdict pinned.
#[test] fn v3_a13_whole_state_model_at_the_mainnet_profile(){
    let n=v3_network("mem");let (cp,local)=(n.spec.cp(),n.spec.local());
    let lim=booted(cp,local.k_hist,pocold::node::Limits::default(),0,Default::default());
    let b=pocold::resources::derive(cp,local.k_hist,local.side_max,&lim,2).expect("no overflow");print("A13-V3-MAINNET-MEM",&b);
    let need=pocold::resources::budget_required(b.total).unwrap();
    assert!(need>u128::from(pocold::node::MEM_BUDGET),"whole-state mode does not fit 6 GiB at the fresh network's caps (if this fails, the model changed: record it)");
}
/// The pinned v3 totals (Linux x86-64; the Entry's size is the platform's).
const V3_MAINNET_STORE_TOTAL:u128=5_512_153_289;

/// Why the fresh network's profile boots, gate by gate: the profile at its declared local timing
/// (1, 1, 1; Δ_rel 7, Γ 5, K_hist 32, retentionStress as the launcher sets it), each R1-R26 row re-derived on CP schema v2:
/// R8 with the v3 registry window (U >= W_act + 2, C_react >= U; gate.rs v3_registry_window_violated), R10 with the
/// late-share item sizing, R13 for a continuous profile (Simnet profile on a Simnet node), R21 with the simnet
/// retentionStress exception. Each row must agree with gate::check's own verdict; the launch admission (Simnet,
/// ExperimentalDevnet) passes, and the Specification scope's refusal is printed. Then pocold's A13 check in both modes.
#[test] fn v3_mainnet_profile_boot_gates_itemized(){
    use pocol_lp4_execution::{admission::{self,Request,Scope},gate::{self,FactClass,calibration_target},param_gate::registry_metrics,
        profile::{NodeKind,ProfileKind},timing::metrics};
    let n=v3_network("gates");let c=n.spec.cp().clone();let local=n.spec.local().clone();
    assert_eq!((local.delta,local.phi,local.t_exec,local.delta_rel,local.gamma,local.k_hist),(1,1,1,7,5,32),"the profile's declared local timing");
    let bytes=pocold::types::unhex(std::fs::read_to_string(n.dir.join("genesis.spec.hex")).unwrap().trim()).unwrap();
    let alloc=devnet::alloc_from_json(&serde_json::from_str(&std::fs::read_to_string(n.dir.join("alloc.json")).unwrap()).unwrap()).unwrap();
    let forks=[(1u16,0u64)];let hash_rate=16u64;
    let timing=admission::measure(&bytes,&alloc,1).expect("the calibration blocks apply");
    let req=|kind,node,scope|Request{spec_bytes:&bytes,alloc:&alloc,forks:&forks,kind,node,local:&local,
        calibration:Some((hash_rate,FactClass::SyntheticFixture)),timing:Some(&timing),scope};
    let gate=admission::admit(&req(ProfileKind::Simnet,NodeKind::Simnet,Scope::ExperimentalDevnet)).err().map(|e|e.rules).unwrap_or_default();
    let (t,big_phi,dl)=(u128::from(local.t_exec),u128::from(local.capital_phi()),u128::from(local.delta));
    let m=metrics(local.delta.into(),big_phi as u64,local.t_exec.into(),local.delta_rel.into(),c.dFbWait.into());
    let reg=registry_metrics(c.U,c.E_win,c.D,c.M_max,c.R_max,c.REG_RECORDS_MAX,c.RET,c.P2).ok();
    let p99=timing.phase_ms.iter().copied().max().unwrap();
    let hist=admission::historical_bytes(&c,&local);
    let continuous=c.H_END==0&&c.H_CLOSE==0&&c.T_END==0&&c.T_CLOSE==0;assert!(continuous);
    let rows:Vec<(&str,bool,String,String,String)>=vec![
        ("R2",u128::from(local.delta_rel)<m.sigma+dl+2*t,format!("Δ_rel={} σ={} Δ={} t_exec={}",local.delta_rel,m.sigma,dl,t),format!("Δ_rel ≥ σ+Δ+2t_exec = {}",m.sigma+dl+2*t),local.delta_rel.to_string()),
        ("R3",u128::from(c.dFbWait)<big_phi+dl+2*t,format!("dFbWait={} Φ={big_phi} Δ={dl} t_exec={t}",c.dFbWait),format!("dFbWait ≥ Φ+Δ+2t_exec = {}",big_phi+dl+2*t),c.dFbWait.to_string()),
        ("R4",big_phi>=u128::from(c.dFbWait)||u128::from(local.delta_rel)<t,format!("Φ={big_phi} dFbWait={} Δ_rel={} t_exec={t}",c.dFbWait,local.delta_rel),"Φ < dFbWait and Δ_rel ≥ t_exec".into(),format!("{big_phi} < {}, {} ≥ {t}",c.dFbWait,local.delta_rel)),
        ("R5",3*u128::from(c.dFbWait)>u128::from(c.D_att),format!("dFbWait={} D_att={}",c.dFbWait,c.D_att),format!("3·dFbWait ≤ D_att = {}",c.D_att),(3*u128::from(c.dFbWait)).to_string()),
        ("R7",u128::from(c.D_att)+u128::from(local.gamma)<2*(m.x_w+u128::from(c.c)*u128::from(c.T_blk))+big_phi,format!("D_att={} Γ={} X^W={} c={} T_blk={} Φ={big_phi}",c.D_att,local.gamma,m.x_w,c.c,c.T_blk),format!("D_att+Γ ≥ 2(X^W+c·T_blk)+Φ = {}",2*(m.x_w+u128::from(c.c)*u128::from(c.T_blk))+big_phi),(u128::from(c.D_att)+u128::from(local.gamma)).to_string()),
        // The v3 R8 (V3-09): the spec-1 terms and the reactivation window and clock.
        ("R8",c.D<2||c.U<c.E_win+u32::from(c.D)||c.E_win>admission::R_KEEP||u128::from(c.U)<u128::from(c.W_act)+2||c.C_react<c.U,
            format!("D={} U={} E_win={} R_keep={} W_act={} C_react={}",c.D,c.U,c.E_win,admission::R_KEEP,c.W_act,c.C_react),"D ≥ 2, U ≥ E_win+D, E_win ≤ R_keep, U ≥ W_act+2, C_react ≥ U".into(),
            format!("{}, {} vs {}, {}, {} vs {}, {} vs {}",c.D,c.U,c.E_win+u32::from(c.D),c.E_win,c.U,c.W_act+2,c.C_react,c.U)),
        ("R9",u128::from(c.S_max)<4*u128::from(c.kappa)*u128::from(c.m)||c.B_code_max<24577,format!("S_max={} κ={} m={} B_code_max={}",c.S_max,c.kappa,c.m,c.B_code_max),format!("S_max ≥ 4κm = {}, B_code_max ≥ 24577",4*u128::from(c.kappa)*u128::from(c.m)),format!("{}, {}",c.S_max,c.B_code_max)),
        ("R10",u128::from(c.BODY_MAX)<u128::from(c.TX_MAX)+16384||gate::v3_late_share_sizing_violated(&c),format!("BODY_MAX={} TX_MAX={} EVID_BYTES_MAX={}",c.BODY_MAX,c.TX_MAX,c.EVID_BYTES_MAX),
            format!("BODY_MAX ≥ TX_MAX+16384 = {}, EVID_BYTES_MAX ≥ a full late-share item = {}",u128::from(c.TX_MAX)+16384,gate::late_item_bytes(u64::from(c.S_max).min(u64::from(c.kappa)*u64::from(c.m)-1))),c.BODY_MAX.to_string()),
        // A continuous profile (H_END = H_CLOSE = T_END = T_CLOSE = 0) is admitted only as a Simnet profile on a Simnet node.
        ("R13",false,format!("H_END={} H_CLOSE={} T_END={} T_CLOSE={}",c.H_END,c.H_CLOSE,c.T_END,c.T_CLOSE),"continuous: a Simnet profile on a Simnet node".into(),"Simnet, Simnet".into()),
        ("R14",u128::from(p99)>t*1000,format!("validate_p99 = max phase ms measured here {:?}",timing.phase_ms),format!("≤ t_exec·1000 = {}",t*1000),p99.to_string()),
        ("R16",reg.as_ref().map_or(true,|r|u128::from(c.REG_RECORDS_MAX)<r.records_required),format!("REG_RECORDS_MAX={} U={} E_win={} D={} M_max={} R_max={} RET={} P2={}",c.REG_RECORDS_MAX,c.U,c.E_win,c.D,c.M_max,c.R_max,c.RET,c.P2),format!("≥ records_required = {:?}",reg.as_ref().map(|r|r.records_required)),c.REG_RECORDS_MAX.to_string()),
        ("R17",u128::from(c.SWEEP_MAX)<u128::from(c.R_max)+u128::from(c.E_max),format!("SWEEP_MAX={} R_max={} E_max={}",c.SWEEP_MAX,c.R_max,c.E_max),format!("≥ R_max+E_max = {}",u128::from(c.R_max)+u128::from(c.E_max)),c.SWEEP_MAX.to_string()),
        ("R19",timing.phase_ms.iter().any(|ms|u128::from(*ms)>t*1000),format!("phase ms measured here {:?}",timing.phase_ms),format!("each ≤ {}",t*1000),format!("{:?}",timing.phase_ms)),
        ("R20",reg.as_ref().map_or(true,|r|u128::from(c.Z_close)<r.close_blocks_required),format!("Z_close={}",c.Z_close),format!("≥ close_blocks_required = {:?}",reg.as_ref().map(|r|r.close_blocks_required)),c.Z_close.to_string()),
        ("R21",hist>admission::HIST_TRIE_BUDGET&&!local.retention_stress,format!("(K_hist+side_max)·snapshot(ACCT_CAP+5, SLOT_CAP+SYS_SLOT_MAX), K_hist={} side_max={} retentionStress={}",local.k_hist,local.side_max,local.retention_stress),
            format!("≤ HIST_TRIE_BUDGET = {}, or the simnet retentionStress exception",admission::HIST_TRIE_BUDGET),hist.to_string()),
        ("R25",calibration_target(hash_rate,c.T_blk)!=Some(c.target_g),format!("H_cal={hash_rate} (synthetic) T_blk={}",c.T_blk),"target_g = 2^256/(H_cal·T_blk)".into(),"equal (target_g derived from the same H_cal)".into()),
    ];
    for (rule,fails,inputs,required,actual) in &rows{
        println!("V3-MAINNET-GATE {}",serde_json::json!({"rule":rule,"fails":fails,"inputs":inputs,"required":required,"actual":actual}));
        assert_eq!(*fails,gate.iter().any(|g|g==rule),"{rule}: the re-derivation disagrees with gate::check ({gate:?})");
    }
    assert!(gate.is_empty(),"the launch admission (Simnet, Simnet, ExperimentalDevnet) passes R1-R26: {gate:?}");
    assert!(hist>admission::HIST_TRIE_BUDGET&&local.retention_stress,"R21 holds only through the retentionStress exception (FRESH-NETWORK-CP.md §4)");
    // R8's v3 terms are the binding ones at the fresh network's values: U = 380 against W_act + 2 = 34 and C_react = 380 = U.
    assert!(!gate::v3_registry_window_violated(&c)&&c.C_react==c.U);
    let mut tight=c.clone();tight.C_react=c.U-1;assert!(gate::v3_registry_window_violated(&tight),"one block under U fails the v3 R8");
    println!("V3-MAINNET-GATE-VERDICT {}",serde_json::json!({"gateViolations":gate,"t_execMs":t*1000,"phaseMs":timing.phase_ms,"sigma":m.sigma.to_string(),"x_h":m.x_h.to_string(),"x_w":m.x_w.to_string()}));
    let spec=admission::admit(&req(ProfileKind::Synthetic,NodeKind::Simnet,Scope::Specification)).err().expect("refused in Specification scope");
    println!("V3-MAINNET-SPEC-SCOPE {}",serde_json::json!({"rules":spec.rules,"missing":spec.missing,"notes":spec.notes}));
    for store in [false,true]{
        let lim=booted(&c,local.k_hist,pocold::node::Limits{state_store:store,..Default::default()},0,Default::default());
        let b=pocold::resources::derive(&c,local.k_hist,local.side_max,&lim,2).unwrap();let need=pocold::resources::budget_required(b.total).unwrap();
        let r=pocold::resources::check(&c,local.k_hist,local.side_max,&lim,2);
        println!("V3-MAINNET-A13 {}",serde_json::json!({"stateStore":store,"modelBytes":b.total.to_string(),"budgetRequiredBytes":need.to_string(),"budgetBytes":lim.mem_budget,"passes":r.is_ok()}));
        assert_eq!(r.is_ok(),need<=u128::from(lim.mem_budget));assert_eq!(r.is_ok(),store,"store mode only");
    }
}

/// The live chain's A13 stop (A13 projects a continuous chain from its journal), at the fresh network's profile
/// and a seed's settings (store mode, checkpoints every 128 records, discovery on; for each (peers_max, dials) below):
/// - eviction off (`evict_depth` Some(0)): the largest admitted journal record count R (binary search), or the fact that
///   the profile does not open at all with eviction off;
/// - eviction on (the default depth): the largest admitted history.redb size at 0 records and at one year of records
///   (8,640 blocks a day for 365.25 days), and the largest admitted record count with empty files. The v3 bound can
///   grow with the records: the first-import index (one row a record, `FIRST_IMPORT_ROW`) is
///   counted with its redb allocator (resources.rs `first_imports`), so the stop is in both the files and the records
///   (`V3_LIVE_HALTS` records the figures). A setting at which a new node does not open records zeros for eviction on.
/// Pinned for the target the evidence is produced on, so a term that moves the stop is seen.
#[test] fn v3_a13_live_chain_halts_at_the_mainnet_profile(){
    use pocold::{net,node::Limits,resources::DiskBytes};
    let n=v3_network("halt");let (cp,k,side)=(n.spec.cp(),n.spec.local().k_hist,n.spec.local().side_max);
    let lim=|peers_max:usize,evict:Option<u64>,records:u64,history:u64|{
        let base=Limits{state_store:true,checkpoint_every:128,evict_depth:evict,net:net::Limits{peers_max,..Default::default()},..Default::default()};
        booted(cp,k,base,records,DiskBytes{history,..Default::default()})
    };
    let passes=|p:usize,d:usize,evict:Option<u64>,r:u64,x:u64|pocold::resources::check(cp,k,side,&lim(p,evict,r,x),d).is_ok();
    let search=|lo0:u64,cap:u64,ok:&dyn Fn(u64)->bool|->u64{
        if !ok(lo0){return 0}
        let mut hi=lo0.max(1);while ok(hi){hi=hi.saturating_mul(2);assert!(hi<cap,"no stop below {cap}");}
        let mut lo=hi/2;while hi-lo>1{let mid=lo+(hi-lo)/2;if ok(mid){lo=mid}else{hi=mid}}lo
    };
    const YEAR:u64=3_155_760;
    let mut got=Vec::new();
    for (p,d) in [(8usize,2usize),(8,7),(32,2),(32,7),(64,2),(64,7)]{
        let off_opens=passes(p,d,Some(0),0,0);
        let off=if off_opens{search(1,1<<40,&|r|passes(p,d,Some(0),r,0))}else{0};
        // A new node (no records, empty files) at these settings: at the largest (64 established, 7 dials) it does not open.
        if !passes(p,d,None,0,0){
            let b=pocold::resources::derive(cp,k,side,&lim(p,None,0,0),d).unwrap();
            println!("V3-A13-LIVE-HALT {}",serde_json::json!({"peersMax":p,"dials":d,"opens":false,"evictionOffOpens":off_opens,"evictionOffRecords":off,"boundAt0":b.total.to_string(),
                "budgetRequired":pocold::resources::budget_required(b.total).unwrap().to_string()}));
            got.push((off,0,0,0));continue;
        }
        let disk0=search(1<<30,1<<60,&|x|passes(p,d,None,0,x));
        let disk_year=search(1<<30,1<<60,&|x|passes(p,d,None,YEAR,x));
        let records=search(1,1<<50,&|r|passes(p,d,None,r,0));
        let total=|r:u64|pocold::resources::derive(cp,k,side,&lim(p,None,r,0),d).unwrap().total;
        assert!(total(YEAR)>=total(0)&&total(10*YEAR)>=total(YEAR),"the bound never falls as the records grow at ({p}, {d})");
        assert!(disk_year<=disk0&&records>0);
        println!("V3-A13-LIVE-HALT {}",serde_json::json!({"peersMax":p,"dials":d,"evictionOffOpens":off_opens,"evictionOffRecords":off,
            "historyBytesAt0":disk0,"historyTiBAt0":disk0 as f64/(1u64<<40) as f64,"historyBytesAtYear":disk_year,"historyTiBAtYear":disk_year as f64/(1u64<<40) as f64,
            "opens":true,"recordsAtEmptyFiles":records,"yearsAtEmptyFiles":records as f64/YEAR as f64,"boundAt0":total(0).to_string(),"boundAtYear":total(YEAR).to_string(),"boundAtTenYears":total(10*YEAR).to_string()}));
        got.push((off,disk0,disk_year,records));
    }
    // The default (32 established, 2 dials) opens and runs for centuries of records before the index's allocator stops it.
    assert!(got[2].1>=1<<38&&got[2].3>=100*YEAR,"the default seed settings: {:?}",got[2]);
    if cfg!(all(target_os="linux",target_arch="x86_64")){
        assert_eq!(got,V3_LIVE_HALTS.to_vec(),"the fresh network's stop moved (record it in the PR)");
    }
}
/// (eviction-off records, history.redb bytes at 0 records, at one year of records, records at empty files) per
/// (peers_max, dials) = (8, 2), (8, 7), (32, 2), (32, 7), (64, 2), (64, 7).
/// Phase 5c, first measured at main daabfbd: the default (32, 2) opens at 5,512,153,289 B of model and stops at
/// 511,964,312,319 B (0.47 TiB) of history.redb, or at 1,606,693,734 records (509 years) with empty files, or at 71,735
/// records (8.3 days) with eviction off; one year of records moves no stop (the index stays within one region's allocator).
/// A seed at (64, 2) stops at 63,092,519,679 B (58.8 GiB) of history.redb, and at (64, 7) a new node does not open with
/// eviction on (model 5,685,499,169 B, budget required 6,497,713,336 B > 6 GiB): seeds on the fresh network keep the
/// default 32 established places.
const V3_LIVE_HALTS:[(u64,u64,u64,u64);6]=[(99_067,848_618_156_799,848_618_156_799,2_658_736_998),(77_954,587_465_224_959,587_465_224_959,1_842_634_086),
    (71_735,511_964_312_319,511_964_312_319,1_606_693_734),(50_622,250_811_380_479,250_811_380_479,790_590_822),(35_415,63_092_519_679,63_092_519_679,203_969_382),(14_242,0,0,0)];
