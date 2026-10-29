// Does a finished joinable thread retain its 256 KiB stack until join? Measure RSS.
use std::io::Read;
fn rss()->i64{let mut s=String::new();std::fs::File::open("/proc/self/statm").unwrap().read_to_string(&mut s).unwrap();s.split_whitespace().nth(1).unwrap().parse::<i64>().unwrap()*4096}
fn spawn_n(n:usize)->Vec<std::thread::JoinHandle<()>>{
    (0..n).map(|_|std::thread::Builder::new().stack_size(256*1024).spawn(||{
        // touch the stack so it is really resident, then exit
        let mut buf=[0u8;200*1024];for i in (0..buf.len()).step_by(4096){buf[i]=1}std::hint::black_box(&buf);
    }).unwrap()).collect()
}
fn main(){
    let base=rss();
    let hs=spawn_n(200);
    std::thread::sleep(std::time::Duration::from_millis(300)); // all finished, NOT joined
    let finished_not_joined=rss();
    for h in hs{let _=h.join();}
    std::thread::sleep(std::time::Duration::from_millis(100));
    let after_join=rss();
    // second round, joined promptly
    let base2=rss();
    for _ in 0..200{let h=spawn_n(1);for x in h{let _=x.join();}}
    let joined_promptly=rss();
    println!("baseKiB={} finishedNotJoinedKiB={} (delta {}) afterJoinKiB={} promptJoinDeltaKiB={}",
        base/1024,finished_not_joined/1024,(finished_not_joined-base)/1024,after_join/1024,(joined_promptly-base2)/1024);
}
