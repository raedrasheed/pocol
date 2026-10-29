//! Loopback JSON-RPC over HTTP/1.1 (one request per connection). The HTTP
//! threads only parse/forward; every method runs on the consensus thread over
//! an immutable head snapshot. RPC never affects validity (consensus §13).
use std::io::{Read,Write};
use std::net::{SocketAddr,TcpListener,TcpStream};
use std::sync::{Arc,Mutex,atomic::{AtomicBool,AtomicI64,AtomicU8,AtomicUsize,Ordering},mpsc::{self,Sender,SyncSender}};
use std::time::{Duration,Instant};
use serde_json::{json,Value};
use revm::primitives::{Address,B256,Bytes,TxEnv,TxKind,U256,keccak256,ExecutionResult};
use pocol_lp4_execution::{native::{self,NativeBlockContext},wire,apply};
use crate::{node::{Event,Node},types::*,chain::{self,Entry}};

/// RpcGuard REQ_BUF (network.md:221/229): request line, headers and body together; headers alone
/// stop at HEAD_MAX.
pub const REQ_BUF:usize=1024*1024;pub const HEAD_MAX:usize=16384;
/// One socket read. The header buffer is allocated once at HEAD_MAX+READ_CHUNK; the body gets a
/// buffer of exactly Content-Length bytes and its reads stop there (A13 rpc term).
pub const READ_CHUNK:usize=8192;
/// JSON values in one request (members, parameters and keys together). Counted on the raw body
/// before parsing, so a request's parse tree stays within `parse_bytes_max` (A13; local limit,
/// D-A13-RPC). A larger request is -32600 without being parsed.
pub const JSON_VALUES_MAX:usize=4096;
/// RpcGuard RESP_MAX and LOGS_SCAN_BYTES (governance.md): a larger response is replaced by -32020
/// {resultBytes} (network.md:210); eth_getLogs stops after scanning this many log data bytes, or once
/// its result would exceed RESP_MAX or LOGS_COUNT_MAX logs: -32020 {reason, fromBlock,
/// lastCompleteBlock, nextFromBlock} (network.md:298), so it never builds more.
pub const RESP_MAX:usize=8*1024*1024;pub const LOGS_SCAN_BYTES:usize=32*1024*1024;pub const LOGS_COUNT_MAX:usize=10000;
/// Batches (network.md): at most BATCH_MAX members; the member that makes the delivered bytes
/// exceed BATCH_RESP_MAX, and every later one, is answered -32020 {batchBytes} without running.
pub const BATCH_MAX:usize=20;pub const BATCH_RESP_MAX:usize=16*1024*1024;
/// ORIGIN_DENY (governance.md): local mining interfaces are never reachable from a site.
/// eth_call/eth_estimateGas gas cap. It must not exceed the largest per-execution gas of the
/// A13 execution model (`memory::execution_gas`), checked at compile time in resources.rs.
pub const CALL_GAS_MAX:u64=30_000_000;
pub const ORIGIN_DENY:[&str;2]=["pocol_submitShare","pocol_getTemplate"];

/// RpcGuard local service policy (network.md:182-330). These are hypotheses for testing, exposed
/// in pocol_getParams.rpcLimits; none is a validity rule (D87, consensus.md §13). Within pocold's
/// architecture (one consensus thread plus a light lane, HTTP/1.1 one request per connection that
/// closes after its response) some mechanisms are enforced exactly, some by an equivalent or
/// stronger bound, and some are not enforced; development/audit-2026-10-22/RPCGUARD.md has the
/// per-item status, test and mutant. Every field here is configurable so tests reach the bounds.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Limits{
    /// RPC_CONN_MAX: concurrent connections; an excess connection gets 503 and is closed without
    /// being read (network.md:213).
    pub conn_max:usize,
    /// REQ_READ_DEADLINE (whole request: line, headers and body), one small response write, and the
    /// node's answer (the nobody-waits work deadline: a request whose caller already left is not run).
    pub read_deadline:Duration,pub write_deadline:Duration,pub work_deadline:Duration,
    /// HEAVY_QUEUE and LIGHT_QUEUE places, and QUEUE_WAIT (network.md:214, :234): a request is
    /// admitted (the consensus thread takes it with its reservation) within QUEUE_WAIT of the end of
    /// its parsing, or answered -32021 {busy, retryAfterMs}.
    pub heavy_queue:usize,pub light_queue:usize,pub queue_wait:Duration,
    /// The per-connection token bucket (network.md:215-218): rate tokens per second, capacity
    /// bucket_cap. A connection carries one request, which takes k tokens (1, or k batch members)
    /// from a full bucket; k > bucket_cap is -32021 {rate, retryAfterMs}. At the spec values
    /// (20/s, 20 = BATCH_MAX) no request is ever refused (RPCGUARD.md).
    pub rate:u32,pub bucket_cap:u32,
    /// The eth_getLogs/pocol_getLogs working-set budget (network.md:220 SCRATCH, enforced for the log
    /// handlers only; the other handlers are not charged, so SCRATCH itself is reported absent).
    pub scratch_heavy:usize,
    /// COMPUTE_DEADLINE (network.md:235): from admission, including encoding, 5 s heavy, 500 ms light.
    pub compute_heavy:Duration,pub compute_light:Duration,
    /// SendPacer (network.md:237-256): MIN_SEND_RATE B/s, GRACE, ENFORCE_SLACK and TIMER_EPS (the
    /// backstop is PACER_BACKSTOP).
    pub min_send_rate:u64,pub grace:Duration,pub enforce_slack:Duration,pub timer_eps:Duration,
    /// BATCH_WALL (network.md:282): from read-end to sendDone; reaching it closes the connection.
    pub batch_wall:Duration,
}
impl Default for Limits{
    fn default()->Self{Self{conn_max:32,read_deadline:Duration::from_secs(10),write_deadline:Duration::from_secs(10),work_deadline:Duration::from_secs(30),
        heavy_queue:16,light_queue:32,queue_wait:Duration::from_secs(2),rate:20,bucket_cap:20,
        scratch_heavy:16<<20,compute_heavy:Duration::from_secs(5),compute_light:Duration::from_millis(500),
        min_send_rate:65536,grace:Duration::from_secs(2),enforce_slack:Duration::from_millis(200),timer_eps:Duration::from_millis(1),
        batch_wall:Duration::from_secs(420)}}
}
/// SEND_MAX(size) = GRACE + size/R + ENFORCE_SLACK, the longest a response of `size` bytes may hold
/// its reservation before the SendPacer closes (network.md:252). ms, saturating.
pub fn send_max_ms(l:&Limits,size:u64)->u128{
    let r=l.min_send_rate.max(1) as u128;
    l.grace.as_millis()+(size as u128*1000)/r+l.enforce_slack.as_millis()
}
/// Connection and work counters, for status and tests: connections refused at the cap,
/// connection threads alive, requests skipped because their work deadline had passed before
/// the consensus thread reached them, and batch members not run for the same reason.
/// Also: heavy requests refused because no HEAVY_QUEUE place freed within QUEUE_WAIT, requests
/// refused by JSON_VALUES_MAX, and requests answered busy (any cause).
/// Gauges: heavy bodies queued now (all servers of the process), light bodies in the lane, heavy and
/// light requests running; Wake events sent for the light lane, and light requests pushed into it.
/// RpcGuard: requests not admitted within QUEUE_WAIT, light requests refused by a full lane,
/// rate-limited requests, SendPacer closes and the largest pacer lateness (ms), COMPUTE_DEADLINE
/// answers and the largest lateness of one past its deadline (ms), getLogs SCRATCH hits and the
/// largest getLogs working set (bytes), per-block log limits, ORIGIN_DENY answers, sendDone events,
/// the largest send overrun past GRACE+size/R (ms; i64::MIN until a sendDone), BATCH_WALL hits and
/// the largest batch send time and read-end-to-sendDone time (ms); connections refused because their
/// SO_SNDBUF could not be set to SEND_SOCK_BUF (`sockbuf_ok`), and the SendPacer's blocked waits.
#[derive(Debug,Default)]
pub struct RpcCounters{pub refused:AtomicUsize,pub active:AtomicUsize,pub expired:AtomicUsize,pub expired_members:AtomicUsize,pub queue_full:AtomicUsize,pub too_complex:AtomicUsize,pub busy:AtomicUsize,
    pub queued:AtomicUsize,pub light_queued:AtomicUsize,pub heavy_active:AtomicUsize,pub light_active:AtomicUsize,pub wakes:AtomicUsize,pub light_pushed:AtomicUsize,
    pub queue_wait:AtomicUsize,pub light_full:AtomicUsize,pub rate:AtomicUsize,pub pacer_closed:AtomicUsize,pub pacer_late_max_ms:AtomicUsize,pub deadline_hits:AtomicUsize,pub cancel_late_max_ms:AtomicUsize,
    pub scratch_hits:AtomicUsize,pub scratch_heavy_max:AtomicUsize,pub limit_hits:AtomicUsize,pub origin_denied:AtomicUsize,pub send_done:AtomicUsize,pub send_overrun_max_ms:AtomicI64,
    pub batch_wall_hits:AtomicUsize,pub batch_send_max_ms:AtomicUsize,pub batch_wall_max_ms:AtomicUsize,pub sockbuf_refused:AtomicUsize,pub pacer_waits:AtomicUsize}
pub static COUNTERS:RpcCounters=RpcCounters{refused:AtomicUsize::new(0),active:AtomicUsize::new(0),expired:AtomicUsize::new(0),expired_members:AtomicUsize::new(0),
    queue_full:AtomicUsize::new(0),too_complex:AtomicUsize::new(0),busy:AtomicUsize::new(0),queued:AtomicUsize::new(0),light_queued:AtomicUsize::new(0),heavy_active:AtomicUsize::new(0),
    light_active:AtomicUsize::new(0),wakes:AtomicUsize::new(0),light_pushed:AtomicUsize::new(0),queue_wait:AtomicUsize::new(0),light_full:AtomicUsize::new(0),rate:AtomicUsize::new(0),
    pacer_closed:AtomicUsize::new(0),pacer_late_max_ms:AtomicUsize::new(0),deadline_hits:AtomicUsize::new(0),cancel_late_max_ms:AtomicUsize::new(0),scratch_hits:AtomicUsize::new(0),
    scratch_heavy_max:AtomicUsize::new(0),limit_hits:AtomicUsize::new(0),origin_denied:AtomicUsize::new(0),send_done:AtomicUsize::new(0),send_overrun_max_ms:AtomicI64::new(i64::MIN),
    batch_wall_hits:AtomicUsize::new(0),batch_send_max_ms:AtomicUsize::new(0),batch_wall_max_ms:AtomicUsize::new(0),sockbuf_refused:AtomicUsize::new(0),pacer_waits:AtomicUsize::new(0)};
/// SEND_SOCK_BUF (RpcGuard SO_SNDBUF): the kernel send buffer of every RPC connection. Set explicitly
/// it also stops the kernel from growing it (Linux locks it against autotuning; Windows stops its
/// ideal-send-backlog tuning), so an unread answer holds at most this much kernel memory beyond the
/// client's receive window, and the SendPacer's accepted count A stays close to what the client read.
/// Windows run 3 (a39379b) showed the default: a 14 MiB unread answer taken by the kernel within a
/// second. Linux doubles the value it is given (bookkeeping) and reads back 2·SEND_SOCK_BUF.
pub const SEND_SOCK_BUF:usize=32*1024;
/// SO_SNDBUF through the platform's setsockopt/getsockopt (std has neither).
pub mod sockbuf{
    use std::net::TcpStream;
    #[cfg(unix)] pub fn set(s:&TcpStream,n:usize)->std::io::Result<()>{
        use std::os::unix::io::AsRawFd;let v=n as libc::c_int;
        let r=unsafe{libc::setsockopt(s.as_raw_fd(),libc::SOL_SOCKET,libc::SO_SNDBUF,&v as *const libc::c_int as *const libc::c_void,std::mem::size_of::<libc::c_int>() as libc::socklen_t)};
        if r==0{Ok(())}else{Err(std::io::Error::last_os_error())}
    }
    #[cfg(unix)] pub fn get(s:&TcpStream)->std::io::Result<usize>{
        use std::os::unix::io::AsRawFd;let mut v:libc::c_int=0;let mut len=std::mem::size_of::<libc::c_int>() as libc::socklen_t;
        let r=unsafe{libc::getsockopt(s.as_raw_fd(),libc::SOL_SOCKET,libc::SO_SNDBUF,&mut v as *mut libc::c_int as *mut libc::c_void,&mut len)};
        if r==0{Ok(v.max(0) as usize)}else{Err(std::io::Error::last_os_error())}
    }
    #[cfg(windows)] pub fn set(s:&TcpStream,n:usize)->std::io::Result<()>{
        use std::os::windows::io::AsRawSocket;use windows_sys::Win32::Networking::WinSock::{setsockopt,SOL_SOCKET,SO_SNDBUF};let v=n as i32;
        let r=unsafe{setsockopt(s.as_raw_socket() as usize,SOL_SOCKET,SO_SNDBUF,&v as *const i32 as *const u8,std::mem::size_of::<i32>() as i32)};
        if r==0{Ok(())}else{Err(std::io::Error::last_os_error())}
    }
    #[cfg(windows)] pub fn get(s:&TcpStream)->std::io::Result<usize>{
        use std::os::windows::io::AsRawSocket;use windows_sys::Win32::Networking::WinSock::{getsockopt,SOL_SOCKET,SO_SNDBUF};let (mut v,mut len)=(0i32,std::mem::size_of::<i32>() as i32);
        let r=unsafe{getsockopt(s.as_raw_socket() as usize,SOL_SOCKET,SO_SNDBUF,&mut v as *mut i32 as *mut u8,&mut len)};
        if r==0{Ok(v.max(0) as usize)}else{Err(std::io::Error::last_os_error())}
    }
    #[cfg(not(any(unix,windows)))] pub fn set(_:&TcpStream,_:usize)->std::io::Result<()>{Err(std::io::Error::new(std::io::ErrorKind::Unsupported,"SO_SNDBUF"))}
    #[cfg(not(any(unix,windows)))] pub fn get(_:&TcpStream)->std::io::Result<usize>{Err(std::io::Error::new(std::io::ErrorKind::Unsupported,"SO_SNDBUF"))}
}
/// Waiting for a socket to become readable or writable with a timeout, through poll(2) (Unix) or
/// WSAPoll (Windows); std has neither. Ok(true): ready (data, space, EOF or an error the next read or
/// write reports); Ok(false): the timeout passed (or a signal interrupted the wait). The timeout is
/// rounded up to whole milliseconds, so on Unix a wait does not end before it (Windows waits run on
/// its timer tick). The socket's blocking mode is not changed (the P2P reader shares its socket with
/// a blocking writer thread).
pub mod sockwait{
    use std::{net::TcpStream,time::Duration};
    fn ms(t:Duration)->i32{t.as_nanos().div_ceil(1_000_000).min(i32::MAX as u128) as i32}
    #[cfg(unix)] fn wait(s:&TcpStream,write:bool,t:Duration)->std::io::Result<bool>{
        use std::os::unix::io::AsRawFd;let mut p=libc::pollfd{fd:s.as_raw_fd(),events:if write{libc::POLLOUT}else{libc::POLLIN},revents:0};
        match unsafe{libc::poll(&mut p,1,ms(t))}{n if n>0=>Ok(true),0=>Ok(false),_=>{let e=std::io::Error::last_os_error();if e.kind()==std::io::ErrorKind::Interrupted{Ok(false)}else{Err(e)}}}
    }
    #[cfg(windows)] fn wait(s:&TcpStream,write:bool,t:Duration)->std::io::Result<bool>{
        use std::os::windows::io::AsRawSocket;use windows_sys::Win32::Networking::WinSock::{WSAPoll,WSAPOLLFD,POLLRDNORM,POLLWRNORM};
        let mut p=WSAPOLLFD{fd:s.as_raw_socket() as usize,events:if write{POLLWRNORM}else{POLLRDNORM},revents:0};
        match unsafe{WSAPoll(&mut p,1,ms(t))}{n if n>0=>Ok(true),0=>Ok(false),_=>Err(std::io::Error::last_os_error())}
    }
    #[cfg(not(any(unix,windows)))] fn wait(_:&TcpStream,_:bool,t:Duration)->std::io::Result<bool>{std::thread::sleep(t.min(Duration::from_millis(1)));Ok(true)}
    pub fn writable(s:&TcpStream,t:Duration)->std::io::Result<bool>{wait(s,true,t)}
    pub fn readable(s:&TcpStream,t:Duration)->std::io::Result<bool>{wait(s,false,t)}
}
/// The SO_SNDBUF read back from the last accepted RPC connection (pocol_getRpcStats sendSockBuf).
pub static SEND_SOCK_BUF_SEEN:AtomicUsize=AtomicUsize::new(0);
/// An accepted SO_SNDBUF read-back: SEND_SOCK_BUF (Windows) or the doubled value Linux reports. Any
/// other value (the setting failed or was clamped) refuses the connection (network.md:190).
pub fn sockbuf_ok(n:usize)->bool{(SEND_SOCK_BUF..=2*SEND_SOCK_BUF).contains(&n)}
/// Test hook (fault injection): nonzero makes every accepted RPC connection read back this value as
/// its SO_SNDBUF (usize::MAX: the setting fails), so the refusal path can be exercised. 0 in the node.
pub static SOCKBUF_FAULT:AtomicUsize=AtomicUsize::new(0);
/// The last SendPacer close: (sendTime µs, A bytes) (network.md:252 CloseOK record).
pub static LAST_PACER_CLOSE:Mutex<Option<(u64,usize)>>=Mutex::new(None);
fn bump_max(a:&AtomicUsize,v:usize){a.fetch_max(v,Ordering::SeqCst);}

/// Heavy request bodies waiting in the admission queue, per server: the body of a request whose
/// connection already answered (busy at QUEUE_WAIT, or 504) stays there until the node dequeues it,
/// so the connection cap alone does not bound them (A13 rpc term). HEAVY_QUEUE places; a further
/// heavy request waits for one until QUEUE_WAIT has passed since its parsing, then is -32021 {busy}
/// (network.md:214, :234).
pub fn queued_max(l:&Limits)->usize{l.heavy_queue.max(1)}
/// One queued body (a HEAVY_QUEUE place, or a light-lane entry); dropping it (when the node has
/// handled or skipped the request) frees its place and its gauge.
pub struct Held(Arc<AtomicUsize>,&'static AtomicUsize);
impl Drop for Held{fn drop(&mut self){self.0.fetch_sub(1,Ordering::SeqCst);self.1.fetch_sub(1,Ordering::SeqCst);}}
fn hold(c:&Arc<AtomicUsize>,max:usize,gauge:&'static AtomicUsize)->Option<Held>{
    if c.fetch_add(1,Ordering::SeqCst)>=max{c.fetch_sub(1,Ordering::SeqCst);return None}
    gauge.fetch_add(1,Ordering::SeqCst);Some(Held(c.clone(),gauge))
}
/// Admission states (network.md:232-234): waiting in its queue, taken by the consensus thread with
/// its reservation, or gone (not admitted within QUEUE_WAIT, or its connection stopped waiting).
pub const QUEUED:u8=0;pub const ADMITTED:u8=1;pub const GONE:u8=2;
/// A request for the consensus thread: its body (shared with its connection, which answers busy from
/// it when the request is not admitted in time; never a second copy), reply, whether it carried an
/// Origin header (a site), the instant after which nobody waits for the answer, the end of its
/// parsing (it enters its queue: QUEUE_WAIT and BATCH_WALL run from here), its lane, its queue place
/// and its admission state.
pub struct Request{pub body:Arc<Vec<u8>>,pub reply:Sender<Reply>,pub site:bool,pub deadline:Instant,pub queued:Instant,pub light:bool,pub held:Option<Held>,pub state:Arc<AtomicU8>}
impl Request{
    /// A request queued now, outside any connection (in-process callers and tests).
    pub fn local(body:Vec<u8>,reply:Sender<Reply>,deadline:Instant,light:bool)->Self{
        Self{body:Arc::new(body),reply,site:false,deadline,queued:Instant::now(),light,held:None,state:Arc::new(AtomicU8::new(QUEUED))}
    }
    /// Atomic admission (network.md:232-234): the consensus thread takes the request only within
    /// QUEUE_WAIT of the end of its parsing and only while its connection still waits for it.
    /// Err(true): not admitted, marked gone here (this side answers busy); Err(false): its connection
    /// already marked it gone and answered.
    fn admit(&self,wait:Duration)->Result<(),bool>{
        if self.queued.elapsed()>=wait{return Err(self.state.compare_exchange(QUEUED,GONE,Ordering::SeqCst,Ordering::SeqCst).is_ok())}
        self.state.compare_exchange(QUEUED,ADMITTED,Ordering::SeqCst,Ordering::SeqCst).map(|_|()).map_err(|_|false)
    }
}
/// COMPUTE_DEADLINE and the log handlers' working-set budget, threaded through a handler
/// (network.md:220, :235, :286). The deadline runs from admission and includes encoding: it is
/// checked at the handler's entry, inside long loops (eth_getLogs every 16 blocks; eth_call and
/// eth_estimateGas before each simulation and through its stop hook, native::STOP_GAS) and once the
/// answer is encoded, when -32020 {deadline} replaces any answer of a method without an effect that
/// completed after it, result or error (`member`). eth_getLogs/pocol_getLogs charge the bytes they
/// materialize; an overflow is -32020 {scratch}. `walled`: a batch reached BATCH_WALL and its
/// connection is closed.
pub struct Budget{pub compute_by:Instant,pub scratch_max:usize,pub scratch_used:usize,pub walled:bool}
impl Budget{
    pub fn expired(&self)->bool{Instant::now()>=self.compute_by}
    /// Charges `n` scratch bytes; true if the budget still holds. Records the high-water mark.
    pub fn charge(&mut self,n:usize)->bool{
        self.scratch_used=self.scratch_used.saturating_add(n);bump_max(&COUNTERS.scratch_heavy_max,self.scratch_used);
        self.scratch_used<=self.scratch_max
    }
    /// A -32020 {deadline} answer is produced now: counted, with how late it is (cancelMaxMs).
    fn late(&self){COUNTERS.deadline_hits.fetch_add(1,Ordering::SeqCst);bump_max(&COUNTERS.cancel_late_max_ms,Instant::now().saturating_duration_since(self.compute_by).as_millis() as usize);}
}
/// The per-connection token bucket (network.md:215-218) applied to the connection's one request: the
/// bucket is full (bucket_cap tokens) and the request takes k tokens at parse (1, or k members).
/// Some(retryAfterMs) when k exceeds it (the deficit at `rate`); at the spec values (cap 20 =
/// BATCH_MAX) no request within BATCH_MAX is refused.
pub fn rate_refused(k:u32,l:&Limits)->Option<u64>{
    (k>l.bucket_cap).then(||(u64::from(k-l.bucket_cap)*1000).div_ceil(u64::from(l.rate.max(1))).max(1))
}
/// The SendPacer's backstop check (network.md:246: "فحص احتياطي كل 50 ms").
pub const PACER_BACKSTOP:Duration=Duration::from_millis(50);
/// SendPacer (network.md:237-256). The answer is written in chunks on a non-blocking socket (a write
/// that would block sends nothing, on every platform). A is the bytes the socket has accepted, size
/// the response length, sendTime the wall time since the first byte while A < size (P jumps to size
/// at once, so the server is pending until sendDone). The one binding rule: close if sendTime > GRACE
/// and A < MIN_SEND_RATE·sendTime, evaluated with the exact values after every write attempt. A
/// blocked write waits for the socket to become writable (`sockwait`, poll/WSAPoll), at most until
/// the violation instant s_v + TIMER_EPS (s_v = max(GRACE, A/R)), the BATCH_WALL instant or
/// PACER_BACKSTOP, whichever is first: the timer is re-armed on every change of A, the close
/// lateness (pacerLateMaxMs) is TIMER_EPS plus scheduling (ms rounding included), and a connection
/// whose client does not read costs one wakeup per backstop, not a poll loop. On A = size it is
/// sendDone: the reservation (in `Reply`) drops at return. A response of `size` bytes cannot hold its
/// reservation longer than SEND_MAX(size). `batch_from`: a batch's read-end; BATCH_WALL from it
/// closes the connection (network.md:282).
fn pace_write(s:&mut TcpStream,body:&[u8],l:&Limits,batch_from:Option<Instant>)->std::io::Result<()>{
    let (r,grace,eps)=(l.min_send_rate.max(1) as f64,l.grace.as_secs_f64(),l.timer_eps.as_secs_f64());
    // The answer is written straight from its reservation-backed String; only the small header is
    // allocated here (no second copy of the body, so an in-flight answer stays one reservation).
    let hdr=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nConnection: close\r\n\r\n",body.len());
    let parts:[&[u8];2]=[hdr.as_bytes(),body];let size=hdr.len()+body.len();
    s.set_nonblocking(true)?;
    let (t0,mut a,mut pi,mut off)=(Instant::now(),0usize,0usize,0usize);
    let fail=|m:&'static str|Err(std::io::Error::new(std::io::ErrorKind::TimedOut,m));
    while a<size{
        while pi<2&&off>=parts[pi].len(){pi+=1;off=0;}
        // At most SEND_SOCK_BUF per write: Windows takes a whole send() whenever its buffer is not yet
        // full when the call starts (an unread 14 MiB answer in one call, Windows runs 3, 5 and 7), so
        // SO_SNDBUF bounds the kernel's share only if no single write exceeds it.
        let end=parts[pi].len().min(off+SEND_SOCK_BUF);
        let blocked=match s.write(&parts[pi][off..end]){
            Ok(0)=>return Err(std::io::Error::new(std::io::ErrorKind::WriteZero,"peer closed")),
            Ok(n)=>{a+=n;off+=n;false}
            Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>true,
            Err(e) if e.kind()==std::io::ErrorKind::Interrupted=>false,
            Err(e)=>return Err(e),
        };
        let st=t0.elapsed().as_secs_f64();
        if st>grace&&(a as f64)<r*st{
            // CloseOK (network.md:252): sendTime > GRACE, A < R·sendTime; lateness sendTime − s_v.
            let s_v=grace.max(a as f64/r);let late_ms=((st-s_v)*1000.0).max(0.0) as usize;
            COUNTERS.pacer_closed.fetch_add(1,Ordering::SeqCst);bump_max(&COUNTERS.pacer_late_max_ms,late_ms);
            *LAST_PACER_CLOSE.lock().unwrap_or_else(|e|e.into_inner())=Some(((st*1e6) as u64,a));
            return fail("send pacer: below MIN_SEND_RATE after GRACE")
        }
        if batch_from.is_some_and(|f|f.elapsed()>=l.batch_wall){COUNTERS.batch_wall_hits.fetch_add(1,Ordering::SeqCst);return fail("batch wall")}
        if blocked{
            // Here sendTime <= s_v (the rule did not fire), so the timer is at least TIMER_EPS away.
            COUNTERS.pacer_waits.fetch_add(1,Ordering::SeqCst);let s_v=grace.max(a as f64/r);let wall=batch_from.map_or(PACER_BACKSTOP,|f|(f+l.batch_wall).saturating_duration_since(Instant::now()));
            sockwait::writable(s,PACER_BACKSTOP.min(wall).min(Duration::from_secs_f64((s_v+eps-st).max(0.0))))?;
        }
    }
    // sendDone (network.md:258); sendOverrun = sendTime − (GRACE + size/R) (network.md:348).
    let st=t0.elapsed();COUNTERS.send_done.fetch_add(1,Ordering::SeqCst);
    let over=st.as_millis().min(i64::MAX as u128) as i64-(l.grace.as_millis()+(size as u128*1000)/l.min_send_rate.max(1) as u128).min(i64::MAX as u128) as i64;
    COUNTERS.send_overrun_max_ms.fetch_max(over,Ordering::SeqCst);
    if let Some(f)=batch_from{bump_max(&COUNTERS.batch_send_max_ms,st.as_millis() as usize);bump_max(&COUNTERS.batch_wall_max_ms,f.elapsed().as_millis() as usize);}
    Ok(())
}

/// Response memory (local; after RpcGuard LIGHT_RESP_MAX, HEAVY_RESP_MEM and LIGHT_RESP_MEM,
/// network.md (ج)). When it admits a request, the consensus thread reserves its class maximum:
/// LIGHT_RESP_MAX for a light request, RESP_MAX for a heavy one, BATCH_BYTES_HARD for a batch.
/// After it runs, the reservation shrinks to the response's allocation, and it is released when
/// the connection has written the response or failed (the Reply is dropped). So answers waiting
/// on slow readers hold at most the pools, whatever conn_max is (A13 rpc term).
/// - Light: (conn_max + 1) x LIGHT_RESP_MAX, one per connection plus the one being computed for a
///   connection that already left, so a light reservation never fails (network.md S1). A light
///   response larger than LIGHT_RESP_MAX is answered -32020 {resultBytes} (network.md:210).
/// - Heavy: HEAVY_RESP_POOL. A full pool answers -32021 {busy, retryAfterMs} before anything
///   runs (each batch member gets its own busy item), so a state-changing call never loses its
///   answer. The request is not held for the pool: it is answered busy at once.
pub const LIGHT_RESP_MAX:usize=512*1024;
pub const HEAVY_RESP_POOL:usize=48<<20;
pub const ERR_ITEM_MAX:usize=256;
/// The largest batch answer: BATCH_RESP_MAX of answers plus one error item per member.
pub const BATCH_BYTES_HARD:usize=BATCH_RESP_MAX+BATCH_MAX*ERR_ITEM_MAX+64;
/// Answers outside the pools: a busy or rate answer (<= BATCH_MAX error items, each <= ERR_ITEM_MAX)
/// and the HTTP literals. It is pocold's CONN_BUF: the per-connection buffer the spec writes a
/// batch's k error items from (network.md:217, :221), smaller than the spec's 64 KiB.
pub const SMALL_REPLY_MAX:usize=BATCH_MAX*ERR_ITEM_MAX+64;
/// The heap a connection thread takes while it builds and writes a busy or rate answer (A13 rpc term,
/// per connection): the k kept id encodings (<= ID_ECHO_MAX each, `scan_ids`), the answer
/// (SMALL_REPLY_MAX, one buffer), its HTTP header, and one transient id parse (ID_PARSE_PEAK).
/// `a_busy_answer_allocates_no_parse_tree` measures adversarial shapes against it.
pub const SMALL_REPLY_PEAK:usize=BATCH_MAX*(ID_ECHO_MAX+24)+SMALL_REPLY_MAX+512+ID_PARSE_PEAK;
/// One id parse within `id_encoding`'s filter (<= ID_RAW_MAX bytes, <= ID_ECHO_MAX/2 containers,
/// <= 2·(1+ID_ECHO_MAX) values, so <= 1+ID_ECHO_MAX object keys): B-tree nodes of <= 1 KiB, one per
/// object plus one per 5 keys (a non-root node holds >= 5); two 32-byte Vec slots per value (growth by
/// doubling) and 4 per array; the decoded strings and the parser's scratch (2·ID_RAW_MAX); the
/// re-encoding, grown by doubling (6·ID_RAW_MAX).
pub const ID_PARSE_PEAK:usize=(ID_ECHO_MAX/2+(ID_ECHO_MAX+1)/5)*1024+2*(1+ID_ECHO_MAX)*2*32+ID_ECHO_MAX/2*4*32+2*ID_RAW_MAX+6*ID_RAW_MAX;
pub fn light_pool(l:&Limits)->usize{(l.conn_max+1)*LIGHT_RESP_MAX}
type Pool=(Arc<AtomicUsize>,usize);
#[derive(Clone)]
pub struct RespPools{heavy:Pool,light:Pool}
impl RespPools{
    pub fn new(l:&Limits)->Self{Self{heavy:(Arc::new(AtomicUsize::new(0)),HEAVY_RESP_POOL),light:(Arc::new(AtomicUsize::new(0)),light_pool(l))}}
    fn take(p:&Pool,n:usize)->Option<Resv>{
        let mut cur=p.0.load(Ordering::SeqCst);
        loop{if cur+n>p.1{return None}match p.0.compare_exchange(cur,cur+n,Ordering::SeqCst,Ordering::SeqCst){Ok(_)=>return Some(Resv{pool:p.0.clone(),n}),Err(c)=>cur=c}}
    }
    /// Reserved bytes (heavy, light), for status and tests.
    pub fn in_use(&self)->(usize,usize){(self.heavy.0.load(Ordering::SeqCst),self.light.0.load(Ordering::SeqCst))}
    /// Reserves `n` heavy bytes directly (tests hold the pool full with it).
    pub fn hold_heavy(&self,n:usize)->Option<Resv>{Self::take(&self.heavy,n)}
}
pub struct Resv{pool:Arc<AtomicUsize>,n:usize}
impl Resv{fn shrink(&mut self,to:usize){if to<self.n{self.pool.fetch_sub(self.n-to,Ordering::SeqCst);self.n=to;}}}
impl Drop for Resv{fn drop(&mut self){self.pool.fetch_sub(self.n,Ordering::SeqCst);}}
/// An answer and the reservation that covers it until the connection drops it. `close`: the
/// connection closes without writing (the batch reached BATCH_WALL while it ran); `batch_from`: a
/// batch's read-end (BATCH_WALL bounds its send too).
pub struct Reply{pub body:String,_resv:Option<Resv>,pub close:bool,batch_from:Option<Instant>}
impl Reply{fn small(body:String)->Self{Self{body,_resv:None,close:false,batch_from:None}}}
/// The heavyActive / lightActive gauge while a request runs (<= 1 each: one consensus thread).
struct Active(&'static AtomicUsize);
impl Active{fn new(g:&'static AtomicUsize)->Self{g.fetch_add(1,Ordering::SeqCst);Self(g)}}
impl Drop for Active{fn drop(&mut self){self.0.fetch_sub(1,Ordering::SeqCst);}}
/// Runs one request. Atomic admission (network.md:232-234): within QUEUE_WAIT of the end of its
/// parsing, the consensus thread takes it together with its class's reservation (see
/// LIGHT_RESP_MAX); a request not admitted in time is -32021 {busy, retryAfterMs} without running
/// (its connection may already have answered that itself). Its COMPUTE_DEADLINE then runs from
/// this admission, including encoding (network.md:235): time spent queued never counts against it.
/// A batch is also bounded by BATCH_WALL from its read-end (network.md:282).
pub fn answer(n:&mut Node,q:&Request)->Reply{
    let lr=n.cfg.limits.rpc.clone();
    match q.admit(lr.queue_wait){
        Ok(())=>{}
        Err(true)=>{COUNTERS.queue_wait.fetch_add(1,Ordering::SeqCst);COUNTERS.busy.fetch_add(1,Ordering::SeqCst);return Reply::small(busy(&q.body))}
        Err(false)=>return Reply::small(String::new()),
    }
    let batch=q.body.iter().find(|b|!b.is_ascii_whitespace())==Some(&b'[');
    let (pool,need)=if q.light{(n.resp_pools().light,LIGHT_RESP_MAX)}else if batch{(n.resp_pools().heavy,BATCH_BYTES_HARD)}else{(n.resp_pools().heavy,RESP_MAX)};
    let Some(mut resv)=RespPools::take(&pool,need) else{COUNTERS.busy.fetch_add(1,Ordering::SeqCst);return Reply::small(busy(&q.body))};
    let _active=Active::new(if q.light{&COUNTERS.light_active}else{&COUNTERS.heavy_active});
    // The class's deadline by method (network.md (ب), :235), as for batch members: a light-table entry
    // whose body exceeds LIGHT_BODY_MAX, or a conditional one, runs on the heavy lane (and reservation)
    // under the light deadline; its answer has the light limit (`member`).
    let light=q.light||(!batch&&light_class(&q.body));
    let mut budget=Budget{compute_by:Instant::now()+if light{lr.compute_light}else{lr.compute_heavy},scratch_max:lr.scratch_heavy,scratch_used:0,walled:false};
    let wall_by=batch.then(||q.queued+lr.batch_wall);
    let mut body=handle_until_budgeted(n,&q.body,q.site,Some(q.deadline),wall_by,&mut budget);
    if budget.walled{return Reply{body:String::new(),_resv:None,close:true,batch_from:None}}
    // Over the class's reservation: -32020 {resultBytes}, and the reservation shrinks to the error
    // (network.md:210, RG12).
    if body.len()>need{body=result_bytes(&request_id(&q.body));}
    body.shrink_to_fit();resv.shrink(body.capacity());
    Reply{body,_resv:Some(resv),close:false,batch_from:batch.then_some(q.queued)}
}
fn request_id(body:&[u8])->Value{
    if json_values(body)>JSON_VALUES_MAX{return Value::Null}
    serde_json::from_slice::<Value>(body).ok().map(|v|id_of(&v)).map(|id|if id.to_string().len()<=128{id}else{Value::Null}).unwrap_or(Value::Null)
}
/// -32021 {busy, retryAfterMs}: one error object, or one item per batch member; nothing runs.
fn busy(body:&[u8])->String{reply_32021(body,"busy",250)}
/// -32021 {reason, retryAfterMs}: one error object (singleton) or one item per batch member, each
/// echoing the member id; nothing runs. The reason is "busy" (a full pool, no HEAVY_QUEUE place, a
/// full light lane, or not admitted within QUEUE_WAIT) or "rate" (the per-connection token bucket).
/// Connection threads build it too, so the request is never parsed into a tree here (A13: only the
/// consensus thread holds one): `scan_ids` reads the members' ids, each kept as its encoding. The k
/// items are each <= ERR_ITEM_MAX, so the answer is <= SMALL_REPLY_MAX (network.md:217); the heap it
/// takes to build is <= SMALL_REPLY_PEAK.
fn reply_32021(body:&[u8],reason:&'static str,retry:u64)->String{
    use std::fmt::Write as _;
    // The encoding of json!({"jsonrpc","id","error":{"code","message","data":{"reason","retryAfterMs"}}})
    // (serde_json objects are sorted maps), written into one buffer.
    let e=|o:&mut String,id:&str|{let _=write!(o,r#"{{"error":{{"code":-32021,"data":{{"reason":"{reason}","retryAfterMs":{retry}}},"message":"{reason}"}},"id":{id},"jsonrpc":"2.0"}}"#);};
    let mut o=String::with_capacity(SMALL_REPLY_MAX);
    if json_values(body)>JSON_VALUES_MAX{e(&mut o,"null");return o}
    match scan_ids(body){
        Ok(ids)=>{o.push('[');for (k,id) in ids.iter().enumerate(){if k>0{o.push(',')}e(&mut o,id)}o.push(']');}
        Err(id)=>e(&mut o,&id),
    }
    o
}
fn ws(b:&[u8],mut i:usize)->usize{while i<b.len()&&b[i].is_ascii_whitespace(){i+=1}i}
/// One past the closing quote of the string at b[i] (b.len() if unterminated).
fn str_end(b:&[u8],mut i:usize)->usize{
    i+=1;let mut esc=false;
    while i<b.len(){let c=b[i];i+=1;if esc{esc=false}else if c==b'\\'{esc=true}else if c==b'"'{return i}}
    b.len()
}
/// One past the value at b[i], scanned without allocating (a container by depth outside strings).
fn value_end(b:&[u8],i:usize)->usize{
    match b.get(i){
        Some(b'"')=>str_end(b,i),
        Some(b'{'|b'[')=>{let (mut d,mut j)=(0usize,i);
            while j<b.len(){match b[j]{b'"'=>{j=str_end(b,j);continue}b'{'|b'['=>d+=1,b'}'|b']'=>{d-=1;if d==0{return j+1}}_=>{}}j+=1}
            b.len()}
        _=>{let mut j=i;while j<b.len()&&!matches!(b[j],b','|b'}'|b']')&&!b[j].is_ascii_whitespace(){j+=1}j}
    }
}
/// The id the object at b[i] carries: its last top-level "id" member (serde_json keeps the last), as
/// `id_encoding` gives it.
fn obj_id(b:&[u8],i:usize)->String{
    let (mut id,mut j)=(String::from("null"),ws(b,i+1));
    while b.get(j)==Some(&b'"'){
        let k=str_end(b,j);let key=&b[j..k];j=ws(b,k);if b.get(j)!=Some(&b':'){break}
        j=ws(b,j+1);let e=value_end(b,j);
        if key==b"\"id\""{id=id_encoding(&b[j..e]);}
        j=ws(b,e);if b.get(j)==Some(&b','){j=ws(b,j+1)}else{break}
    }
    id
}
/// The longest id encoding a busy or rate item echoes (a longer id is echoed as null), and the
/// longest raw id parsed for it.
pub const ID_ECHO_MAX:usize=128;pub const ID_RAW_MAX:usize=1024;
/// An id's encoding as serde_json writes its parsed value, kept when <= ID_ECHO_MAX bytes; else "null".
/// The parse tree is transient (dropped at once), and only a raw id of at most ID_RAW_MAX bytes, at
/// most ID_ECHO_MAX/2 containers and 2·(1+ID_ECHO_MAX) values (`json_values`) is parsed. An id whose
/// encoding fits ID_ECHO_MAX has no more (each container takes two encoded bytes, and the structural
/// bytes outside strings are the raw id's), so the filter drops no id that would be echoed.
fn id_encoding(raw:&[u8])->String{
    let containers=||{let (mut n,mut q,mut esc)=(0usize,false,false);
        for &c in raw{if q{if esc{esc=false}else if c==b'\\'{esc=true}else if c==b'"'{q=false}}else if c==b'"'{q=true}else if c==b'{'||c==b'['{n+=1}}n};
    if raw.len()>ID_RAW_MAX||json_values(raw)>2*(1+ID_ECHO_MAX)||containers()>ID_ECHO_MAX/2{return "null".into()}
    serde_json::from_slice::<Value>(raw).ok().map(|v|v.to_string()).filter(|e|e.len()<=ID_ECHO_MAX).unwrap_or_else(||"null".into())
}
/// The ids a -32021 answer echoes, scanned without building a tree: Ok(one per member) for a batch of
/// 1..=BATCH_MAX members, Err(id) for one error object (a singleton object's id, null otherwise). It
/// checks the framing (members start a value, no trailing comma, nothing after the closing bracket),
/// not full JSON validity; a key spelled with escapes is not recognised, so its id is echoed as null.
/// Either way the answer is an error, never a result.
fn scan_ids(b:&[u8])->Result<Vec<String>,String>{
    let i=ws(b,0);let null=||String::from("null");
    match b.get(i){
        Some(b'{')=>Err(if ws(b,value_end(b,i))==b.len(){obj_id(b,i)}else{null()}),
        Some(b'[')=>{
            let (mut v,mut j)=(Vec::with_capacity(BATCH_MAX),ws(b,i+1));
            if b.get(j)!=Some(&b']'){loop{
                if v.len()==BATCH_MAX||!matches!(b.get(j),Some(b'{'|b'['|b'"'|b'-'|b'0'..=b'9'|b't'|b'f'|b'n')){return Err(null())}
                v.push(if b[j]==b'{'{obj_id(b,j)}else{null()});
                j=ws(b,value_end(b,j));if b.get(j)==Some(&b','){j=ws(b,j+1)}else{break}
            }}
            if v.is_empty()||b.get(j)!=Some(&b']')||ws(b,j+1)!=b.len(){return Err(null())}
            Ok(v)
        }
        _=>Err(null()),
    }
}
/// The token cost of a request without parsing a tree: 1 for a singleton, else the number of
/// top-level array members (network.md:216, "k tokens at parse"), capped at BATCH_MAX+1 (a larger
/// batch is rejected later). A cheap depth/quote scan, allocation-free like `json_values`.
pub fn request_tokens(body:&[u8])->u32{
    let first=body.iter().find(|b|!b.is_ascii_whitespace());
    if first!=Some(&b'['){return 1}
    let (mut depth,mut members,mut in_str,mut esc,mut saw)=(0i32,0u32,false,false,false);
    for &c in body{
        if in_str{if esc{esc=false}else if c==b'\\'{esc=true}else if c==b'"'{in_str=false}continue}
        match c{
            b'"'=>{in_str=true;if depth==1{saw=true}}
            b'{'|b'['=>{depth+=1;if depth==2{saw=true}}
            b'}'|b']'=>depth-=1,
            b',' if depth==1=>members+=1,
            _ if depth==1&&!c.is_ascii_whitespace()=>saw=true,
            _=>{}
        }
        if members>=BATCH_MAX as u32{break}
    }
    (if saw{members+1}else{0}).clamp(1,BATCH_MAX as u32+1)
}

/// The light lane: LIGHT_QUEUE places and a wake flag, so the main queue holds at most one Wake
/// for it at a time, whatever the light request rate.
#[derive(Clone)]
pub struct LightLane{pub tx:SyncSender<Event>,pub wake:Arc<AtomicBool>}
impl LightLane{
    pub fn new(cap:usize)->(Self,mpsc::Receiver<Event>){let (tx,rx)=mpsc::sync_channel(cap.max(1));(Self{tx,wake:Arc::new(AtomicBool::new(false))},rx)}
    /// Asks the node to drain the lane unless a Wake is already pending. A full main queue
    /// needs none: the node drains the lane before every event it takes.
    pub fn wake(&self,node:&SyncSender<Event>){
        if !self.wake.swap(true,Ordering::SeqCst){
            if node.try_send(Event::Wake).is_ok(){COUNTERS.wakes.fetch_add(1,Ordering::SeqCst);}else{self.wake.store(false,Ordering::SeqCst);}
        }
    }
}

/// Methods the network.md classification table makes light unconditionally (worst encoded
/// response <= LIGHT_RESP_MAX). Conditional entries (pocol_getHeaders by count, eth_getBlockBy*
/// by the full flag), unlisted methods and batches are scheduled on the heavy lane; the conditional
/// entries still have the light class's deadline and answer limit (`light_class`, `light_value`).
pub const LIGHT_METHODS:&[&str]=&["eth_chainId","net_version","web3_clientVersion","eth_blockNumber","eth_gasPrice","eth_maxPriorityFeePerGas","eth_getBalance",
    "eth_getStorageAt","eth_getTransactionCount","eth_sendRawTransaction","pocol_getTiming","pocol_getTarget","pocol_getCaps","pocol_syncStatus","pocol_getPot",
    "pocol_getSettlement","pocol_getBurnTotals","pocol_getEscrow","pocol_getRetention","pocol_getClosing","pocol_getRpcStats","pocol_submitShare","eth_getCode",
    "eth_getTransactionByHash","eth_getTransactionReceipt","pocol_getParams","pocol_getAssignment","pocol_getMinerSet","pocol_getTemplate"];
/// A light request is a single object of at most LIGHT_BODY_MAX bytes whose method is listed.
/// The probe borrows the method name and skips every other field, so classification allocates
/// nothing in proportion to the body (A13); any parse error (duplicate keys included) is heavy.
pub const LIGHT_BODY_MAX:usize=4096;
pub fn is_light(body:&[u8])->bool{body.len()<=LIGHT_BODY_MAX&&light_method(body)}
/// A single object whose method is light in the classification table (its class, whatever its size).
pub fn light_method(body:&[u8])->bool{
    #[derive(serde::Deserialize)] struct Probe<'a>{#[serde(borrow,default)] method:Option<std::borrow::Cow<'a,str>>}
    body.iter().find(|b|!b.is_ascii_whitespace())==Some(&b'{')&&matches!(serde_json::from_slice::<Probe>(body),Ok(Probe{method:Some(m)}) if LIGHT_METHODS.contains(&m.as_ref()))
}
/// The conditional light entries of network.md (ب), read as their handlers read params[1]:
/// eth_getBlockBy* with hashes only (the full flag not `true`) and pocol_getHeaders with count <= 64.
fn light_cond(m:&str,flag:Option<bool>,count:Option<u64>)->bool{
    match m{"eth_getBlockByNumber"|"eth_getBlockByHash"=>flag!=Some(true),"pocol_getHeaders"=>count.is_some_and(|c|c<=64),_=>false}
}
/// params[1] of a probed body: its bool or its count (a number or a hex string, as `parse_u64`);
/// any other value is skipped without allocating.
#[derive(Default)] struct Arg1(Option<bool>,Option<u64>);
impl<'de> serde::Deserialize<'de> for Arg1{fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error>{d.deserialize_any(Arg1V)}}
struct Arg1V;
impl<'de> serde::de::Visitor<'de> for Arg1V{type Value=Arg1;
    fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("a value")}
    fn visit_bool<E>(self,b:bool)->Result<Arg1,E>{Ok(Arg1(Some(b),None))}
    fn visit_u64<E>(self,c:u64)->Result<Arg1,E>{Ok(Arg1(None,Some(c)))}
    fn visit_i64<E>(self,_:i64)->Result<Arg1,E>{Ok(Arg1::default())}
    fn visit_f64<E>(self,_:f64)->Result<Arg1,E>{Ok(Arg1::default())}
    fn visit_str<E>(self,s:&str)->Result<Arg1,E>{Ok(Arg1(None,u64::from_str_radix(s.trim_start_matches("0x"),16).ok()))}
    fn visit_unit<E>(self)->Result<Arg1,E>{Ok(Arg1::default())}
    fn visit_seq<S:serde::de::SeqAccess<'de>>(self,mut q:S)->Result<Arg1,S::Error>{while q.next_element::<serde::de::IgnoredAny>()?.is_some(){}Ok(Arg1::default())}
    fn visit_map<M:serde::de::MapAccess<'de>>(self,mut q:M)->Result<Arg1,M::Error>{while q.next_entry::<serde::de::IgnoredAny,serde::de::IgnoredAny>()?.is_some(){}Ok(Arg1::default())}
}
/// A params array, of which only params[1] is kept (anything but an array or null is a parse error: heavy).
struct Params(Arg1);
impl<'de> serde::Deserialize<'de> for Params{fn deserialize<D:serde::Deserializer<'de>>(d:D)->Result<Self,D::Error>{d.deserialize_seq(ParamsV)}}
struct ParamsV;
impl<'de> serde::de::Visitor<'de> for ParamsV{type Value=Params;
    fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("an array")}
    fn visit_seq<S:serde::de::SeqAccess<'de>>(self,mut q:S)->Result<Params,S::Error>{
        let a=if q.next_element::<serde::de::IgnoredAny>()?.is_some(){q.next_element::<Arg1>()?.unwrap_or_default()}else{Arg1::default()};
        while q.next_element::<serde::de::IgnoredAny>()?.is_some(){}Ok(Params(a))
    }
}
/// The class of a single object by the classification table (network.md (ب)): a listed light method,
/// or a conditional light entry by its params[1]. Like `light_method` it borrows and skips, allocating
/// nothing in proportion to the body; any parse error is heavy.
pub fn light_class(body:&[u8])->bool{
    #[derive(serde::Deserialize)] struct Probe<'a>{#[serde(borrow,default)] method:Option<std::borrow::Cow<'a,str>>,#[serde(default)] params:Option<Params>}
    light_method(body)||body.iter().find(|b|!b.is_ascii_whitespace())==Some(&b'{')&&matches!(serde_json::from_slice::<Probe>(body),
        Ok(Probe{method:Some(m),params})if{let (f,c)=params.as_ref().map_or((None,None),|p|(p.0.0,p.0.1));light_cond(&m,f,c)})
}
/// `light_class` of a parsed request or batch member.
pub fn light_value(v:&Value)->bool{
    let m=v.get("method").and_then(|m|m.as_str()).unwrap_or("");
    LIGHT_METHODS.contains(&m)||match v.get("params"){None|Some(Value::Null)=>light_cond(m,None,None),
        Some(Value::Array(p))=>{let x=p.get(1);light_cond(m,x.and_then(|x|x.as_bool()),x.and_then(|x|x.as_u64().or_else(||parse_u64(x))))}
        Some(_)=>false}
}
/// Shared across one server's connections: the heavy admission queue depth (an abandoned heavy
/// body stays here until the node dequeues it) and the light lane's (bounded by its capacity).
pub struct Server{pub queued:Arc<AtomicUsize>,pub light:Arc<AtomicUsize>}
/// Connection threads are joined by the acceptor before their slot is reused, so at most
/// conn_max connection threads exist, exiting ones included (A13 thread inventory).
pub fn serve(addr:SocketAddr,node:SyncSender<Event>,light:LightLane,limits:Limits)->Result<(),String>{
    if !addr.ip().is_loopback(){return Err("RPC binds loopback only".into())}
    let l=TcpListener::bind(addr).map_err(|e|format!("rpc bind {addr}: {e}"))?;
    let srv=Arc::new(Server{queued:Arc::new(AtomicUsize::new(0)),light:Arc::new(AtomicUsize::new(0))});
    crate::threads::spawn("rpc",crate::threads::IO_STACK,move||{
        let mut next=0u64;let mut conns:Vec<std::thread::JoinHandle<()>>=Vec::new();
        for s in l.incoming(){
            // No Nagle delay: with delayed ACKs (Windows) a reply written in two parts would wait
            // about 200 ms for the first part's ACK.
            let Ok(mut s)=s else{continue};let _=s.set_nodelay(true);next+=1;let id=next;
            // A connection whose send buffer cannot be bounded is not served (SEND_SOCK_BUF holds for
            // every answer written): the setting fails, or its read-back is not SEND_SOCK_BUF.
            let got=sockbuf::set(&s,SEND_SOCK_BUF).and_then(|_|sockbuf::get(&s));
            let got=match SOCKBUF_FAULT.load(Ordering::SeqCst){0=>got,usize::MAX=>Err(std::io::Error::other("SO_SNDBUF (test hook)")),n=>Ok(n)};
            match got{
                Ok(n) if sockbuf_ok(n)=>SEND_SOCK_BUF_SEEN.store(n,Ordering::SeqCst),
                _=>{COUNTERS.sockbuf_refused.fetch_add(1,Ordering::SeqCst);let _=s.shutdown(std::net::Shutdown::Both);continue}
            }
            crate::threads::reap(&mut conns);
            if conns.len()>=limits.conn_max{
                COUNTERS.refused.fetch_add(1,Ordering::SeqCst);
                // Closed without reading or lingering: the acceptor never waits on a refused client.
                let _=s.set_write_timeout(Some(Duration::from_millis(100)));let _=write_reply(&mut s,503,"{}");continue
            }
            let (n,lt,lim,srv)=(node.clone(),light.clone(),limits.clone(),srv.clone());
            COUNTERS.active.fetch_add(1,Ordering::SeqCst);
            match crate::threads::spawn(format!("rpc-{id}"),crate::threads::IO_STACK,move||{let _=conn(s,n,lt,&lim,&srv);COUNTERS.active.fetch_sub(1,Ordering::SeqCst);}){
                Ok(h)=>conns.push(h),Err(_)=>{COUNTERS.active.fetch_sub(1,Ordering::SeqCst);}
            }
        }
    }).map_err(|e|e.to_string())?;
    Ok(())
}
/// Reads into `buf` before `deadline`; Ok(0) at EOF.
fn read_until(s:&mut TcpStream,tmp:&mut [u8],deadline:Instant)->std::io::Result<usize>{
    let left=deadline.saturating_duration_since(Instant::now());
    if left.is_zero(){return Err(std::io::Error::new(std::io::ErrorKind::TimedOut,"request deadline"))}
    s.set_read_timeout(Some(left.max(Duration::from_millis(1))))?;s.read(tmp)
}
fn conn(mut s:TcpStream,node:SyncSender<Event>,light:LightLane,l:&Limits,srv:&Server)->std::io::Result<()>{
    // SO_SNDBUF (network.md:190) is SEND_SOCK_BUF, set by the acceptor (`serve`) before this thread
    // starts, and the SendPacer writes at most SEND_SOCK_BUF per call: A, the bytes the socket has
    // accepted, runs at most the send buffer plus the client's receive window ahead of its reads.
    let deadline=Instant::now()+l.read_deadline;s.set_write_timeout(Some(l.write_deadline))?;
    // Request memory (A13 rpc term): this header buffer, never reallocated (it stops past
    // HEAD_MAX, at most one read later), then a body buffer of exactly Content-Length bytes.
    let mut buf=Vec::with_capacity(HEAD_MAX+READ_CHUNK);let mut tmp=[0u8;READ_CHUNK];let head_end;
    loop{let n=read_until(&mut s,&mut tmp,deadline)?;if n==0{return Ok(())}buf.extend_from_slice(&tmp[..n]);
        if let Some(i)=buf.windows(4).position(|w|w==b"\r\n\r\n"){head_end=i+4;break}if buf.len()>HEAD_MAX{return respond(&mut s,431,"{}")}}
    let (method,len,site)={
        let head=String::from_utf8_lossy(&buf[..head_end]);
        let mut lines=head.split("\r\n");let first=lines.next().unwrap_or("");
        let mut len=0usize;let mut site=false;
        for l in lines{let mut kv=l.splitn(2,':');let k=kv.next().unwrap_or("").trim().to_ascii_lowercase();
            if k=="content-length"{len=kv.next().unwrap_or("").trim().parse().unwrap_or(usize::MAX)}if k=="origin"{site=true}}
        (if first.starts_with("OPTIONS"){0}else if first.starts_with("POST"){1}else{2},len,site)
    };
    if method==0{return respond(&mut s,204,"")}
    if method==2{return respond(&mut s,405,"{}")}
    if len>REQ_BUF.saturating_sub(head_end){
        // Closing with unread input makes the kernel reset the connection, which can discard the
        // 413 before the client reads it: shut down the write side and drain at most REQ_BUF more
        // bytes within the read deadline first.
        respond(&mut s,413,"{}")?;let _=s.shutdown(std::net::Shutdown::Write);
        let mut left=REQ_BUF;while left>0{match read_until(&mut s,&mut tmp,deadline){Ok(0)|Err(_)=>break,Ok(n)=>left=left.saturating_sub(n)}}
        return Ok(())
    }
    let mut body=Vec::with_capacity(len);body.extend_from_slice(&buf[head_end..buf.len().min(head_end+len)]);drop(buf);
    while body.len()<len{let want=(len-body.len()).min(READ_CHUNK);let n=read_until(&mut s,&mut tmp[..want],deadline)?;if n==0{break}body.extend_from_slice(&tmp[..n]);}
    // Stage 2 (network.md:231): the per-connection token bucket on this connection's one request (k
    // tokens; a batch over BATCH_MAX is left to its -32600 check). Insufficient is -32021 {rate,
    // retryAfterMs} without admission or run. ORIGIN_DENY (A10) applies to the request and to every
    // batch member on the consensus thread, before any member executes.
    let k=request_tokens(&body);
    if let Some(retry)=rate_refused(k,l).filter(|_|k<=BATCH_MAX as u32){COUNTERS.rate.fetch_add(1,Ordering::SeqCst);return respond(&mut s,200,&reply_32021(&body,"rate",retry))}
    // The request enters its queue: QUEUE_WAIT and BATCH_WALL run from here, and the work deadline
    // (the node skips the request, or stops a batch between members, once nobody waits for it).
    let queued=Instant::now();let work_by=queued+l.work_deadline;let admit_by=(queued+l.queue_wait).min(work_by);
    let (body,state)=(Arc::new(body),Arc::new(AtomicU8::new(QUEUED)));let (tx,rx)=mpsc::channel();
    if is_light(&body){
        // LIGHT_QUEUE places (network.md:214): a full lane is -32021 {busy}.
        let held=hold(&srv.light,usize::MAX,&COUNTERS.light_queued);
        if light.tx.try_send(Event::Rpc(Request{body:body.clone(),reply:tx,site,deadline:work_by,queued,light:true,held,state:state.clone()})).is_err(){
            COUNTERS.light_full.fetch_add(1,Ordering::SeqCst);COUNTERS.busy.fetch_add(1,Ordering::SeqCst);return respond(&mut s,200,&busy(&body));
        }
        COUNTERS.light_pushed.fetch_add(1,Ordering::SeqCst);
        light.wake(&node);
    }else{
        // A HEAVY_QUEUE place (network.md:214), waited for until QUEUE_WAIT has passed. The single
        // consensus thread makes active heavy work <= 1 <= HEAVY_ACTIVE.
        let Some(held)=reserve_heavy(&srv.queued,l.heavy_queue,admit_by) else{COUNTERS.queue_full.fetch_add(1,Ordering::SeqCst);COUNTERS.busy.fetch_add(1,Ordering::SeqCst);return respond(&mut s,200,&busy(&body))};
        // A full main event queue is waited for until QUEUE_WAIT too (the place is released on busy).
        let mut ev=Event::Rpc(Request{body:body.clone(),reply:tx,site,deadline:work_by,queued,light:false,held:Some(held),state:state.clone()});
        loop{match node.try_send(ev){
            Ok(())=>break,
            Err(mpsc::TrySendError::Full(e)) if Instant::now()<admit_by=>{ev=e;std::thread::sleep(Duration::from_millis(1));}
            Err(mpsc::TrySendError::Full(_))=>{COUNTERS.queue_full.fetch_add(1,Ordering::SeqCst);COUNTERS.busy.fetch_add(1,Ordering::SeqCst);return respond(&mut s,200,&busy(&body))}
            Err(mpsc::TrySendError::Disconnected(_))=>return respond(&mut s,503,"{}"),
        }}
    }
    // Admission within QUEUE_WAIT (network.md:234): the answer, or the node's admission, by then;
    // otherwise this connection marks the request gone (the node will not run it) and answers busy
    // itself. Admitted work is waited for until the work deadline (then 504).
    let r=match rx.recv_timeout(admit_by.saturating_duration_since(Instant::now())){
        Ok(r)=>Some(r),
        Err(_)=>match state.compare_exchange(QUEUED,GONE,Ordering::SeqCst,Ordering::SeqCst){
            Err(ADMITTED)=>rx.recv_timeout(work_by.saturating_duration_since(Instant::now())).ok(),
            // Gone: marked here (counted here) or by the node at dequeue (counted there).
            g if Instant::now()<work_by=>{if g.is_ok(){COUNTERS.queue_wait.fetch_add(1,Ordering::SeqCst);COUNTERS.busy.fetch_add(1,Ordering::SeqCst);}return respond(&mut s,200,&busy(&body))}
            _=>None,
        }
    };
    // The reservation in the Reply is released when it drops, after the write or its failure. The
    // 200 answer is written under the SendPacer (`pace_write`), so a slow reader cannot hold its
    // reservation beyond SEND_MAX(size); `r` stays alive across the write, holding the reservation.
    // A BATCH_WALL close is immediate (network.md:282); an answer that was sent ends with `linger`.
    match r{Some(r) if r.close=>Ok(()),Some(r)=>{pace_write(&mut s,r.body.as_bytes(),l,r.batch_from)?;linger(&mut s);Ok(())},None=>respond(&mut s,504,"{}")}
}
/// How long, at most, a connection drains unread input after its answer before it is closed.
pub const LINGER:Duration=Duration::from_millis(500);
/// After an answer: the write side is shut down (the client reads its end) and input the server never
/// reads (a trailing CRLF, a pipelined request: network.md:187 has no pipelining) is drained for at
/// most LINGER and REQ_BUF bytes, so the close does not make the kernel reset the connection, which
/// can discard an answer the client has not read yet (review R2-4). A client that closes after reading
/// ends it at once.
fn linger(s:&mut TcpStream){
    let _=s.shutdown(std::net::Shutdown::Write);let by=Instant::now()+LINGER;let mut tmp=[0u8;READ_CHUNK];let mut left=REQ_BUF;
    while left>0{match read_until(s,&mut tmp,by){Ok(0)|Err(_)=>break,Ok(n)=>left=left.saturating_sub(n)}}
}
/// Reserves one HEAVY_QUEUE place, waiting for one until `until` if the queue is full
/// (network.md:234). The place is released when the node dequeues the request (`Held` drop).
fn reserve_heavy(c:&Arc<AtomicUsize>,max:usize,until:Instant)->Option<Held>{
    loop{
        if let Some(h)=hold(c,max,&COUNTERS.queued){return Some(h)}
        let left=until.saturating_duration_since(Instant::now());if left.is_zero(){return None}
        std::thread::sleep(Duration::from_millis(5).min(left));
    }
}
/// A request connection's small answer, then `linger`.
fn respond(s:&mut TcpStream,code:u16,body:&str)->std::io::Result<()>{write_reply(s,code,body)?;linger(s);Ok(())}
fn write_reply(s:&mut TcpStream,code:u16,body:&str)->std::io::Result<()>{
    let reason=match code{200=>"OK",204=>"No Content",405=>"Method Not Allowed",413=>"Payload Too Large",431=>"Request Header Fields Too Large",503=>"Service Unavailable",_=>"Error"};
    // One write for the header and one for the body (write! would issue a write per piece).
    let head=format!("HTTP/1.1 {code} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: content-type\r\nConnection: close\r\n\r\n",body.len());
    s.write_all(head.as_bytes())?;s.write_all(body.as_bytes())?;s.flush()
}

fn qty(v:u64)->String{format!("0x{v:x}")}
fn qty256(v:U256)->String{format!("0x{v:x}")}
fn err(id:&Value,code:i64,m:&str)->Value{json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":m}})}
fn parse_u64(v:&Value)->Option<u64>{v.as_str().and_then(|s|u64::from_str_radix(s.trim_start_matches("0x"),16).ok())}
fn parse_addr(v:&Value)->Option<Address>{v.as_str()?.parse().ok()}
fn parse_u256(v:&Value)->Option<U256>{v.as_str().and_then(|s|U256::from_str_radix(s.trim_start_matches("0x"),16).ok())}

/// An unbounded budget: no COMPUTE_DEADLINE and effectively unlimited SCRATCH (for `handle`,
/// used where the caller is a test or an in-process consumer, not a network client).
fn unbounded()->Budget{Budget{compute_by:Instant::now()+Duration::from_secs(365*24*3600),scratch_max:usize::MAX,scratch_used:0,walled:false}}
pub fn handle(n:&mut Node,body:&[u8])->String{handle_until_budgeted(n,body,false,None,None,&mut unbounded())}
/// One response, or -32020 {resultBytes} with the request's id when it exceeds RESP_MAX.
fn capped(v:Value,id:&Value)->String{let s=v.to_string();if s.len()<=RESP_MAX{s}else{result_bytes(id)}}
/// "Exceeding the limit while encoding: -32020 {resultBytes} with a clean release" (network.md:210;
/// the reason is in network.md's error list). The answer built so far is dropped by the caller; the
/// id is echoed while its encoding is <= 128 bytes (an inflating id cannot inflate the error too).
fn result_bytes(id:&Value)->String{
    let id=if id.to_string().len()<=128{id}else{&Value::Null};
    json!({"jsonrpc":"2.0","id":id,"error":{"code":-32020,"message":"response too large","data":{"reason":"resultBytes"}}}).to_string()
}
fn id_of(v:&Value)->Value{v.get("id").cloned().unwrap_or(Value::Null)}
/// The id an error item echoes: the request's while its encoding is <= ID_ECHO_MAX bytes, else null,
/// so a guard or dispatch error stays within ERR_ITEM_MAX (network.md:276; review R2-3).
fn echo_id(id:Value)->Value{if id.to_string().len()<=ID_ECHO_MAX{id}else{Value::Null}}
/// -32601 for a method not in the table: the name is echoed only while its JSON encoding is short, so
/// the item stays within ERR_ITEM_MAX with an echoed id (review R2-3).
fn not_found(m:&str)->(i64,String){(-32601,if serde_json::to_string(m).map_or(usize::MAX,|e|e.len())<=METHOD_ECHO_MAX{format!("method {m} not found")}else{"method not found".into()})}
pub const METHOD_ECHO_MAX:usize=42;
/// `site`: the request carried an Origin header. ORIGIN_DENY is decided for the request
/// and for every batch member before any member executes; a denied member is answered
/// -32601 and never reaches its method; malformed members are -32600 (A10).
pub fn handle_with(n:&mut Node,body:&[u8],site:bool)->String{handle_until_budgeted(n,body,site,None,None,&mut unbounded())}
/// As `handle_with`, under a COMPUTE_DEADLINE and SCRATCH budget (`Budget`). Batch members are not
/// started after `deadline` (the nobody-waits work deadline: each is answered -32020 {deadline}
/// without running). Each member that runs is admitted in turn on the one consensus thread with its
/// own COMPUTE_DEADLINE (its class's, from when it starts) and SCRATCH budget; light requests waiting
/// in their lane, and queued P2P, miner and timer events, run between members (`drain_between_members`). Once `wall_by` (BATCH_WALL from the read-end, network.md:282)
/// has passed, no further member runs and the batch is closed (`Budget::walled`).
pub fn handle_until_budgeted(n:&mut Node,body:&[u8],site:bool,deadline:Option<Instant>,wall_by:Option<Instant>,budget:&mut Budget)->String{
    if json_values(body)>JSON_VALUES_MAX{COUNTERS.too_complex.fetch_add(1,Ordering::SeqCst);return err(&Value::Null,-32600,"request has too many JSON values").to_string()}
    let v:Value=match serde_json::from_slice(body){Ok(v)=>v,Err(_)=>return err(&Value::Null,-32700,"parse error").to_string()};
    let denied=|i:&Value|site&&i.get("method").and_then(|m|m.as_str()).map_or(false,|m|ORIGIN_DENY.contains(&m));
    let site_err=|i:&Value|{COUNTERS.origin_denied.fetch_add(1,Ordering::SeqCst);err(&echo_id(id_of(i)),-32601,"method not available to sites")};
    if let Value::Array(items)=&v{
        if items.is_empty()||items.len()>BATCH_MAX{return err(&Value::Null,-32600,"batch").to_string()}
        let deny:Vec<bool>=items.iter().map(|i|denied(i)).collect();
        let (heavy,light)=(n.cfg.limits.rpc.compute_heavy,n.cfg.limits.rpc.compute_light);
        return batch(items,BATCH_RESP_MAX,|k,i|{
            if budget.walled{return String::new()}
            if wall_by.is_some_and(|w|Instant::now()>=w){budget.walled=true;COUNTERS.batch_wall_hits.fetch_add(1,Ordering::SeqCst);return String::new()}
            if k>0{n.drain_between_members()}
            if deny[k]{site_err(i).to_string()}
            else if deadline.is_some_and(|d|Instant::now()>=d){COUNTERS.expired_members.fetch_add(1,Ordering::SeqCst);item_err(i,-32020,"work deadline","deadline")}
            else{
                let lm=light_value(i);
                let by=Instant::now()+if lm{light}else{heavy};budget.compute_by=wall_by.map_or(by,|w|by.min(w));budget.scratch_used=0;member(n,i,budget)
            }
        });
    }
    if denied(&v){return site_err(&v).to_string()}
    member(n,&v,budget)
}
/// An upper bound on the JSON values serde_json can build from `body`, counted without
/// allocating: every value is the top-level one, or a member of a container that is its first
/// member or follows a comma, so values <= 1 + containers + commas outside strings (an object
/// member's key counts again: x2). A prefix serde_json accepts before an error has no more.
pub fn json_values(body:&[u8])->usize{
    let (mut n,mut in_str,mut esc)=(0usize,false,false);
    for &b in body{
        if in_str{if esc{esc=false}else if b==b'\\'{esc=true}else if b==b'"'{in_str=false}continue}
        match b{b'"'=>in_str=true,b'{'|b'['|b','=>n+=1,_=>{}}
    }
    2*(n+1)
}
/// The parse tree of a request within JSON_VALUES_MAX (A13 rpc term), serde_json 1.0 with
/// BTreeMap objects: per value one 32-byte slot in a Vec grown by doubling (<= 2 slots) and at
/// most one B-tree node (<= 1 KiB: 11 keys and values plus edges); array buffers of at least 4
/// slots; strings and the escape scratch buffer <= 3 x the body; decoded parameters <= the body.
pub fn parse_bytes_max(body_max:usize)->u128{(JSON_VALUES_MAX as u128)*(64+1024+128)+4*(body_max as u128)}
/// Batch members run and are serialized one at a time, so a batch holds at most `limit` bytes
/// of answers plus the member being built (A13). The member that makes the delivered bytes
/// exceed `limit`, and every later one, is answered -32020 {batchBytes} without running.
fn batch(items:&[Value],limit:usize,mut run:impl FnMut(usize,&Value)->String)->String{
    // Every delivered byte counts (brackets and separators included). An error item echoes the
    // request id only while it stays within ERR_ITEM_MAX.
    let mut out=String::from("[");let mut over=false;
    for (k,i) in items.iter().enumerate(){
        let mut s=if over{String::new()}else{run(k,i)};
        if over||out.len()+1+s.len()+1>limit{
            over=true;s=item_err(i,-32020,"batch response limit","batchBytes");
        }
        if k>0{out.push(',');}out.push_str(&s);
    }
    out.push(']');out
}
/// An error item that echoes the request id only while it stays within ERR_ITEM_MAX (256 B).
fn item_err(i:&Value,code:i64,m:&str,reason:&str)->String{
    json!({"jsonrpc":"2.0","id":echo_id(id_of(i)),"error":{"code":code,"message":m,"data":{"reason":reason}}}).to_string()
}
/// Methods whose effect happens when they run: their answer is returned even when it completes past
/// the deadline (a late -32020 would hide an accepted transaction or share). This is the running
/// reading of network.md:235 pending a decision (proposals/RPCGUARD-R2-PROPOSALS.md P3, option A).
const EFFECTFUL:&[&str]=&["eth_sendRawTransaction","pocol_submitShare"];
/// The log handlers: they check COMPUTE_DEADLINE themselves, so their -32020 carries resume data.
const LOGS:&[&str]=&["eth_getLogs","pocol_getLogs"];
/// Test hook (a slow encoding): nonzero holds the answer of a request or member whose id is
/// ENCODE_DELAY_ID this many ms once it is encoded, so the deadline check after encoding can be
/// exercised. 0 in the node.
pub static ENCODE_DELAY_MS:AtomicUsize=AtomicUsize::new(0);
pub const ENCODE_DELAY_ID:&str="encode-delay";
/// One request or batch member, encoded. Its class's answer limit (network.md (ب), :210): a light
/// entry's answer over LIGHT_RESP_MAX is -32020 {resultBytes} on either lane, a heavy one's over
/// RESP_MAX (`capped`). COMPUTE_DEADLINE includes encoding (network.md:235): any answer completed
/// after the deadline, a result or an error (an eth_call that reverts or halts after an
/// uninterruptible precompile run), is replaced by -32020 {deadline} (the log handlers' with resume
/// data). Kept: a -32020 {deadline} the handler produced itself (it carries its own resume data), and
/// the answers of EFFECTFUL methods, which are never lost.
fn member(n:&mut Node,v:&Value,b:&mut Budget)->String{
    let id=id_of(v);let r=one(n,v,b);let own=r["error"]["code"]==-32020&&r["error"]["data"]["reason"]=="deadline";let mut s=capped(r,&id);
    if s.len()>LIGHT_RESP_MAX&&light_value(v){s=result_bytes(&id);}
    let d=ENCODE_DELAY_MS.load(Ordering::SeqCst);if d>0&&id.as_str()==Some(ENCODE_DELAY_ID){std::thread::sleep(Duration::from_millis(d as u64));}
    let m=v.get("method").and_then(|m|m.as_str()).unwrap_or("");
    if !own&&b.expired()&&!EFFECTFUL.contains(&m){b.late();return fail_json(&id,deadline_fail(n,m,v)).to_string()}
    s
}
/// -32020 {deadline}; for a log handler with the resume data of no progress (network.md:300:
/// lastCompleteBlock = fromBlock−1, encoded null as at a per-block limit on fromBlock; nextFromBlock
/// = fromBlock).
fn deadline_fail(n:&Node,m:&str,v:&Value)->Fail{
    let from=if LOGS.contains(&m){logs_from(n,m,v["params"].get(0).unwrap_or(&Value::Null))}else{None};
    Fail(-32020,"compute deadline".into(),Some(match from{Some(f)=>json!({"reason":"deadline","fromBlock":qty(f),"lastCompleteBlock":Value::Null,"nextFromBlock":qty(f)}),None=>json!({"reason":"deadline"})}))
}
fn one(n:&mut Node,v:&Value,b:&mut Budget)->Value{
    if !v.is_object(){return err(&Value::Null,-32600,"invalid request")}
    let id=v["id"].clone();let p=v["params"].as_array().cloned().unwrap_or_default();
    let m=match v["method"].as_str(){Some(m)=>m,None=>return err(&echo_id(id),-32600,"invalid request")};
    // COMPUTE_DEADLINE at the handler entry: a request that reaches its handler past it is -32020
    // {deadline} without running (network.md:235). The log handlers check it at their first block.
    if b.expired()&&!LOGS.contains(&m){b.late();return fail_json(&id,deadline_fail(n,m,v))}
    // A method not in the table is a dispatch error: its item is bounded like the guard's (review R2-3).
    match call(n,m,&p,b){Ok(r)=>json!({"jsonrpc":"2.0","id":id,"result":r}),Err(f) if f.0==-32601=>fail_json(&echo_id(id),f),Err(f)=>fail_json(&id,f)}
}
/// An RPC error with optional data (network.md: −32020 carries {reason, fromBlock, …}).
struct Fail(i64,String,Option<Value>);
impl From<(i64,String)> for Fail{fn from((c,m):(i64,String))->Self{Fail(c,m,None)}}
type R=Result<Value,Fail>;
fn fail_json(id:&Value,f:Fail)->Value{match f.2{None=>err(id,f.0,&f.1),Some(d)=>json!({"jsonrpc":"2.0","id":id,"error":{"code":f.0,"message":f.1,"data":d}})}}
fn bad(m:&str)->(i64,String){(-32602,m.to_string())}

/// The canonical entry a tag names, metadata and state only (C-1 S1: its body and receipts may be off
/// the heap; `whole` gives them).
fn meta_by_tag<'a>(n:&'a Node,tag:&Value)->Option<&'a Entry>{
    match tag.as_str(){
        None|Some("latest")|Some("pending")|Some("safe")|Some("finalized")=>{
            let h=n.chain.head();let k=match tag.as_str(){Some("safe")=>6,Some("finalized")=>60,_=>0};n.chain.canonical_at(h.height.saturating_sub(k))
        }
        Some("earliest")=>n.chain.canonical_at(0),
        Some(_)=>parse_u64(tag).and_then(|x|n.chain.canonical_at(x)),
    }
}
/// A stored entry with its body, encoded header and receipts (`Node::view`). C-1 S1: when its history
/// cannot be read the answer is an error, never a block without its receipts or a shortened list.
fn whole<'a>(n:&'a Node,e:&'a Entry)->Result<std::borrow::Cow<'a,Entry>,Fail>{
    n.view(e).ok_or_else(||Fail(-32603,"block history unavailable (logged; the store is rebuilt at the next open)".into(),None))
}
/// The state a tag names. C-1 S3a: a retained state that is not the head's is materialized from the head
/// through the diffs (the same answer as when every retained state was held full).
/// C-1 E3: "pending" is read in place on the pool's layer over the head.
enum TagState<'a>{Full(crate::chain::StateView<'a>),Pending(pocol_lp4_execution::view::LayerView<'a,crate::chain::StateHandle>)}
impl TagState<'_>{fn v(&self)->&dyn pocol_lp4_execution::view::View{match self{TagState::Full(c)=>c.v(),TagState::Pending(l)=>l}}}
fn state_for<'a>(n:&'a Node,tag:&Value)->Result<TagState<'a>,(i64,String)>{
    if tag.as_str()==Some("pending"){return Ok(TagState::Pending(n.pool_view()))}
    let e=meta_by_tag(n,tag).ok_or((-32001,"unknown block".to_string()))?;
    let pruned=||(-32017,"state outside the retained trie window (K_hist)".to_string());
    if e.pruned{return Err(pruned())}
    n.chain.view_of(&e.hash).map(TagState::Full).ok_or_else(pruned)
}
fn call(n:&mut Node,m:&str,p:&[Value],b:&mut Budget)->R{
    let cp=n.chain.spec.cp();
    match m{
        "web3_clientVersion"=>Ok(json!("pocold/0.1.0 (PoCol local devnet)")),
        "net_version"=>Ok(json!(n.chain.spec.chain_id().to_string())),
        "net_listening"=>Ok(json!(true)),
        "net_peerCount"=>Ok(json!(qty(n.peer_count() as u64))),
        "eth_chainId"=>Ok(json!(qty(n.chain.spec.chain_id()))),
        "eth_syncing"=>Ok(json!(false)),
        "eth_mining"=>Ok(json!(n.cfg.miner.is_some())),
        "eth_accounts"=>Ok(json!([])),
        "eth_blockNumber"=>Ok(json!(qty(n.chain.head().height))),
        "eth_gasPrice"=>{let b=next_base(n);Ok(json!(qty256(b+U256::from(1_000_000_000u64))))}
        "eth_maxPriorityFeePerGas"=>Ok(json!(qty(1_000_000_000))),
        "eth_feeHistory"=>{
            let count=p.first().and_then(|x|x.as_u64().or_else(||parse_u64(x))).unwrap_or(1).clamp(1,128);let h=n.chain.head().height;
            let start=h.saturating_sub(count-1).max(1);let mut fees=vec![];let mut ratio=vec![];
            for x in start..=h{if let Some(e)=n.chain.canonical_at(x){fees.push(qty256(e.base_fee));ratio.push(json!(e.gas_used as f64/cp.GAS_LIMIT as f64));}}
            fees.push(qty256(next_base(n)));Ok(json!({"oldestBlock":qty(start),"baseFeePerGas":fees,"gasUsedRatio":ratio,"reward":[]}))
        }
        "eth_getBalance"=>{let a=p.first().and_then(parse_addr).ok_or(bad("address"))?;let s=state_for(n,p.get(1).unwrap_or(&Value::Null))?;
            Ok(json!(qty256(s.v().account(&a).map(|x|x.info.balance).unwrap_or_default())))}
        "eth_getTransactionCount"=>{let a=p.first().and_then(parse_addr).ok_or(bad("address"))?;let s=state_for(n,p.get(1).unwrap_or(&Value::Null))?;
            Ok(json!(qty(s.v().account(&a).map(|x|x.info.nonce).unwrap_or(0))))}
        "eth_getCode"=>{let a=p.first().and_then(parse_addr).ok_or(bad("address"))?;let s=state_for(n,p.get(1).unwrap_or(&Value::Null))?;
            Ok(json!(hex(&s.v().account(&a).and_then(|x|x.info.code).map(|c|c.original_bytes().to_vec()).unwrap_or_default())))}
        "eth_getStorageAt"=>{let a=p.first().and_then(parse_addr).ok_or(bad("address"))?;let k=p.get(1).and_then(parse_u256).ok_or(bad("slot"))?;
            let s=state_for(n,p.get(2).unwrap_or(&Value::Null))?;let v=s.v().slot(&a,&k);
            Ok(json!(hex(&v.to_be_bytes::<32>())))}
        "eth_call"=>{let o=p.first().ok_or(bad("call"))?;let r=simulate(n,o,p.get(1).unwrap_or(&Value::Null),None,b)?;
            match r{ExecutionResult::Success{output,..}=>Ok(json!(hex(output.data()))),ExecutionResult::Revert{output,..}=>Err(Fail::from((3,format!("execution reverted:{}",hex(&output))))),ExecutionResult::Halt{reason,..}=>Err(Fail::from((-32000,format!("halt {reason:?}"))))}}
        "eth_estimateGas"=>{
            let o=p.first().ok_or(bad("call"))?;let tag=Value::String("latest".into());
            let first=simulate(n,o,&tag,Some(cp.GAS_LIMIT),b)?;
            let used=match first{ExecutionResult::Success{gas_used,..}=>gas_used,ExecutionResult::Revert{output,..}=>return Err(Fail::from((3,format!("execution reverted:{}",hex(&output))))),ExecutionResult::Halt{reason,..}=>return Err(Fail::from((-32000,format!("halt {reason:?}"))))};
            let (mut lo,mut hi)=(used,cp.GAS_LIMIT);
            while lo+1000<hi{let mid=(lo+hi)/2;match simulate(n,o,&tag,Some(mid),b){Ok(ExecutionResult::Success{..})=>hi=mid,Err(f) if f.0==-32020=>return Err(f),_=>lo=mid}}
            Ok(json!(qty(hi)))
        }
        "eth_sendRawTransaction"=>{let raw=p.first().and_then(|x|x.as_str()).and_then(|s|unhex(s).ok()).ok_or(bad("raw"))?;
            // Admission runs under the request's COMPUTE_DEADLINE (review R2-2); past it nothing is pooled.
            n.submit_tx_by(raw,true,Some(b.compute_by)).map(|h|json!(format!("{h}"))).map_err(|e|{
                if e==crate::node::SUBMIT_DEADLINE{b.late();return Fail(-32020,"compute deadline".into(),Some(json!({"reason":"deadline"})))}
                // D-7: the pool was still revalidating a new head at the deadline; nothing was pooled.
                if e==crate::node::SUBMIT_BUSY{COUNTERS.busy.fetch_add(1,Ordering::SeqCst);return Fail(-32021,"busy".into(),Some(json!({"reason":"busy","retryAfterMs":250})))}
                let code=e.split(' ').next().and_then(|c|c.parse().ok()).unwrap_or(-32000);Fail(code,e,None)})}
        "eth_getBlockByNumber"=>{let full=p.get(1).and_then(|x|x.as_bool()).unwrap_or(false);
            match meta_by_tag(n,p.first().unwrap_or(&Value::Null)){None=>Ok(Value::Null),Some(e)=>Ok(block_json(n,&*whole(n,e)?,full))}}
        "eth_getBlockByHash"=>{let h=p.first().and_then(|x|x.as_str()).and_then(|s|unhex(s).ok()).ok_or(bad("hash"))?;
            let full=p.get(1).and_then(|x|x.as_bool()).unwrap_or(false);
            match h.try_into().ok().and_then(|h:[u8;32]|n.chain.get(&h)){None=>Ok(Value::Null),Some(e)=>Ok(block_json(n,&*whole(n,e)?,full))}}
        "eth_getTransactionByHash"=>{let h=p.first().and_then(|x|x.as_str()).and_then(|s|s.parse::<B256>().ok()).ok_or(bad("hash"))?;
            Ok(find_tx(n,&h)?.map(|(e,i)|tx_json(n,&e,i)).unwrap_or(Value::Null))}
        "eth_getTransactionReceipt"=>{let h=p.first().and_then(|x|x.as_str()).and_then(|s|s.parse::<B256>().ok()).ok_or(bad("hash"))?;
            Ok(find_tx(n,&h)?.map(|(e,i)|receipt_json(n,&e,i)).unwrap_or(Value::Null))}
        "eth_getLogs"=>logs(n,p.first().unwrap_or(&Value::Null),b),
        // LogClient endpoint (browser.md BR10): numeric range of at most 1024 blocks, or one blockHash.
        "pocol_getLogs"=>{let f=p.first().unwrap_or(&Value::Null);
            if let Some(h)=f.get("blockHash"){let h=h.as_str().and_then(|s|unhex(s).ok()).ok_or(bad("blockHash"))?;
                let h:[u8;32]=h.try_into().map_err(|_|bad("blockHash"))?;
                let e=n.chain.get(&h).filter(|e|n.chain.canonical_at(e.height).map(|c|c.hash==e.hash).unwrap_or(false)).ok_or((-32001,"unknown or non-canonical block".to_string()))?;let mut g=f.clone();g["fromBlock"]=json!(qty(e.height));g["toBlock"]=json!(qty(e.height));
                if let Some(o)=g.as_object_mut(){o.remove("blockHash");}
                return logs(n,&g,b)}
            if f["fromBlock"].as_str().map(|s|!s.starts_with("0x")).unwrap_or(true)||f["toBlock"].as_str().map(|s|!s.starts_with("0x")).unwrap_or(true){return Err(bad("numeric fromBlock/toBlock").into())}
            logs(n,f,b)}
        "pocol_status"=>Ok(n.status_json()),
        // pocold's own RPC counters and gauges (process-wide), a superset of pocol_getRpcStats.
        "pocold_rpcCounters"=>Ok(rpc_stats(n)),
        // network.md:347 pocol_getRpcStats: every field of the record; a field for a mechanism pocold
        // does not have (snapshots, pocol_getLogs anchors, PacerTrace, light SCRATCH) is null.
        "pocol_getRpcStats"=>Ok(rpc_stats(n)),
        "pocol_getParams"=>{let sp=&n.chain.spec;let l=&n.cfg.limits.rpc;
            Ok(json!({"profileKind":format!("{:?}",sp.profile().kind()),"nodeKind":format!("{:?}",sp.profile().node_kind()),"nonceMode":sp.cp().nonceMode,
                "chainId":sp.chain_id(),"genesisHash":hex(&sp.genesis_hash()),"genesisPre":hex(sp.spec_bytes()),"netKey":hex(&net_key(sp.chain_id(),&sp.genesis_hash())),
                "forkSchedule":sp.profile().forks().entries().iter().map(|(v,h)|json!([v,h])).collect::<Vec<_>>(),
                "activeVersion":sp.active_version(n.chain.head().height),"rpcLimits":rpc_limits_json(l)}))}
        "pocol_getRejects"=>Ok(json!(n.rejects.iter().map(|(h,w)|json!({"hash":h,"reason":w})).collect::<Vec<_>>())),
        "pocol_getHeaders"=>{let from=p.first().and_then(|x|x.as_u64().or_else(||parse_u64(x))).ok_or(bad("from"))?;let count=p.get(1).and_then(|x|x.as_u64().or_else(||parse_u64(x))).ok_or(bad("count"))?;
            if count==0||count>512{return Err(bad("count 1..512").into())}
            let mut out=vec![];for h in from..from+count{let Some(e)=n.chain.canonical_at(h) else{break};if h>0{out.push(hex(&whole(n,e)?.header.encoded));}}Ok(json!(out))}
        "pocol_getBlockRlp"=>{let e=meta_by_tag(n,p.first().unwrap_or(&Value::Null)).ok_or((-32001,"unknown block".to_string()))?;let e=whole(n,e)?;
            Ok(json!({"header":hex(&e.header.encoded),"body":hex(&e.body.encode())}))}
        "pocol_getMiners"=>{let head=n.chain.head;let s=n.chain.view_of(&head).ok_or((-32001,"head state".to_string()))?;let s=s.v();Ok(json!(apply::record_list(s).iter().map(|id|{let r=apply::miner_record(s,*id).unwrap();
            json!({"id":format!("{id:#x}"),"status":r.status,"tomb":r.tomb,"bondState":r.bond_state,"lastShare":r.last_share,"reward":format!("{:#x}",r.reward),"rewardBalance":qty256(s.account(&r.reward).map(|a|a.info.balance).unwrap_or_default())})}).collect::<Vec<_>>()))}
        "pocol_getTemplate"=>Ok(n.template_json()),
        _=>Err(Fail::from(not_found(m))),
    }
}
fn next_base(n:&Node)->U256{let h=n.chain.head();if h.height==0{chain::from_fixed_lp4(&n.chain.spec.cp().baseFee0)}else{pocol_lp4_execution::fees::next_base_fee(h.base_fee,h.gas_used,h.header.gas_limit).unwrap_or(h.base_fee)}}
/// netKey = keccak256(RLP(['PoCol-net-v1', chainId, genesisHash])) (FINAL_DESIGN.md:981), as the
/// extension derives it (pocol-extension/src/sw.js netKeyOf).
pub fn net_key(chain_id:u64,genesis:&[u8;32])->[u8;32]{
    let (mut p,mut o)=(Vec::new(),Vec::new());let c=chain_id.to_be_bytes();let f=c.iter().position(|x|*x!=0).unwrap_or(8);
    crate::rlp::encode_bytes(b"PoCol-net-v1",&mut p);crate::rlp::encode_bytes(&c[f..],&mut p);crate::rlp::encode_bytes(genesis,&mut p);
    crate::rlp::encode_list_payload(&p,&mut o);keccak256(&o).0
}
fn ms(d:Duration)->u64{d.as_millis().min(u64::MAX as u128) as u64}
/// pocol_getParams.rpcLimits: the running RpcGuard limits (network.md:184), byte and ms integers. A
/// limit pocold does not enforce is null, never nominal (RPCGUARD.md): SCRATCH (only
/// the log handlers are charged: logsScratch), CANCEL_MAX and RPC_MEM_TOTAL. CONN_BUF is pocold's
/// structural bound on a connection's small answers (SMALL_REPLY_MAX); heavyActive and lightActive
/// the single consensus thread's 1.
fn rpc_limits_json(l:&Limits)->Value{
    json!({"connMax":l.conn_max,"reqBuf":REQ_BUF,"connBuf":SMALL_REPLY_MAX,"sendSockBuf":SEND_SOCK_BUF,
        "heavyActive":1,"lightActive":1,"heavyQueue":l.heavy_queue,"lightQueue":l.light_queue,"queueWaitMs":ms(l.queue_wait),
        "heavyRespMem":HEAVY_RESP_POOL,"lightRespMem":light_pool(l),"respMax":RESP_MAX,"lightRespMax":LIGHT_RESP_MAX,"batchRespMax":BATCH_RESP_MAX,
        "scratchHeavy":Value::Null,"scratchLight":Value::Null,"logsScratch":l.scratch_heavy,"memTotal":Value::Null,"batchMax":BATCH_MAX,"errItemMax":ERR_ITEM_MAX,
        "logsCountMax":LOGS_COUNT_MAX,"logsScanBytes":LOGS_SCAN_BYTES,"rate":l.rate,"bucketCap":l.bucket_cap,
        "computeHeavyMs":ms(l.compute_heavy),"computeLightMs":ms(l.compute_light),"cancelMaxMs":Value::Null,"reqReadDeadlineMs":ms(l.read_deadline),
        "minSendRate":l.min_send_rate,"graceMs":ms(l.grace),"enforceSlackMs":ms(l.enforce_slack),"timerEpsMs":ms(l.timer_eps),"batchWallMs":ms(l.batch_wall)})
}
/// pocol_getRpcStats / pocold_rpcCounters: the RpcGuard stat record (network.md:347) and pocold's
/// own counters. Times are ms (`*Ms`) or seconds (`*S`, lastPacerClose.sendTime). A field for a
/// mechanism pocold does not have is null: scratchLightMax (light handlers are not charged),
/// snapshotsOpen (handlers read the chain on the one consensus thread; no snapshot objects),
/// anchorRejects (pocol_getLogs takes no anchorHash), traceDropped (no PacerTrace).
fn rpc_stats(n:&Node)->Value{
    let c=&COUNTERS;let g=|a:&AtomicUsize|a.load(Ordering::SeqCst);let secs=|a:&AtomicUsize|g(a) as f64/1000.0;let (h,l)=n.resp_pools().in_use();
    let over=c.send_overrun_max_ms.load(Ordering::SeqCst);let close=*LAST_PACER_CLOSE.lock().unwrap_or_else(|e|e.into_inner());
    let mut v=json!({"conns":g(&c.active),"heavyActive":g(&c.heavy_active),"heavyQueued":g(&c.queued),"lightActive":g(&c.light_active),"lightQueued":g(&c.light_queued),
        "heavyResv":h,"lightResv":l,"scratchHeavyMax":g(&c.scratch_heavy_max),"scratchLightMax":Value::Null,"snapshotsOpen":Value::Null,"maxResPerConn":1,
        "cancelled":g(&c.deadline_hits)+g(&c.scratch_hits),"cancelMaxMs":g(&c.cancel_late_max_ms),"busyCount":g(&c.busy),"rateCount":g(&c.rate),"limitCount":g(&c.limit_hits),
        "anchorRejects":Value::Null,"originDenied":g(&c.origin_denied),"pacerClosed":g(&c.pacer_closed),"pacerLateMaxMs":g(&c.pacer_late_max_ms),
        "lastPacerClose":close.map(|(t,a)|json!({"sendTime":t as f64/1e6,"accepted":a})),"sendDoneCount":g(&c.send_done),
        "sendOverrunMaxMs":if over==i64::MIN{Value::Null}else{json!(over)},"batchSendMaxS":secs(&c.batch_send_max_ms),"batchWallMaxS":secs(&c.batch_wall_max_ms),
        "batchWallHits":g(&c.batch_wall_hits),"traceDropped":Value::Null});
    let own=json!({"refused":g(&c.refused),"active":g(&c.active),"expired":g(&c.expired),"expiredMembers":g(&c.expired_members),"queueFull":g(&c.queue_full),"queueWait":g(&c.queue_wait),
        "lightFull":g(&c.light_full),"tooComplex":g(&c.too_complex),"busy":g(&c.busy),"queued":g(&c.queued),"wakes":g(&c.wakes),"lightPushed":g(&c.light_pushed),
        "heavyReserved":h,"lightReserved":l,"deadlineHits":g(&c.deadline_hits),"scratchHits":g(&c.scratch_hits),"sendSockBuf":g(&SEND_SOCK_BUF_SEEN),"sockbufRefused":g(&c.sockbuf_refused),"pacerWaits":g(&c.pacer_waits)});
    if let (Some(m),Value::Object(o))=(v.as_object_mut(),own){m.extend(o);}
    v
}
fn simulate(n:&Node,o:&Value,tag:&Value,gas:Option<u64>,b:&Budget)->Result<ExecutionResult,Fail>{
    // COMPUTE_DEADLINE before each simulation (eth_estimateGas runs several): none starts past it.
    let deadline=||{b.late();Err(Fail(-32020,"compute deadline".into(),Some(json!({"reason":"deadline"}))))};
    if b.expired(){return deadline()}
    let cp=n.chain.spec.cp();let e=meta_by_tag(n,tag).ok_or(Fail::from((-32001,"unknown block".to_string())))?;
    // As state_for: a pruned state (outside the trie window, or a block adopted after its state was
    // pruned as a side block) is -32017 (consensus.md §10), never a simulation on an empty state.
    if e.pruned{return Err(Fail::from((-32017,"state outside the retained trie window (K_hist)".into())))}
    let to=o["to"].as_str().and_then(|s|s.parse::<Address>().ok());
    let data=o.get("data").or_else(||o.get("input")).and_then(|x|x.as_str()).map(|s|unhex(s)).transpose().map_err(|_|Fail::from(bad("data")))?.unwrap_or_default();
    let tx=TxEnv{caller:o["from"].as_str().and_then(|s|s.parse().ok()).unwrap_or(Address::ZERO),transact_to:to.map(TxKind::Call).unwrap_or(TxKind::Create),
        gas_limit:gas.or_else(||parse_u64(&o["gas"])).unwrap_or(CALL_GAS_MAX).min(CALL_GAS_MAX),gas_price:U256::ZERO,value:parse_u256(&o["value"]).unwrap_or_default(),
        data:Bytes::from(data),nonce:None,chain_id:Some(n.chain.spec.chain_id()),..Default::default()};
    let ctx=NativeBlockContext{height:U256::from(e.height+1),timestamp:U256::from(e.ts+1),gas_limit:U256::from(cp.GAS_LIMIT.max(CALL_GAS_MAX)),base_fee:U256::ZERO,prevrandao:B256::ZERO};
    let base=n.chain.view_of(&e.hash).ok_or_else(||Fail::from((-32017,"state outside the retained trie window (K_hist)".to_string())))?;
    // eth_call simulation: fund the caller for value transfers, never committed (C-1 E3: on an overlay,
    // not a copy of the state).
    let mut st=pocol_lp4_execution::view::Overlay::new(&base);
    {use pocol_lp4_execution::view::WorkState;if let Some(a)=st.info_mut(&tx.caller){a.balance=a.balance.max(tx.value);}}
    // Cancellation (network.md:286): the stop hook ends the execution at its next poll once the
    // deadline has passed (native::simulate_for_profile: every STOP_GAS gas of a frame, before every
    // call). A precompile runs to its end, so the cancellation gap is not bounded there (CANCEL_MAX
    // is not enforced); `member` still answers a late result -32020.
    let by=b.compute_by;let stop:std::rc::Rc<dyn Fn()->bool>=std::rc::Rc::new(move||Instant::now()>=by);
    match native::simulate_for_profile(&st,tx,&ctx,n.chain.spec.profile(),stop){
        Ok(r)=>Ok(r.result),
        Err(revm::primitives::EVMError::Custom(m)) if m==native::SIMULATION_STOPPED=>deadline(),
        Err(e)=>Err(Fail::from((-32000,format!("{e:?}")))),
    }
}
/// The canonical block holding a transaction and its index (C-1 S1: through the transaction index in
/// history.redb, which lists every stored block holding it; the canonical one is taken). An unreadable
/// store is an error, never "not found".
fn find_tx<'a>(n:&'a Node,h:&B256)->Result<Option<(std::borrow::Cow<'a,Entry>,usize)>,Fail>{
    let unavailable=||Fail(-32603,"transaction index unavailable (logged; the store is rebuilt at the next open)".into(),None);
    let at=n.history.tx_locations(h).map_err(|e|{n.log(&format!("history: {}",e.0));unavailable()})?;
    let Some((b,i))=at.into_iter().find(|(b,_)|n.chain.is_canonical(b)) else{return Ok(None)};
    let e=whole(n,n.chain.get(&b).ok_or_else(unavailable)?)?;
    // The index is derived data: an entry that does not name this transaction marks the store damaged.
    if e.tx_hashes.get(i)!=Some(h){n.log(&n.history.mark_damaged("transaction index entry does not match its block").0);return Err(unavailable())}
    Ok(Some((e,i)))
}
fn block_json(n:&Node,e:&Entry,full:bool)->Value{
    let x=&e.header;let winner=e.result.as_ref().map(|r|format!("{:#x}",r.winner)).unwrap_or(format!("{:#x}",Address::ZERO));
    let txs:Vec<Value>=if full{(0..e.tx_hashes.len()).map(|i|tx_json(n,e,i)).collect()}else{e.tx_hashes.iter().map(|h|json!(format!("{h}"))).collect()};
    json!({"number":qty(e.height),"hash":hex(&e.hash),"parentHash":hex(&x.parent_hash),"nonce":hex(&x.nonce.to_be_bytes()),"sha3Uncles":"0x1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347",
        "logsBloom":hex(&x.logs_bloom),"transactionsRoot":hex(&x.tx_root),"stateRoot":hex(&x.state_root),"receiptsRoot":hex(&x.receipts_root),"miner":winner,
        "difficulty":qty256(chain::work(&x.target)),"totalDifficulty":qty256(e.total_work),"extraData":"0x","size":qty(x.encoded.len() as u64),"gasLimit":qty(x.gas_limit),"gasUsed":qty(x.gas_used),
        "timestamp":qty(x.ts),"transactions":txs,"uncles":[],"baseFeePerGas":qty256(e.base_fee),"mixHash":hex(&[0u8;32]),
        "pocol":{"attempt":x.a,"signed":e.signed,"proposer":hex(&x.proposer),"shares":x.shares.len(),"templateId":hex(&template_id(&x.ut_raw)),"final":e.is_final}})
}
fn tx_json(n:&Node,e:&Entry,i:usize)->Value{
    let raw=&e.body.txs[i];let env=wire::decode_for_profile(raw,n.chain.spec.profile());
    // The standard fields; `raw` only when the transaction does not decode (then there is no
    // `input`), so an answer stays within network.md's 2·TX_MAX + 2 KiB and LIGHT_RESP_MAX.
    let mut v=json!({"hash":format!("{}",e.tx_hashes[i]),"blockHash":hex(&e.hash),"blockNumber":qty(e.height),"transactionIndex":qty(i as u64)});
    if env.is_err(){v["raw"]=json!(hex(raw));}
    if let Ok(env)=env{let t=env.tx_env();v["from"]=json!(format!("{:#x}",env.sender()));v["to"]=t.transact_to.to().map(|a|json!(format!("{a:#x}"))).unwrap_or(Value::Null);
        v["nonce"]=json!(qty(t.nonce.unwrap_or(0)));v["gas"]=json!(qty(t.gas_limit));v["value"]=json!(qty256(t.value));v["input"]=json!(hex(&t.data));v["type"]=json!(qty(env.kind() as u64));
        v["gasPrice"]=json!(qty256(t.gas_price));if let Some(pf)=t.gas_priority_fee{v["maxPriorityFeePerGas"]=json!(qty256(pf));v["maxFeePerGas"]=json!(qty256(t.gas_price));}v["chainId"]=json!(qty(n.chain.spec.chain_id()));}
    v
}
fn decode_receipt(r:&[u8])->Option<(bool,u64,Vec<(Address,Vec<B256>,Vec<u8>)>)>{
    let body=if r.first().map(|b|*b<0x7f).unwrap_or(false){&r[1..]}else{r};
    let it=crate::rlp::decode(body).ok()?;let f=it.list()?;
    let status=f[0].bytes()?==[1];let cum=f[1].bytes()?.iter().fold(0u64,|a,b|(a<<8)|*b as u64);
    let mut logs=vec![];for l in f[3].list()?{let lf=l.list()?;let a=Address::from_slice(lf[0].bytes()?);let topics=lf[1].list()?.iter().map(|t|B256::from_slice(t.bytes().unwrap())).collect();logs.push((a,topics,lf[2].bytes()?.to_vec()));}
    Some((status,cum,logs))
}
fn receipt_json(n:&Node,e:&Entry,i:usize)->Value{
    let Some((status,cum,logs))=decode_receipt(&e.receipts[i]) else{return Value::Null};
    let prev=if i==0{0}else{decode_receipt(&e.receipts[i-1]).map(|r|r.1).unwrap_or(0)};
    let env=wire::decode_for_profile(&e.body.txs[i],n.chain.spec.profile()).ok();
    let (from,to,contract,price)=match &env{Some(env)=>{let t=env.tx_env();let from=env.sender();
        let created=if t.transact_to.is_create(){let mut p=Vec::new();crate::rlp::encode_bytes(from.as_slice(),&mut p);let nb=t.nonce.unwrap_or(0).to_be_bytes();let f=nb.iter().position(|x|*x!=0).unwrap_or(8);crate::rlp::encode_bytes(&nb[f..],&mut p);let mut o=Vec::new();crate::rlp::encode_list_payload(&p,&mut o);
            Some(Address::from_slice(&keccak256(&o)[12..]))}else{None};
        let tip=t.gas_priority_fee.map(|pf|pf.min(t.gas_price-e.base_fee)).unwrap_or(t.gas_price-e.base_fee);
        (json!(format!("{from:#x}")),t.transact_to.to().map(|a|json!(format!("{a:#x}"))).unwrap_or(Value::Null),created.map(|a|json!(format!("{a:#x}"))).unwrap_or(Value::Null),e.base_fee+tip)}
        None=>(Value::Null,Value::Null,Value::Null,e.base_fee)};
    let base_index:usize=(0..i).map(|k|decode_receipt(&e.receipts[k]).map(|r|r.2.len()).unwrap_or(0)).sum();
    let logs:Vec<Value>=logs.iter().enumerate().map(|(k,(a,t,d))|json!({"address":format!("{a:#x}"),"topics":t.iter().map(|x|format!("{x}")).collect::<Vec<_>>(),"data":hex(d),
        "blockNumber":qty(e.height),"blockHash":hex(&e.hash),"transactionHash":format!("{}",e.tx_hashes[i]),"transactionIndex":qty(i as u64),"logIndex":qty((base_index+k) as u64),"removed":false})).collect();
    let bloom=pocol_lp4_execution::receipts::bloom(&[]);let _=bloom;
    json!({"transactionHash":format!("{}",e.tx_hashes[i]),"transactionIndex":qty(i as u64),"blockHash":hex(&e.hash),"blockNumber":qty(e.height),"from":from,"to":to,
        "cumulativeGasUsed":qty(cum),"gasUsed":qty(cum-prev),"contractAddress":contract,"logs":logs,"logsBloom":hex(&e.header.logs_bloom),"status":qty(status as u64),
        "effectiveGasPrice":qty256(price),"type":qty(env.map(|x|x.kind() as u64).unwrap_or(0))})
}
fn logs(n:&Node,f:&Value,b:&mut Budget)->R{
    let head=n.chain.head().height;
    let from=match f["fromBlock"].as_str(){None|Some("latest")=>head,Some("earliest")=>0,Some(_)=>parse_u64(&f["fromBlock"]).ok_or(bad("fromBlock"))?};
    let to=match f["toBlock"].as_str(){None|Some("latest")=>head,Some(_)=>parse_u64(&f["toBlock"]).ok_or(bad("toBlock"))?};
    if to<from||to-from+1>1024{COUNTERS.limit_hits.fetch_add(1,Ordering::SeqCst);return Err(Fail::from((-32020,"range > 1024 blocks".into())))}
    let addrs:Vec<Address>=match &f["address"]{Value::String(s)=>s.parse().ok().into_iter().collect(),Value::Array(a)=>a.iter().filter_map(parse_addr).collect(),_=>vec![]};
    let topics:Vec<Option<Vec<B256>>>=f["topics"].as_array().map(|t|t.iter().map(|x|match x{Value::Null=>None,Value::String(s)=>s.parse().ok().map(|b|vec![b]),Value::Array(a)=>Some(a.iter().filter_map(|y|y.as_str()?.parse().ok()).collect()),_=>None}).collect()).unwrap_or_default();
    // network.md:298: the block x whose logs would pass a limit is not returned; the client resumes from x.
    // -32020 for a per-block limit, a COMPUTE_DEADLINE or a SCRATCH overflow all carry the same resume
    // data {reason, fromBlock, lastCompleteBlock, nextFromBlock} (network.md (ز)).
    let stop=|reason:&str,x:u64|{if !matches!(reason,"deadline"|"scratch"){COUNTERS.limit_hits.fetch_add(1,Ordering::SeqCst);}
        Fail(-32020,format!("log query limit ({reason})"),Some(json!({"reason":reason,"fromBlock":qty(from),"lastCompleteBlock":if x>from{Value::from(qty(x-1))}else{Value::Null},"nextFromBlock":qty(x)})))};
    let mut out=vec![];let mut scanned=0usize;let mut bytes=2usize;
    for h in from..=to{let Some(e)=n.chain.canonical_at(h) else{break};let e=whole(n,e)?;let e=&*e;
        // Cancellation checks (network.md:286): the COMPUTE_DEADLINE at the first block and every 16
        // blocks scanned (the handler's entry check, with resume data: network.md:300); the scratch
        // budget is charged at every matched log.
        if (h-from)%16==0&&b.expired(){b.late();return Err(stop("deadline",h))}
        scanned+=e.receipts.iter().map(|r|r.len()).sum::<usize>();if scanned>LOGS_SCAN_BYTES{return Err(stop("scanBytes",h))}
        for i in 0..e.receipts.len(){let r=receipt_json(n,e,i);for l in r["logs"].as_array().cloned().unwrap_or_default(){
            let a:Address=l["address"].as_str().unwrap().parse().unwrap();if !addrs.is_empty()&&!addrs.contains(&a){continue}
            let lt:Vec<B256>=l["topics"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().parse().unwrap()).collect();
            if topics.iter().enumerate().any(|(k,want)|match want{None=>false,Some(w)=>lt.get(k).map(|t|!w.contains(t)).unwrap_or(true)}){continue}
            let sz=l.to_string().len()+1;
            // SCRATCH (network.md:220): the working bytes this heavy handler materializes; overflow is
            // -32020 {scratch} with the same resume data. A partial result is dropped entirely.
            if !b.charge(sz){COUNTERS.scratch_hits.fetch_add(1,Ordering::SeqCst);return Err(stop("scratch",h))}
            bytes+=sz;if bytes>RESP_MAX-1024{return Err(stop("resultBytes",h))}
            out.push(l);if out.len()>LOGS_COUNT_MAX{return Err(stop("resultCount",h))}}}}
    Ok(Value::Array(out))
}
/// The fromBlock a log handler resolves from its filter (as `logs`, and pocol_getLogs's blockHash form).
fn logs_from(n:&Node,m:&str,f:&Value)->Option<u64>{
    if m=="pocol_getLogs"{if let Some(h)=f.get("blockHash"){let h:[u8;32]=h.as_str().and_then(|s|unhex(s).ok())?.try_into().ok()?;return n.chain.get(&h).map(|e|e.height)}}
    match f["fromBlock"].as_str(){None|Some("latest")=>Some(n.chain.head().height),Some("earliest")=>Some(0),Some(_)=>parse_u64(&f["fromBlock"])}
}

#[cfg(test)]
mod tests{
    /// Review 3 (RG3V-m1): the class by the table's rows (network.md (ب)), from the body probe and from
    /// the parsed member alike: listed methods, the conditional entries by params[1] as their handlers
    /// read it, and heavy for everything else (a probe parse error included).
    #[test] fn the_class_probe_agrees_with_the_parsed_class(){
        let rows:&[(&str,bool)]=&[(r#"{"method":"eth_blockNumber"}"#,true),(r#"{"method":"eth_call","params":[{},"latest"]}"#,false),
            (r#"{"method":"eth_getBlockByNumber","params":["latest",false]}"#,true),(r#"{"method":"eth_getBlockByNumber","params":["latest"]}"#,true),
            (r#"{"method":"eth_getBlockByNumber"}"#,true),(r#"{"method":"eth_getBlockByNumber","params":null}"#,true),
            (r#"{"method":"eth_getBlockByNumber","params":["latest",true]}"#,false),(r#"{"method":"eth_getBlockByHash","params":["0x00",true,{"x":[1]}]}"#,false),
            (r#"{"method":"eth_getBlockByHash","params":["0x00",{"a":true}]}"#,true),(r#"{"method":"eth_getBlockByNumber","params":{"full":true}}"#,false),
            (r#"{"params":["latest",false],"method":"eth_getBlockByHash"}"#,true),
            (r#"{"method":"pocol_getHeaders","params":[1,64]}"#,true),(r#"{"method":"pocol_getHeaders","params":[1,"0x40"]}"#,true),
            (r#"{"method":"pocol_getHeaders","params":[1,65]}"#,false),(r#"{"method":"pocol_getHeaders","params":[1,"0x41"]}"#,false),
            (r#"{"method":"pocol_getHeaders","params":[1]}"#,false),(r#"{"method":"pocol_getHeaders","params":[1,-1]}"#,false),
            (r#"{"method":"pocol_getHeaders","params":[1,64.0]}"#,false),(r#"{"method":"pocol_getHeaders","params":[[1,2],64]}"#,true),
            (r#"[{"method":"eth_blockNumber"}]"#,false),(r#"{"method":"pocol_status"}"#,false)];
        for (b,want) in rows{
            assert_eq!(super::light_class(b.as_bytes()),*want,"probe: {b}");
            let v:serde_json::Value=serde_json::from_str(b).unwrap();assert_eq!(!v.is_array()&&super::light_value(&v),*want,"parsed: {b}");
        }
        // Duplicate keys are a probe parse error: heavy (the parsed form keeps the last key).
        assert!(!super::light_class(br#"{"method":"pocol_status","method":"eth_getBlockByNumber","params":["latest",false]}"#));
    }
    /// Review 2 (M4): an answer over RESP_MAX is -32020 {resultBytes} (network.md:210), not -32008.
    #[test] fn responses_above_resp_max_become_an_error(){
        let id=serde_json::json!(7);
        let small=super::capped(serde_json::json!({"jsonrpc":"2.0","id":7,"result":"0x1"}),&id);assert!(small.contains("\"result\""));
        let big=super::capped(serde_json::json!({"jsonrpc":"2.0","id":7,"result":"a".repeat(super::RESP_MAX)}),&id);
        assert!(big.len()<200,"{}",&big[..big.len().min(200)]);
        // The error answers the request (A14 clients require the request id).
        let v:serde_json::Value=serde_json::from_str(&big).unwrap();assert_eq!(v["id"],7);
        assert_eq!((v["error"]["code"].as_i64(),v["error"]["data"]["reason"].as_str()),(Some(-32020),Some("resultBytes")),"{v}");
    }
    /// The wait helper behind the SendPacer and the P2P reader (poll/WSAPoll): a socket without data is
    /// not readable, and the wait lasts at least its timeout; data makes it readable at once; a socket
    /// with room is writable; one whose send buffer is full is not, until its peer reads.
    #[test] fn sockwait_reports_readiness_within_its_timeout(){
        use std::io::{Read,Write};use std::time::{Duration,Instant};use super::sockwait::{readable,writable};
        let l=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let mut c=std::net::TcpStream::connect(l.local_addr().unwrap()).unwrap();let (mut s,_)=l.accept().unwrap();
        // Windows waits run on its timer tick and may end up to a tick early: the exact bounds are Unix's.
        let t=Instant::now();assert!(!readable(&s,Duration::from_millis(30)).unwrap());assert!(t.elapsed()>=Duration::from_millis(if cfg!(unix){30}else{10}),"{:?}",t.elapsed());
        let t=Instant::now();assert!(!readable(&s,Duration::from_micros(300)).unwrap());assert!(!cfg!(unix)||t.elapsed()>=Duration::from_micros(300),"rounded up, never down");
        c.write_all(b"x").unwrap();let t=Instant::now();assert!(readable(&s,Duration::from_secs(5)).unwrap());assert!(t.elapsed()<Duration::from_secs(2));
        let mut b=[0u8;1];s.read_exact(&mut b).unwrap();assert!(writable(&s,Duration::from_secs(5)).unwrap());
        super::sockbuf::set(&s,super::SEND_SOCK_BUF).unwrap();s.set_nonblocking(true).unwrap();let chunk=vec![0u8;super::SEND_SOCK_BUF];
        let mut sent=0usize;while let Ok(n)=s.write(&chunk){sent+=n;assert!(sent<64<<20,"the send buffer never filled");}
        assert!(!writable(&s,Duration::from_millis(30)).unwrap(),"full after {sent} B");
        let r=std::thread::spawn(move||{let mut b=vec![0u8;1<<16];let mut n=0;c.set_read_timeout(Some(Duration::from_secs(5))).unwrap();while n<sent{match c.read(&mut b){Ok(0)|Err(_)=>break,Ok(k)=>n+=k}}n});
        assert!(writable(&s,Duration::from_secs(5)).unwrap(),"writable once the peer reads");drop(s);r.join().unwrap();
    }
    /// Response pools: a reservation is refused once the pool would exceed its size; shrinking
    /// returns the difference; dropping returns the rest; light holds conn_max+1 class maxima.
    /// Second review: a busy answer to a body over JSON_VALUES_MAX is built without parsing it, so
    /// its heap stays far below the parse-tree term (the body itself is the caller's).
    #[test] fn a_busy_answer_does_not_parse_an_over_limit_body(){
        let body=format!("[{}0]","0,".repeat(400_000));assert!(super::json_values(body.as_bytes())>super::JSON_VALUES_MAX);
        let (a,peak)=crate::alloc_meter::peak(||super::busy(body.as_bytes()));
        assert!(a.contains("-32021")&&a.contains("\"id\":null"),"{a}");
        assert!(peak<64*1024,"the busy path requested {peak} B for an unparsed body");
    }
    #[test] fn response_pools_reserve_shrink_and_release(){
        let l=super::Limits{conn_max:3,..Default::default()};let p=super::RespPools::new(&l);
        let a=p.hold_heavy(super::HEAVY_RESP_POOL-10).unwrap();assert!(p.hold_heavy(11).is_none());let mut b=p.hold_heavy(10).unwrap();
        assert_eq!(p.in_use().0,super::HEAVY_RESP_POOL);b.shrink(4);assert_eq!(p.in_use().0,super::HEAVY_RESP_POOL-6);drop(a);assert_eq!(p.in_use().0,4);drop(b);assert_eq!(p.in_use().0,0);
        let lights:Vec<_>=(0..4).map(|_|super::RespPools::take(&p.light,super::LIGHT_RESP_MAX).expect("a light reservation never fails within conn_max+1")).collect();
        assert!(super::RespPools::take(&p.light,1).is_none());drop(lights);assert_eq!(p.in_use().1,0);
    }
    /// RpcGuard round 2: busy and rate answers are built on connection threads by scanning the ids
    /// (no parse tree there, A13). On well-formed requests they equal what the parsing implementation
    /// they replace produced; a key spelled with escapes is the documented difference (id null).
    #[test] fn busy_answers_scan_the_ids_a_parse_would_echo(){
        use serde_json::{json,Value};
        let reference=|body:&[u8]|->String{
            let e=|i:&Value|{let id=super::id_of(i);let id=if id.to_string().len()<=128{id}else{Value::Null};
                json!({"jsonrpc":"2.0","id":id,"error":{"code":-32021,"message":"busy","data":{"reason":"busy","retryAfterMs":250}}}).to_string()};
            match serde_json::from_slice::<Value>(body){Ok(Value::Array(items)) if !items.is_empty()&&items.len()<=super::BATCH_MAX=>format!("[{}]",items.iter().map(e).collect::<Vec<_>>().join(",")),Ok(v)=>e(&v),Err(_)=>e(&Value::Null)}
        };
        let long=format!(r#"{{"id":"{}","method":"m"}}"#,"x".repeat(200));let many=format!("[{}]",vec![r#"{"id":1}"#;21].join(","));
        let cases=[r#"{"jsonrpc":"2.0","id":7,"method":"m"}"#,r#"{"id":"abc","params":[{"id":99}]}"#,r#" {"params":{"id":5},"id":null} "#,r#"{"id":1,"id":2}"#,
            r#"{"id":"a\"b}c]","method":"x"}"#,r#" [ {"id":1}, {"id":"two"}, 3, {"x":{"id":9}}, {"id":[1,{"k":"]"}]}, "s" ] "#,r#"{"method":"m"}"#,r#"{"id":"é"}"#,
            r#"{"id":-1.5e3}"#,r#"[{"id":true},{"id":false},null]"#,&long,&many,"[]","not json","",r#"[{"id":1},]"#,r#"{"id":7} x"#,r#"[{"id":1}] ]"#,"5",r#""s""#];
        for c in cases{assert_eq!(super::busy(c.as_bytes()),reference(c.as_bytes()),"{c}");}
        // JSON escapes, written without a backslash in this source: X stands for one.
        let esc=|s:&str|s.replace('X',&char::from(92u8).to_string());
        for c in [esc(r#"{"id":"aXu00e9Xn","method":"m"}"#),esc(r#"[{"id":"Xu0041"},{"id":"qX"]X"q"}]"#)]{assert_eq!(super::busy(c.as_bytes()),reference(c.as_bytes()),"{c}");}
        let v:Value=serde_json::from_str(&super::busy(esc(r#"{"Xu0069d":5}"#).as_bytes())).unwrap();assert_eq!(v["id"],Value::Null,"an escaped key is not recognised");
        // Round 3: ids are kept as their encodings, and `id_encoding` parses only ids that can fit
        // ID_ECHO_MAX; at the filter's limits the answers are still the parse's. (A body serde_json
        // refuses as a whole, such as a batch nesting past its depth limit, is framed by the scan only.)
        for (name,id) in adversarial_ids(){
            for c in [format!(r#"{{"id":{id},"method":"m"}}"#),format!(r#"[{{"id":{id}}},{{"id":1}}]"#)]{
                if serde_json::from_str::<Value>(&c).is_ok(){assert_eq!(super::busy(c.as_bytes()),reference(c.as_bytes()),"{name}");}}
        }
    }
    /// A13 / CONN_BUF: a busy answer to a 20-member batch carrying 800 KiB of parameters allocates a few
    /// KiB (the answer and the ids), not a parse tree; it fits SMALL_REPLY_MAX. Review 2 (m2): the heap
    /// a connection thread takes for a busy answer, for every adversarial id shape as a singleton and in a
    /// 20-member batch, is within SMALL_REPLY_PEAK (the A13 term per connection).
    #[test] fn a_busy_answer_allocates_no_parse_tree(){
        let body=format!("[{}]",(0..20).map(|i|format!(r#"{{"jsonrpc":"2.0","id":"{}{i:03}","method":"eth_call","params":["{}"]}}"#,"i".repeat(117),"ab".repeat(20_000))).collect::<Vec<_>>().join(","));
        assert!(super::json_values(body.as_bytes())<=super::JSON_VALUES_MAX);
        let (a,peak)=crate::alloc_meter::peak(||super::busy(body.as_bytes()));
        let v:serde_json::Value=serde_json::from_str(&a).unwrap();assert_eq!(v.as_array().unwrap().len(),20);assert_eq!(v[19]["id"],format!("{}019","i".repeat(117)));
        assert!(a.len()<=super::SMALL_REPLY_MAX,"{} B",a.len());assert!(peak<64*1024,"the busy path requested {peak} B");
        let mut worst=(0isize,"");
        for (name,id) in adversarial_ids(){
            let member=format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"eth_call","params":[]}}"#);
            for body in [member.clone(),format!("[{}]",vec![member.as_str();20].join(","))]{
                let (a,peak)=crate::alloc_meter::peak(||super::busy(body.as_bytes()));
                assert!(serde_json::from_str::<serde_json::Value>(&a).is_ok()&&a.len()<=super::SMALL_REPLY_MAX,"{name}: {} B",a.len());
                assert!(peak>=0&&peak as usize<=super::SMALL_REPLY_PEAK,"{name}: the busy path requested {peak} B > SMALL_REPLY_PEAK {}",super::SMALL_REPLY_PEAK);
                if peak>worst.0{worst=(peak,name)}
            }
        }
        println!("BUSY-PEAK {}",serde_json::json!({"worstBytes":worst.0,"shape":worst.1,"smallReplyPeak":super::SMALL_REPLY_PEAK,"idParsePeak":super::ID_PARSE_PEAK}));
    }
    /// CONN_BUF (network.md:217, :221) as pocold bounds it: every error item stays within ERR_ITEM_MAX
    /// with a 128-byte id, so a batch of BATCH_MAX of them is within SMALL_REPLY_MAX.
    #[test] fn error_items_fit_err_item_max_and_a_batch_of_them_small_reply_max(){
        let member=format!(r#"{{"jsonrpc":"2.0","id":"{}","method":"eth_chainId"}}"#,"i".repeat(126));let body=format!("[{}]",vec![member.as_str();20].join(","));
        for (r,t) in [("rate",u64::MAX),("busy",2000)]{let a=super::reply_32021(body.as_bytes(),r,t);assert!(a.len()<=super::SMALL_REPLY_MAX,"{r}: {} B",a.len());
            let v:serde_json::Value=serde_json::from_str(&a).unwrap();for x in v.as_array().unwrap(){assert!(x.to_string().len()<=super::ERR_ITEM_MAX);assert_eq!(x["id"],"i".repeat(126));}}
        let m:serde_json::Value=serde_json::from_str(&member).unwrap();
        for (msg,r) in [("batch response limit","batchBytes"),("work deadline","deadline"),("batch wall","batchWall")]{assert!(super::item_err(&m,-32020,msg,r).len()<=super::ERR_ITEM_MAX);}
    }
    /// The per-connection token bucket on a connection's one request: refused only when its k tokens
    /// exceed the (full) bucket; never at the spec values for k <= BATCH_MAX.
    #[test] fn the_per_connection_bucket_refuses_only_more_tokens_than_its_capacity(){
        let l=super::Limits::default();for k in 1..=super::BATCH_MAX as u32{assert_eq!(super::rate_refused(k,&l),None);}
        let l=super::Limits{bucket_cap:5,rate:20,..Default::default()};assert_eq!(super::rate_refused(5,&l),None);assert_eq!(super::rate_refused(6,&l),Some(50));assert_eq!(super::rate_refused(20,&l),Some(750));
        let l=super::Limits{bucket_cap:0,rate:0,..Default::default()};assert_eq!(super::rate_refused(1,&l),Some(1000));
    }
    /// pocol_getParams.netKey equals the extension's derivation (vectors from pocol-extension netKeyOf).
    #[test] fn net_key_matches_the_extension(){
        for (c,b,k) in [(777001,0x11,"0xacfd846cb4e99df47de0b04af39136202c3d466fdcdd2b1ad7a6a8586b01d044"),(777940,0xab,"0xe8677898e49937cb1b94f29f50d90e54eaef8f91b580fa052f54f0e19b7405a1"),
            (0,0x00,"0xed9c0c48a180be3b26eaa18815306fab3af2e3978f6af428fd3c84f25eef4b72"),(127,0x7f,"0x83e5a43d431ff83f16ee09cbf3c5638baba94d62cfe838a6697217bb023acb60")]{
            assert_eq!(crate::types::hex(&super::net_key(c,&[b;32])),k,"chainId {c}");}
    }
    /// Ids of adversarial shapes (review 2, m2): echoed ones at the limits of `id_encoding`'s filter
    /// (64 nested arrays, 63 elements, 21 keys, 25 nested objects, whitespace), and dropped ones whose
    /// parse tree is the largest the filter lets through (63 objects, 64 nested objects, 128 keys, 1 KiB
    /// of escapes) or that the filter refuses unparsed (1 KiB arrays and objects, 127 nested objects).
    fn adversarial_ids()->Vec<(&'static str,String)>{
        let keys=|n:usize,k:fn(usize)->String|format!("{{{}}}",(0..n).map(|i|format!("\"{}\":0",k(i))).collect::<Vec<_>>().join(","));
        let two=|i:usize|format!("{}{}",(b'a'+(i/26) as u8) as char,(b'a'+(i%26) as u8) as char);
        vec![("string128",format!("\"{}\"","i".repeat(126))),("nestedArrays64",format!("{}{}","[".repeat(64),"]".repeat(64))),
            ("array63",format!("[{}]",vec!["0";63].join(","))),("keys21",keys(21,|i|((b'a'+i as u8) as char).to_string())),
            ("nestedObjects25",format!("{}0{}","{\"\":".repeat(25),"}".repeat(25))),("spacedArray",format!("[{}0]","0  ,  ".repeat(62))),
            ("objects63",format!("[{}]",vec!["{\"a\":0}";63].join(","))),("nestedObjects64",format!("{}0{}","{\"\":".repeat(64),"}".repeat(64))),
            ("keys128",keys(128,two)),("escapes1k",format!("\"{}\"",format!("{}u0041",char::from(92u8)).repeat(170))),
            ("array1k",format!("[{}0]","0,".repeat(511))),("keys120",keys(120,|i|format!("k{i:03}"))),("nestedObjects127",format!("{}0{}","{\"\":".repeat(127),"}".repeat(127)))]
    }
    #[test] fn batch_members_beyond_the_byte_limit_are_not_run(){
        let items:Vec<serde_json::Value>=(0..5).map(|i|serde_json::json!({"jsonrpc":"2.0","id":i,"method":"m"})).collect();let mut ran=vec![];
        let out=super::batch(&items,1000,|k,_|{ran.push(k);serde_json::json!({"jsonrpc":"2.0","id":k,"result":"a".repeat(380)}).to_string()});
        let v:serde_json::Value=serde_json::from_str(&out).unwrap();let a=v.as_array().unwrap();
        assert_eq!(ran,vec![0,1,2],"members after the one that crossed the limit never run");
        assert!(a[0]["result"].is_string()&&a[1]["result"].is_string());
        for (k,x) in a.iter().enumerate().skip(2){assert_eq!(x["error"]["code"],-32020);assert_eq!(x["error"]["data"]["reason"],"batchBytes");assert_eq!(x["id"],k as u64);}
    }
}
