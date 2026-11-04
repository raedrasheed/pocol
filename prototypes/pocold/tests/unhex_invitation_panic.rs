//! SECURITY PROOF (defensive, in-session, synthetic) for the lp1-node-gap finding that confirms
//! scan F4's invitation vector: pocold::types::unhex byte-slices a &str, so a non-ASCII char at an
//! even byte index (even total length) cuts mid-character and panics. This is the decoder reached
//! from the genesis/invitation path (lib.rs:215 load_spec_for on genesis.spec.hex; also
//! network_display / launcher invitation describe), i.e. an attacker-supplied invitation blob can
//! abort the node's boot/import/preview path. Synthetic input only.

use pocold::types::unhex;

#[test]
fn unhex_valid_and_odd_are_graceful() {
    assert!(unhex("0x00ff").is_ok(), "valid hex decodes");
    assert!(unhex("00ff").is_ok(), "no-prefix valid hex decodes");
    assert!(unhex("0x0").is_err(), "odd length is a graceful Err, not a panic");
    assert!(unhex("0xzz").is_err(), "non-hex ascii is a graceful Err, not a panic");
}

#[test]
fn unhex_panics_on_non_ascii_genesis_string() {
    // "00\u{20AC}0" after 0x-strip = bytes 30 30 E2 82 AC 30 (len 6, even -> passes the %2 check).
    // s[2..4] falls inside the 3-byte '\u{20AC}' -> Rust aborts: "byte index is not a char boundary".
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // silence the backtrace noise for this expected panic
    let caught = std::panic::catch_unwind(|| unhex("0x00\u{20AC}0"));
    std::panic::set_hook(prev);
    eprintln!("unhex(\"0x00<U+20AC>0\") panicked = {}", caught.is_err());
    assert!(caught.is_err(), "a non-ASCII char at an even index panics unhex (DoS on the invitation/boot path)");
}
