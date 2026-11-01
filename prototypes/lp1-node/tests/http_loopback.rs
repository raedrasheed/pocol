//! In-process transport tests over a real 127.0.0.1 socket (the process-level end-to-end run is
//! tests/e2e_http.py, which root executes against the built binary).

mod common;

use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use lp1_node::http::{self, HttpConfig};
use lp1_node::rpc::Router;

fn start(max_requests: u64, conn_timeout_ms: u64, empty: bool) -> (u16, thread::JoinHandle<(u64, &'static str)>) {
    let node: &'static lp1_node::verify::Node = Box::leak(Box::new(lp1_node::verify::load_node(&common::dir(), empty).unwrap()));
    let cfg = HttpConfig { port: 0, max_requests: Some(max_requests), max_runtime_ms: Some(60_000), conn_timeout_ms };
    let listener: TcpListener = http::bind(&cfg).unwrap();
    let port = listener.local_addr().unwrap().port();
    assert!(listener.local_addr().unwrap().ip().is_loopback());
    let h = thread::spawn(move || {
        let router = Router::new(node.profile.chain_id, &node.chain);
        http::serve(&router, &listener, &cfg, &mut std::io::sink()).unwrap()
    });
    (port, h)
}

fn exchange(port: u16, raw: &[u8]) -> String {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    let _ = s.write_all(raw);
    let mut out = Vec::new();
    let _ = s.read_to_end(&mut out);
    String::from_utf8_lossy(&out).into_owned()
}

fn post(port: u16, body: &str) -> String {
    exchange(port, format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes())
}

fn status(resp: &str) -> u16 {
    resp.get(9..12).and_then(|s| s.parse().ok()).unwrap_or(0)
}

#[test]
fn transport_positive_negative_and_shutdown() {
    let big_head = format!("POST / HTTP/1.1\r\nX-Fill: {}\r\nContent-Length: 2\r\n\r\n{{}}", "a".repeat(9000));
    let many_lines = format!("POST / HTTP/1.1\r\n{}Content-Length: 2\r\n\r\n{{}}", "X-A: 1\r\n".repeat(33));
    let deep = format!("{}{}", "[".repeat(17), "]".repeat(17));
    let mut plan: Vec<(Vec<u8>, &str)> = vec![
        (post_bytes(r#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber"}"#), "\"result\":\"0x14\""),
        (post_bytes(r#"{"jsonrpc":"2.0","id":2,"method":"pocol_getHeaders","params":["0x14","0x2"]}"#), "beyondHead"),
        (post_bytes(r#"{"jsonrpc":"2.0","id":3,"method":"eth_sendRawTransaction","params":["0x00"]}"#), "-32601"),
        (post_bytes(r#"{"jsonrpc":"2.0","id":4,"id":4,"method":"eth_chainId"}"#), "duplicateKey"),
        (post_bytes(&deep), "\"depth\""),
        (b"GET / HTTP/1.1\r\nContent-Length: 0\r\n\r\n".to_vec(), "HTTP/1.1 405"),
        (b"POST /x HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}".to_vec(), "HTTP/1.1 404"),
        (b"POST / HTTP/1.1\r\nHost: a\r\n\r\n".to_vec(), "HTTP/1.1 411"),
        (b"POST / HTTP/1.1\r\nContent-Length: 4097\r\n\r\n".to_vec(), "HTTP/1.1 413"),
        (b"POST / HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}".to_vec(), "HTTP/1.1 400"),
        (b"POST / HTTP/1.1\r\nContent-Length: +2\r\n\r\n{}".to_vec(), "HTTP/1.1 400"),
        (b"POST / HTTP/1.1\r\nContent-Length: 1\r\n\r\n{}".to_vec(), "excessBytes"),
        (b"POST / HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n\r\n".to_vec(), "HTTP/1.1 501"),
        (b"POST / HTTP/1.1\r\nContent-Type: text/plain\r\nContent-Length: 2\r\n\r\n{}".to_vec(), "HTTP/1.1 415"),
        (b"POST / HTTP/2.0\r\nContent-Length: 2\r\n\r\n{}".to_vec(), "HTTP/1.1 505"),
        (big_head.into_bytes(), "HTTP/1.1 431"),
        (many_lines.into_bytes(), "HTTP/1.1 431"),
        (b"POST / HTTP/1.1\r\nContent-Length: 3\r\n\r\n\"\xff\"".to_vec(), "\"utf8\""),
    ];
    let total = plan.len() as u64 + 3;
    let (port, h) = start(total, 300, false);
    for (raw, want) in plan.drain(..) {
        let resp = exchange(port, &raw);
        assert!(resp.contains(want), "want {want} in {resp}");
        assert!(resp.contains("Connection: close"), "{resp}");
    }
    // Client disconnect mid-head, then a stalled client hitting the per-connection deadline.
    {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.write_all(b"POST / HTTP/1.1\r\nContent-Le").unwrap();
        s.shutdown(Shutdown::Both).unwrap();
    }
    {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        s.write_all(b"POST / HTTP/1.1\r\nContent-Length: 10\r\n\r\n{").unwrap();
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        assert_eq!(status(&String::from_utf8_lossy(&out)), 408);
    }
    // Recovery: the next normal request is served.
    assert!(post(port, r#"{"jsonrpc":"2.0","id":9,"method":"eth_chainId"}"#).contains("\"result\":\"0xbdb2a\""));
    let (served, why) = h.join().unwrap();
    assert_eq!((served, why), (total, "maxRequests"));
}

fn post_bytes(body: &str) -> Vec<u8> {
    format!("POST / HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).into_bytes()
}

#[test]
fn empty_chain_transport() {
    let (port, h) = start(2, 1000, true);
    assert!(post(port, r#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber"}"#).contains("\"result\":\"0x0\""));
    assert!(post(port, r#"{"jsonrpc":"2.0","id":2,"method":"pocol_getHeaders","params":["0x1","0x1"]}"#).contains("fromAboveHead"));
    assert_eq!(h.join().unwrap(), (2, "maxRequests"));
}
