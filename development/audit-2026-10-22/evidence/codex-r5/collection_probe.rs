use std::{alloc::{GlobalAlloc,Layout,System},collections::{HashSet,VecDeque},sync::atomic::{AtomicUsize,Ordering}};
struct A; static LIVE:AtomicUsize=AtomicUsize::new(0);
unsafe impl GlobalAlloc for A {
 unsafe fn alloc(&self,l:Layout)->*mut u8{let p=System.alloc(l);if !p.is_null(){LIVE.fetch_add(l.size(),Ordering::SeqCst);}p}
 unsafe fn dealloc(&self,p:*mut u8,l:Layout){System.dealloc(p,l);LIVE.fetch_sub(l.size(),Ordering::SeqCst);}
 unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{let q=System.realloc(p,l,n);if !q.is_null(){LIVE.fetch_add(n,Ordering::SeqCst);LIVE.fetch_sub(l.size(),Ordering::SeqCst);}q}
}
#[global_allocator] static ALLOC:A=A;
fn main(){
 let mut set=HashSet::<[u8;32]>::new();let mut fifo=VecDeque::<[u8;32]>::new();
 let base=LIVE.load(Ordering::SeqCst);
 for i in 0..1025u64{let mut h=[0;32];h[..8].copy_from_slice(&i.to_le_bytes());set.insert(h);fifo.push_back(h);}
 while fifo.len()>1024{set.remove(&fifo.pop_front().unwrap());}
 let bytes=LIVE.load(Ordering::SeqCst)-base;
 println!("evicted_len={} hash_capacity={} fifo_capacity={} allocated_bytes={} model_bytes={}",fifo.len(),set.capacity(),fifo.capacity(),bytes,1024*2*(32+16));
 assert!(bytes>1024*2*(32+16),"probe must establish the undercount on this compiler");
}
