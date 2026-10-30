use lp1_node::finality_core::*;
#[test]
fn cf1_cf2_triggers_and_sticky_identity() {
    assert_eq!(closing(None, 99, 999, 100, 1000), None);
    assert_eq!(closing(None, 100, 999, 100, 1000), Some(100));
    assert_eq!(closing(None, 99, 1000, 100, 1000), Some(99));
    assert_eq!(closing(Some(40), 99, 999, 100, 1000), Some(40));
}
#[test]
fn cf3_cursor_bounds_checked() {
    for (c, n, r, w) in [(40, 128, 128, 40), (40, 129, 128, 41), (553, 1024, 8, 680), (40, 0, 128, 40), (u64::MAX, 1, 1, u64::MAX)] {
        assert_eq!(cursor_bound(c, n, r), Ok(w));
    }
    assert_eq!(cursor_bound(40, 128, 0), Err("ret"));
    assert_eq!(cursor_bound(u64::MAX, 2, 1), Err("overflow"));
}
#[test]
fn cf4_gate2_boundaries() {
    assert_eq!(gate2(697, 9979, 700, 10000, 2, 10), Ok(false));
    assert_eq!(gate2(698, 9979, 700, 10000, 2, 10), Ok(true));
    assert_eq!(gate2(697, 9980, 700, 10000, 2, 10), Ok(true));
    assert_eq!(gate2(0, 0, 0, 0, 1, 1), Err("threshold"));
}
#[test]
fn cf5_cf6_each_conjunct_and_u64() {
    let p = ParentFacts { closing: true, close_h: 40, cursor_done: true, is_final: false };
    assert!(final_predicate(p, 42, 1000, 2, 700, 1000));
    assert!(final_predicate(p, 700, 0, 2, 700, 1000));
    assert!(!final_predicate(ParentFacts { closing: false, ..p }, 42, 1000, 2, 700, 1000));
    assert!(!final_predicate(ParentFacts { cursor_done: false, ..p }, 42, 1000, 2, 700, 1000));
    assert!(!final_predicate(p, 41, 1000, 2, 700, 1000));
    assert!(!final_predicate(p, 42, 999, 2, 700, 1000));
    let q = ParentFacts { close_h: u64::MAX, ..p };
    assert!(final_predicate(q, u64::MAX, u64::MAX, 0, u64::MAX, u64::MAX));
    assert!(!final_predicate(q, u64::MAX, u64::MAX, 1, u64::MAX, u64::MAX));
    assert!(!child_allowed(true, 700, 700));
    assert!(!child_allowed(false, 701, 700));
    assert!(child_allowed(false, 700, 700));
}
