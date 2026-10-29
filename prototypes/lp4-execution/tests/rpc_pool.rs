#[path="support/admission.rs"] mod f;
use pocol_lp4_execution::{rpc_pool::Router,rpc_http,pool::MAX_POOL_BYTES};
use std::{net::{TcpListener,TcpStream},io::{Read,Write},time::Duration};
fn call(router:&Router<'_>,raw:&str)->serde_json::Value{serde_json::from_str(&router.handle(serde_json::json!({"jsonrpc":"2.0","id":7,"method":"eth_sendRawTransaction","params":[format!("0x{raw}")]}).to_string().as_bytes())).unwrap()}
#[test] fn actual_rpc_handler_rejects_chain_type_unprotected_and_keeps_pool_and_snapshot(){
 let(v,db,block)=f::fixture();let before=format!("{db:?}");let router=Router::new(f::profile(777902,None),&db,&block,MAX_POOL_BYTES);let mut rejected=0;
 for row in v["cases"].as_array().unwrap().iter().filter(|r|matches!(r["error"].as_str(),Some("ChainId"|"UnprotectedLegacy"|"UnsupportedType"))){let response=call(&router,row["raw"].as_str().unwrap());assert_eq!(response["error"]["code"],-32015);assert_eq!(response["id"],7);assert_eq!(router.pending_bytes(),0);rejected+=1;}assert_eq!(rejected,8);
 let row=&v["cases"][0];let response=call(&router,row["raw"].as_str().unwrap());assert_eq!(response["result"],row["view"]["hash"]);assert_eq!(router.pending_bytes(),row["raw"].as_str().unwrap().len()/2);assert_eq!(format!("{db:?}"),before);
 for body in [r#"{"jsonrpc":"2.0","id":1,"id":2,"method":"eth_chainId"}"#,r#"[{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction"}]"#]{let r:serde_json::Value=serde_json::from_str(&router.handle(body.as_bytes())).unwrap();assert!(r["error"].is_object());}
 let r:serde_json::Value=serde_json::from_str(&router.handle(br#"{"jsonrpc":"2.0","id":7,"method":"eth_chainId","params":[]}"#)).unwrap();assert_eq!(r["result"],"0xbdeae");
 for params in [serde_json::json!(["0x0"]),serde_json::json!(["0xgg"]),serde_json::json!([5]),serde_json::json!([])]{let r:serde_json::Value=serde_json::from_str(&router.handle(serde_json::json!({"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":params}).to_string().as_bytes())).unwrap();assert_eq!(r["error"]["code"],-32602);}
}
fn exchange(router:&Router<'_>,body:&str,host:Option<&str>,origin:bool)->(String,usize){
 let listener=TcpListener::bind("127.0.0.1:0").expect("required loopback transport unavailable, NOT PASS");let addr=listener.local_addr().unwrap();let default_host=format!("127.0.0.1:{}",addr.port());let host=host.unwrap_or(&default_host);let request=format!("POST / HTTP/1.1\r\nHost: {host}\r\n{}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",if origin{"Origin: https://invalid.example\r\n"}else{""},body.len());
 let response=std::thread::scope(|scope|{let server=scope.spawn(||{let(socket,_)=listener.accept().unwrap();rpc_http::handle_conn(router,socket,Duration::from_secs(5))});let mut socket=TcpStream::connect(addr).unwrap();socket.set_read_timeout(Some(Duration::from_secs(5))).unwrap();socket.write_all(request.as_bytes()).unwrap();let mut result=String::new();socket.read_to_string(&mut result).unwrap();server.join().unwrap();result});(response,router.pending_bytes())
}
#[test] fn real_loopback_http_submission_delivers_ci2_error_without_state_or_pool_write(){
 let(v,db,block)=f::fixture();let before=format!("{db:?}");let router=Router::new(f::profile(777902,None),&db,&block,MAX_POOL_BYTES);
 let row=v["cases"].as_array().unwrap().iter().find(|r|r["name"]=="wrong-chain-2").unwrap();let body=serde_json::json!({"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":[format!("0x{}",row["raw"].as_str().unwrap())]}).to_string();let(response,bytes)=exchange(&router,&body,None,false);assert!(response.starts_with("HTTP/1.1 200"));let value:serde_json::Value=serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap();assert_eq!(value["error"]["code"],-32015);assert_eq!(bytes,0);
 for (host,origin) in [(Some("bad.example"),false),(None,true)]{let(response,bytes)=exchange(&router,&body,host,origin);assert!(response.starts_with("HTTP/1.1 403"));assert_eq!(bytes,0);}
 assert_eq!(format!("{db:?}"),before);
}
