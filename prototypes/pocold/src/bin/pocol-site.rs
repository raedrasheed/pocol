//! pocol-site: publish a static site directory to a PoCol devnet (ChunkFactory
//! chunks + Website version) and fetch/verify it back from any node.
//!   pocol-site publish --rpc 127.0.0.1:8540 --chain-id N --account alice --dir SITE [--site 0x..] [--entry /index.html]
//!       [--exclude GLOB]... [--include-list FILE] [--max-total-pcol PCOL] [--max-fee-gwei GWEI] [--priority-gwei GWEI] [--economy]
//!   pocol-site fetch   --rpc 127.0.0.1:8541 --site 0x.. [--out DIR]
//! Sites W5: hidden files and directories, config.*, *.key, *.pem, *.sql, *.bak*, api/** and SHA256SUMS* are never
//! published (each is listed with its reason); --exclude and --include-list leave files out explicitly (each listed);
//! an unsupported extension still fails loudly. Sites W4: every transaction is signed just in time at
//! 2 * base fee + tip, within --max-fee-gwei (default 200) and --max-total-pcol (default: no budget).
use std::collections::BTreeMap;
use std::path::{Path,PathBuf};
use pocold::site::{self,Rpc,Publisher};
fn arg(a:&[String],k:&str)->Option<String>{a.iter().position(|x|x==k).and_then(|i|a.get(i+1)).cloned()}
fn args(a:&[String],k:&str)->Vec<String>{a.windows(2).filter(|w|w[0]==k).map(|w|w[1].clone()).collect()}
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
/// The unfiltered walk (F12 regression test): same link rules, no publish-input filter.
#[cfg(test)]
fn walk(root:&Path,dir:&Path,out:&mut BTreeMap<String,Vec<u8>>)->Result<(),String>{walk_inner(root,dir,out,None)}
/// What --exclude, --include-list and the hard-deny list leave out of a walk (site::select_paths' rules).
struct Filter<'a>{exclude:&'a [String],include:Option<&'a [String]>,denied:Vec<(String,&'static str)>,excluded:Vec<String>}
/// Sites W5: a hard-denied directory (.git/, api/ …) is listed once and not entered, so its files are never read.
fn walk_inner(root:&Path,dir:&Path,out:&mut BTreeMap<String,Vec<u8>>,mut filter:Option<&mut Filter<'_>>)->Result<(),String>{
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
        let rel=format!("/{}",p.strip_prefix(root).unwrap().to_string_lossy().replace('\\',"/"));
        if ft.is_dir(){
            if let Some(f)=filter.as_deref_mut(){if let Some(r)=site::deny_reason(&format!("{rel}/x")){f.denied.push((format!("{rel}/"),r));continue}}
            walk_inner(root,&p,out,filter.as_deref_mut())?;continue
        }
        if !ft.is_file(){return Err(format!("{}: not a regular file",p.display()))}
        if let Some(f)=filter.as_deref_mut(){
            let s=site::select_paths(std::slice::from_ref(&rel),f.exclude,f.include);
            if let Some(d)=s.denied.into_iter().next(){f.denied.push(d);continue}
            if !s.excluded.is_empty(){f.excluded.push(rel);continue}
        }
        out.insert(rel,std::fs::read(&p).map_err(|e|e.to_string())?);}
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
                let dir=PathBuf::from(arg(&a,"--dir").ok_or("--dir")?);let mut files=BTreeMap::new();
                let exclude=args(&a,"--exclude");for g in &exclude{if g.trim().is_empty()||g.len()>256{return Err(format!("invalid --exclude {g:?}"))}}
                let include=arg(&a,"--include-list").map(|f|std::fs::read_to_string(&f).map_err(|e|format!("--include-list {f}: {e}")).and_then(|t|site::parse_include_list(&t))).transpose()?;
                let mut filter=Filter{exclude:&exclude,include:include.as_deref(),denied:Vec::new(),excluded:Vec::new()};
                walk_inner(&dir,&dir,&mut files,Some(&mut filter))?;
                for (p,r) in &filter.denied{eprintln!("refused {p}: {r} (never published)");}
                for p in &filter.excluded{eprintln!("excluded {p}");}
                if let Some(w)=&include{let missing:Vec<&String>=w.iter().filter(|p|!files.contains_key(*p)&&!filter.denied.iter().any(|d|&d.0==*p||(d.0.ends_with('/')&&p.starts_with(&d.0)))).collect();if !missing.is_empty(){return Err(format!("--include-list names files that are not in --dir: {missing:?}"))}}
                let fees=site::FeePolicy::from_cli(arg(&a,"--max-total-pcol").as_deref(),arg(&a,"--max-fee-gwei").as_deref(),arg(&a,"--priority-gwei").as_deref(),a.iter().any(|x|x=="--economy"))?;
                let site=arg(&a,"--site").map(|s|s.parse().map_err(|_|"--site address")).transpose()?;
                let p=Publisher::new(&rpc,&key,chain_id,true,fees);
                let (addr,v,m)=p.publish(site,&files,&arg(&a,"--entry").unwrap_or("/index.html".into()))?;
                println!("{}",serde_json::json!({"site":format!("{addr:#x}"),"version":v,"files":m.files.len(),"manifestBytes":m.encode().len(),"manifestHash":format!("{:#x}",revm::primitives::keccak256(m.encode())),
                    "refused":filter.denied.iter().map(|(p,r)|serde_json::json!({"path":p,"reason":r})).collect::<Vec<_>>(),"excluded":filter.excluded,"committedFeeWei":p.committed().to_string()}));Ok(())
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
    #[test]
    fn filtered_walk_lists_refusals_and_exclusions_and_never_reads_a_denied_directory(){
        let base=std::env::temp_dir().join(format!("pocol-site-w5-{}",std::process::id()));let _=std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join(".git/objects")).unwrap();std::fs::create_dir_all(base.join("api")).unwrap();std::fs::create_dir_all(base.join("assets")).unwrap();
        for (p,d) in [("index.html","<p>"),("assets/app.js","1"),("assets/app.js.map","{}"),("api/rpc.php","<?php"),("SHA256SUMS.txt","x"),("config.json","{}"),("notes.txt","n")]{std::fs::write(base.join(p),d).unwrap();}
        // A link inside a denied directory is never followed: the directory is not entered.
        std::os::unix::fs::symlink("/etc/passwd",base.join(".git/objects/link")).unwrap();
        let exclude=vec!["**/*.map".to_string()];let mut f=Filter{exclude:&exclude,include:None,denied:Vec::new(),excluded:Vec::new()};let mut files=BTreeMap::new();
        walk_inner(&base,&base,&mut files,Some(&mut f)).unwrap();
        assert_eq!(files.keys().cloned().collect::<Vec<_>>(),vec!["/assets/app.js","/index.html","/notes.txt"]);
        assert_eq!(f.excluded,vec!["/assets/app.js.map"]);
        let mut denied=f.denied.clone();denied.sort();
        assert_eq!(denied,vec![("/.git/".to_string(),"dotfile"),("/SHA256SUMS.txt".to_string(),"checksums"),("/api/".to_string(),"server-api"),("/config.json".to_string(),"config")]);
        let include=vec!["/index.html".to_string()];let mut g=Filter{exclude:&[],include:Some(&include),denied:Vec::new(),excluded:Vec::new()};let mut only=BTreeMap::new();
        walk_inner(&base,&base,&mut only,Some(&mut g)).unwrap();assert_eq!(only.keys().cloned().collect::<Vec<_>>(),vec!["/index.html"]);assert_eq!(g.excluded.len(),3);
        let _=std::fs::remove_dir_all(&base);
    }
}
