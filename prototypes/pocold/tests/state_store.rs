//! C-1 E4.2: state.redb, the node store of the state trie (pocold::state_store).
use pocold::state_store::StateStore;
use pocol_lp4_execution::node_trie::{Batch,Id,NodeStore,NodeTrie,TrieState};
use pocol_lp4_execution::state_trie::{StateTrie,Touched};
use pocol_lp4_execution::view::View;
use revm::{db::AccountState,primitives::{AccountInfo,Address,Bytecode,Bytes,B256,U256},InMemoryDB};

struct Rng(u64);
impl Rng{fn next(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}fn below(&mut self,n:u64)->u64{self.next()%n}}
fn codes()->Vec<Bytecode>{vec![Bytecode::default(),Bytecode::new_raw(Bytes::from_static(&[0x60,1,0x60,0,0x55])),Bytecode::new_raw(Bytes::from_static(&[0x00]))]}
fn addr(r:&mut Rng,pool:u64)->Address{Address::from_word(B256::from(U256::from(r.below(pool))))}
fn info(r:&mut Rng,c:&[Bytecode])->AccountInfo{let code=c[r.below(c.len() as u64) as usize].clone();AccountInfo{balance:U256::from(r.below(1<<40)),nonce:r.below(5),code_hash:code.hash_slow(),code:Some(code)}}
fn state(r:&mut Rng,n:u64,c:&[Bytecode])->InMemoryDB{
    let mut db=InMemoryDB::default();
    for _ in 0..n{let a=addr(r,4*n);db.insert_account_info(a,info(r,c));for _ in 0..r.below(6){db.insert_account_storage(a,U256::from(r.below(50)),U256::from(r.below(4))).unwrap();}}
    db
}
fn mutate(r:&mut Rng,db:&InMemoryDB,c:&[Bytecode],pool:u64)->InMemoryDB{
    let mut d=db.clone();let addrs:Vec<Address>=d.accounts.keys().copied().collect();
    for _ in 0..1+r.below(20){
        let a=if addrs.is_empty()||r.below(4)==0{addr(r,pool)}else{addrs[r.below(addrs.len() as u64) as usize]};
        match r.below(7){
            0=>{d.accounts.remove(&a);}
            1=>{if let Some(x)=d.accounts.get_mut(&a){x.account_state=AccountState::NotExisting}}
            2=>{if let Some(x)=d.accounts.get_mut(&a){x.storage.clear();x.account_state=AccountState::StorageCleared}}
            3=>d.insert_account_info(a,info(r,c)),
            _=>{if !d.accounts.contains_key(&a){d.insert_account_info(a,info(r,c))}for _ in 0..1+r.below(4){d.insert_account_storage(a,U256::from(r.below(60)),U256::from(r.below(3))).unwrap();}}
        }
    }
    d
}
/// Every account, code and slot of `db` reads the same through the stored version.
fn same(db:&InMemoryDB,t:&TrieState<'_>,ctx:&str){
    for (a,x) in &db.accounts{
        let exists=!matches!(x.account_state,AccountState::NotExisting);let g=t.account(a);assert_eq!(g.is_some(),exists,"{ctx}: {a}");
        let Some(g)=g else{continue};
        assert_eq!((g.info.nonce,g.info.balance,g.info.code_hash),(x.info.nonce,x.info.balance,x.info.code_hash),"{ctx}: {a}");
        assert_eq!(g.info.code.map(|c|c.original_bytes()),x.info.code.clone().map(|c|c.original_bytes()),"{ctx}: code of {a}");
        let mut s1:Vec<(U256,U256)>=x.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(*k,*v)).collect();let mut s2=Vec::new();
        t.each_slot(a,&mut |k,v|s2.push((*k,*v)));s1.sort();s2.sort();assert_eq!(s1,s2,"{ctx}: slots of {a}");
        for (k,v) in &x.storage{assert_eq!(t.slot(a,k),*v,"{ctx}: {a} slot {k}");}
    }
}
fn dir(tag:&str)->std::path::PathBuf{let d=std::env::temp_dir().join(format!("pocold-e4-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);std::fs::create_dir_all(&d).unwrap();d}

#[test] fn stored_versions_have_their_roots_and_states_and_survive_reopen(){
    let c=codes();let mut r=Rng(0xe42);let d=dir("versions");
    let mut db=state(&mut r,200,&c);let pool=800;
    let mut pinned:std::collections::VecDeque<(Option<Id>,InMemoryDB)>=Default::default();
    {
        let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt);
        let mut b=Batch::default();let mut t=NodeTrie::build(&db,&s,&mut b);
        assert_eq!(t.root(&s,&b),StateTrie::build(&db).root());
        s.commit(&b,t.world,true).unwrap();pinned.push_back((t.world,db.clone()));
        for step in 0..40{
            let next=mutate(&mut r,&db,&c,pool);let touched=Touched::between(&db,&next);
            let mut b=Batch::default();let u=t.update(&s,&mut b,&next,&touched);
            assert_eq!(u.root(&s,&b),StateTrie::build(&next).root(),"step {step}");
            assert_eq!(u.root(&s,&b),pocol_lp4_execution::apply::state_root(&next).unwrap(),"step {step}");
            s.commit(&b,u.world,step%3==0).unwrap();pinned.push_back((u.world,next.clone()));
            if pinned.len()>5{if let (Some(old),_)=pinned.pop_front().unwrap(){s.unpin(&old,false).unwrap()}}
            s.recount().unwrap_or_else(|e|panic!("step {step}: {e}"));
            t=u;db=next;
        }
        for (w,st) in &pinned{same(st,&TrieState{store:&s,trie:NodeTrie{world:*w},block_hashes:&st.block_hashes},"live");}
        assert!(s.writes.load(std::sync::atomic::Ordering::Relaxed)>500,"positive control: nodes were written");
        // A durable commit makes the earlier non-durable ones durable.
        let b=Batch::default();s.commit(&b,None,true).unwrap();
    }
    let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt,"a clean store reopens as it is");
    s.recount().unwrap();assert_eq!(s.pins().unwrap().len(),pinned.iter().filter(|x|x.0.is_some()).count());
    for (w,st) in &pinned{same(st,&TrieState{store:&s,trie:NodeTrie{world:*w},block_hashes:&st.block_hashes},"reopened");}
    for (w,_) in &pinned{if let Some(w)=w{s.unpin(w,true).unwrap()}}
    assert_eq!(s.node_count().unwrap(),0,"releasing every pin empties the node table");
    let _=std::fs::remove_dir_all(&d);
}

#[test] fn a_damaged_node_is_never_served_and_the_store_is_rebuilt(){
    let c=codes();let mut r=Rng(5);let d=dir("damage");let db=state(&mut r,60,&c);
    let world={let s=StateStore::open(&d).unwrap();let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();t.world.unwrap()};
    // Overwrite the root node's bytes with another node's (a flipped record that still decodes).
    {
        let db2=redb::Database::open(d.join("state.redb")).unwrap();const NODES:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("node");
        let w=db2.begin_write().unwrap();{let mut t=w.open_table(NODES).unwrap();
            let other:Vec<u8>={use redb::ReadableTable;let it=t.iter().unwrap();let mut v=None;for kv in it{let (k,x)=kv.unwrap();if *k.value()!=world{v=Some(x.value().to_vec());break}}v.unwrap()};
            t.insert(&world,other.as_slice()).unwrap();}
        w.commit().unwrap();
    }
    let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt,"redb's checksums hold: the damage is in a record");
    let e=s.try_node(&world).unwrap_err();assert!(e.0.contains("hash to its id"),"{}",e.0);
    assert!(s.is_damaged()&&d.join("state.redb.damaged").exists());
    assert!(s.try_node(&world).is_err(),"a damaged store serves nothing");
    assert!(s.pins().is_err()&&s.node_count().is_err(),"not even a node it holds intact");
    drop(s);let s=StateStore::open(&d).unwrap();assert!(s.rebuilt&&s.node_count().unwrap()==0,"the next open rebuilds it empty");
    drop(s);
    // A torn file is set aside and the store recreated.
    let f=d.join("state.redb");let len=std::fs::metadata(&f).unwrap().len();
    {let s=StateStore::open(&d).unwrap();let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();}
    let len2=std::fs::metadata(&f).unwrap().len();assert!(len2>=len);
    std::fs::OpenOptions::new().write(true).open(&f).unwrap().set_len(len2/2).unwrap();
    let s=StateStore::open(&d).unwrap();assert!(s.rebuilt,"a torn store is rebuilt");assert!(d.join("state.redb.aside").exists());
    let _=std::fs::remove_dir_all(&d);
}

#[test] fn a_small_node_cache_reads_the_same(){
    // The cache bound only changes how often the table is read: 4 KiB forces misses on nearly every node.
    let c=codes();let mut r=Rng(77);let d=dir("cache");let db=state(&mut r,300,&c);
    let s=StateStore::open_with(&d,4096).unwrap();let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();
    let reads0=s.reads.load(std::sync::atomic::Ordering::Relaxed);
    same(&db,&TrieState{store:&s,trie:t,block_hashes:&db.block_hashes},"small cache");
    assert!(s.reads.load(std::sync::atomic::Ordering::Relaxed)>reads0+1000,"positive control: the table was read on misses");
    assert_eq!(s.code(&c[1].hash_slow()).map(|x|x.original_bytes()),Some(c[1].original_bytes()));
    assert!(s.cache_bytes()<=4096,"the cache holds at most its bound: {}",s.cache_bytes());
    let _=std::fs::remove_dir_all(&d);
}

#[test] fn a_released_version_is_not_served_from_the_cache(){
    let c=codes();let mut r=Rng(19);let d=dir("release");let db=state(&mut r,80,&c);
    let s=StateStore::open(&d).unwrap();let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();
    let root=t.world.unwrap();
    same(&db,&TrieState{store:&s,trie:t,block_hashes:&db.block_hashes},"before release");assert!(s.cache_bytes()>0,"positive control: nodes are cached");
    s.unpin(&root,true).unwrap();
    assert!(s.try_node(&root).unwrap().is_none(),"a released node is gone from the table and the cache");
    assert_eq!(s.node_count().unwrap(),0);assert_eq!(s.cache_bytes(),0,"every released node left the cache");
    let _=std::fs::remove_dir_all(&d);
}

#[test] fn code_that_does_not_hash_to_its_key_is_never_served(){
    let c=codes();let mut r=Rng(23);let d=dir("code");let db=state(&mut r,40,&c);let h=c[1].hash_slow();
    {let s=StateStore::open(&d).unwrap();let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();assert!(s.code(&h).is_some());}
    {
        let db2=redb::Database::open(d.join("state.redb")).unwrap();const CODE:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("code");
        let w=db2.begin_write().unwrap();{let mut t=w.open_table(CODE).unwrap();t.insert(&h.0,&[0x60u8,9,0x00][..]).unwrap();}w.commit().unwrap();
    }
    let s=StateStore::open(&d).unwrap();
    let got=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||s.code(&h)));
    assert!(got.is_err(),"the code read refuses bytes that do not hash to the key");assert!(s.is_damaged(),"and marks the store damaged");
    drop(s);assert!(StateStore::open(&d).unwrap().rebuilt);
    let _=std::fs::remove_dir_all(&d);
}

/// C-1 E5: a store of another format (a store written before leaves kept their keys' preimages) is set
/// aside and rebuilt, never read; one of the current format reopens as it is.
#[test] fn a_store_of_another_format_is_rebuilt(){
    let d=std::env::temp_dir().join(format!("pocold-ss-format-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);std::fs::create_dir_all(&d).unwrap();
    {let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt);}
    {let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt,"the current format reopens as it is");}
    {let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
     {let meta:redb::TableDefinition<&str,u64>=redb::TableDefinition::new("meta");let mut m=w.open_table(meta).unwrap();m.insert("format",1).unwrap();}w.commit().unwrap();}
    let s=StateStore::open(&d).unwrap();assert!(s.rebuilt,"format 1 is set aside and rebuilt");assert!(d.join(format!("{}.aside",pocold::state_store::FILE)).exists());
    drop(s);let _=std::fs::remove_dir_all(&d);
}

/// C-1 E4.4: a node a stored node names but the table does not hold (a record lost from the file) is not
/// served: the read answers None, which every reader of a version treats as fatal, and the store is marked
/// damaged, so nothing more is served from it and the next open rebuilds it.
#[test] fn a_missing_node_marks_the_store_damaged(){
    let c=codes();let mut r=Rng(31);let d=dir("missing");let db=state(&mut r,60,&c);
    let (world,child)={let s=StateStore::open(&d).unwrap();let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();
        let w=t.world.unwrap();(w,s.try_node(&w).unwrap().unwrap().children()[0])};
    {
        let db2=redb::Database::open(d.join("state.redb")).unwrap();const NODES:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("node");
        let w=db2.begin_write().unwrap();{let mut t=w.open_table(NODES).unwrap();assert!(t.remove(&child).unwrap().is_some());}w.commit().unwrap();
    }
    let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt,"redb's checksums hold: a record is missing, not a page");
    assert!(s.try_node(&world).unwrap().is_some()&&!s.is_damaged(),"positive control: the root itself is intact");
    assert!(NodeStore::node(&s,&child).is_none(),"the missing node is not served");
    assert!(s.is_damaged()&&d.join("state.redb.damaged").exists(),"and the store is marked damaged");
    assert!(s.try_node(&world).is_err(),"a damaged store serves nothing");
    drop(s);assert!(StateStore::open(&d).unwrap().rebuilt,"the next open rebuilds it");
    let _=std::fs::remove_dir_all(&d);
}

/// A13 counts a stored node as `node_trie::ROW` plus its encoding: checked against the rows state.redb
/// really keeps (its own `stored_bytes`), so the constant is not checked against a copy of itself.
#[test] fn a13_row_is_what_state_redb_keeps_per_node(){
    let c=codes();let mut r=Rng(0x13);let d=dir("row");let db=state(&mut r,150,&c);
    let s=StateStore::open(&d).unwrap();let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);
    let root=t.world.unwrap();s.commit(&b,Some(root),true).unwrap();
    let mut seen=std::collections::HashSet::new();let mut stack=vec![root];let mut nodes=0u128;
    while let Some(id)=stack.pop(){if !seen.insert(id){continue}let n=&b.nodes[&id];nodes+=pocol_lp4_execution::node_trie::ROW+n.encode().len() as u128;stack.extend(n.children());}
    let code:u128=b.code.values().map(|x|32+x.original_bytes().len() as u128).sum();
    assert!(seen.len()>300,"positive control: a trie of {} nodes",seen.len());
    assert_eq!(s.node_count().unwrap(),seen.len() as u64);
    assert_eq!(u128::from(s.stored_bytes().unwrap()),nodes+code+36,"nodes, code and the one pin row");
    drop(s);let _=std::fs::remove_dir_all(&d);
}

/// D-6 (2026-10-27): a checkpoint's write drops the oldest checkpoint's pins and owes its nodes, and
/// `release_step` works them off a few at a time. After every step the counts recount (what is owed
/// included) and every pinned version reads as its state. When nothing is owed, the store holds exactly
/// what the pins reach, as a release inside the write left it. The next write finishes what is still owed
/// before it owes more; a store that owes work reopens owing it, with its counts whole.
#[test] fn a_dropped_checkpoint_is_released_in_steps_and_ends_as_a_release_in_the_write(){
    let c=codes();let mut r=Rng(0xd6);let d=dir("owed");let pool=800;
    let mut db=state(&mut r,200,&c);let mut versions:Vec<(Option<Id>,InMemoryDB)>=vec![];
    let reads=|s:&StateStore,v:&[(Option<Id>,InMemoryDB)],ctx:&str|{for (w,st) in v{same(st,&TrieState{store:s,trie:NodeTrie{world:*w},block_hashes:&st.block_hashes},ctx);}};
    let (steps,owed_at_reopen)={
        let s=StateStore::open(&d).unwrap();
        let mut b=Batch::default();let mut t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();versions.push((t.world,db.clone()));
        let mut grow=|s:&StateStore,versions:&mut Vec<(Option<Id>,InMemoryDB)>,k:usize|{for _ in 0..k{
            let next=mutate(&mut r,&db,&c,pool);let touched=Touched::between(&db,&next);let mut b=Batch::default();
            let u=t.update(s,&mut b,&next,&touched);s.commit(&b,u.world,true).unwrap();versions.push((u.world,next.clone()));t=u;db=next;}};
        // Three checkpoints of five versions each; with two kept, the third's write drops the first.
        let roots=|v:&[(Option<Id>,InMemoryDB)]|v.iter().filter_map(|x|x.0).collect::<Vec<Id>>();
        grow(&s,&mut versions,4);s.write_checkpoint(b"c0",&roots(&versions[0..5]),2).unwrap();assert!(!s.release_pending(),"nothing dropped yet");
        grow(&s,&mut versions,10);s.write_checkpoint(b"c1",&roots(&versions[10..15]),2).unwrap();
        grow(&s,&mut versions,10);
        // The entries' own pins of versions 0..25 are released as entries drop theirs; the checkpoints keep theirs.
        for (w,_) in &versions[0..20]{if let Some(w)=w{s.unpin(w,false).unwrap()}}
        s.recount().unwrap();let before=s.node_count().unwrap();
        // The nodes only the first checkpoint holds, read once so that the cache holds them.
        let live:Vec<(Option<Id>,InMemoryDB)>=versions[10..15].iter().chain(&versions[20..]).cloned().collect();
        let only_c0:Vec<Id>={let kept=reach(&s,&roots(&live));reach(&s,&roots(&versions[0..5])).into_iter().filter(|x|!kept.contains(x)).collect()};
        assert!(!only_c0.is_empty(),"positive control: the first checkpoint holds nodes of its own");
        s.write_checkpoint(b"c2",&roots(&versions[20..25]),2).unwrap();
        assert!(s.release_pending(),"the first checkpoint's nodes are owed");
        assert_eq!(s.node_count().unwrap(),before,"the write released no node");
        s.recount().unwrap_or_else(|e|panic!("right after the write: {e}"));
        let live=&live[..];let mut steps=0;
        while s.release_pending(){let n=s.release_step(3).unwrap();assert!(n<=3);steps+=1;
            s.recount().unwrap_or_else(|e|panic!("after step {steps}: {e}"));
            if steps%5==0{reads(&s,live,&format!("after step {steps}"));}}
        assert!(steps>3,"positive control: the release took {steps} steps of 3");
        assert!(s.node_count().unwrap()<before,"the first checkpoint's own nodes are gone");
        assert!(s.owed_refs().unwrap().is_empty());reads(&s,live,"released");
        assert!(only_c0.iter().all(|id|s.try_node(id).unwrap().is_none()),"a released node is neither stored nor served from the cache");
        // Owed work at the next write: it is finished first, then the second checkpoint is owed.
        grow(&s,&mut versions,10);for (w,_) in &versions[20..30]{if let Some(w)=w{s.unpin(w,false).unwrap()}}
        s.write_checkpoint(b"c3",&roots(&versions[30..35]),2).unwrap();assert!(s.release_pending());
        s.release_step(2).unwrap();let owed=s.owed_refs().unwrap().len();
        grow(&s,&mut versions,5);
        s.write_checkpoint(b"c4",&roots(&versions[35..40]),2).unwrap();s.recount().unwrap();
        // What c1's drop left owed was finished in c4's write: what is owed now is c2's alone.
        let c2_nodes=reach(&s,&roots(&versions[20..25]));
        assert!(s.owed_refs().unwrap().iter().all(|(id,_)|c2_nodes.contains(id)),"only the checkpoint c4's write dropped is owed");
        assert!(owed>0,"positive control: c1's drop was still owed at c4's write");
        // One more step, then the store is closed with work still owed.
        s.release_step(2).unwrap();let owed=s.owed_refs().unwrap();
        (steps,owed.len())
    };
    let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt);
    assert!(s.release_pending(),"a store that owes work reopens owing it");assert!(owed_at_reopen>0);
    s.recount().unwrap_or_else(|e|panic!("reopened: {e}"));
    let n=s.release_all().unwrap();assert!(n>0);assert!(!s.release_pending());s.recount().unwrap();
    reads(&s,&versions[30..],"reopened and released");
    println!("D6-RELEASE steps {steps} ownedAtReopen {owed_at_reopen} releasedAtReopen {n}");
    let _=std::fs::remove_dir_all(&d);
}
/// Every node the roots reach.
fn reach(s:&StateStore,roots:&[Id])->std::collections::HashSet<Id>{
    let mut seen=std::collections::HashSet::new();let mut st=roots.to_vec();
    while let Some(id)=st.pop(){if seen.insert(id){st.extend(s.try_node(&id).unwrap().unwrap().children())}}seen
}

/// A store that owes a dropped checkpoint's nodes: four versions, each the only pin of a checkpoint
/// (keeping two), so the last two writes each drop one.
fn owing_store(d:&std::path::Path,seed:u64)->StateStore{
    let c=codes();let mut r=Rng(seed);let s=StateStore::open(d).unwrap();let mut db=state(&mut r,150,&c);
    let mut b=Batch::default();let mut t=NodeTrie::build(&db,&s,&mut b);s.commit(&b,t.world,true).unwrap();let mut roots=vec![t.world.unwrap()];
    for _ in 0..3{let next=mutate(&mut r,&db,&c,600);let tc=Touched::between(&db,&next);let mut b=Batch::default();
        let u=t.update(&s,&mut b,&next,&tc);s.commit(&b,u.world,true).unwrap();roots.push(u.world.unwrap());t=u;db=next;}
    for (i,x) in roots.iter().enumerate(){s.write_checkpoint(format!("c{i}").as_bytes(),&[*x],2).unwrap();s.unpin(x,true).unwrap();}
    assert!(s.release_pending(),"positive control: the last write's dropped checkpoint is owed");s
}
/// D-6 (review 2026-10-27, finding 1): a release step that fails on a storage error loses only the step.
/// Before the fix, any failed step marked the store damaged, so the next open threw its checkpoints away and
/// replayed from genesis, while the same error in a block commit leaves the store whole. A child process
/// (the file size limit is the whole process's) builds a store owing a dropped checkpoint, then sets
/// RLIMIT_FSIZE below the file's size with SIGXFSZ ignored, so redb's writes fail with EFBIG. The step
/// returns the error, the store is not marked damaged and no more steps are tried. Reopened, the store is
/// not rebuilt, keeps both checkpoints and owes what it owed; its counts recount and the release finishes.
#[cfg(unix)]
#[test] fn a_release_step_that_fails_on_io_leaves_the_store_whole(){
    let d=dir("io-step");
    let out=std::process::Command::new(std::env::current_exe().unwrap()).args(["release_step_io_child","--exact","--ignored","--nocapture","--test-threads=1"])
        .env("POCOLD_IO_CHILD",&d).output().unwrap();
    let text=String::from_utf8_lossy(&out.stdout).to_string();
    let owed:usize=text.split("IO-CHILD owed ").nth(1).and_then(|x|x.split_whitespace().next()).and_then(|x|x.parse().ok())
        .unwrap_or_else(||panic!("the child: {text}\n{}",String::from_utf8_lossy(&out.stderr)));
    assert!(out.status.success()&&text.contains("IO-CHILD done"),"the child: {text}\n{}",String::from_utf8_lossy(&out.stderr));
    println!("{}",text.lines().filter(|l|l.contains("IO-CHILD")).collect::<Vec<_>>().join("\n"));
    let s=StateStore::open(&d).unwrap();
    assert!(!s.rebuilt&&!d.join(pocold::state_store::DAMAGED).exists(),"an I/O error in a release step does not damage the store");
    assert_eq!(s.checkpoints().unwrap().len(),2,"both checkpoints are kept");
    assert!(s.release_pending(),"the work is still owed");assert_eq!(s.owed_refs().unwrap().len(),owed,"the failed step lost no row");
    s.recount().unwrap();
    let mut steps=0;while s.release_pending(){s.release_step(16).unwrap();steps+=1;}
    assert!(steps>0);s.recount().unwrap();assert!(s.owed_refs().unwrap().is_empty());
    drop(s);let _=std::fs::remove_dir_all(&d);
}
#[cfg(unix)]
#[test] #[ignore="the child process of a_release_step_that_fails_on_io_leaves_the_store_whole"]
fn release_step_io_child(){
    let Ok(d)=std::env::var("POCOLD_IO_CHILD") else{return};let d=std::path::PathBuf::from(d);
    let s=owing_store(&d,0x10e);let owed=s.owed_refs().unwrap().len();let len=std::fs::metadata(s.path()).unwrap().len();
    assert!(len>4096,"positive control: the file is larger than the limit");
    let old=unsafe{libc::signal(libc::SIGXFSZ,libc::SIG_IGN);let mut old=libc::rlimit{rlim_cur:0,rlim_max:0};assert_eq!(libc::getrlimit(libc::RLIMIT_FSIZE,&mut old),0);
        assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE,&libc::rlimit{rlim_cur:4096,rlim_max:old.rlim_max}),0);old};
    let r=s.release_step(256);
    unsafe{assert_eq!(libc::setrlimit(libc::RLIMIT_FSIZE,&old),0);}
    println!("IO-CHILD owed {owed}");println!("IO-CHILD file {len} B; the step under RLIMIT_FSIZE 4096: {:?}",r.as_ref().map_err(|e|e.0.clone()));
    assert!(r.is_err(),"positive control: the step fails on I/O");
    assert!(!s.is_damaged()&&!d.join(pocold::state_store::DAMAGED).exists(),"a storage error does not mark the store damaged");
    assert!(!s.release_pending(),"no more steps are tried");
    println!("IO-CHILD done");
}
/// D-6 (review 2026-10-27): what a count means changed with the `release` table (format 3). A format-2 store
/// opens without a rebuild and is marked format 3, so a build of format 2 sets it aside rather than miss what
/// it owes; a store of another format is rebuilt.
#[test] fn a_format_2_store_opens_as_format_3_and_another_format_is_rebuilt(){
    const META:redb::TableDefinition<&str,u64>=redb::TableDefinition::new("meta");
    let set=|d:&std::path::Path,f:u64|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
        {let mut m=w.open_table(META).unwrap();m.insert("format",f).unwrap();}w.commit().unwrap();};
    let get=|d:&std::path::Path|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let r=db.begin_read().unwrap();
        let v=r.open_table(META).unwrap().get("format").unwrap().map(|v|v.value());v};
    assert_eq!(pocold::state_store::FORMAT,3);
    let d=dir("format");{let s=owing_store(&d,0xf2);drop(s);}
    assert_eq!(get(&d),Some(3));set(&d,2);
    {let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt,"a format-2 store is not rebuilt");assert!(s.release_pending());s.recount().unwrap();}
    assert_eq!(get(&d),Some(3),"it is marked format 3");
    set(&d,4);{let s=StateStore::open(&d).unwrap();assert!(s.rebuilt,"a store of another format is rebuilt");assert!(!s.release_pending());}
    let _=std::fs::remove_dir_all(&d);
}
/// D-6 (review 2026-10-27): a `release` row that contradicts the counts still marks the store damaged,
/// whichever path works it off (a release step, a checkpoint's write, the release at open), and the next
/// open rebuilds the store. A row is written straight into state.redb for a node that is not stored.
#[test] fn a_release_row_that_contradicts_the_counts_marks_the_store_damaged(){
    const RELEASE:redb::TableDefinition<&[u8;32],u32>=redb::TableDefinition::new("release");
    for (i,path) in ["step","write","open"].into_iter().enumerate(){
        let d=dir(&format!("broken-{path}"));
        {let s=owing_store(&d,0xb0+i as u64);while s.release_pending(){s.release_step(256).unwrap();}s.recount().unwrap();}
        {let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
         {let mut t=w.open_table(RELEASE).unwrap();t.insert(&[7u8;32],1).unwrap();}w.commit().unwrap();}
        let s=StateStore::open(&d).unwrap();assert!(s.release_pending(),"{path}: positive control: the row is owed");
        let r=match path{"step"=>s.release_step(256).map(|_|()),"write"=>s.write_checkpoint(b"c9",&[],2).map(|_|()),_=>s.release_all().map(|_|())};
        let e=r.expect_err(path).0;assert!(e.contains("not stored"),"{path}: {e}");
        assert!(s.is_damaged()&&d.join(pocold::state_store::DAMAGED).exists(),"{path}: the store is marked damaged");
        drop(s);let s=StateStore::open(&d).unwrap();assert!(s.rebuilt,"{path}: the next open rebuilds it");
        drop(s);let _=std::fs::remove_dir_all(&d);
    }
}
/// Seeded fault RV10 (2026-10-28): `stored_bytes` is every key and value of every table but `meta`, the
/// `release` rows among them. Checked, while rows are owed, against redb's own count of each table's key
/// and value bytes (`TableStats::stored_bytes`), read from the file once the store is closed.
#[test] fn stored_bytes_is_every_table_s_keys_and_values_while_rows_are_owed(){
    use redb::{ReadableTableMetadata,TableHandle};
    let d=dir("stored-bytes");let s=owing_store(&d,0x5b);let owed=s.owed_refs().unwrap().len();assert!(owed>0,"positive control: rows are owed");
    let counted=s.stored_bytes().unwrap();let path=s.path().to_path_buf();drop(s);
    let db=redb::Database::open(&path).unwrap();let r=db.begin_read().unwrap();let (mut all,mut rows,mut tables)=(0u64,0u64,vec![]);
    for t in r.list_tables().unwrap(){let name=t.name().to_string();if name=="meta"{continue}
        let b=r.open_untyped_table(t).unwrap().stats().unwrap().stored_bytes();all+=b;if name=="release"{rows=b;}tables.push((name,b));}
    println!("STORED-BYTES {counted} B counted; redb: {tables:?}");
    assert_eq!(rows,owed as u64*36,"positive control: redb counts 36 bytes for each of the {owed} release rows");
    assert_eq!(counted,all,"stored_bytes is redb's count of every table's keys and values");
    drop(r);drop(db);let _=std::fs::remove_dir_all(&d);
}
/// Storage faults (`--features io-fault`): an I/O error injected into state.redb once the store reaches a
/// named step, deterministic and on every platform (the RLIMIT_FSIZE test above is Unix only). redb then
/// refuses the store (`PreviousIo`) until it is opened again, as after a real I/O error. A failed write
/// (`Op::Write`, the commit's first page write) leaves the commit undone: redb's header still names the
/// transaction before. A failed fsync (`Op::Sync`) comes after the commit's pages and header were written, so
/// the commit may be durable all the same, as on a real disk; every test below takes both outcomes.
#[cfg(feature="io-fault")]
const IO_OPS:[pocold::io_fault::Op;2]=[pocold::io_fault::Op::Write,pocold::io_fault::Op::Sync];
/// D-6: a release step whose commit fails on I/O returns the error, does not mark the store damaged and
/// stops the steps. Reopened, the store is not rebuilt and keeps both checkpoints; it owes every row it
/// owed (a failed write) or what the step left (a failed fsync after which the step was durable). Its counts
/// recount and the release finishes.
#[cfg(feature="io-fault")]
#[test] fn an_io_fault_in_a_release_step_leaves_the_store_whole(){
    use pocold::io_fault;
    for op in IO_OPS{
        let d=dir(&format!("iof-step-{op:?}"));let s=owing_store(&d,0x10f);let owed=s.owed_refs().unwrap();let path=s.path().to_path_buf();
        io_fault::arm(&path,"release-step",op);
        let e=s.release_step(256).expect_err("the step fails on the injected fault").0;
        assert_eq!(io_fault::fired(&path),vec![("release-step".to_string(),op)],"{op:?}: positive control: one fault, at the step's commit ({e})");
        assert!(!s.is_damaged()&&!d.join(pocold::state_store::DAMAGED).exists(),"{op:?}: an I/O error is not damage");
        assert!(!s.release_pending(),"{op:?}: no more steps are tried");
        assert!(s.release_step(256).is_err(),"{op:?}: redb refuses the store until it is opened again");
        drop(s);let s=StateStore::open(&d).unwrap();
        assert!(!s.rebuilt,"{op:?}: the store is not rebuilt");assert_eq!(s.checkpoints().unwrap().len(),2,"{op:?}: both checkpoints are kept");
        let now=s.owed_refs().unwrap();s.recount().unwrap();
        match op{io_fault::Op::Write=>assert_eq!(now,owed,"a failed write: the step is undone and no row is lost"),
            io_fault::Op::Sync=>assert!(now==owed||now.len()<owed.len()+256,"a failed fsync: the step is undone or durable"),
            io_fault::Op::Read=>unreachable!("state.redb's faults are writes and fsyncs")}
        println!("IOF-STEP {op:?}: {} rows owed before, {} after reopening",owed.len(),now.len());
        assert_eq!(s.release_pending(),!now.is_empty(),"{op:?}: open reads the flag from the table");
        while s.release_pending(){s.release_step(16).unwrap();}s.recount().unwrap();assert!(s.owed_refs().unwrap().is_empty());
        drop(s);let _=std::fs::remove_dir_all(&d);
    }
}
/// Seeded fault RV12 (2026-10-28): a checkpoint's write that does not commit leaves `release_pending` as the
/// table it leaves; the flag is set only once the write commits. The write's commit fails on an injected
/// page-write fault, so it is undone:
/// 1. nothing owed, two checkpoints kept: the write would drop the oldest and owe its nodes. Nothing is owed
///    after the failure, and the flag stays clear;
/// 2. rows owed, one checkpoint kept (the other dropped): the write would finish what is owed and drop none.
///    The rows are still owed after the failure, and the flag stays set.
/// Each store is then reopened (redb refuses it until then): the table and the checkpoints are as they were.
/// With an fsync fault instead the write may be durable; the flag then keeps what the store was known to
/// hold, and the next open, which reads it from the table, gives the table either outcome left.
#[cfg(feature="io-fault")]
#[test] fn a_checkpoint_write_that_fails_on_io_leaves_the_flag_as_the_table(){
    use pocold::io_fault;
    for op in IO_OPS{for case in 1..=2u64{
        let d=dir(&format!("iof-write-{op:?}-{case}"));let s=owing_store(&d,0x120+case);
        if case==1{s.release_all().unwrap();}else{let c=s.checkpoints().unwrap();s.drop_checkpoint(c[0].0).unwrap();}
        let (owed,before)=(s.owed_refs().unwrap(),s.checkpoints().unwrap());
        assert_eq!((owed.is_empty(),before.len(),s.release_pending()),if case==1{(true,2,false)}else{(false,1,true)},"{op:?} {case}: positive control");
        let roots=before.last().unwrap().2.clone();let path=s.path().to_path_buf();
        io_fault::arm(&path,"checkpoint-txn",op);
        let e=s.write_checkpoint(b"c-fails",&roots,2).expect_err("the write fails on the injected fault").0;
        assert_eq!(io_fault::fired(&path).len(),1,"{op:?} {case}: positive control: the fault fired at the write's commit ({e})");
        assert!(!s.is_damaged(),"{op:?} {case}: an I/O error is not damage");
        assert_eq!(s.release_pending(),case==2,"{op:?} {case}: the flag keeps what the store was known to hold");
        drop(s);let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt);s.recount().unwrap();
        let (now,after)=(s.owed_refs().unwrap(),s.checkpoints().unwrap());let seqs=|c:&[(u64,Vec<u8>,Vec<Id>)]|c.iter().map(|x|x.0).collect::<Vec<_>>();
        let undone=now==owed&&seqs(&after)==seqs(&before);
        println!("IOF-WRITE {op:?} case {case}: the write was {} on disk",if undone{"undone"}else{"durable"});
        if op==io_fault::Op::Write{assert!(undone,"case {case}: a failed write is undone: what is owed and the checkpoints are as they were")}
        else{assert!(undone||(after.len()==2&&after.last().unwrap().1==b"c-fails"),"case {case}: a failed fsync leaves the write undone or whole")}
        assert_eq!(s.release_pending(),!now.is_empty(),"{op:?} {case}: open reads the flag from the table");
        drop(s);let _=std::fs::remove_dir_all(&d);
    }}
}
/// An I/O error in a block's commit (state-txn): with a failed write the version is not stored; with a failed
/// fsync it may be stored and pinned. Either way the store opens without a rebuild and its counts recount
/// (the node's open then resets the pins to the checkpoints'). The same commit then succeeds and reads back.
#[cfg(feature="io-fault")]
#[test] fn a_commit_that_fails_on_io_is_undone_or_whole(){
    use pocold::io_fault;
    for op in IO_OPS{
        let c=codes();let mut r=Rng(0xc0);let d=dir(&format!("iof-commit-{op:?}"));let s=StateStore::open(&d).unwrap();let db=state(&mut r,120,&c);
        let mut b=Batch::default();let t=NodeTrie::build(&db,&s,&mut b);let path=s.path().to_path_buf();
        io_fault::arm(&path,"state-txn",op);
        assert!(s.commit(&b,t.world,true).is_err(),"{op:?}: the commit fails on the injected fault");assert_eq!(io_fault::fired(&path).len(),1);
        assert!(!s.is_damaged());drop(s);
        let s=StateStore::open(&d).unwrap();assert!(!s.rebuilt);s.recount().unwrap();
        let stored=s.count(&t.world.unwrap()).unwrap();println!("IOF-COMMIT {op:?}: the root's count after reopening {stored:?}");
        if op==io_fault::Op::Write{assert_eq!((stored,s.node_count().unwrap()),(None,0),"a failed write: the version is not stored")}
        else{assert!(stored.is_none()||stored==Some(1),"a failed fsync: undone, or stored and pinned once")}
        s.commit(&b,t.world,true).unwrap();s.recount().unwrap();same(&db,&TrieState{store:&s,trie:t,block_hashes:&db.block_hashes},"after the retried commit");
        drop(s);let _=std::fs::remove_dir_all(&d);
    }
}

/// 2026-10-28, replay's late release (`StateStore::set_deferred`): a version an entry stops holding is released
/// inside the next block's commit, after that block's nodes are counted, while the next block's batch was built
/// with the version still stored (so it leaves out the nodes both hold). After every step the store holds the
/// same rows (every node with its count, every pin) as one that released each version in its own transaction
/// before the next batch was built, and each kept version reads its state.
#[test] fn a_late_release_leaves_the_rows_an_immediate_one_leaves(){
    use pocol_lp4_execution::node_trie::TrieStore;
    let c=codes();let (da,db_)=(dir("late-a"),dir("late-b"));
    let (a,b)=(StateStore::open(&da).unwrap(),StateStore::open(&db_).unwrap());b.set_deferred(true).unwrap();
    let mut r=Rng(0x1a7e);let mut st=state(&mut r,150,&c);
    let (mut ta,mut tb)={let mut x=Batch::default();let t=NodeTrie::build(&st,&a,&mut x);a.commit(&x,t.world,true).unwrap();
        let mut y=Batch::default();let u=NodeTrie::build(&st,&b,&mut y);b.commit(&y,u.world,true).unwrap();(t,u)};
    let mut kept:std::collections::VecDeque<(Id,InMemoryDB)>=[(ta.world.unwrap(),st.clone())].into();
    let rows=|s:&StateStore,roots:&[Id]|{let mut v:Vec<(Id,u32)>=reach(s,roots).into_iter().map(|id|(id,s.count(&id).unwrap().unwrap())).collect();v.sort();v};
    let mut late_seen=0;
    for step in 0..40{
        let next=mutate(&mut r,&st,&c,600);let touched=Touched::between(&st,&next);
        // On odd steps the late store releases the parent after the child's batch is built and before the child is
        // committed (allowed only while releases wait; the other store releases it after the commit). The nodes only
        // the two share must outlive the release.
        let early=step%2==1&&kept.len()==1;
        let mut x=Batch::default();let ua=ta.update(&a,&mut x,&next,&touched);
        let mut y=Batch::default();let ub=tb.update(&b,&mut y,&next,&touched);
        assert_eq!(ua.world,ub.world,"step {step}: the same root");
        let parent=kept.back().unwrap().0;
        if early{TrieStore::unpin(&b,&parent).unwrap();}
        if b.deferred_releases()>0{late_seen+=1;}
        a.commit(&x,ua.world,true).unwrap();if early{TrieStore::unpin(&a,&parent).unwrap();kept.pop_back();}
        b.commit(&y,ub.world,true).unwrap_or_else(|e|panic!("step {step}: {}",e.0));assert_eq!(b.deferred_releases(),0,"step {step}: the commit released what waited");
        kept.push_back((ua.world.unwrap(),next.clone()));
        // Otherwise up to three versions are kept, released after the commit as replay's retain does.
        while kept.len()>if step%2==0{1}else{3}{let (old,_)=kept.pop_front().unwrap();TrieStore::unpin(&a,&old).unwrap();TrieStore::unpin(&b,&old).unwrap();}
        ta=ua;tb=ub;st=next;
        let roots:Vec<Id>=kept.iter().map(|k|k.0).collect();
        if b.deferred_releases()==0{assert_eq!(rows(&a,&roots),rows(&b,&roots),"step {step}: the same nodes and counts");assert_eq!(a.node_count().unwrap(),b.node_count().unwrap(),"step {step}");}
    }
    assert!(late_seen>=20,"positive control: releases waited for the next commit ({late_seen})");
    // What waits when deferral ends is released then.
    let (old,_)=kept.pop_front().unwrap();TrieStore::unpin(&a,&old).unwrap();TrieStore::unpin(&b,&old).unwrap();
    assert_eq!(b.deferred_releases(),1,"positive control: a release waits");
    b.set_deferred(false).unwrap();assert_eq!(b.deferred_releases(),0);
    let roots:Vec<Id>=kept.iter().map(|k|k.0).collect();
    assert_eq!(rows(&a,&roots),rows(&b,&roots));assert_eq!(a.node_count().unwrap(),b.node_count().unwrap(),"no node outlives its last version");
    let mut pa=a.pins().unwrap();let mut pb=b.pins().unwrap();pa.sort();pb.sort();assert_eq!(pa,pb,"the same pins");
    a.recount().unwrap();b.recount().unwrap();
    for (w,s) in &kept{same(s,&TrieState{store:&b,trie:NodeTrie{world:Some(*w)},block_hashes:&s.block_hashes},"late");}
    drop((a,b));for d in [da,db_]{let _=std::fs::remove_dir_all(&d);}
}
