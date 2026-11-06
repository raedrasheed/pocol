//! Replay the full C-backend corpus through the pure Rust ECRECOVER fallback.
//! This scratch package must depend on revm-precompile with default-features=false.
use revm_precompile::{secp256k1::ec_recover_run, Bytes};
use std::io::{self, BufRead, BufWriter, Write};
fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn hex(s: &[u8]) -> String {
    s.iter().map(|b| format!("{b:02x}")).collect()
}
fn main() {
    let input = std::env::args().nth(1).expect("corpus jsonl path");
    let mut out = BufWriter::new(io::stdout());
    let mut count = 0u64;
    for line in io::BufReader::new(std::fs::File::open(input).unwrap()).lines() {
        let row: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        let message = unhex(row["message"].as_str().unwrap());
        let signature = unhex(row["signature"].as_str().unwrap());
        let mut input = [0; 128];
        input[..32].copy_from_slice(&message);
        if signature.len() == 65 {
            input[63] = signature[64].wrapping_add(27);
            input[64..].copy_from_slice(&signature[..64]);
        }
        let results: Vec<_> = [0, 2999, 3000, 3001, 100_000]
            .into_iter()
            .map(
                |gas| match ec_recover_run(&Bytes::copy_from_slice(&input), gas) {
                    Ok(result) => format!("{}:{}", result.gas_used, hex(&result.bytes)),
                    Err(error) => format!("error:{error:?}"),
                },
            )
            .collect();
        assert_eq!(
            serde_json::json!(results),
            row["ecrecover"],
            "C / Rust fallback mismatch at {}",
            row["id"]
        );
        writeln!(
            out,
            "{}",
            serde_json::json!({"id":row["id"],"ecrecover":results})
        )
        .unwrap();
        count += 1;
    }
    out.flush().unwrap();
    eprintln!("fallback cases: {count}");
}
