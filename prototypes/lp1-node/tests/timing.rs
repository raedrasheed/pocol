use lp1_node::timing::{self, AttemptWindow as W, TimingError as E, TIMESTAMP_LIMIT};

#[test]
fn network_and_p11_derived_bounds() {
    let n = timing::metrics(5, 10, 3, 26, 25);
    assert_eq!((n.sigma, n.x_h, n.x_w), (14, 22, 36));
    let p = timing::metrics(1, 2, 1, 8, 6);
    assert_eq!((p.sigma, p.x_h, p.x_w), (4, 6, 10));
    let m = u64::MAX as u128;
    let huge = timing::metrics(u64::MAX, u64::MAX, u64::MAX, u64::MAX, u64::MAX);
    assert_eq!((huge.sigma, huge.x_h, huge.x_w), (2 * m + 1, 5 * m, 3 * m));
}

#[test]
fn inclusive_windows_fallback_and_proposer_edges() {
    let w = W::new(1000, 0, 300).unwrap();
    assert_eq!((w.lower(), w.upper()), (1001, 1300));
    for (ts, ok) in [(1000, false), (1001, true), (1300, true), (1301, false)] {
        assert_eq!(w.contains(ts), ok);
    }
    assert_eq!(w.fallback_timestamp(25), Ok(1026));
    assert_eq!(w.fallback_timestamp(299), Ok(1300));
    assert_eq!(w.fallback_timestamp(300), Err(E::OutsideWindow));
    assert_eq!(w.proposer_timestamp(999), Ok(1001));
    assert_eq!(w.proposer_timestamp(1300), Ok(1300));
    assert_eq!(w.proposer_timestamp(1301), Err(E::OutsideWindow));
    assert!(!w.later_fallback_due(u64::MAX));
    let later = W::new(1000, 1, 300).unwrap();
    assert_eq!((later.lower(), later.upper()), (1301, 1600));
    assert_eq!(later.fallback_timestamp(u32::MAX), Ok(1301));
    assert!(!later.later_fallback_due(1300));
    assert!(later.later_fallback_due(1301));
    assert_eq!(later.proposer_timestamp(1301), Err(E::NotFirstAttempt));
    assert_eq!(later.deadline_ms(0, 0, 0), Err(E::NotFirstAttempt));
}

#[test]
fn exact_millisecond_deadline_and_strict_attempt_transition() {
    let w = W::new(1000, 0, 300).unwrap();
    let tj = w.deadline_ms(1_020_123, 26, 25).unwrap();
    assert_eq!(tj, 1_046_123);
    assert!(timing::arrives_by_deadline(tj - 1, tj));
    assert!(timing::arrives_by_deadline(tj, tj));
    assert!(!timing::arrives_by_deadline(tj + 1, tj));
    assert_eq!(w.deadline_ms(0, 26, 25), Ok(1_026_000));
    assert_eq!(w.deadline_ms(u128::MAX, 1, 0), Err(E::MillisecondOverflow));
    assert_eq!(w.deadline_ms(u128::MAX, 0, 0), Ok(u128::MAX));
    assert!(!w.advance_due(1330, 30));
    assert!(w.advance_due(1331, 30));
    assert!(!w.advance_due(u64::MAX, u64::MAX));
}

#[test]
fn timestamp_limit_and_wire_attempt_limits() {
    let top = (TIMESTAMP_LIMIT - 1) as u64;
    assert_eq!(W::new(0, 0, 0), Err(E::ZeroAttemptDuration));
    assert_eq!(timing::max_attempt(0, 0), Err(E::ZeroAttemptDuration));
    assert_eq!(W::new(top, 0, 1), Err(E::TimestampLimit));
    assert_eq!(W::new(top - 1, 0, 1).unwrap().upper(), top);
    assert_eq!(timing::max_attempt(top, 1), Ok(None));
    assert_eq!(timing::max_attempt(top - 1, 1), Ok(Some(0)));
    assert_eq!(timing::max_attempt(u64::MAX, 1), Err(E::TimestampLimit));
    assert_eq!(W::new(u64::MAX, u32::MAX, u32::MAX), Err(E::TimestampLimit));
    assert_eq!(timing::max_attempt(0, 1), Ok(Some(u32::MAX)));
    let a = timing::max_attempt(0, u32::MAX).unwrap().unwrap();
    assert!(W::new(0, a, u32::MAX).is_ok());
    assert_eq!(W::new(0, a + 1, u32::MAX), Err(E::TimestampLimit));
}

#[test]
fn future_equality_and_u64_overflow_edges() {
    assert!(!timing::is_future(1010, 1000, 10));
    assert!(timing::is_future(1011, 1000, 10));
    assert!(!timing::is_future(u64::MAX, u64::MAX, u64::MAX));
    assert!(timing::is_future(u64::MAX, 0, u64::MAX - 1));
}

#[test]
fn seeded_adversarial_windows_never_wrap_or_panic() {
    let mut seed = 0x533320261017u64;
    for _ in 0..10000 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let parent = seed;
        let a = (seed >> 32) as u32;
        let d = seed as u32;
        let r = W::new(parent, a, d);
        let raw_upper = parent as u128 + (a as u128 + 1) * d as u128;
        assert_eq!(r.is_ok(), d != 0 && raw_upper < TIMESTAMP_LIMIT);
        if let Ok(w) = r {
            assert!(w.contains(w.lower()) && w.contains(w.upper()));
            assert!(!w.contains(w.lower() - 1));
            assert!(!w.contains(w.upper() + 1));
            assert!(a <= timing::max_attempt(parent, d).unwrap().unwrap());
        }
    }
}
