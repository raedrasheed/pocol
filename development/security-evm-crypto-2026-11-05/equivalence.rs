//! Run the same source against the unchanged delivery and the candidate graphs.
//! Synthetic scalars, messages, bytecode, state, and transactions only.
use pocol_lp4_execution::{
    apply, native, output, receipts, state_trie::StateTrie, txsign, user_phase,
};
use revm::{
    primitives::{
        keccak256, AccountInfo, Address, Authorization, Bytecode, Bytes, ExecutionResult,
        PrimitiveSignature, SignedAuthorization, TxEnv, TxKind, B256, U256,
    },
    InMemoryDB,
};
use serde_json::{json, Value};
use std::io::{self, BufWriter, Write};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn hash(tag: &str, n: u64) -> [u8; 32] {
    keccak256([tag.as_bytes(), &n.to_be_bytes()].concat()).0
}
fn number(n: u8) -> [u8; 32] {
    let mut b = [0; 32];
    b[31] = n;
    b
}
fn previous(mut n: [u8; 32]) -> [u8; 32] {
    for b in n.iter_mut().rev() {
        let (next, borrow) = b.overflowing_sub(1);
        *b = next;
        if !borrow {
            break;
        }
    }
    n
}
fn next(mut n: [u8; 32]) -> [u8; 32] {
    for b in n.iter_mut().rev() {
        let (v, carry) = b.overflowing_add(1);
        *b = v;
        if !carry {
            break;
        }
    }
    n
}
fn authorization(id: u64) -> Authorization {
    Authorization {
        chain_id: U256::from(777902u64),
        address: Address::from_slice(&hash("public-authorization", id)[12..]),
        nonce: id,
    }
}
fn precompile(message: &[u8; 32], signature: &[u8], gas: u64) -> String {
    let mut input = [0; 128];
    input[..32].copy_from_slice(message);
    if signature.len() == 65 {
        input[63] = signature[64].wrapping_add(27);
        input[64..].copy_from_slice(&signature[..64]);
    }
    match revm_precompile::secp256k1::ec_recover_run(&Bytes::copy_from_slice(&input), gas) {
        Ok(result) => format!("{}:{}", result.gas_used, hex(&result.bytes)),
        Err(error) => format!("error:{error:?}"),
    }
}
fn crypto_case(out: &mut impl Write, id: u64, message: [u8; 32], signature: &[u8], class: &str) {
    let node = match pocold::hashes::recover_address(&message, signature) {
        Ok(address) => format!("ok:{}", hex(&address)),
        Err(error) => format!("error:{error:?}"),
    };
    let execution = txsign::recover(&B256::from(message), signature).map(|a| hex(a.as_slice()));
    let (primitive, authority) = if signature.len() == 65 {
        let r = U256::from_be_slice(&signature[..32]);
        let s = U256::from_be_slice(&signature[32..64]);
        let primitive = PrimitiveSignature::new(r, s, signature[64] & 1 == 1)
            .recover_address_from_prehash(&B256::from(message))
            .ok()
            .map(|a| hex(a.as_slice()));
        let signed = SignedAuthorization::new_unchecked(authorization(id), signature[64], r, s);
        let authority = signed.recover_authority().ok().map(|a| hex(a.as_slice()));
        (primitive, authority)
    } else {
        (None, None)
    };
    let precompile: Vec<_> = [0, 2999, 3000, 3001, 100_000]
        .into_iter()
        .map(|gas| precompile(&message, signature, gas))
        .collect();
    writeln!(
        out,
        "{}",
        json!({"id":id,"class":class,"message":hex(&message),"signature":hex(signature),
        "node":node,"execution":execution,"primitive":primitive,"authority":authority,
        "ecrecover":precompile})
    )
    .unwrap();
}
fn crypto(out: &mut impl Write) {
    let mut id = 0u64;
    for i in 0..5000u64 {
        let key = txsign::Key::devnet(&format!("public-evm-compat-{i}"));
        let message = authorization(id).signature_hash().0;
        let signature = key.sign_hash(&B256::from(message));
        assert_eq!(
            authorization(id)
                .clone()
                .into_signed(PrimitiveSignature::new(
                    U256::from_be_slice(&signature[..32]),
                    U256::from_be_slice(&signature[32..64]),
                    signature[64] == 1
                ))
                .recover_authority()
                .unwrap(),
            key.address()
        );
        crypto_case(out, id, message, &signature, "honest-authorization");
        id += 1;
    }
    for i in 0..50_000u64 {
        let mut signature = [0; 65];
        signature[..32].copy_from_slice(&hash("public-r", i));
        signature[32..64].copy_from_slice(&hash("public-s", i));
        if i & 1 == 0 {
            signature[32] &= 0x7f;
        }
        signature[64] = (i % 4) as u8;
        crypto_case(
            out,
            id,
            hash("public-message", i),
            &signature,
            "arbitrary-r-s-v",
        );
        id += 1;
    }
    let order = pocold::hashes::N_ORDER;
    let half = pocold::hashes::HALF_N;
    for r in [
        [0; 32],
        number(1),
        number(2),
        previous(order),
        order,
        next(order),
        [0xff; 32],
    ] {
        for s in [
            [0; 32],
            number(1),
            half,
            next(half),
            previous(order),
            order,
            [0xff; 32],
        ] {
            for message in [
                [0; 32],
                number(1),
                previous(order),
                order,
                next(order),
                [0xff; 32],
                hash("edge", 0),
            ] {
                for v in 0..4 {
                    let mut signature = [0; 65];
                    signature[..32].copy_from_slice(&r);
                    signature[32..64].copy_from_slice(&s);
                    signature[64] = v;
                    crypto_case(out, id, message, &signature, "boundaries");
                    id += 1;
                }
            }
        }
    }
    for length in [0, 1, 64, 66] {
        crypto_case(out, id, [0; 32], &vec![0; length], "bad-length");
        id += 1;
    }
    eprintln!("crypto cases: {id}");
}
fn account(db: &mut InMemoryDB, address: Address, balance: U256, code: &[u8]) {
    let code = Bytecode::new_raw(Bytes::copy_from_slice(code));
    db.insert_account_info(
        address,
        AccountInfo::new(balance, 1, code.hash_slow(), code),
    );
}
fn state_diff(state: &revm::primitives::EvmState) -> Vec<Value> {
    let mut rows:Vec<_>=state.iter().map(|(address,account)|{
        let mut slots:Vec<_>=account.storage.iter().map(|(slot,v)|
            (format!("{slot:064x}"),format!("{:064x}",v.present_value()))).collect();slots.sort();
        (hex(address.as_slice()),json!({"balance":account.info.balance.to_string(),"nonce":account.info.nonce,
            "code_hash":hex(account.info.code_hash.as_slice()),"status":format!("{:?}",account.status),"slots":slots}))
    }).collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.into_iter()
        .map(|(address, state)| json!({"address":address,"state":state}))
        .collect()
}
fn evm(out: &mut impl Write) {
    let caller = Address::from([0x11; 20]);
    let target = Address::from([0x22; 20]);
    let key = txsign::Key::devnet("public-precompile-evm-fixture");
    for id in 0..4096u64 {
        let mode = id % 16;
        let byte = (id / 16) as u8;
        let mut code = match mode {
            0 => vec![0],
            1 => vec![
                0x60, byte, 0x60, 7, 1, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3,
            ],
            2 => vec![0x60, byte, 0x60, 3, 0x55, 0],
            3 => vec![0x60, 3, 0x54, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3],
            4 => vec![
                0x60, byte, 0x60, 0, 0x52, 0x60, 9, 0x60, 32, 0x60, 0, 0xa1, 0,
            ],
            5 => vec![0x60, byte, 0x60, 3, 0x55, 0x60, 0, 0x60, 0, 0xfd],
            6 => vec![0xfe],
            7 => vec![
                0x46, 0x60, 0, 0x52, 0x44, 0x60, 32, 0x52, 0x60, 64, 0x60, 0, 0xf3,
            ],
            8 => vec![
                0x60, byte, 0x60, 0, 0x5d, 0x60, 0, 0x5c, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3,
            ],
            9 => vec![
                0x60, byte, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0x60, 32, 0x5e, 0x60, 32, 0x60, 32,
                0xf3,
            ],
            10 => vec![
                0x60, 32, 0x60, 0, 0x60, 0, 0x37, 0x60, 32, 0x60, 0, 0x20, 0x60, 0, 0x52, 0x60, 32,
                0x60, 0, 0xf3,
            ],
            11 => vec![0x60, 0x33, 0xff],
            12 => vec![0x60, 0, 0x60, 0, 0xf3],
            13 => vec![0x60, 0, 0x35, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3],
            14 => vec![
                0x60, 0, 0x60, 0, 0x60, 0, 0x60, 0, 0x60, 0, 0x60, 2, 0x61, 0xff, 0xff, 0xf1, 0x50,
                0,
            ],
            _ => vec![0x60, 0, 0x40, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3],
        };
        let mut db = InMemoryDB::default();
        account(&mut db, caller, U256::from(10u64.pow(18)), &[]);
        account(&mut db, target, U256::from(100_000), &code);
        db.insert_account_storage(target, U256::from(3), U256::from(id + 1))
            .unwrap();
        db.block_hashes
            .insert(U256::ZERO, B256::from(hash("cached-block", 0)));
        let gas = [
            21_000, 21_001, 21_100, 23_000, 43_111, 43_112, 100_000, 3_000_000,
        ][(id / 16 % 8) as usize];
        let mut data = hash("calldata", id).to_vec();
        let transact_to = if mode == 12 {
            code = vec![0x60, 1, 0x60, 0, 0x53, 0x60, 1, 0x60, 0, 0xf3];
            data = code;
            TxKind::Create
        } else if mode == 13 {
            let message = hash("evm-ecrecover", id);
            let signature = key.sign_hash(&B256::from(message));
            data = vec![0; 128];
            data[..32].copy_from_slice(&message);
            data[63] = signature[64] + 27;
            data[64..].copy_from_slice(&signature[..64]);
            TxKind::Call(Address::from_word(U256::from(1).into()))
        } else {
            TxKind::Call(target)
        };
        let tx = TxEnv {
            caller,
            transact_to,
            data: Bytes::from(data),
            gas_limit: gas,
            gas_price: U256::from(id % 3),
            value: if id % 71 == 0 {
                U256::MAX
            } else {
                U256::from(id % 7)
            },
            nonce: Some(1),
            chain_id: Some(if id % 67 == 0 { 777903 } else { 777902 }),
            ..Default::default()
        };
        let block = native::NativeBlockContext {
            height: U256::from(100 + id),
            timestamp: U256::from(123456 + id),
            gas_limit: U256::from(30_000_000),
            base_fee: U256::ZERO,
            prevrandao: B256::from(hash("randao", id)),
        };
        let before = apply::state_root(&db).unwrap();
        let record = match native::execute_in_context(&db, tx, &block) {
            Err(error) => {
                json!({"id":id,"mode":mode,"error":format!("{error:?}"),"root":hex(before.as_slice())})
            }
            Ok(result) => {
                let status = match &result.result {
                    ExecutionResult::Success { reason, .. } => format!("success:{reason:?}"),
                    ExecutionResult::Revert { .. } => "revert".into(),
                    ExecutionResult::Halt { reason, .. } => format!("halt:{reason:?}"),
                };
                let gas_used = result.result.gas_used();
                let logs = result.result.logs();
                let gas_refunded = match &result.result {
                    ExecutionResult::Success { gas_refunded, .. } => *gas_refunded,
                    _ => 0,
                };
                let encoded =
                    receipts::encode(2, result.result.is_success(), gas_used, logs).unwrap();
                let receipt_root = receipts::ordered_root(&[encoded.clone()], 1).unwrap();
                let diff = state_diff(&result.state);
                let bytes = hex(output(&result.result));
                user_phase::commit_tx(&mut db, result.state, false);
                let after = apply::state_root(&db).unwrap();
                assert_eq!(after, StateTrie::build(&db).root());
                json!({"id":id,"mode":mode,"status":status,"gas_used":gas_used,"gas_refunded":gas_refunded,"output":bytes,
                    "state_diff":diff,"before_root":hex(before.as_slice()),"after_root":hex(after.as_slice()),
                    "receipt":hex(&encoded),"receipt_root":hex(receipt_root.as_slice())})
            }
        };
        writeln!(out, "{record}").unwrap();
    }
    eprintln!("EVM execution cases: 4096");
}
fn main() {
    let mode = std::env::args().nth(1).expect("crypto or evm");
    let mut out = BufWriter::new(io::stdout());
    match mode.as_str() {
        "crypto" => crypto(&mut out),
        "evm" => evm(&mut out),
        _ => panic!("unknown mode"),
    };
    out.flush().unwrap();
}
