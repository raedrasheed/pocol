//! LP4 P9 persistence: torn tail at EVERY byte offset of the last record,
//! complete-record corruption, binding mismatch, single writer.
//! Audit A04: framing corruption is never an interrupted append (exhaustive bit flips, boundary
//! edits, truncation of committed data, commit-file damage, v1 migration).
use pocold::store::{self,StoreError};
use std::fs;
use std::path::{Path,PathBuf};

fn tmp(name:&str)->PathBuf{let d=std::env::temp_dir().join(format!("pocold-store-{name}-{}",std::process::id()));let _=fs::remove_dir_all(&d);d}
const B:[u8;32]=[9;32];
const OV:usize=4+4+32+1; // v2 record overhead: len, lenCheck, digest, marker
/// Copies a store's journal and commit file into a fresh directory with the journal replaced by `journal`.
fn store_with(src:&Path,name:&str,journal:&[u8])->PathBuf{
    let d=tmp(name);fs::create_dir_all(&d).unwrap();fs::write(d.join("blocks.journal"),journal).unwrap();
    fs::copy(src.join("blocks.commit"),d.join("blocks.commit")).unwrap();d
}

#[test] fn append_reopen_and_torn_tail_at_every_offset(){
    let dir=tmp("torn");
    let payloads:Vec<Vec<u8>>=(0..4u8).map(|i|vec![i;100+i as usize]).collect();
    {let mut r=store::open(&dir,&B).unwrap();assert!(r.payloads().is_empty());for p in &payloads[..3]{r.journal.append(p).unwrap();}}
    // The fourth append is interrupted: its bytes reach the journal, the commit count stays at 3.
    let committed=fs::read(dir.join("blocks.journal")).unwrap();
    let mut full=committed.clone();full.extend(store::encode_frame(&payloads[3]));
    {let d=store_with(&dir,"extra",&full);let r=store::open(&d,&B).unwrap();assert_eq!(r.payloads(),payloads,"a complete record beyond the commit count is kept");assert!(!r.torn_tail);}
    let last=OV+payloads[3].len();let prefix=full.len()-last;assert_eq!(prefix,committed.len());
    for cut in prefix+1..full.len(){
        let d=store_with(&dir,&format!("cut{cut}"),&full[..cut]);
        let r=store::open(&d,&B).unwrap_or_else(|e|panic!("cut {cut}: {e:?}"));
        assert!(r.torn_tail,"cut {cut}");assert_eq!(r.payloads(),payloads[..3].to_vec(),"cut {cut}: only complete records are committed");
        // Original torn bytes are preserved; the new journal holds exactly the verified prefix.
        assert_eq!(fs::read(d.join("blocks.journal.torn-0")).unwrap(),full[..cut].to_vec());
        assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),full[..prefix].to_vec());
        drop(r);let again=store::open(&d,&B).unwrap();assert!(!again.torn_tail);assert_eq!(again.payloads().len(),3);
        let _=fs::remove_dir_all(&d);
    }
    // Cut exactly at a record boundary is a clean shorter journal (not torn).
    let d=store_with(&dir,"boundary",&full[..prefix]);
    let r=store::open(&d,&B).unwrap();assert!(!r.torn_tail);assert_eq!(r.payloads().len(),3);
}

#[test] fn complete_record_corruption_is_never_skipped(){
    let dir=tmp("corrupt");
    {let mut r=store::open(&dir,&B).unwrap();for i in 0..3u8{r.journal.append(&[i;64]).unwrap();}}
    let full=fs::read(dir.join("blocks.journal")).unwrap();
    // Flip one byte inside each region of record 1 (length, length check, digest, payload, marker).
    let rec=OV+64;let start=40+rec;
    for off in [start,start+5,start+8,start+44,start+rec-1]{
        let mut bad=full.clone();bad[off]^=0x5a;let d=store_with(&dir,&format!("flip{off}"),&bad);
        // All three records are committed: damage to any of them fails closed, files untouched (A04).
        assert!(matches!(store::open(&d,&B),Err(StoreError::Corrupt(1))),"off {off}: committed record 1 damaged");
        assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),bad,"off {off}: nothing modified");
        let _=fs::remove_dir_all(&d);
    }
}

#[test] fn binding_mismatch_and_second_writer_rejected(){
    let dir=tmp("bind");
    let first=store::open(&dir,&B).unwrap();
    assert!(matches!(store::open(&dir,&B),Err(StoreError::Locked)),"second writer while first is open");
    drop(first);
    assert!(matches!(store::open(&dir,&[1;32]),Err(StoreError::Binding)),"journal bound to another genesisHash");
}

#[test] fn missing_journal_with_recovery_artifacts_refuses_to_start(){
    let dir=tmp("missing");
    {let mut r=store::open(&dir,&B).unwrap();r.journal.append(&[1;10]).unwrap();}
    fs::rename(dir.join("blocks.journal"),dir.join("blocks.journal.torn-0")).unwrap();
    assert!(matches!(store::open(&dir,&B),Err(StoreError::MissingJournal)));
}

// ---------------- audit A04: strict corruption detection ----------------
fn three_records(name:&str)->(PathBuf,Vec<u8>){
    let dir=tmp(name);{let mut r=store::open(&dir,&B).unwrap();for i in 0..3u8{r.journal.append(&[0x30+i;50]).unwrap();}}
    let j=fs::read(dir.join("blocks.journal")).unwrap();(dir,j)
}
#[test] fn a04_audit_case_one_bit_in_the_first_length_is_corruption(){
    let (dir,mut j)=three_records("a04-case");j[40]^=0x01;
    let d=store_with(&dir,"a04-case-copy",&j);
    assert_eq!(store::open(&d,&B).err(),Some(StoreError::Corrupt(0)),"must fail closed, not truncate committed records");
    assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),j,"evidence untouched");
    assert!(!d.join("blocks.journal.torn-0").exists());
}
#[test] fn a04_every_single_bit_flip_of_a_committed_journal_fails_closed(){
    let (dir,j)=three_records("a04-flips");let mut checked=0;
    for off in 0..j.len(){for bit in 0..8{
        let mut bad=j.clone();bad[off]^=1<<bit;let d=store_with(&dir,"a04-flip",&bad);
        let r=store::open(&d,&B);
        assert!(matches!(r,Err(StoreError::Corrupt(_))|Err(StoreError::Binding)),"offset {off} bit {bit}: {:?}",r.as_ref().map(|r|(r.payloads().len(),r.torn_tail)));
        assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),bad,"offset {off} bit {bit}: files are never modified on corruption");
        let _=fs::remove_dir_all(&d);checked+=1;
    }}
    assert_eq!(checked,j.len()*8);
}
#[test] fn a04_truncation_of_committed_records_is_corruption(){
    let (dir,j)=three_records("a04-trunc");let rec=OV+50;
    for cut in [40,40+rec,40+rec+7,40+2*rec,40+2*rec+1,j.len()-1]{
        let d=store_with(&dir,"a04-cut",&j[..cut]);
        assert!(matches!(store::open(&d,&B),Err(StoreError::Corrupt(_))),"cut {cut}: committed records are missing");
        assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),j[..cut].to_vec());let _=fs::remove_dir_all(&d);
    }
}
#[test] fn a04_boundary_edits_and_duplicates_are_corruption(){
    let (dir,j)=three_records("a04-edit");let rec=OV+50;let b1=40+rec;
    let mut ins=j.clone();ins.insert(b1,0x00);
    let mut del=j.clone();del.remove(b1);
    let mut dup2=j.clone();dup2.extend_from_slice(&j[40..40+rec]);dup2.extend_from_slice(&j[40..40+rec]);
    let mut swap=j[..40].to_vec();swap.extend_from_slice(&j[40+rec..40+2*rec]);swap.extend_from_slice(&j[40..40+rec]);swap.extend_from_slice(&j[40+2*rec..]);
    for (what,bad) in [("insert at boundary",ins),("delete at boundary",del),("two records beyond the commit count",dup2)]{
        let d=store_with(&dir,"a04-e",&bad);let r=store::open(&d,&B);
        assert!(matches!(r,Err(StoreError::Corrupt(_))),"{what}: {:?}",r.as_ref().map(|r|r.payloads().len()));let _=fs::remove_dir_all(&d);
    }
    // Reordered complete records pass the framing layer; replay rejects them (unknown parent), so the
    // node still fails closed. Recorded here so the framing guarantee is not overstated.
    let d=store_with(&dir,"a04-swap",&swap);let r=store::open(&d,&B).unwrap();assert_eq!(r.payloads().len(),3);assert_ne!(r.payloads()[0],vec![0x30;50]);
}
#[test] fn a04_zero_filled_tail_is_an_interrupted_append(){
    let (dir,j)=three_records("a04-zero");
    for n in [1usize,7,8,41,4096]{
        let mut z=j.clone();z.extend(std::iter::repeat_n(0u8,n));let d=store_with(&dir,"a04-z",&z);
        let r=store::open(&d,&B).unwrap_or_else(|e|panic!("{n} zero bytes: {e:?}"));
        assert!(r.torn_tail&&r.payloads().len()==3,"{n} zero bytes");drop(r);let _=fs::remove_dir_all(&d);
    }
    // Non-zero bytes after the committed end were never committed (the commit file counts 3 and ends
    // there): an interrupted append, recovered, with the original bytes kept (review finding).
    let mut g=j.clone();g.extend([0xde,0xad,0xbe,0xef,0x01,0x02,0x03,0x04,0x05]);let d=store_with(&dir,"a04-g",&g);
    let r=store::open(&d,&B).unwrap();assert!(r.torn_tail&&r.payloads().len()==3);drop(r);
    assert_eq!(fs::read(d.join("blocks.journal.torn-0")).unwrap(),g,"original bytes kept");assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),j);let _=fs::remove_dir_all(&d);
    // More than one maximal record of such bytes cannot be a single interrupted append.
    let mut big=j.clone();big.extend(std::iter::repeat_n(0x5au8,store::MAX_RECORD+OV+1));let d=store_with(&dir,"a04-big",&big);
    assert!(matches!(store::open(&d,&B),Err(StoreError::Corrupt(3))));assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),big,"untouched");let _=fs::remove_dir_all(&d);
    // Any tail after a complete record beyond the commit count would be a second uncommitted append.
    for (what,tail) in [("garbage",vec![0xde,0xad,0xbe,0xef,0x01,0x02,0x03,0x04,0x05]),("strict prefix",store::encode_frame(&[8u8;30])[..20].to_vec()),("zeros",vec![0u8;12])]{
        let mut two=j.clone();two.extend(store::encode_frame(&[7u8;20]));two.extend(tail);let d=store_with(&dir,"a04-two",&two);
        assert!(matches!(store::open(&d,&B),Err(StoreError::Corrupt(4))),"{what} after an uncommitted record");let _=fs::remove_dir_all(&d);
    }
    // With a damaged commit slot the surviving older slot may predate the last commit: no leniency.
    let c=fs::read(dir.join("blocks.commit")).unwrap();
    let d=store_with(&dir,"a04-dmg",&g);let mut bad=c.clone();let newest=if c[4096..4104].iter().any(|b|*b!=0)&&u64::from_be_bytes(c[4096..4104].try_into().unwrap())>u64::from_be_bytes(c[..8].try_into().unwrap()){4096}else{0};
    bad[newest+30]^=0x01;fs::write(d.join("blocks.commit"),&bad).unwrap();
    assert!(matches!(store::open(&d,&B),Err(StoreError::Corrupt(_))),"damaged newest slot + non-record tail fails closed");let _=fs::remove_dir_all(&d);
}
#[test] fn a04_commit_file_damage(){
    let (dir,_)=three_records("a04-commit");let c=fs::read(dir.join("blocks.commit")).unwrap();
    // One damaged slot: the other (older) slot still bounds the committed count; the journal satisfies it.
    // Slots at 0 and 4096 (separate sectors).
    assert_eq!(c.len(),4096+64);
    for slot in [0usize,4096]{
        let d=tmp("a04-c1");fs::create_dir_all(&d).unwrap();fs::copy(dir.join("blocks.journal"),d.join("blocks.journal")).unwrap();
        let mut bad=c.clone();bad[slot+3]^=0x10;fs::write(d.join("blocks.commit"),&bad).unwrap();
        let r=store::open(&d,&B).unwrap_or_else(|e|panic!("slot {slot}: {e:?}"));assert_eq!(r.payloads().len(),3);drop(r);let _=fs::remove_dir_all(&d);
    }
    let d=tmp("a04-c2");fs::create_dir_all(&d).unwrap();fs::copy(dir.join("blocks.journal"),d.join("blocks.journal")).unwrap();
    let mut bad=c.clone();bad[3]^=0x10;bad[4099]^=0x10;fs::write(d.join("blocks.commit"),&bad).unwrap();
    assert_eq!(store::open(&d,&B).err(),Some(StoreError::CommitCorrupt));
    fs::remove_file(d.join("blocks.commit")).unwrap();assert_eq!(store::open(&d,&B).err(),Some(StoreError::MissingCommit));
    // The first v2 layout (both slots adjacent, 128 bytes) is read and rewritten in the current layout.
    let d=tmp("a04-c3");fs::create_dir_all(&d).unwrap();fs::copy(dir.join("blocks.journal"),d.join("blocks.journal")).unwrap();
    let legacy=[&c[..64],&c[4096..4160]].concat();fs::write(d.join("blocks.commit"),&legacy).unwrap();
    {let r=store::open(&d,&B).unwrap();assert_eq!(r.payloads().len(),3);}
    assert_eq!(fs::read(d.join("blocks.commit")).unwrap().len(),4096+64);
    {let mut r=store::open(&d,&B).unwrap();r.journal.append(&[1u8;10]).unwrap();}assert_eq!(store::open(&d,&B).unwrap().payloads().len(),4);let _=fs::remove_dir_all(&d);
}
#[test] fn a04_v1_journal_migrates_only_when_fully_verified(){
    use sha2::{Digest,Sha256};
    let v1=|recs:&[Vec<u8>]|{let mut b=b"POCOLJ1\n".to_vec();b.extend(B);for p in recs{b.extend((p.len() as u32).to_be_bytes());b.extend(Sha256::digest(p));b.extend(p);b.push(1);}b};
    let recs:Vec<Vec<u8>>=(0..3u8).map(|i|vec![i;20]).collect();
    let d=tmp("a04-v1");fs::create_dir_all(&d).unwrap();fs::write(d.join("blocks.journal"),v1(&recs)).unwrap();
    let r=store::open(&d,&B).unwrap();assert!(r.migrated_v1);assert_eq!(r.payloads(),recs);drop(r);
    assert_eq!(fs::read(d.join("blocks.journal.v1-0")).unwrap(),v1(&recs),"original kept");
    let again=store::open(&d,&B).unwrap();assert!(!again.migrated_v1);assert_eq!(again.payloads(),recs);drop(again);
    // A v1 journal with a tail (torn or a corrupted length) is ambiguous: refused, nothing modified.
    let mut t=v1(&recs);t.truncate(t.len()-3);let d=tmp("a04-v1t");fs::create_dir_all(&d).unwrap();fs::write(d.join("blocks.journal"),&t).unwrap();
    assert_eq!(store::open(&d,&B).err(),Some(StoreError::LegacyNeedsReview));assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),t);
    let mut l=v1(&recs);l[40]^=0x01;let d=tmp("a04-v1l");fs::create_dir_all(&d).unwrap();fs::write(d.join("blocks.journal"),&l).unwrap();
    assert_eq!(store::open(&d,&B).err(),Some(StoreError::LegacyNeedsReview));assert_eq!(fs::read(d.join("blocks.journal")).unwrap(),l);
}

/// D-REORG-REDB: append returns each record's frame offset, and read_at reads it back by offset,
/// verifying the length check, payload digest and marker (a bad offset is a hard error, never an
/// empty body). This is the off-heap block-store primitive.
#[test] fn append_returns_offsets_and_read_at_round_trips(){
    let dir=tmp("readat");
    let mut offs=vec![];
    {
        let mut r=store::open(&dir,&B).unwrap();
        for i in 0..5u8{
            let p=store::encode_record(&[i;20],&[i;100],&[i;4]);
            offs.push(r.journal.append(&p).unwrap());
        }
    }
    // Reopen (fresh handle) and read every record back by its recorded offset.
    let r=store::open(&dir,&B).unwrap();
    assert_eq!(r.payloads().len(),5);
    for (i,off) in offs.iter().enumerate(){
        let (h,b,res)=r.journal.read_at(*off).expect("read_at a committed offset");
        assert_eq!(h,vec![i as u8;20],"header bytes at record {i}");
        assert_eq!(b,vec![i as u8;100],"body bytes at record {i}");
        assert_eq!(res,vec![i as u8;4],"result bytes at record {i}");
    }
    // The first record's offset is 40 (after MAGIC(8) + binding(32)).
    assert_eq!(offs[0],40);
    // A bad offset (into the middle of a record) is a hard error, not an empty read.
    assert!(r.journal.read_at(50).is_err(),"a mid-record offset must not decode");
    let _=fs::remove_dir_all(&dir);
}
