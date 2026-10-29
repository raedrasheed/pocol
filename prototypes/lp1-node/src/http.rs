//! Bounded std-only HTTP/1.1 transport for the local experiment.
//!
//! Binds 127.0.0.1 only (ephemeral port by default). Serial: one connection at a time, one request
//! per connection, `Connection: close` after the response. Bounded request head (8192 bytes,
//! 32 header lines) and body (4096 bytes); POST to "/" only; exactly one decimal Content-Length;
//! Transfer-Encoding refused; extra bytes after the body refused; per-connection deadline.
//!
//! NOT implemented (no M7 claim): hyper / socket2, RpcGuard pacing and fairness, S1 reservation
//! accounting, keep-alive, concurrent connections, resource budgets beyond the fixed limits above,
//! signal-driven graceful shutdown (shutdown is via --max-requests / --max-runtime-ms or process kill).

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::thread;
use std::time::{Duration, Instant};

use crate::json;
use crate::rpc::Router;

pub const MAX_HEAD: usize = 8192;
pub const MAX_HEADER_LINES: usize = 32;
pub const MAX_BODY: usize = 4096;

#[derive(Clone, Debug)]
pub struct HttpConfig {
    pub port: u16,
    pub max_requests: Option<u64>,
    pub max_runtime_ms: Option<u64>,
    pub conn_timeout_ms: u64,
}

/// Only the literal IPv4 loopback address 127.0.0.1 is accepted.
pub fn check_bind(addr: &str) -> Result<Ipv4Addr, String> {
    if addr == "127.0.0.1" {
        Ok(Ipv4Addr::LOCALHOST)
    } else {
        Err(format!("refusing to bind {addr}: only 127.0.0.1 is allowed"))
    }
}

pub fn bind(cfg: &HttpConfig) -> io::Result<TcpListener> {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, cfg.port)))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnOutcome {
    Responded(u16),
    Disconnected,
    TimedOut,
}

impl ConnOutcome {
    pub fn label(&self) -> &'static str {
        match self {
            ConnOutcome::Responded(_) => "responded",
            ConnOutcome::Disconnected => "disconnected",
            ConnOutcome::TimedOut => "timedOut",
        }
    }
}

enum ReqErr {
    Disconnect,
    Timeout,
    Status(u16, &'static str),
}

fn status_text(code: u16) -> &'static str {
    match code {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        411 => "Length Required",
        413 => "Payload Too Large",
        415 => "Unsupported Media Type",
        431 => "Request Header Fields Too Large",
        501 => "Not Implemented",
        505 => "HTTP Version Not Supported",
        _ => "Error",
    }
}

fn find_crlfcrlf(b: &[u8]) -> Option<usize> {
    b.windows(4).position(|w| w == b"\r\n\r\n")
}

/// One read bounded by the connection deadline.
fn read_some(s: &mut TcpStream, buf: &mut [u8], deadline: Instant) -> Result<usize, ReqErr> {
    let now = Instant::now();
    if now >= deadline {
        return Err(ReqErr::Timeout);
    }
    let left = deadline - now;
    let _ = s.set_read_timeout(Some(left.max(Duration::from_millis(1))));
    match s.read(buf) {
        Ok(0) => Err(ReqErr::Disconnect),
        Ok(n) => Ok(n),
        Err(e) if e.kind() == io::ErrorKind::WouldBlock || e.kind() == io::ErrorKind::TimedOut => Err(ReqErr::Timeout),
        Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(0),
        Err(_) => Err(ReqErr::Disconnect),
    }
}

fn is_token_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c)
}

/// Reads and validates one request; returns the body.
fn read_request(s: &mut TcpStream, deadline: Instant) -> Result<Vec<u8>, ReqErr> {
    let mut buf: Vec<u8> = Vec::with_capacity(1024);
    let mut chunk = [0u8; 1024];
    let head_end = loop {
        if let Some(p) = find_crlfcrlf(&buf) {
            break p;
        }
        if buf.len() >= MAX_HEAD {
            return Err(ReqErr::Status(431, "headTooLarge"));
        }
        let n = read_some(s, &mut chunk, deadline)?;
        buf.extend_from_slice(&chunk[..n]);
    };
    if head_end + 4 > MAX_HEAD {
        return Err(ReqErr::Status(431, "headTooLarge"));
    }
    let head = &buf[..head_end];
    if head.iter().any(|c| *c >= 0x80 || (*c < 0x20 && *c != b'\r' && *c != b'\n' && *c != b'\t')) {
        return Err(ReqErr::Status(400, "headBytes"));
    }
    let text = std::str::from_utf8(head).map_err(|_| ReqErr::Status(400, "headBytes"))?;
    let mut lines = text.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = request_line.split(' ').collect();
    if parts.len() != 3 {
        return Err(ReqErr::Status(400, "requestLine"));
    }
    if parts[2] != "HTTP/1.1" && parts[2] != "HTTP/1.0" {
        return Err(ReqErr::Status(505, "version"));
    }
    if parts[0] != "POST" {
        return Err(ReqErr::Status(405, "method"));
    }
    if parts[1] != "/" {
        return Err(ReqErr::Status(404, "target"));
    }
    let mut content_length: Option<usize> = None;
    let mut n_lines = 0usize;
    for line in lines {
        n_lines += 1;
        if n_lines > MAX_HEADER_LINES {
            return Err(ReqErr::Status(431, "headerCount"));
        }
        if line.is_empty() || line.starts_with(' ') || line.starts_with('\t') || line.contains('\n') || line.contains('\r') {
            return Err(ReqErr::Status(400, "headerLine"));
        }
        let (name, value) = match line.split_once(':') {
            Some(x) => x,
            None => return Err(ReqErr::Status(400, "headerLine")),
        };
        if name.is_empty() || !name.bytes().all(is_token_char) {
            return Err(ReqErr::Status(400, "headerName"));
        }
        let value = value.trim_matches(|c: char| c == ' ' || c == '\t');
        let lname = name.to_ascii_lowercase();
        match lname.as_str() {
            "content-length" => {
                if content_length.is_some() {
                    return Err(ReqErr::Status(400, "contentLengthRepeated"));
                }
                if value.is_empty() || value.len() > 10 || !value.bytes().all(|c| c.is_ascii_digit()) {
                    return Err(ReqErr::Status(400, "contentLength"));
                }
                let n: u64 = value.parse().map_err(|_| ReqErr::Status(400, "contentLength"))?;
                if n > MAX_BODY as u64 {
                    return Err(ReqErr::Status(413, "bodyTooLarge"));
                }
                content_length = Some(n as usize);
            }
            "transfer-encoding" => return Err(ReqErr::Status(501, "transferEncoding")),
            "content-type" => {
                let base = value.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
                if base != "application/json" {
                    return Err(ReqErr::Status(415, "contentType"));
                }
            }
            _ => {}
        }
    }
    let cl = match content_length {
        Some(n) => n,
        None => return Err(ReqErr::Status(411, "contentLengthMissing")),
    };
    let mut body: Vec<u8> = buf[head_end + 4..].to_vec();
    if body.len() > cl {
        return Err(ReqErr::Status(400, "excessBytes"));
    }
    while body.len() < cl {
        let n = read_some(s, &mut chunk, deadline)?;
        body.extend_from_slice(&chunk[..n]);
        if body.len() > cl {
            return Err(ReqErr::Status(400, "excessBytes"));
        }
    }
    Ok(body)
}

fn write_response(s: &mut TcpStream, code: u16, body: &str) {
    let msg =
        format!("HTTP/1.1 {code} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", status_text(code), body.len());
    let _ = s.set_write_timeout(Some(Duration::from_millis(2000)));
    let _ = s.write_all(msg.as_bytes());
    let _ = s.flush();
    let _ = s.shutdown(std::net::Shutdown::Write);
}

/// After an early error response, read and discard a bounded amount so closing does not reset the
/// connection before the client has read the response.
fn drain(s: &mut TcpStream) {
    let _ = s.set_read_timeout(Some(Duration::from_millis(200)));
    let mut chunk = [0u8; 1024];
    let mut total = 0usize;
    let until = Instant::now() + Duration::from_millis(500);
    while total < 65536 && Instant::now() < until {
        match s.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => total += n,
        }
    }
}

/// Serves exactly one request on an accepted connection, then closes it. Never panics on input.
pub fn handle_conn(router: &Router<'_>, mut s: TcpStream, conn_timeout: Duration) -> ConnOutcome {
    let _ = s.set_nonblocking(false);
    let deadline = Instant::now() + conn_timeout;
    let out = match read_request(&mut s, deadline) {
        Ok(body) => {
            let resp = router.handle(&body);
            write_response(&mut s, 200, &resp);
            ConnOutcome::Responded(200)
        }
        Err(ReqErr::Status(code, why)) => {
            write_response(&mut s, code, &format!("{{\"error\":{}}}", json::quote(why)));
            drain(&mut s);
            ConnOutcome::Responded(code)
        }
        Err(ReqErr::Timeout) => {
            write_response(&mut s, 408, "{\"error\":\"timeout\"}");
            drain(&mut s);
            ConnOutcome::TimedOut
        }
        Err(ReqErr::Disconnect) => ConnOutcome::Disconnected,
    };
    drop(s);
    out
}

/// Accept loop. Returns (connections handled, stop reason).
pub fn serve(router: &Router<'_>, listener: &TcpListener, cfg: &HttpConfig, log: &mut dyn Write) -> io::Result<(u64, &'static str)> {
    listener.set_nonblocking(true)?;
    let start = Instant::now();
    let conn_timeout = Duration::from_millis(cfg.conn_timeout_ms.max(1));
    let mut served = 0u64;
    loop {
        if let Some(m) = cfg.max_requests {
            if served >= m {
                return Ok((served, "maxRequests"));
            }
        }
        if let Some(rt) = cfg.max_runtime_ms {
            if start.elapsed() >= Duration::from_millis(rt) {
                return Ok((served, "maxRuntime"));
            }
        }
        match listener.accept() {
            Ok((stream, peer)) => {
                served += 1;
                if !peer.ip().is_loopback() {
                    drop(stream);
                    let _ = writeln!(log, "{{\"event\":\"request\",\"n\":{served},\"outcome\":\"refusedNonLoopbackPeer\"}}");
                    continue;
                }
                let out = handle_conn(router, stream, conn_timeout);
                let status = match out {
                    ConnOutcome::Responded(c) => c,
                    ConnOutcome::TimedOut => 408,
                    ConnOutcome::Disconnected => 0,
                };
                let _ = writeln!(log, "{{\"event\":\"request\",\"n\":{served},\"outcome\":\"{}\",\"status\":{status}}}", out.label());
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(5)),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => {
                let _ = writeln!(log, "{{\"event\":\"acceptError\",\"error\":{}}}", json::quote(&e.to_string()));
                thread::sleep(Duration::from_millis(5));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bind_policy() {
        assert!(check_bind("127.0.0.1").is_ok());
        for bad in ["0.0.0.0", "::", "::1", "127.0.0.2", "192.168.1.1", "localhost", "", "127.0.0.1:80"] {
            assert!(check_bind(bad).is_err(), "{bad}");
        }
    }
}
