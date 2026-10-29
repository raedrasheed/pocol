//! Hex helpers. Fixture hex is accepted in either case; RPC quantities are handled in `rpc`.

pub fn encode(b: &[u8]) -> String {
    const D: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(b.len() * 2);
    for x in b {
        s.push(D[(x >> 4) as usize] as char);
        s.push(D[(x & 15) as usize] as char);
    }
    s
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Even-length hex without prefix.
pub fn decode(s: &str) -> Option<Vec<u8>> {
    let b = s.as_bytes();
    if b.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(b.len() / 2);
    for pair in b.chunks(2) {
        out.push(nibble(pair[0])? << 4 | nibble(pair[1])?);
    }
    Some(out)
}

pub fn decode_fixed<const N: usize>(s: &str) -> Option<[u8; N]> {
    let v = decode(s)?;
    if v.len() != N {
        return None;
    }
    let mut out = [0u8; N];
    out.copy_from_slice(&v);
    Some(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trip() {
        assert_eq!(super::encode(&[0, 0xab, 0x10]), "00ab10");
        assert_eq!(super::decode("00AB10"), Some(vec![0, 0xab, 0x10]));
        assert_eq!(super::decode("0"), None);
        assert_eq!(super::decode("zz"), None);
        assert_eq!(super::decode_fixed::<2>("abcd"), Some([0xab, 0xcd]));
        assert_eq!(super::decode_fixed::<2>("ab"), None);
    }
}
