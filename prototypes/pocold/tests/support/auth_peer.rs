//! A real transport handshake for tests that then inject deterministic handler events.
//! No production authentication bypass or test-only constructor is added to the node.
use std::{io::Write, net::{TcpListener, TcpStream}, sync::mpsc, time::Duration};
use pocold::{net::{self, Msg, NetEvent, PeerId}, node::{Event, Node}};

pub struct Connection(TcpStream);
impl Drop for Connection { fn drop(&mut self) { let _ = self.0.shutdown(std::net::Shutdown::Both); } }

pub fn connect(node: &mut Node) -> (PeerId, Connection) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (tx, rx) = mpsc::sync_channel(net::NET_EVENT_QUEUE);
    net::listen(listener, tx, net::Net::new(net::Limits::default()));
    let mut socket = TcpStream::connect(address).unwrap();
    let connected = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    let id = match &connected { NetEvent::Connected(id, _) => *id, _ => panic!("expected Connected") };
    node.handle(Event::Net(connected));
    let hello = Msg::Hello { chain_id: node.chain.spec.chain_id(), genesis: node.chain.spec.genesis_hash(),
        head: node.chain.spec.genesis_hash(), height: 0, work: [0; 32], node: "test-peer".into(),
        protocol: pocold::identity::PROTOCOL_VERSION, compatibility: pocold::identity::compatibility() }.encode();
    socket.write_all(&(hello.len() as u32).to_be_bytes()).unwrap(); socket.write_all(&hello).unwrap();
    let accepted = rx.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(matches!(&accepted, NetEvent::Message(i, Msg::Hello { .. }) if *i == id));
    node.handle(Event::Net(accepted));
    assert!(node.peer_status.contains_key(&id));
    let mut drain = socket.try_clone().unwrap();
    std::thread::spawn(move || { let _ = std::io::copy(&mut drain, &mut std::io::sink()); });
    // Forward closure/transport events while the test keeps using the handler/channel.
    let dest = node.tx.clone();
    std::thread::spawn(move || { for event in rx { if dest.send(Event::Net(event)).is_err() { break; } } });
    (id, Connection(socket))
}
