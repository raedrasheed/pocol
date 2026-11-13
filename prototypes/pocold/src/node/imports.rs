//! V3-04 local timing journal/index plumbing. No lifetime map is kept in RAM.
use crate::{chain::{Chain,Entry,Spec},store::{self,FirstImport,Journal,Recovered},types::{block_hash,decode_header}};
use super::canon::{ImportCoverage,Index};

/// This also records pre-activation receipts on a future version-3 schedule.
/// Every currently admissible spec-1 schedule leaves its legacy files unchanged.
pub(super) fn enabled(spec:&Spec)->bool{spec.v3_active(u64::MAX)}

/// The index is derived: its offset must name the original block in the verified
/// journal. Corruption is a local storage failure, never a verdict on a peer's block.
pub(super) fn first(index:&Index,journal:&Journal,hash:&[u8;32])->Result<Option<FirstImport>,String>{
    let Some(off)=index.first_import_offset(hash)? else{return Ok(None)};
    let payload=journal.payload_at(off).map_err(|e|format!("first import journal {off}: {e:?}"))?;
    let record=store::decode_record_ref(&payload).map_err(|e|index.mark_damaged(&format!("first import record: {e}")))?;
    let header=decode_header(record.header).map_err(|e|index.mark_damaged(&format!("first import header: {e}")))?;
    if block_hash(&header)!=*hash{return Err(index.mark_damaged("first import offset names another block"))}
    record.first_import.map(Some).ok_or_else(||index.mark_damaged("first import offset has no timestamp"))
}

/// A checked coverage marker avoids rescanning an unchanged checkpoint prefix.
/// A missing index is rebuilt one journal payload at a time, including records a
/// checkpoint skips executing. Journal locking and the memory gate precede this.
pub(super) fn open(chain:&Chain,index:&Index,rec:&Recovered,end:impl Fn(usize)->u64,roll:impl Fn(usize)->Result<[u8;32],String>,roll_end:[u8;32],now_ms:u128)->Result<Option<u128>,String>{
    if !enabled(&chain.spec){return Ok(None)}
    index.prepare_imports()?;
    let n=rec.offsets.len();let mut coverage=index.import_coverage()?;
    if let Some(c)=coverage{
        let k=usize::try_from(c.binding.records).ok().filter(|k|*k<=n);
        if k.map_or(true,|k|end(k)!=c.binding.end)||match k{Some(k)=>roll(k)?!=c.binding.hash,None=>true}{index.reset_imports()?;coverage=None;}
    }
    let mut coverage=coverage.unwrap_or(ImportCoverage{binding:crate::checkpoint::Binding{records:0,end:end(0),hash:[0;32]},genesis_received_ms:None});
    let genesis=chain.spec.genesis_hash();
    for i in coverage.binding.records as usize..n{
        let payload=rec.payload(i).map_err(|e|format!("first import record {i}: {e:?}"))?;
        let record=store::decode_record_ref(&payload)?;let header=decode_header(record.header).map_err(|e|format!("first import header {i}: {e}"))?;
        let hash=block_hash(&header);
        if chain.spec.v3_active(header.h)&&record.first_import.is_none(){return Err(format!("record {i}: a v3 journal record has no first-import timing; its historical flags cannot be reconstructed"))}
        if let Some(stamp)=record.first_import{
            if let Some(original)=first(index,&rec.journal,&hash)?{if original!=stamp{return Err(format!("record {i}: a re-import changed the original timing"))}}
            if header.h==1&&header.parent_hash==genesis{
                if coverage.genesis_received_ms.is_some_and(|t|t!=stamp.parent_received_ms){return Err(format!("record {i}: genesis receipt context changed"))}
                coverage.genesis_received_ms=Some(stamp.parent_received_ms);
            }
        }
        coverage.binding=crate::checkpoint::Binding{records:(i+1) as u64,end:end(i+1),hash:crate::checkpoint::roll(&coverage.binding.hash,&payload)};
        index.note_import(record.first_import.map(|_|(hash,rec.offsets[i])),coverage)?;
    }
    if coverage.binding.hash!=roll_end{return Err("the journal changed while first imports were indexed".into())}
    index.sync()?;
    Ok(Some(coverage.genesis_received_ms.unwrap_or(now_ms)))
}

/// Compute once from the timestamp captured BEFORE validation/rebuild/fsync.
/// The original boolean is persisted because local limits can change on restart.
/// `template_ms` is when this node first locked the signed template of `entry` (validated
/// as minable, by T_j), if it still remembers that and the signer did not equivocate there
/// (`Node::template_seen`, bounded, not persisted).
/// It decides a signed block's bit; the stored arrival stays the block's own receipt,
/// which is the b_j of its children. A block classified once is never classified again.
pub(super) fn classify(spec:&Spec,parent:&Entry,entry:&Entry,received_ms:u128,parent_received_ms:u128,template_ms:Option<u128>)->Result<FirstImport,String>{
    let timely=if !spec.v3_active(entry.height)||entry.header.a>=1{true}else{
        let local=spec.local();let cp=spec.cp();
        let deadline=crate::timing::AttemptWindow::new(parent.ts,0,cp.D_att).and_then(|w|w.deadline_ms(parent_received_ms,u64::from(local.delta_rel),cp.dFbWait)).map_err(|e|format!("first import deadline: {e:?}"))?;
        crate::timing::is_timely_import(entry.signed,entry.header.a,received_ms,template_ms.filter(|_|entry.signed),deadline,u64::from(local.delta),u64::from(local.capital_phi()))
    };
    Ok(FirstImport{received_ms,parent_received_ms,timely})
}
