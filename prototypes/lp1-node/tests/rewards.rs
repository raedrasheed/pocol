use lp1_node::fixed::U256;
use lp1_node::rewards::*;
fn row(n: u16, count: u16, tomb: bool, account_exists: bool) -> ShareInput {
    let mut id = [0; 20];
    id[18..].copy_from_slice(&n.to_be_bytes());
    ShareInput { id, count, tomb, account_exists }
}
fn amount(n: u64) -> U256 {
    U256::from_u64(n)
}
#[test]
fn sv1_sv2_sv3_integer_rounding() {
    let v = shares(&amount(64000), &[row(1, 1, false, true)], 2, 32).unwrap();
    assert_eq!(v.amount, amount(1000));
    assert_eq!(v.base_burn, amount(63000));
    let rows: Vec<_> = (1..=65).map(|i| row(i, 1, false, true)).collect();
    let v = shares(&amount(101), &rows, 2, 32).unwrap();
    assert_eq!(v.amount, amount(101));
    assert_eq!(v.paid, amount(65));
    assert_eq!(v.round_burn, amount(36));
    let v = shares(&amount(1000), &[row(1, 3, false, true), row(2, 2, false, true)], 2, 32).unwrap();
    assert_eq!(v.payments[0].1, amount(46));
    assert_eq!(v.payments[1].1, amount(31));
    assert_eq!(v.round_burn, amount(1));
    assert_eq!(v.base_burn, amount(922));
}
#[test]
fn sv4_sv5_routes_only_sv6_state_is_deferred() {
    for (tomb, exists, penalty, pay) in [(true, true, 100, 0), (false, false, 0, 100)] {
        let v = shares(&amount(6400), &[row(1, 1, false, true), row(2, 1, tomb, exists)], 2, 32).unwrap();
        assert_eq!(v.paid, amount(100));
        assert_eq!(v.penalty_burn, amount(penalty));
        assert_eq!(v.pay_burn, amount(pay));
        assert_eq!(v.base_burn, amount(6200));
    }
    // SV6's numeric identity is trivial: invalid pot 5000 is burned in full.
    // Actual invalid-pot state transition is LP4, not fabricated as an executed Escrow test.
}
#[test]
fn sv7_and_sv7s_parts_and_conservation() {
    for (pot, winner, proposer, remainder, share, base, paid) in
        [(80000, 8000, 4000, 68000, 1062, 66938, 13062), (164000, 16400, 8200, 139400, 2178, 137222, 26778)]
    {
        let split = split(&amount(pot), 1000, 500).unwrap();
        assert_eq!(split.winner, amount(winner));
        assert_eq!(split.proposer, amount(proposer));
        assert_eq!(split.remainder, amount(remainder));
        let v = shares(&split.remainder, &[row(1, 1, false, true)], 2, 32).unwrap();
        assert_eq!(v.paid, amount(share));
        assert_eq!(v.base_burn, amount(base));
        assert_eq!(winner + proposer + share, paid);
        assert_eq!(paid + base, pot);
    }
}
#[test]
fn maximal_amounts_counts_and_shape_negatives() {
    let v = shares(&U256::MAX, &[row(1, 257, false, true)], 2, 32).unwrap();
    assert_eq!(v.paid, U256::MAX);
    assert_eq!(v.base_burn, U256::ZERO);
    assert_eq!(v.round_burn, U256::ZERO);
    assert!(shares(&amount(1), &[row(1, 258, false, true)], 2, 32).is_err());
    assert!(shares(&amount(1), &[row(1, 0, false, true)], 2, 32).is_err());
    assert!(shares(&amount(1), &[row(1, 1, false, true), row(1, 1, false, true)], 2, 32).is_err());
    assert!(split(&U256::MAX, 10000, 1).is_err());
}
