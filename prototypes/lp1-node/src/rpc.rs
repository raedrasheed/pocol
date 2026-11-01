//! Read-only JSON-RPC router over an immutable `FixtureChain`.
//!
//! Served: eth_chainId, eth_blockNumber, pocol_getHeaders (network.md:172-181, F0-F4 in fixed
//! priority). Every other method, including wallet, account, signing, write and submission
//! methods, is -32601 and touches no state (the router only holds a shared reference).
//!
//! Prototype wire convention (LP1, not an M1 amendment):
//!   -32700 "parse"   data {reason: utf8 | json | duplicateKey | depth}          id null
//!   -32600 "request" data {reason: batch | kind | id | jsonrpc | method | member} id null if the id is unusable
//!   -32601 "method"  data {reason: unsupported}
//!   -32602 "params"  data {path: "params" | "params[i]"}  (browser.md / br-messages convention)
//!   -32018 "params"  data {reason: fromZero | countRange | fromAboveHead | beyondHead}
//!                    (as in m1-draft-0.26/vectors/v1-window-cases.json replyError)
//! The id must be present and be a JSON number whose accepted C19 value (IEEE-754 binary64, as
//! JSON.parse) is finite, integral and in 0..=2^32-1; it is echoed normalised ("7.0" -> 7,
//! "-0" -> 0, "1e-400" -> 0). Booleans, strings and null are refused.

use crate::chain::FixtureChain;
use crate::hex;
use crate::json::{self, JVal};

pub const MAX_COUNT: u64 = 512;
pub const SERVED: [&str; 3] = ["eth_chainId", "eth_blockNumber", "pocol_getHeaders"];

pub struct Router<'a> {
    chain_id: u64,
    chain: &'a FixtureChain,
}

/// Canonical quantity: "0x" + 1..16 lowercase hex digits, no leading zero except "0x0".
pub fn parse_qty(s: &str) -> Option<u64> {
    let body = s.strip_prefix("0x")?;
    if body.is_empty() || body.len() > 16 || !body.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) {
        return None;
    }
    if body.len() > 1 && body.starts_with('0') {
        return None;
    }
    u64::from_str_radix(body, 16).ok()
}

pub fn qty(v: u64) -> String {
    format!("0x{v:x}")
}

fn id_text(id: Option<u32>) -> String {
    match id {
        Some(v) => v.to_string(),
        None => "null".to_string(),
    }
}

fn error(id: Option<u32>, code: i64, message: &str, data: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"error\":{{\"code\":{code},\"message\":{},\"data\":{data}}}}}", id_text(id), json::quote(message))
}

fn reason(r: &str) -> String {
    format!("{{\"reason\":{}}}", json::quote(r))
}

fn path(p: &str) -> String {
    format!("{{\"path\":{}}}", json::quote(p))
}

fn result(id: u32, value_json: &str) -> String {
    format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":{value_json}}}")
}

impl<'a> Router<'a> {
    pub fn new(chain_id: u64, chain: &'a FixtureChain) -> Router<'a> {
        Router { chain_id, chain }
    }

    pub fn head(&self) -> u64 {
        self.chain.head()
    }

    /// Handles one request body; always returns one JSON response text.
    pub fn handle(&self, body: &[u8]) -> String {
        let v = match json::parse(body) {
            Ok(v) => v,
            Err(e) => return error(None, -32700, "parse", &reason(e.reason())),
        };
        let members = match &v {
            JVal::Obj(m) => m,
            JVal::Arr(_) => return error(None, -32600, "request", &reason("batch")),
            _ => return error(None, -32600, "request", &reason("kind")),
        };
        let id = match v.get("id") {
            Some(JVal::Num(lex)) => match json::integral_u32(lex) {
                Some(i) => i,
                None => return error(None, -32600, "request", &reason("id")),
            },
            _ => return error(None, -32600, "request", &reason("id")),
        };
        if v.get("jsonrpc") != Some(&JVal::Str("2.0".to_string())) {
            return error(Some(id), -32600, "request", &reason("jsonrpc"));
        }
        let method = match v.get("method") {
            Some(JVal::Str(m)) => m.as_str(),
            _ => return error(Some(id), -32600, "request", &reason("method")),
        };
        if members.iter().any(|(k, _)| !matches!(k.as_str(), "jsonrpc" | "id" | "method" | "params")) {
            return error(Some(id), -32600, "request", &reason("member"));
        }
        let params = v.get("params");
        match method {
            "eth_chainId" => self.no_params(id, params, || qty(self.chain_id)),
            "eth_blockNumber" => self.no_params(id, params, || qty(self.chain.head())),
            "pocol_getHeaders" => self.get_headers(id, params),
            _ => error(Some(id), -32601, "method", &reason("unsupported")),
        }
    }

    fn no_params<F: Fn() -> String>(&self, id: u32, params: Option<&JVal>, f: F) -> String {
        match params {
            None => {}
            Some(JVal::Arr(a)) if a.is_empty() => {}
            _ => return error(Some(id), -32602, "params", &path("params")),
        }
        result(id, &json::quote(&f()))
    }

    fn get_headers(&self, id: u32, params: Option<&JVal>) -> String {
        // F0: two canonical u64 quantities, nothing missing or extra.
        let a = match params {
            Some(JVal::Arr(a)) if a.len() == 2 => a,
            _ => return error(Some(id), -32602, "params", &path("params")),
        };
        let mut q = [0u64; 2];
        for (i, p) in a.iter().enumerate() {
            q[i] = match p {
                JVal::Str(s) => match parse_qty(s) {
                    Some(x) => x,
                    None => return error(Some(id), -32602, "params", &path(&format!("params[{i}]"))),
                },
                _ => return error(Some(id), -32602, "params", &path(&format!("params[{i}]"))),
            };
        }
        let (from, count) = (q[0], q[1]);
        // F1, F2: no state read.
        if from < 1 {
            return error(Some(id), -32018, "params", &reason("fromZero"));
        }
        if count < 1 || count > MAX_COUNT {
            return error(Some(id), -32018, "params", &reason("countRange"));
        }
        // F3, F4: head from this request's snapshot (the chain is immutable).
        let head = self.chain.head();
        if from > head {
            return error(Some(id), -32018, "params", &reason("fromAboveHead"));
        }
        if from as u128 + count as u128 - 1 > head as u128 {
            return error(Some(id), -32018, "params", &reason("beyondHead"));
        }
        match self.chain.headers_rlp(from, count) {
            Some(b) => result(id, &json::quote(&format!("0x{}", hex::encode(&b)))),
            None => error(Some(id), -32603, "internal", &reason("range")),
        }
    }
}

/// Window view source that talks JSON text to a router (the same bytes an HTTP client would send),
/// so the RP checker can be run against this node's own replies.
pub struct RouterSource<'r, 'c> {
    pub router: &'r Router<'c>,
    next_id: u32,
}

impl<'r, 'c> RouterSource<'r, 'c> {
    pub fn new(router: &'r Router<'c>) -> RouterSource<'r, 'c> {
        RouterSource { router, next_id: 1 }
    }

    fn call(&mut self, method: &str, params: &str) -> Option<String> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        let req = format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"method\":{},\"params\":{params}}}", json::quote(method));
        let resp = json::parse(self.router.handle(req.as_bytes()).as_bytes()).ok()?;
        if resp.get("jsonrpc") != Some(&JVal::Str("2.0".into())) || resp.get("id") != Some(&JVal::Num(id.to_string())) {
            return None;
        }
        match resp.get("result") {
            Some(JVal::Str(s)) if resp.get("error").is_none() => Some(s.clone()),
            _ => None,
        }
    }
}

impl<'r, 'c> crate::window::ViewSource for RouterSource<'r, 'c> {
    fn block_number(&mut self) -> Option<u64> {
        self.call("eth_blockNumber", "[]").and_then(|s| parse_qty(&s))
    }

    fn headers(&mut self, from: u64, count: u64) -> Result<Vec<u8>, String> {
        let s = self.call("pocol_getHeaders", &format!("[\"{}\",\"{}\"]", qty(from), qty(count))).ok_or("error reply")?;
        let body = s.strip_prefix("0x").ok_or("result prefix")?;
        hex::decode(body).ok_or_else(|| "result hex".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantities() {
        assert_eq!(parse_qty("0x0"), Some(0));
        assert_eq!(parse_qty("0x1"), Some(1));
        assert_eq!(parse_qty("0xffffffffffffffff"), Some(u64::MAX));
        for bad in ["0x", "0x01", "0X1", "0xA", "1", "0x10000000000000000", "0x-1", " 0x1", "0x1 "] {
            assert_eq!(parse_qty(bad), None, "{bad}");
        }
        assert_eq!(qty(777002), "0xbdb2a");
        assert_eq!(qty(0), "0x0");
    }

    fn call(r: &Router<'_>, s: &str) -> String {
        r.handle(s.as_bytes())
    }

    #[test]
    fn empty_chain_priority() {
        let c = FixtureChain::empty();
        let r = Router::new(777002, &c);
        let rq = |p: &str| call(&r, &format!("{{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"pocol_getHeaders\",\"params\":{p}}}"));
        assert!(rq("[\"0x1\",\"0x1\"]").contains("fromAboveHead"));
        assert!(rq("[\"0x0\",\"0x0\"]").contains("fromZero")); // F1 before F2
        assert!(rq("[\"0x1\",\"0x0\"]").contains("countRange"));
        assert!(rq("[\"0x1\",\"0x201\"]").contains("countRange"));
        assert!(rq("[\"0x01\",\"0x0\"]").contains("\"params[0]\"")); // F0 before F1/F2
        assert!(rq("[\"0x1\",\"0x01\"]").contains("\"params[1]\""));
        assert!(rq("[\"0x1\"]").contains("\"path\":\"params\""));
        assert!(rq("[\"0x1\",\"0x1\",\"0x1\"]").contains("\"path\":\"params\""));
        assert!(rq("[1,\"0x1\"]").contains("\"params[0]\""));
        assert!(rq("{}").contains("\"path\":\"params\""));
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":1,"method":"pocol_getHeaders"}"#).contains("\"path\":\"params\""));
        assert_eq!(call(&r, r#"{"jsonrpc":"2.0","id":2,"method":"eth_blockNumber"}"#), r#"{"jsonrpc":"2.0","id":2,"result":"0x0"}"#);
    }

    #[test]
    fn envelope_rules() {
        let c = FixtureChain::empty();
        let r = Router::new(777002, &c);
        assert_eq!(call(&r, r#"{"jsonrpc":"2.0","id":7.0,"method":"eth_chainId","params":[]}"#), r#"{"jsonrpc":"2.0","id":7,"result":"0xbdb2a"}"#);
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":"7","method":"eth_chainId"}"#).contains("\"id\":null,\"error\":{\"code\":-32600"));
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":true,"method":"eth_chainId"}"#).contains("-32600"));
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":4294967296,"method":"eth_chainId"}"#).contains("-32600"));
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":-1,"method":"eth_chainId"}"#).contains("-32600"));
        assert!(call(&r, r#"{"jsonrpc":"2.0","method":"eth_chainId"}"#).contains("\"reason\":\"id\""));
        assert!(call(&r, r#"{"jsonrpc":"1.0","id":1,"method":"eth_chainId"}"#).contains("\"reason\":\"jsonrpc\""));
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":1,"method":"eth_chainId","x":1}"#).contains("\"reason\":\"member\""));
        assert!(call(&r, r#"[{"jsonrpc":"2.0","id":1,"method":"eth_chainId"}]"#).contains("\"reason\":\"batch\""));
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":1,"id":1,"method":"eth_chainId"}"#).contains("duplicateKey"));
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[1]}"#).contains("-32602"));
        for m in [
            "eth_sendRawTransaction",
            "eth_sendTransaction",
            "eth_accounts",
            "eth_requestAccounts",
            "eth_sign",
            "personal_sign",
            "wallet_addEthereumChain",
            "eth_call",
            "pocol_submit",
            "",
        ] {
            let resp = call(&r, &format!("{{\"jsonrpc\":\"2.0\",\"id\":3,\"method\":\"{m}\",\"params\":[]}}"));
            assert_eq!(resp, r#"{"jsonrpc":"2.0","id":3,"error":{"code":-32601,"message":"method","data":{"reason":"unsupported"}}}"#);
        }
    }

    /// Every row of the 16-row native C19 oracle, sent as the request id over the router.
    #[test]
    fn c19_oracle_ids_over_rpc() {
        let c = FixtureChain::empty();
        let r = Router::new(777002, &c);
        for (lex, want) in json::C19_ORACLE.iter() {
            let resp = call(&r, &format!("{{\"jsonrpc\":\"2.0\",\"id\":{lex},\"method\":\"eth_chainId\"}}"));
            match want {
                Some(id) => assert_eq!(resp, format!("{{\"jsonrpc\":\"2.0\",\"id\":{id},\"result\":\"0xbdb2a\"}}"), "{lex}"),
                None => assert_eq!(resp, r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"request","data":{"reason":"id"}}}"#, "{lex}"),
            }
        }
        // An invalid id is refused even for an otherwise unknown method (id is checked first).
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":2.5,"method":"eth_sendRawTransaction"}"#).contains("\"reason\":\"id\""));
        // A valid normalised id is echoed on errors too.
        assert!(call(&r, r#"{"jsonrpc":"2.0","id":1e-400,"method":"eth_sendRawTransaction"}"#).starts_with("{\"jsonrpc\":\"2.0\",\"id\":0,\"error\""));
    }
}
