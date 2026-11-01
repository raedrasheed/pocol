use lp1_node::{fixed::U256, hashes::keccak256, hex, trie::*};
#[test]
fn frozen_empty_root_and_secure_empty_storage() {
    let expected = hex::decode("56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421").unwrap();
    assert_eq!(root(&[], 0).unwrap().as_slice(), expected);
    assert_eq!(storage_root(&[(U256::ONE, U256::ZERO)], 1).unwrap(), root(&[], 0).unwrap());
    assert_eq!(ordered_root(&[], 0).unwrap(), root(&[], 0).unwrap());
}
#[test]
fn prefix_inline_branch_and_budget_boundaries() {
    let mut v = vec![(vec![], vec![1]), (vec![0x12], vec![2]), (vec![0x12, 0x34], vec![3]), (vec![0x12, 0x35], vec![4; 32]), (vec![0xff; 64], vec![5; 60])];
    let expected = root(&v, 5).unwrap();
    v.reverse();
    assert_eq!(root(&v, 5).unwrap(), expected);
    assert_eq!(root(&v, 4), Err(TrieError::EntryBudget));
    v.push(v[0].clone());
    assert_eq!(root(&v, 6), Err(TrieError::DuplicateKey));
    assert_eq!(root(&[(vec![0; 65], vec![1])], 1), Err(TrieError::KeyTooLong));
    assert_eq!(root(&[(vec![1], vec![])], 1), Err(TrieError::EmptyValue));
}
#[test]
fn state_storage_and_ordered_semantics() {
    let empty = root(&[], 0).unwrap();
    let slots = [(U256::ZERO, U256::from_u64(128)), (U256::ONE, U256::ZERO)];
    let sr = storage_root(&slots, 2).unwrap();
    assert_ne!(sr, empty);
    assert_eq!(storage_root(&[(U256::ONE, U256::ZERO), (U256::ONE, U256::ZERO)], 2), Err(TrieError::DuplicateKey));
    let a = Account { address: [1; 20], nonce: 0, balance: U256::ONE, storage_root: sr, code_hash: keccak256(&[]) };
    let r = state_root(&[a.clone()], 1).unwrap();
    assert_ne!(r, empty);
    assert_eq!(state_root(&[a.clone(), a], 2), Err(TrieError::DuplicateKey));
    assert_eq!(ordered_root(&[vec![1], vec![2]], 2).unwrap(), root(&[(vec![0x80], vec![1]), (vec![1], vec![2])], 2).unwrap());
    assert_ne!(ordered_root(&[vec![1], vec![2]], 2), ordered_root(&[vec![2], vec![1]], 2));
}
#[test]
fn frozen_independent_insertion_oracle_roots() {
    use serde_json::Value;
    let rows: Value = serde_json::from_str(include_str!("data/lp4-trie-vectors.json")).unwrap();
    let h = |v: &Value| hex::decode(v.as_str().unwrap()).unwrap();
    let u = |v: &Value| U256::from_dec_str(v.as_str().unwrap()).unwrap();
    for row in rows.as_array().unwrap() {
        let v = &row["input"];
        let budget = v["budget"].as_u64().unwrap() as usize;
        let result = match v["type"].as_str().unwrap() {
            "raw" => root(&v["entries"].as_array().unwrap().iter().map(|x| (h(&x[0]), h(&x[1]))).collect::<Vec<_>>(), budget),
            "storage" => storage_root(&v["entries"].as_array().unwrap().iter().map(|x| (u(&x[0]), u(&x[1]))).collect::<Vec<_>>(), budget),
            "ordered" => ordered_root(&v["entries"].as_array().unwrap().iter().map(h).collect::<Vec<_>>(), budget),
            "state" => state_root(
                &v["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|x| {
                        let mut a = [0; 20];
                        a.copy_from_slice(&h(&x[0]));
                        let mut s = [0; 32];
                        s.copy_from_slice(&h(&x[3]));
                        let mut c = [0; 32];
                        c.copy_from_slice(&h(&x[4]));
                        Account { address: a, nonce: x[1].as_str().unwrap().parse().unwrap(), balance: u(&x[2]), storage_root: s, code_hash: c }
                    })
                    .collect::<Vec<_>>(),
                budget,
            ),
            _ => panic!("type"),
        };
        assert_eq!(hex::encode(&result.unwrap()), row["expected"]["root"].as_str().unwrap());
    }
}
#[test]
fn maximum_prefix_depth_remains_bounded() {
    let entries = (0..=64).map(|n| (vec![0; n], vec![1])).collect::<Vec<_>>();
    let expected = root(&entries, 65).unwrap();
    let actual = std::thread::Builder::new().stack_size(512 * 1024).spawn(move || root(&entries, 65)).unwrap().join().unwrap().unwrap();
    assert_eq!(actual, expected);
}
