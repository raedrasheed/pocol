use lp1_node::asert::{asert, Early};
use lp1_node::fixed::{U256, U512};
#[test]
fn as1_through_as19_actual_internal_traces_and_targets() {
    let tg = U256::pow2(240).unwrap();
    let data: [(i128, i128, i128, i128, i128, u16, Option<u64>); 13] = [
        (0, 0, 10, 600, 0, 0, Some(65536)),
        (700, 10, 10, 600, 65536, 0, Some(65536)),
        (400, 100, 10, 600, -65536, 0, Some(65536)),
        (400, 10, 10, 600, 32768, 32768, Some(92674)),
        (700, 100, 10, 600, -32768, 32768, Some(92674)),
        (11, 1, 10, 65536, 1, 1, Some(65537)),
        (9, 1, 10, 65536, -1, 65535, Some(131071)),
        (65545, 1, 10, 65536, 65535, 65535, Some(131071)),
        (34463, 10000, 10, 65536, -65537, 65535, Some(131071)),
        (9, 1, 10, 65537, -1, 65535, Some(131071)),
        (10300, 10, 10, 600, 17 * 65536, 0, Some(65536)),
        (55400, 20000, 10, 600, -241 * 65536, 0, Some(65536)),
        (0, 0, 10, 600, 0, 0, Some(65536)),
    ];
    let scaled = |n, shift| U512::mul_u256_u64(&U256::pow2(shift).unwrap(), n).to_u256().unwrap();
    let targets = [
        tg,
        U256::pow2(241).unwrap(),
        U256::pow2(239).unwrap(),
        scaled(92674, 224),
        scaled(46337, 224),
        scaled(65537, 224),
        scaled(131071, 223),
        scaled(131071, 224),
        scaled(131071, 222),
        scaled(131071, 223),
        U256::MAX,
        U256::ONE,
        U256::MAX,
    ];
    for (i, (ts, h, t, tau, e, f, factor)) in data.iter().copied().enumerate() {
        let target = if i == 12 { U256::MAX } else { tg };
        let o = asert(&target, 0, t, tau, ts, h).unwrap();
        assert_eq!(o.trace.delta_t, ts - t * h, "AS{} dt", i + 1);
        assert_eq!(o.trace.e, e, "AS{} e", i + 1);
        assert_eq!(o.trace.s, e.div_euclid(65536));
        assert_eq!(o.trace.f, f);
        assert_eq!(o.trace.factor, factor);
        assert_eq!(o.early, Early::No);
        assert!(o.max_bits <= 512);
        assert_eq!(o.target, targets[i], "AS{} target", i + 1);
    }
    let cases = [
        ((1i128 << 62), 7, 60, 3600, tg, Early::High, U256::MAX),
        (0, u64::MAX as i128, u32::MAX as i128, 1, tg, Early::Low, U256::ONE),
        (153000, 0, 10, 600, U256::ONE, Early::No, U256::pow2(255).unwrap()),
        (153600, 0, 10, 600, U256::ONE, Early::High, U256::MAX),
        (0, 15420, 10, 600, U256::MAX, Early::Low, U256::ONE),
        (0, 15360, 10, 600, U256::MAX, Early::No, U256::ONE),
    ];
    for (i, (ts, h, t, tau, target, early, result)) in cases.iter().copied().enumerate() {
        let o = asert(&target, 0, t, tau, ts, h).unwrap();
        assert_eq!(o.target, result, "AS{}", i + 14);
        assert_eq!(o.early, early);
        assert_eq!(o.trace.delta_t, ts - t * h);
        let e = (ts - t * h) * 65536;
        assert_eq!(o.trace.e, e.div_euclid(tau));
        assert_eq!(o.trace.s, o.trace.e.div_euclid(65536));
        assert_eq!(o.trace.f as i128, o.trace.e - 65536 * o.trace.s);
        assert_eq!(o.trace.factor, if early == Early::No { Some(65536) } else { None });
        assert!(o.max_bits <= 512);
    }
}
