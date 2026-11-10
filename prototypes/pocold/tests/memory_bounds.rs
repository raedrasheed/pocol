//! PR7 (C-1 S5): resident memory and per-block work that stay O(window), not O(height). The pieces the
//! eviction is built from, each checked on its own: the canonical window type and the checkpoint record
//! codec it shares with the node's canonical index.
use pocold::chain::Canon;

fn hash(i:u64)->[u8;32]{let mut h=[0u8;32];h[..8].copy_from_slice(&(i+1).to_be_bytes());h[31]=0xc5;h}
/// Genesis and heights 1..=n.
fn window(n:u64)->Canon{let mut c=Canon::new(hash(0));c.extend((1..=n).map(hash));c}

#[test] fn a_window_moved_down_and_back_up_equals_the_one_never_moved(){
    let mut c=window(10);let all=c.clone();
    assert_eq!((c.base(),c.len()),(0,11));assert_eq!(c.iter().copied().collect::<Vec<_>>(),(0..=10).map(hash).collect::<Vec<_>>());
    // From base 0 the genesis slot goes with height 1: the base is never 1.
    let mut bases=vec![];for _ in 0..3{let h=c.pop_low().unwrap();bases.push((c.base(),h));}
    assert_eq!(bases,vec![(2,hash(1)),(3,hash(2)),(4,hash(3))]);
    assert_eq!(c.len(),11,"the heights below the window still count");
    assert_eq!((c.get(0),c.get(1),c.get(3),c.get(4),c.get(10),c.get(11)),(Some(&hash(0)),None,None,Some(&hash(4)),Some(&hash(10)),None));
    assert_eq!(c[0],hash(0));assert_eq!(c[7],hash(7));
    assert_eq!(c.iter().copied().collect::<Vec<_>>(),[0,4,5,6,7,8,9,10].map(hash).to_vec(),"genesis, then the window");
    assert_eq!(c.iter_from(2).copied().collect::<Vec<_>>(),(4..=10).map(hash).collect::<Vec<_>>(),"a start below the window starts at its base");
    assert_eq!(c.iter_from(6).copied().collect::<Vec<_>>(),(6..=10).map(hash).collect::<Vec<_>>());
    assert!(c.iter_from(11).next().is_none());
    assert_ne!(c,all);
    for h in (1..=3).rev(){c.push_low(hash(h));}
    assert_eq!(c.base(),0,"back at height 1 the window is base 0 again");assert_eq!(c,all,"and equal to the window never moved");
    assert_eq!(Canon::at_base(hash(0),1),Canon::new(hash(0)),"a window from height 1 is base 0");
    let mut r=Canon::at_base(hash(0),4);r.extend((4..=10).map(hash));let mut d=window(10);for _ in 0..3{d.pop_low();}
    assert_eq!(r,d,"a window restored at its base equals one evicted down to it");
}
#[test] fn a_drain_keeps_genesis_and_the_heights_below_the_window(){
    let mut c=window(6);assert_eq!(c.drain_from(4),(4..=6).map(hash).collect::<Vec<_>>());assert_eq!(c.len(),4);
    assert_eq!(c.drain_from(9),Vec::<[u8;32]>::new(),"nothing above the head");
    let mut w=window(8);w.pop_low();w.pop_low();assert_eq!(w.base(),3);
    assert_eq!(w.drain_from(3),(3..=8).map(hash).collect::<Vec<_>>(),"the whole window from its base");
    assert_eq!((w.len(),w.get(0)),(3,Some(&hash(0))));w.extend([hash(30)]);assert_eq!(w[3],hash(30));
    assert!(window(0).pop_low().is_none(),"genesis never leaves");
    let mut e=Canon::at_base(hash(0),5);assert!(e.pop_low().is_none());assert_eq!(e.base(),5);
}
#[test] #[should_panic(expected="canonical height 2 is not held")]
fn indexing_a_height_below_the_window_panics(){let mut c=window(5);c.pop_low();c.pop_low();let _=c[2];}
