//! Bounded deterministic parser mutation smoke test. No sockets or chain state.
//! This supplements, rather than replaces, coverage-guided fuzzing.
use pocold::net::{Msg, ADDR_MAX};
use std::net::{IpAddr, SocketAddr};

fn check(bytes: &[u8]) {
    // A panic fails the test. Accepted frames must have exactly one canonical encoding.
    if let Ok(message) = Msg::decode(bytes) {
        assert_eq!(message.encode(), bytes);
        assert_eq!(Msg::decode(&message.encode()), Ok(message));
    }
}

#[test]
fn bounded_wire_parser_mutations_do_not_panic_or_accept_noncanonical_frames() {
    // Peer discovery: the 11-item Hello, GetAddr, and a full Addr mixing IPv4 (sent mapped) and IPv6 records.
    let addr = |i: usize| if i % 2 == 0 { SocketAddr::new(IpAddr::from([81, 2, 69, i as u8]), 30300) }
        else { SocketAddr::new(IpAddr::from([0x2606, 0x4700, 0, 0, 0, 0, 0, i as u16]), 1024 + i as u16) };
    let seeds = [
        Msg::Hello { chain_id: 777900, genesis: [1; 32], head: [2; 32],
            height: 1, work: [3; 32], node: "local-test".into(), protocol: 2,
            compatibility: [4; 32], listen_port: 30300, nonce: 0x0102030405060708 },
        Msg::GetAddr,
        Msg::Addr { addrs: (0..ADDR_MAX).map(addr).collect() },
        Msg::Block { header: vec![0xc0], body: vec![0xc0] },
        Msg::Template { shell: vec![0xc0], body: vec![0xc0] },
        Msg::Share { tid: [5; 32], nonce: u64::MAX },
        Msg::GetBlock { hash: [6; 32] },
        Msg::GetRange { start: 0, count: 512 },
        Msg::Tx { raw: vec![2, 0xc0] },
        Msg::GetFrom { locator: vec![[7; 32]; 32], count: 512 },
    ].map(|m| m.encode());
    let mut state = 0x504f434f4c_u64;
    let mut next = || {
        state ^= state << 13; state ^= state >> 7; state ^= state << 17; state
    };
    for seed in &seeds {
        assert!(Msg::decode(seed).is_ok());
        for end in 0..seed.len() { check(&seed[..end]); }
    }
    for i in 0..20000 {
        let mut bytes = seeds[i % seeds.len()].clone();
        for _ in 0..1 + next() % 8 {
            let at = next() as usize % bytes.len();
            bytes[at] = next() as u8;
        }
        check(&bytes);
    }
    // Exercise long-form RLP lengths and nesting with a fixed allocation bound.
    for len in [0, 1, 55, 56, 255, 256, 4096] {
        for marker in [0x80, 0xb7, 0xb8, 0xbf, 0xc0, 0xf7, 0xf8, 0xff] {
            check(&vec![marker; len]);
        }
    }
}
