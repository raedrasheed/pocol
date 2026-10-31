//! Independent local key generation and public address output. No secret output.
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let result = match a.as_slice() {
        [_, cmd, flag, path] if cmd == "generate" && flag == "--out" => {
            pocold::keyfile::generate(Path::new(path))
        }
        [_, cmd, flag, path] if cmd == "address" && flag == "--key-file" => {
            pocold::keyfile::load(Path::new(path))
        }
        _ => Err("usage: pocol-key generate --out FILE | address --key-file FILE".into()),
    };
    match result {
        Ok(key) => println!("{:#x}", key.address()),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}
