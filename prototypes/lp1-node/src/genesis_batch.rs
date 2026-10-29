//! `lp1-node genesis-decode`: GenesisSpec v1 decode of one hex input per line, for differential
//! checks. The input is read as a stream; it is never loaded whole.
//!
//! Line format: optional `0x`, hex digits, optional `\r`, then `\n` (the last line may omit it). An
//! empty line is the empty input. Output, one JSON row per line and one summary row:
//! - accepted: `{"line","ok":true,"genesisHash","chainId","m0","reencodeEqual","bootable":false}`;
//! - rejected by the decoder: `{"line","ok":false,"code","detail"}` with the gs* / L0 code;
//! - not decoded by this tool: `{"line","ok":false,"refused":"inputTooLarge"|"hex",...}`.
//!
//! Resource limits. They belong to this tool and are never GenesisSpec validity rules: a refused
//! line gets no gs* code and no verdict.
//! - One input is at most `MAX_SPEC_BYTES` bytes, the largest encoding that can pass decode and
//!   ParamGate R12 (|M_0List| <= M_max <= 65535). Every input that could be a valid GenesisSpec fits.
//!   A longer line is consumed in the reader's own chunks without being stored, reported as
//!   `inputTooLarge`, and the next line is processed.
//! - The line buffer (`MAX_LINE_BYTES`) and the byte buffer (`MAX_SPEC_BYTES`) are reserved once, with
//!   `try_reserve_exact`; if that fails the command stops with an error rather than aborting. They are
//!   reused for every line, so memory does not grow with the number of lines or the file size.
//! - Per input, the decoder adds its framing stack (one usize per open list, grown by doubling), the
//!   decoded result and, for an accepted input, its re-encoding; for a gsVersion rejection, the
//!   decimal detail (about 2.41 digits per byte) and its O(n) conversion workspace. Time is linear
//!   except that conversion, O(n^1.59 log n). `tests/genesis_batch_alloc.rs` measures the peak heap
//!   for the largest shapes and asserts bounds; `tests/lp3_genesis_cli.py` runs the binary under an
//!   OS address-space limit.

use std::collections::TryReserveError;
use std::io::{self, BufRead, Write};

use crate::genesis_spec::{self, MAX_SPEC_BYTES};
use crate::{hex, json};

/// `0x`, the hex digits of the largest input, `\r` and `\n`.
pub const MAX_LINE_BYTES: usize = 2 * MAX_SPEC_BYTES + 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Summary {
    pub inputs: usize,
    pub accepted: usize,
    pub rejected: usize,
    pub refused: usize,
    pub reencode_equal: bool,
}

impl Summary {
    /// Every line was decoded or rejected by the decoder, and every accepted input re-encoded exactly.
    pub fn ok(&self) -> bool {
        self.refused == 0 && self.reencode_equal
    }
}

fn reserve(v: &mut Vec<u8>, n: usize, what: &str) -> Result<(), String> {
    v.try_reserve_exact(n).map_err(|e: TryReserveError| format!("cannot reserve the {what} buffer of {n} bytes: {e}"))
}

/// Discards the rest of the current line through the reader's own buffer; returns the bytes skipped
/// before the newline.
fn skip_line(input: &mut dyn BufRead) -> io::Result<usize> {
    let mut skipped = 0;
    loop {
        let buf = input.fill_buf()?;
        if buf.is_empty() {
            return Ok(skipped);
        }
        match buf.iter().position(|x| *x == b'\n') {
            Some(p) => {
                input.consume(p + 1);
                return Ok(skipped + p);
            }
            None => {
                let n = buf.len();
                input.consume(n);
                skipped += n;
            }
        }
    }
}

fn nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// Even-length hex into `dst` (cleared first), without reallocating a reserved buffer.
fn hex_into(src: &[u8], dst: &mut Vec<u8>) -> bool {
    dst.clear();
    if src.len() % 2 != 0 {
        return false;
    }
    for pair in src.chunks(2) {
        match (nibble(pair[0]), nibble(pair[1])) {
            (Some(h), Some(l)) => dst.push(h << 4 | l),
            _ => return false,
        }
    }
    true
}

/// Runs the batch. `Err` is an I/O or reservation failure; decoder rejections and refused lines are
/// rows and are counted in the summary.
pub fn run(input: &mut dyn BufRead, out: &mut dyn Write) -> Result<Summary, String> {
    let mut line = Vec::new();
    reserve(&mut line, MAX_LINE_BYTES, "line")?;
    let mut bytes = Vec::new();
    reserve(&mut bytes, MAX_SPEC_BYTES, "input")?;
    let io = |e: io::Error| e.to_string();
    let mut s = Summary { inputs: 0, accepted: 0, rejected: 0, refused: 0, reencode_equal: true };
    loop {
        line.clear();
        let reader: &mut dyn BufRead = &mut *input;
        let n = io::Read::take(reader, MAX_LINE_BYTES as u64).read_until(b'\n', &mut line).map_err(io)?;
        if n == 0 {
            break;
        }
        s.inputs += 1;
        let no = s.inputs;
        let mut text = &line[..];
        let complete = text.last() == Some(&b'\n');
        if complete {
            text = &text[..text.len() - 1];
        } else if line.len() == MAX_LINE_BYTES {
            let total = line.len() + skip_line(input).map_err(io)?;
            s.refused += 1;
            writeln!(out, "{{\"line\":{no},\"ok\":false,\"refused\":\"inputTooLarge\",\"lineBytes\":{total},\"maxSpecBytes\":{MAX_SPEC_BYTES}}}")
                .map_err(io)?;
            continue;
        }
        if text.last() == Some(&b'\r') {
            text = &text[..text.len() - 1];
        }
        if text.starts_with(b"0x") {
            text = &text[2..];
        }
        if text.len() > 2 * MAX_SPEC_BYTES {
            s.refused += 1;
            writeln!(out, "{{\"line\":{no},\"ok\":false,\"refused\":\"inputTooLarge\",\"lineBytes\":{},\"maxSpecBytes\":{MAX_SPEC_BYTES}}}", text.len())
                .map_err(io)?;
            continue;
        }
        if !hex_into(text, &mut bytes) {
            s.refused += 1;
            writeln!(out, "{{\"line\":{no},\"ok\":false,\"refused\":\"hex\"}}").map_err(io)?;
            continue;
        }
        match genesis_spec::decode(&bytes) {
            Ok(spec) => {
                s.accepted += 1;
                let same = genesis_spec::encode(&spec) == bytes;
                s.reencode_equal &= same;
                writeln!(
                    out,
                    "{{\"line\":{no},\"ok\":true,\"genesisHash\":\"0x{}\",\"chainId\":{},\"m0\":{},\"reencodeEqual\":{same},\"bootable\":false}}",
                    hex::encode(&genesis_spec::genesis_hash(&bytes)),
                    spec.chain_id,
                    spec.m0_list.len()
                )
                .map_err(io)?;
            }
            Err(e) => {
                s.rejected += 1;
                writeln!(out, "{{\"line\":{no},\"ok\":false,\"code\":\"{}\",\"detail\":{}}}", e.code, json::quote(&e.detail)).map_err(io)?;
            }
        }
    }
    writeln!(
        out,
        "{{\"check\":\"genesis-decode\",\"inputs\":{},\"accepted\":{},\"rejected\":{},\"refused\":{},\"reencodeEqual\":{},\"maxSpecBytes\":{MAX_SPEC_BYTES}}}",
        s.inputs, s.accepted, s.rejected, s.refused, s.reencode_equal
    )
    .map_err(io)?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(input: &[u8]) -> (Summary, Vec<String>) {
        let mut out = Vec::new();
        let s = run(&mut &input[..], &mut out).unwrap();
        (s, String::from_utf8(out).unwrap().lines().map(String::from).collect())
    }

    #[test]
    fn line_forms_and_refusals() {
        let (s, r) = rows(b"c0\n\n0xc10a\r\nzz\nc");
        assert_eq!(s, Summary { inputs: 5, accepted: 0, rejected: 3, refused: 2, reencode_equal: true });
        assert!(!s.ok());
        assert_eq!(r[0], r#"{"line":1,"ok":false,"code":"gsCount","detail":"top 0"}"#);
        assert_eq!(r[1], r#"{"line":2,"ok":false,"code":"L0","detail":"truncated"}"#);
        assert_eq!(r[2], r#"{"line":3,"ok":false,"code":"gsVersion","detail":"10"}"#);
        assert_eq!(r[3], r#"{"line":4,"ok":false,"refused":"hex"}"#);
        assert_eq!(r[4], r#"{"line":5,"ok":false,"refused":"hex"}"#);
        assert!(r[5].starts_with(r#"{"check":"genesis-decode","inputs":5,"accepted":0,"rejected":3,"refused":2,"#));
        let (s, _) = rows(b"");
        assert_eq!(s.inputs, 0);
        assert!(s.ok());
    }

    #[test]
    fn limits_are_consistent() {
        assert_eq!(MAX_LINE_BYTES, 2 * MAX_SPEC_BYTES + 4);
        let mut v = Vec::new();
        assert!(hex_into(b"00ABff", &mut v) && v == [0, 0xab, 0xff]);
        assert!(!hex_into(b"0", &mut v) && !hex_into(b"0g", &mut v));
    }
}
