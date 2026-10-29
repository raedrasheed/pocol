//! F-B regression: a journal record verified at open becomes corrupt before restore reads it.
//! Exercise the real JournalSource, not a mock returning the desired error category.
use pocold::{
    checkpoint::{JournalSource, Source, LOCAL},
    history::History,
    store,
};
use std::collections::HashMap;
use std::io::{Seek, SeekFrom, Write};

#[test]
fn verified_journal_corruption_is_a_local_failure_not_a_checkpoint_refusal() {
    let dir = std::env::temp_dir().join(format!("pocold-fb-source-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    let binding = [0x42; 32];
    let payload = vec![0x5a; 128];
    {
        let mut recovered = store::open(&dir, &binding).unwrap();
        recovered.journal.append(&payload).unwrap();
    }
    // This first pass actually verifies the record and supplies the trusted offset map.
    let recovered = store::open(&dir, &binding).unwrap();
    assert_eq!(recovered.offsets.len(), 1);
    let off = recovered.offsets[0];
    let history = History::open(&dir).unwrap();
    let source = JournalSource {
        by_off: HashMap::from([(off, 0)]),
        journal: &recovered.journal,
        records: 1,
        history: &history,
    };
    assert_eq!(source.payload_at(off).unwrap(), Some(payload));
    assert_eq!(
        source.payload_at(off + 1).unwrap(),
        None,
        "unknown offsets remain refusals"
    );
    let outside_prefix = JournalSource {
        by_off: source.by_off.clone(),
        journal: &recovered.journal,
        records: 0,
        history: &history,
    };
    assert_eq!(outside_prefix.payload_at(off).unwrap(), None);
    let path = dir.join("blocks.journal");
    let original = std::fs::read(&path).unwrap();
    let commit = std::fs::read(dir.join("blocks.commit")).unwrap();
    // Flip length, length checksum, digest, payload and marker after verification.
    // Also cover an actual short read, separately from a checksum mismatch.
    for delta in [0usize, 4, 8, 40, 40 + 128, usize::MAX] {
        let mut bad = original.clone();
        if delta == usize::MAX {
            bad.truncate(off as usize + 10);
        } else {
            bad[off as usize + delta] ^= 1;
        }
        let mut file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.seek(SeekFrom::Start(0)).unwrap();
        file.write_all(&bad).unwrap();
        file.set_len(bad.len() as u64).unwrap();
        file.sync_all().unwrap();
        drop(file);
        let error = source
            .payload_at(off)
            .expect_err("verified bytes changed locally");
        assert!(error.starts_with(LOCAL), "delta={delta}: {error}");
        assert!(error.contains(&format!("offset {off}")), "{error}");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            bad,
            "failure preserves evidence"
        );
        assert_eq!(std::fs::read(dir.join("blocks.commit")).unwrap(), commit);
    }
    // Restoring exact bytes restores successful reading; no cached rejection remains.
    std::fs::write(&path, &original).unwrap();
    assert!(source.payload_at(off).unwrap().is_some());
    drop(history);
    drop(recovered);
    std::fs::remove_dir_all(dir).unwrap();
}
