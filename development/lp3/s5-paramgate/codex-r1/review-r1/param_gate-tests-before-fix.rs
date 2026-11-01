//! Partial S5 arithmetic acceptance only. These tests do not claim a full gate.
use lp1_node::param_gate::{registry_metrics, DeriveError, RpcLimits};

#[test]
fn pg_l_and_end_registry_derivations() {
    let n = registry_metrics(2200, 2048, 16, 256, 2, 16384, 128, 2).unwrap();
    assert_eq!((n.l_max, n.records_required, n.close_blocks_required), (4113, 12578, 147));
    let end = registry_metrics(2200, 2048, 16, 1024, 0, 1024, 8, 2).unwrap();
    assert_eq!((end.l_max, end.records_required, end.close_blocks_required), (4113, 1024, 147));
    assert_eq!(registry_metrics(2200, 2048, 16, 1024, 2, 1024, 8, 2).unwrap().records_required, 13346);
    assert_eq!(registry_metrics(0, 0, 0, 1, 0, 0, 1, 0).unwrap().close_blocks_required, 1);
    assert_eq!(registry_metrics(0, 0, 0, 1, 0, 0, 0, 0), Err(DeriveError::ZeroRetirementRate));
    let edge = registry_metrics(u32::MAX, u32::MAX, u16::MAX, u16::MAX, u8::MAX, u32::MAX, 1, u32::MAX).unwrap();
    assert_eq!(edge.l_max, 2 * u32::MAX as u128 + u16::MAX as u128 + 1);
    assert_eq!(edge.close_blocks_required, 2 * u32::MAX as u128 + u16::MAX as u128 + 1);
}

#[test]
fn exact_default_rpc_derivations() {
    let r = RpcLimits::default();
    let m = r.derive().unwrap();
    assert_eq!(m.memory_required, 178 * 1024 * 1024);
    assert_eq!(m.batch_bytes_hard, 16782400);
    assert_eq!(m.batch_send_max.exact_seconds().as_deref(), Some("258.0791015625"));
    assert_eq!(m.batch_send_watchdog_max.exact_seconds().as_deref(), Some("258.2791015625"));
    assert_eq!(m.batch_wall_min.exact_seconds().as_deref(), Some("404.2791015625"));
    assert_eq!(r.send_max(8 * 1024 * 1024).unwrap().exact_seconds().as_deref(), Some("130.2"));
    assert_eq!(r.send_max(512 * 1024).unwrap().exact_seconds().as_deref(), Some("10.2"));
    assert!(!m.batch_wall_min.at_most_ms(404279));
    assert!(m.batch_wall_min.at_most_ms(404280));
    assert!(r.satisfies_r26());
}

#[test]
fn all_named_pg_rpc_rejections_and_405_acceptance() {
    const M: u64 = 1024 * 1024;
    let mutations: &[(&str, fn(&mut RpcLimits))] = &[
        ("heavy-memory", |r| r.heavy_resp_mem = 4 * M),
        ("light-memory", |r| r.light_resp_mem = 8 * M),
        ("connections", |r| r.conn_max = 64),
        ("logs-count", |r| r.logs_count_max = 500),
        ("light-response", |r| r.light_resp_max = 256 * 1024),
        ("wall-400", |r| r.batch_wall_ms = 400000),
        ("wall-404", |r| r.batch_wall_ms = 404000),
        ("bucket", |r| r.bucket_cap = 10),
        ("error-size", |r| r.err_item_max = 4096),
        ("slack-zero", |r| r.enforce_slack_ms = 0),
        ("slack-600", |r| r.enforce_slack_ms = 600),
        ("epsilon-zero", |r| r.timer_eps_ms = 0),
        ("epsilon-200", |r| r.timer_eps_ms = 200),
        ("socket", |r| r.send_sock_buf = 128 * 1024),
    ];
    for (name, change) in mutations {
        let mut r = RpcLimits::default();
        change(&mut r);
        assert!(!r.satisfies_r26(), "{}", name);
    }
    let mut r = RpcLimits::default();
    r.batch_wall_ms = 405000;
    assert!(r.satisfies_r26());
}

#[test]
fn each_r26_predicate_has_an_isolated_negative_control() {
    let changes: &[fn(&mut RpcLimits)] = &[
        |r| r.heavy_resp_mem = r.resp_max - 1,
        |r| r.light_resp_mem -= 1,
        |r| r.mem_total = 178 * 1024 * 1024 - 1,
        |r| r.batch_resp_max = r.resp_max - 1,
        |r| r.light_queue = r.conn_max - 1,
        |r| r.err_item_max = 4096,
        |r| r.resp_max = 2 * 32768 + 546 * 512 - 1,
        |r| r.logs_count_max = 545,
        |r| r.logs_scan_bytes = 32767,
        |r| r.light_resp_max = 385 * 1024 - 1,
        |r| r.batch_wall_ms = 404279,
        |r| r.rate = 0,
        |r| r.bucket_cap = r.batch_max - 1,
        |r| r.timer_eps_ms = 0,
        |r| r.timer_eps_ms = r.enforce_slack_ms,
        |r| r.enforce_slack_ms = 501,
        |r| r.send_sock_buf = 65537,
        |r| r.min_send_rate = 0,
    ];
    for (i, change) in changes.iter().enumerate() {
        let mut r = RpcLimits::default();
        change(&mut r);
        assert!(!r.satisfies_r26(), "predicate {}", i);
    }
}

#[test]
fn checked_overflow_and_repeating_rationals_never_authorize() {
    let mut r = RpcLimits::default();
    r.min_send_rate = 0;
    assert_eq!(r.derive(), Err(DeriveError::ZeroSendRate));
    r.min_send_rate = 3;
    assert_eq!(r.send_max(1).unwrap().exact_seconds(), None);
    let v = r.send_max(1).unwrap();
    assert_eq!(v.ratio(), (7600, 3));
    assert!(!v.at_most_ms(2533));
    assert!(v.at_most_ms(2534));
    assert_eq!(r.send_max(u128::MAX), Err(DeriveError::Overflow));
    r.conn_max = u64::MAX;
    r.req_buf = u64::MAX;
    r.conn_buf = u64::MAX;
    assert_eq!(r.derive(), Err(DeriveError::Overflow));
    assert!(!r.satisfies_r26());
    let mut r = RpcLimits::default();
    r.batch_max = u64::MAX;
    r.queue_wait_ms = u64::MAX;
    r.min_send_rate = u64::MAX;
    assert_eq!(r.derive(), Err(DeriveError::Overflow));
    assert!(!r.satisfies_r26());
}
