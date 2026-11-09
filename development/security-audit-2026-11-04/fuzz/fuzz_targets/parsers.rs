#![no_main]
use libfuzzer_sys::fuzz_target;

// Pure decoders only: no sockets, persistent state, signing keys or running chain.
fuzz_target!(|input: &[u8]| {
    let Some((&kind, data)) = input.split_first() else { return; };
    match kind % 9 {
        0 => if let Ok(m) = pocold::net::Msg::decode(data) { assert_eq!(m.encode(), data); },
        1 => { let _ = pocold::rlp::decode(data); },
        2 => if let Ok(h) = pocold::types::decode_header(data) { assert_eq!(h.encode(), data); },
        3 => if let Ok(b) = pocold::types::Body::decode(data, 16384) { assert_eq!(b.encode(), data); },
        4 => if let Ok(g) = pocold::genesis_spec::decode(data) { assert_eq!(pocold::genesis_spec::encode(&g), data); },
        5 => if let Ok(tx) = pocol_lp4_execution::wire::decode_signed(data, 16384) { assert_eq!(tx.raw(), data); },
        6 => { let _ = pocold::launcher::Invitation::parse(data); },
        8 => { let _ = pocold::addrbook::parse(data, 777900, &[0; 32], true, 0); },
        _ => if let Ok(text) = std::str::from_utf8(data) { let _ = pocold::types::unhex(text); },
    }
});
