//! Protocol-level controls for integer edges, independent of upstream zero-bit
//! signed integer unit-test assumptions. No private data or network is used.
use pocol_lp4_execution::{output, Sandbox, CHAIN_ID};
use revm::{
    primitives::{AccountInfo, Address, Bytecode, Bytes, U256},
    InMemoryDB,
};

#[test]
fn cancun_signed_edges_and_zero_divisors_match_the_evm_specification() {
    let mut minimum = [0; 32];
    minimum[0] = 0x80;
    let minus_one = [0xff; 32];
    let mut minus_ten = minus_one;
    minus_ten[31] = 0xf6;
    let mut minus_three = minus_one;
    minus_three[31] = 0xfd;
    let mut three = [0; 32];
    three[31] = 3;
    let mut five = [0; 32];
    five[31] = 5;
    for (name, opcode, numerator, divisor, expected) in [
        ("SDIV minimum/-1", 0x05, minimum, minus_one, minimum),
        ("SDIV -10/3", 0x05, minus_ten, three, minus_three),
        ("DIV by zero", 0x04, five, [0; 32], [0; 32]),
        ("SMOD by zero", 0x07, five, [0; 32], [0; 32]),
    ] {
        let mut code = Vec::new();
        for word in [divisor, numerator] {
            code.push(0x7f); // PUSH32
            code.extend(word);
        }
        code.extend([opcode, 0x60, 0, 0x52, 0x60, 32, 0x60, 0, 0xf3]);
        let caller = Address::from([0x11; 20]);
        let target = Address::from([0x22; 20]);
        let mut db = InMemoryDB::default();
        db.insert_account_info(caller, AccountInfo::from_balance(U256::from(1_000_000u64)));
        let code = Bytecode::new_raw(Bytes::from(code));
        db.insert_account_info(
            target,
            AccountInfo::new(U256::ZERO, 0, code.hash_slow(), code),
        );
        let sandbox = Sandbox { snapshot: db };
        let result = sandbox.execute(caller, target, 100_000, CHAIN_ID).unwrap();
        assert!(result.result.is_success(), "{name}: {:?}", result.result);
        assert_eq!(output(&result.result), expected, "{name}");
    }
}
