//! Regression for the historical F4 invitation panic. The old decoder byte-sliced a &str,
//! so a non-ASCII char at an even byte index could cut mid-character and panic. The decoder is reached
//! from the genesis/invitation path (lib.rs:215 load_spec_for on genesis.spec.hex; also
//! network_display / launcher invitation describe), i.e. an attacker-supplied invitation blob can
//! abort the node's boot/import/preview path. The current decoder must reject it. Synthetic input only.

use pocold::types::unhex;

#[test]
fn unhex_valid_and_odd_are_graceful() {
    assert!(unhex("0x00ff").is_ok(), "valid hex decodes");
    assert!(unhex("00ff").is_ok(), "no-prefix valid hex decodes");
    assert!(unhex("0x0").is_err(), "odd length is a graceful Err, not a panic");
    assert!(unhex("0xzz").is_err(), "non-hex ascii is a graceful Err, not a panic");
}

#[test]
fn unhex_rejects_the_historical_invitation_panic_input() {
    // "00\u{20AC}0" after 0x-strip = bytes 30 30 E2 82 AC 30 (len 6, even -> passes the %2 check).
    // s[2..4] falls inside the 3-byte '\u{20AC}' -> Rust aborts: "byte index is not a char boundary".
    let input = "0x00\u{20AC}0";
    // Positive control: this exact input still splits a character in the historical slicer.
    assert!(std::panic::catch_unwind(|| &input[2..][2..4]).is_err());
    let caught = std::panic::catch_unwind(|| unhex(input));
    let result = caught.expect("the current decoder must not panic on an invitation");
    assert!(result.is_err(), "the malformed invitation must be rejected, not accepted");
}
