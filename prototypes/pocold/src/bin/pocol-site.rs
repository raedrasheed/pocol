//! pocol-site: publish a static site directory to a PoCol devnet (ChunkFactory
//! chunks + Website version) and fetch/verify it back from any node.
//!   pocol-site publish --rpc 127.0.0.1:8540 --chain-id N --account alice --dir SITE [--site 0x..] [--entry /index.html]
//!   pocol-site fetch   --rpc 127.0.0.1:8541 --site 0x.. [--out DIR]
use std::collections::BTreeMap;
use std::path::{Path,PathBuf};
use pocold::site::{self,Rpc,Publisher};
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
    for e in entries{
        // F12: decide with file_type() (which does NOT follow the final symlink), and refuse symlinks and
        // any non-regular entry. is_dir()/fs::read() follow links, so a planted link inside --dir would
        // make us recurse or read a file OUTSIDE --dir (secrets, key files) and publish it permanently
        // on-chain, and a link to "/" or a loop would exhaust memory. We descend only into real
        // directories and publish only real files; the bytes we read are therefore always under --dir.
        let ft=e.file_type().map_err(|err|err.to_string())?;
        let p=e.path();
        if ft.is_symlink(){return Err(format!("{}: symlink not allowed (it could publish a file outside --dir)",p.display()))}
        if ft.is_dir(){walk(root,&p,out)?;continue}
        if !ft.is_file(){return Err(format!("{}: not a regular file",p.display()))}
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
                let key=pocold::keyfile::account(&a)?;
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

#[cfg(all(test,unix))]
mod f12_tests{
    //! F12: `walk` must not follow symlinks out of --dir. Encodes the FIXED behavior (an Err on a
    //! planted symlink and the target never read), so it FAILS on the original walk (which published
    //! the symlink target's bytes) and PASSES on the fixed one. Synthetic temp files only.
    use super::*;
    #[test]
    fn walk_refuses_symlink_and_never_reads_outside_dir(){
        let base=std::env::temp_dir().join(format!("pocol-site-f12-{}",std::process::id()));
        let _=std::fs::remove_dir_all(&base);
        let site=base.join("site");std::fs::create_dir_all(&site).unwrap();
        // a real page (must still be published) and a secret OUTSIDE the site dir
        std::fs::write(site.join("index.html"),b"<h1>ok</h1>").unwrap();
        let secret=base.join("secret.key");std::fs::write(&secret,b"TOP-SECRET-KEY-MATERIAL").unwrap();
        std::os::unix::fs::symlink(&secret,site.join("notes.txt")).unwrap();

        let mut files=BTreeMap::new();
        let r=walk(&site,&site,&mut files);
        assert!(r.is_err(),"FAIL-ON-ORIGINAL: walk followed the symlink instead of refusing it");
        assert!(r.as_ref().unwrap_err().contains("symlink"),"error names the symlink: {r:?}");
        assert!(files.values().all(|v|v!=b"TOP-SECRET-KEY-MATERIAL"),"the out-of-dir secret must never be read");
        eprintln!("F12: walk refused the planted symlink ({}); secret not published.",r.unwrap_err());

        // A tree of only regular files and real directories still publishes.
        std::fs::remove_file(site.join("notes.txt")).unwrap();
        std::fs::create_dir_all(site.join("css")).unwrap();std::fs::write(site.join("css/a.css"),b"body{}").unwrap();
        let mut ok=BTreeMap::new();walk(&site,&site,&mut ok).expect("a plain tree publishes");
        assert_eq!(ok.len(),2,"index.html + css/a.css");assert!(ok.contains_key("/index.html")&&ok.contains_key("/css/a.css"));
        let _=std::fs::remove_dir_all(&base);
    }
}
