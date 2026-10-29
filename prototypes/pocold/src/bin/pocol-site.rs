//! pocol-site: publish a static site directory to a PoCol devnet (ChunkFactory
//! chunks + Website version) and fetch/verify it back from any node.
//!   pocol-site publish --rpc 127.0.0.1:8540 --chain-id N --account alice --dir SITE [--site 0x..] [--entry /index.html]
//!   pocol-site fetch   --rpc 127.0.0.1:8541 --site 0x.. [--out DIR]
use std::collections::BTreeMap;
use std::path::{Path,PathBuf};
use pocold::site::{self,Rpc,Publisher};
use pocol_lp4_execution::devnet;
fn arg(a:&[String],k:&str)->Option<String>{a.iter().position(|x|x==k).and_then(|i|a.get(i+1)).cloned()}
/// Writes a verified site into an empty directory. Paths valid on-chain can still collide on disk:
/// case-distinct names on case-insensitive filesystems, or a file that is also a directory prefix.
fn write_out(out:&Path,files:&BTreeMap<String,Vec<u8>>)->Result<(),String>{
    if out.exists()&&std::fs::read_dir(out).map_err(|e|e.to_string())?.next().is_some(){return Err(format!("--out {} is not empty",out.display()))}
    let mut lower=std::collections::HashSet::new();
    for p in files.keys(){
        if !lower.insert(p.to_ascii_lowercase()){return Err(format!("{p}: collides with another path on case-insensitive filesystems"))}
        if files.keys().any(|q|q.len()>p.len()&&q.starts_with(p.as_str())&&q.as_bytes()[p.len()]==b'/'){return Err(format!("{p}: is both a file and a directory prefix"))}
    }
    for (p,d) in files{let f=out.join(&p[1..]);std::fs::create_dir_all(f.parent().unwrap()).map_err(|e|e.to_string())?;std::fs::write(f,d).map_err(|e|e.to_string())?;}
    Ok(())
}
fn walk(root:&Path,dir:&Path,out:&mut BTreeMap<String,Vec<u8>>)->Result<(),String>{
    let mut entries:Vec<_>=std::fs::read_dir(dir).map_err(|e|e.to_string())?.filter_map(|e|e.ok()).collect();entries.sort_by_key(|e|e.path());
    for e in entries{let p=e.path();if p.is_dir(){walk(root,&p,out)?;continue}
        let rel=p.strip_prefix(root).unwrap().to_string_lossy().replace('\\',"/");out.insert(format!("/{rel}"),std::fs::read(&p).map_err(|e|e.to_string())?);}
    Ok(())
}
fn main(){
    let a:Vec<String>=std::env::args().collect();
    let r=(||->Result<(),String>{
        let rpc=Rpc{addr:arg(&a,"--rpc").unwrap_or("127.0.0.1:8540".into())};
        match a.get(1).map(|s|s.as_str()){
            Some("publish")=>{
                let chain_id:u64=arg(&a,"--chain-id").ok_or("--chain-id")?.parse().map_err(|_|"chain id")?;
                let key=devnet::account_key(&arg(&a,"--account").ok_or("--account")?);
                let dir=PathBuf::from(arg(&a,"--dir").ok_or("--dir")?);let mut files=BTreeMap::new();walk(&dir,&dir,&mut files)?;
                let site=arg(&a,"--site").map(|s|s.parse().map_err(|_|"--site address")).transpose()?;
                let p=Publisher{rpc:&rpc,key:&key,chain_id,log:true};
                let (addr,v,m)=p.publish(site,&files,&arg(&a,"--entry").unwrap_or("/index.html".into()))?;
                println!("{}",serde_json::json!({"site":format!("{addr:#x}"),"version":v,"files":m.files.len(),"manifestBytes":m.encode().len(),"manifestHash":format!("{:#x}",revm::primitives::keccak256(m.encode()))}));Ok(())
            }
            Some("fetch")=>{
                let site:revm::primitives::Address=arg(&a,"--site").ok_or("--site")?.parse().map_err(|_|"--site")?;
                let (cur,m,files)=site::fetch(&rpc,site)?;
                if let Some(out)=arg(&a,"--out"){write_out(&PathBuf::from(&out),&files)?;}
                println!("{}",serde_json::json!({"version":cur.version,"publishedBlock":cur.published_block,"entry":m.files[m.entry as usize].path,"files":files.iter().map(|(p,d)|serde_json::json!({"path":p,"bytes":d.len(),"keccak":format!("{}",revm::primitives::keccak256(d))})).collect::<Vec<_>>()}));Ok(())
            }
            _=>Err("usage: pocol-site publish|fetch ...".into())
        }
    })();
    if let Err(e)=r{eprintln!("error: {e}");std::process::exit(1)}
}
