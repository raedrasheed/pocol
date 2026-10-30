#[path="../tests/forced_surplus.rs"] #[allow(dead_code,unused_imports)] mod f;
fn main(){println!("{}",serde_json::to_string(&f::cases()).unwrap());}
