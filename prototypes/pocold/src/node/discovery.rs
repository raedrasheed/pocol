//! Peer discovery, the node side (VA-SPEC §1a). The node tells its connections apart by `net::Origin`: one it
//! dialed is known by the address it dialed (the outbound set of `net::dial`, H2), never by the address its socket
//! reports.
#[cfg(test)]
mod tests{
    use crate::node::*;
    use pocol_lp4_execution::devnet::{self,DevnetConfig};
    /// A devnet node without a network (the relay_tests pattern): events are handed to `handle` directly.
    fn open(tag:&str)->(Node,std::path::PathBuf){
        let c=DevnetConfig::new(777919,1_700_000_000,&["a","b","c"]);
        let spec=Spec::new(devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap().0);
        let dir=std::env::temp_dir().join(format!("pocold-disc-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let cfg=Config{dir:dir.clone(),name:tag.into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
        (Node::open(cfg,spec).unwrap(),dir)
    }
    /// H2: `net::dial` puts the address it dials into the outbound set before the handshake and dials again once
    /// the address has left it, so the node clears the set by that address (`Origin::Configured`). A socket whose
    /// `peer_addr` failed (a reset right after connect) reports none, and its `addr` is empty: clearing by `addr`
    /// would stall that dialer for good. Both ways a configured connection ends clear the set: its Closed, and an
    /// incompatible Hello, which removes the peer before its Closed arrives. An inbound connection never does.
    #[test] fn the_outbound_set_is_cleared_by_the_dialed_address(){
        let (mut n,dir)=open("h2");
        let a:SocketAddr="81.2.69.9:30300".parse().unwrap();
        let dialing=|n:&Node|n.outbound.lock().unwrap().contains(&a.to_string());
        // As net::dial does: the address is in the set before the connection is handed to the node.
        let connect=|n:&mut Node,id:PeerId,origin:net::Origin,remote:Option<SocketAddr>|{n.outbound.lock().unwrap().insert(a.to_string());
            let (p,other)=net::test_peer_as(id,4096,origin,remote);n.handle(Event::Net(NetEvent::Connected(id,p)));other};
        let _c1=connect(&mut n,1,net::Origin::Configured(a),None);assert_eq!(n.peers[&1].addr,"","the socket reported no address");
        n.handle(Event::Net(NetEvent::Closed(1)));
        assert!(!dialing(&n),"after Closed the dialer dials again");
        let _c2=connect(&mut n,2,net::Origin::Configured(a),None);
        let mut wrong=n.hello();if let Msg::Hello{chain_id,..}=&mut wrong{*chain_id+=1;}
        n.handle(Event::Net(NetEvent::Message(2,wrong)));
        assert!(n.peer_count()==0&&!dialing(&n),"an incompatible Hello closes the connection, and the dialer dials again");
        let _c3=connect(&mut n,3,net::Origin::Inbound,Some(a));n.handle(Event::Net(NetEvent::Closed(3)));
        assert!(dialing(&n),"an inbound connection from that very address leaves the set alone");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
}
