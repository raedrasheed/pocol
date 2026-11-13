//! The command line's exit status. A command this build does not have, or no command, fails
//! with status 2 and the usage on stderr, so a script cannot take a command that never ran for
//! success (for example `cutover` given to a release without it). `help`, `--help` and `-h` print
//! the usage on stdout with status 0. Every command the usage lists runs; its own errors exit 1.
use std::path::PathBuf;
use std::process::{Command,Output};

fn pocold(args:&[&str])->Output{Command::new(env!("CARGO_BIN_EXE_pocold")).args(args).output().unwrap()}
fn text(b:&[u8])->String{String::from_utf8_lossy(b).into_owned()}
fn tmp(tag:&str)->PathBuf{let d=std::env::temp_dir().join(format!("pocold-cli-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
const EXIT_LINE:&str="Exit status: 0 done; 1 the command failed; 2 no command, or one this build does not have; 70 a local storage failure stopped the node; 101 an internal failure.";

#[test] fn an_unknown_command_exits_2_names_it_and_runs_nothing(){
    let d=tmp("unknown");std::fs::create_dir_all(&d).unwrap();
    let (net,data,config,out)=(d.join("old"),d.join("data"),d.join("new.json"),d.join("new"));
    std::fs::create_dir_all(&net).unwrap();std::fs::create_dir_all(&data).unwrap();std::fs::write(&config,"{}").unwrap();
    let path=|p:&PathBuf|p.to_str().unwrap().to_string();
    let rest=["--network".to_string(),path(&net),"--data".into(),path(&data),"--height".into(),"5".into(),"--config".into(),path(&config),"--out".into(),path(&out)];
    for cmd in ["cutovr","Cutover","CUTOVER","cut-over","run2","stat","init","build_info","version","--version","-V","--network","-x","HELP","-help","--h",""," run","run ","تشغيل"]{
        for extra in [&[][..],&rest[..]]{
            let mut a=vec![cmd];a.extend(extra.iter().map(|s|s.as_str()));
            let o=pocold(&a);let err=text(&o.stderr);
            assert_eq!(o.status.code(),Some(2),"{a:?}: {err}");
            assert!(err.starts_with(&format!("error: unknown command {cmd:?}\nusage:\n")),"{a:?}: {err}");
            assert!(err.trim_end().ends_with(EXIT_LINE),"{a:?}: {err}");
            assert!(o.stdout.is_empty(),"{a:?}: {}",text(&o.stdout));
            assert!(!out.exists(),"{a:?}: an unknown command must not write anything");
        }
    }
    let _=std::fs::remove_dir_all(&d);
}

#[test] fn no_command_exits_2_with_the_usage_on_stderr(){
    let o=pocold(&[]);let err=text(&o.stderr);
    assert_eq!(o.status.code(),Some(2),"{err}");
    assert!(err.starts_with("usage:\n")&&err.trim_end().ends_with(EXIT_LINE),"{err}");
    assert!(o.stdout.is_empty(),"{}",text(&o.stdout));
    assert_eq!(err,text(&pocold(&["help"]).stdout),"the same usage text as help");
}

#[test] fn help_exits_0_with_the_usage_on_stdout(){
    for a in [&["help"][..],&["--help"],&["-h"],&["help","run"],&["--help","--network","x"]]{
        let o=pocold(a);let out=text(&o.stdout);
        assert_eq!(o.status.code(),Some(0),"{a:?}: {}",text(&o.stderr));
        assert!(out.starts_with("usage:\n")&&out.trim_end().ends_with(EXIT_LINE),"{a:?}: {out}");
        assert!(o.stderr.is_empty(),"{a:?}: {}",text(&o.stderr));
    }
}

/// The usage names exactly the commands this build runs, and each of them is dispatched: without
/// its options it fails as that command (status 1), not as an unknown one (status 2).
#[test] fn every_command_in_the_usage_runs(){
    let usage=text(&pocold(&["help"]).stdout);
    let mut listed:Vec<&str>=usage.lines().filter_map(|l|l.strip_prefix("  pocold ")).map(|l|l.split_whitespace().next().unwrap()).collect();
    listed.sort();
    assert_eq!(listed,["build-info","cutover","export-chain","help","init-devnet","init-testnet","run","status","verify-export"],"{usage}");
    for cmd in listed{
        let o=pocold(&[cmd]);let err=text(&o.stderr);
        match cmd{
            "build-info"=>{
                assert_eq!(o.status.code(),Some(0),"{err}");
                let info:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap_or_else(|e|panic!("build-info is JSON: {e}"));
                assert!(info.is_object(),"{info}");
            }
            "help"=>assert_eq!(o.status.code(),Some(0),"{err}"),
            _=>{
                assert_eq!(o.status.code(),Some(1),"{cmd}: {err}");
                assert!(err.starts_with("error: ")&&!err.contains("unknown command"),"{cmd}: {err}");
                assert!(o.stdout.is_empty(),"{cmd}: {}",text(&o.stdout));
            }
        }
    }
}

/// A first argument that is not valid text (bytes on Unix, an unpaired surrogate on Windows) is an
/// unknown command too, not a panic.
#[test] fn a_command_that_is_not_text_exits_2(){
    #[cfg(unix)] let arg={use std::os::unix::ffi::OsStrExt;std::ffi::OsStr::from_bytes(b"run\xff").to_os_string()};
    #[cfg(windows)] let arg={use std::os::windows::ffi::OsStringExt;std::ffi::OsString::from_wide(&[0x72,0x75,0x6e,0xD800])};
    let o=Command::new(env!("CARGO_BIN_EXE_pocold")).arg(&arg).output().unwrap();let err=text(&o.stderr);
    assert_eq!(o.status.code(),Some(2),"{err}");
    assert!(err.starts_with("error: unknown command ")&&err.trim_end().ends_with(EXIT_LINE)&&!err.contains("panicked"),"{err}");
    assert!(o.stdout.is_empty());
}

/// `help` into a pipe whose reader has already gone still exits 0, without a panic.
#[test] fn help_into_a_closed_pipe_exits_0(){
    use std::process::Stdio;
    for _ in 0..20{
        let mut c=Command::new(env!("CARGO_BIN_EXE_pocold")).arg("help").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
        drop(c.stdout.take());
        let o=c.wait_with_output().unwrap();
        assert_eq!(o.status.code(),Some(0),"{}",text(&o.stderr));assert!(o.stderr.is_empty(),"{}",text(&o.stderr));
    }
}

/// The inbound per-address-group limit is a local P2P setting: `--per-group-max N`, or for `run` (the only
/// command that opens the network) POCOL_PER_GROUP_MAX when the flag is absent, so a service changes it in
/// its environment. A bad value fails the command (status 1) before anything runs; the flag wins over the
/// environment; an empty value counts as unset; other commands do not read the environment.
#[test] fn per_group_max_comes_from_the_flag_or_the_environment(){
    let run=|args:&[&str],env:Option<&str>|{let mut c=Command::new(env!("CARGO_BIN_EXE_pocold"));c.args(args);
        match env{Some(v)=>{c.env("POCOL_PER_GROUP_MAX",v);}None=>{c.env_remove("POCOL_PER_GROUP_MAX");}}c.output().unwrap()};
    for (args,env,want) in [(&["run","--per-group-max","x"][..],None,"--per-group-max x:"),(&["run"][..],Some("y"),"POCOL_PER_GROUP_MAX y:"),
        (&["run","--per-group-max","-1"][..],Some("2"),"--per-group-max -1:"),(&["status","--per-group-max","z"][..],None,"--per-group-max z:")]{
        let o=run(args,env);let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(err.contains(want),"{args:?} {env:?}: {err}");
    }
    // A valid, empty or (outside run) ignored value passes the limits; the command fails later, on the missing --network.
    for (args,env) in [(&["run"][..],"0"),(&["run"][..],""),(&["run"][..],"  "),(&["run","--per-group-max","3"][..],"bad"),(&["status"][..],"bad")]{
        let o=run(args,Some(env));let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(!err.contains("per-group-max")&&!err.contains("PER_GROUP_MAX"),"{args:?} {env:?}: {err}");
    }
}

/// The established inbound peers are a local P2P setting (PR3): `--peers-max N`, 1..=128, or for `run` POCOL_PEERS_MAX
/// when the flag is absent. A value that is not a number, 0 or past 128 fails the command (status 1) before anything
/// runs, by flag and by environment; the flag wins over the environment; an empty value counts as unset; `status` does
/// not read the environment. What a running node took is read back from its pocol_status `inboundMax`.
#[test] fn peers_max_comes_from_the_flag_or_the_environment(){
    let cmd=|args:&[&str],env:Option<&str>|{let mut c=Command::new(env!("CARGO_BIN_EXE_pocold"));c.args(args);
        match env{Some(v)=>{c.env("POCOL_PEERS_MAX",v);}None=>{c.env_remove("POCOL_PEERS_MAX");}}c};
    for (args,env,want) in [(&["run","--peers-max","x"][..],None,"--peers-max x:"),(&["run","--peers-max","0"][..],None,"--peers-max 0: must be 1..=128"),
        (&["run","--peers-max","129"][..],None,"--peers-max 129: must be 1..=128"),(&["run"][..],Some("x"),"POCOL_PEERS_MAX x:"),
        (&["run"][..],Some("0"),"POCOL_PEERS_MAX 0: must be 1..=128"),(&["run"][..],Some("129"),"POCOL_PEERS_MAX 129: must be 1..=128"),
        (&["run","--peers-max","129"][..],Some("5"),"--peers-max 129:"),(&["status","--peers-max","0"][..],None,"--peers-max 0:")]{
        let o=cmd(args,env).output().unwrap();let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(err.contains(want),"{args:?} {env:?}: {err}");
    }
    // A valid, empty or (outside run, or under the flag) ignored value passes the limits; the command fails later, on the missing --network.
    for (args,env) in [(&["run"][..],"1"),(&["run"][..],"128"),(&["run"][..],""),(&["run"][..],"  "),(&["run","--peers-max","3"][..],"bad"),(&["status"][..],"bad")]{
        let o=cmd(args,Some(env)).output().unwrap();let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(err.contains("--network")&&!err.contains("peers-max")&&!err.contains("PEERS_MAX"),"{args:?} {env:?}: {err}");
    }
    let d=tmp("peers-max");let net=d.join("net");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let path=|p:PathBuf|p.to_str().unwrap().to_string();
    for (i,(flags,env,want)) in [(&[][..],None,32u64),(&[],Some(" 5 "),5),(&["--peers-max","7"],Some("5"),7),(&[],Some(" "),32),(&["--peers-max","128"],None,128)].into_iter().enumerate(){
        let data=path(d.join(format!("data{i}")));
        let rpc={let l=std::net::TcpListener::bind("127.0.0.1:0").unwrap();l.local_addr().unwrap()};
        let mut c=cmd(&[&["run","--network",&path(net.clone()),"--data",&data,"--rpc",&rpc.to_string()][..],flags].concat(),env).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().unwrap();
        let t=std::time::Instant::now();let mut seen=None;
        while seen.is_none()&&t.elapsed()<std::time::Duration::from_secs(60){
            if let Some(s)=c.try_wait().unwrap(){panic!("run {flags:?} {env:?} exited with {s}")}
            seen=rpc_status(rpc);if seen.is_none(){std::thread::sleep(std::time::Duration::from_millis(50));}
        }
        let _=c.kill();let _=c.wait();
        let v=seen.unwrap_or_else(||panic!("run {flags:?} {env:?}: no pocol_status within 60 s"));
        assert_eq!(v["inboundMax"].as_u64(),Some(want),"run {flags:?} {env:?}");
    }
    let _=std::fs::remove_dir_all(&d);
}

/// Peer discovery's switches (VA-SPEC §1a, plan §7): `--no-discovery` and `--discover-local`, or for `run` (the only
/// command that opens the network) POCOL_DISCOVERY and POCOL_DISCOVER_LOCAL, each 0 or 1, when the flag is absent. A
/// bad value fails `run` (status 1) before anything runs, naming the variable; a flag wins over its variable; an empty
/// value counts as unset; `status` does not read the environment. The settings a running node took are read back from
/// its pocol_status.
#[test] fn discovery_settings_come_from_flags_or_the_environment(){
    const VARS:[&str;2]=["POCOL_DISCOVERY","POCOL_DISCOVER_LOCAL"];
    let cmd=|args:&[&str],env:[Option<&str>;2]|{let mut c=Command::new(env!("CARGO_BIN_EXE_pocold"));c.args(args);
        for (k,v) in VARS.iter().zip(env){match v{Some(v)=>{c.env(k,v);}None=>{c.env_remove(k);}}}c};
    for (args,env,want) in [(&["run"][..],[Some("x"),None],"POCOL_DISCOVERY x: expected 0 or 1"),(&["run"][..],[None,Some("2")],"POCOL_DISCOVER_LOCAL 2: expected 0 or 1"),
        (&["run"][..],[Some("true"),Some("0")],"POCOL_DISCOVERY true:"),(&["run","--no-discovery"][..],[Some("0"),Some("yes")],"POCOL_DISCOVER_LOCAL yes:")]{
        let o=cmd(args,env).output().unwrap();let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(err.contains(want),"{args:?} {env:?}: {err}");
    }
    // A flag, an empty value or (outside run) any value leaves the variable unread; the command fails later, on the missing --network.
    for (args,env) in [(&["run","--no-discovery","--discover-local"][..],[Some("x"),Some("2")]),(&["run"][..],[Some(""),Some("  ")]),(&["run"][..],[Some(" 1 "),Some("0")]),(&["status"][..],[Some("x"),Some("2")])]{
        let o=cmd(args,env).output().unwrap();let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(err.contains("--network")&&!err.contains("POCOL_DISCOVER"),"{args:?} {env:?}: {err}");
    }
    // What a node took: `run` from its flags and environment, `status` from its flags only.
    let d=tmp("discovery");let net=d.join("net");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let path=|p:PathBuf|p.to_str().unwrap().to_string();
    let took=|v:&serde_json::Value|(v["discovery"]["enabled"].as_bool(),v["discovery"]["allowLocal"].as_bool());
    for (i,(flags,env,want)) in [(&[][..],[None,None],(true,false)),(&[],[Some("0"),Some("1")],(false,true)),(&[],[Some(" 1 "),Some("  ")],(true,false)),
        (&["--no-discovery","--discover-local"],[Some("1"),Some("0")],(false,true))].into_iter().enumerate(){
        let data=path(d.join(format!("data{i}")));
        let st=cmd(&[&["status","--network",&path(net.clone()),"--data",&data][..],flags].concat(),[Some("0"),Some("1")]).output().unwrap();
        assert_eq!(st.status.code(),Some(0),"{}",text(&st.stderr));
        let flags_only=(!flags.contains(&"--no-discovery"),flags.contains(&"--discover-local"));
        assert_eq!(took(&serde_json::from_slice(&st.stdout).unwrap()),(Some(flags_only.0),Some(flags_only.1)),"status {flags:?} reads its flags, never the environment");
        let rpc={let l=std::net::TcpListener::bind("127.0.0.1:0").unwrap();l.local_addr().unwrap()};
        let mut c=cmd(&[&["run","--network",&path(net.clone()),"--data",&data,"--rpc",&rpc.to_string()][..],flags].concat(),env).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().unwrap();
        let t=std::time::Instant::now();let mut seen=None;
        while seen.is_none()&&t.elapsed()<std::time::Duration::from_secs(60){
            if let Some(s)=c.try_wait().unwrap(){panic!("run {flags:?} {env:?} exited with {s}")}
            seen=rpc_status(rpc);if seen.is_none(){std::thread::sleep(std::time::Duration::from_millis(50));}
        }
        let _=c.kill();let _=c.wait();
        let v=seen.unwrap_or_else(||panic!("run {flags:?} {env:?}: no pocol_status within 60 s"));
        assert_eq!(took(&v),(Some(want.0),Some(want.1)),"run {flags:?} {env:?}");
    }
    let _=std::fs::remove_dir_all(&d);
}
/// Automatic outbound and seed settings (VA-SPEC §1a.7-§1a.9, plan §7): `--outbound N|auto` (0..8) or for `run`
/// POCOL_OUTBOUND when the flag is absent, `--seed` (at most 7, distinct, a usable address) and `--bootstrap` (at most
/// 16, ports >= 1024), neither with discovery off, and the 18 outbound places (network.md:157). A bad value fails `run`
/// (status 1) naming it, before the network is read; a good one passes the settings and `run` fails later, on the
/// missing --network. What `status` resolved is read back from `discovery.outbound.automatic`.
#[test] fn outbound_and_seed_settings_are_validated(){
    let cmd=|args:&[String],env:Option<&str>|{let mut c=Command::new(env!("CARGO_BIN_EXE_pocold"));c.args(args);
        match env{Some(v)=>{c.env("POCOL_OUTBOUND",v);}None=>{c.env_remove("POCOL_OUTBOUND");}}c.env_remove("POCOL_DISCOVERY");c.output().unwrap()};
    let v=|a:&[&str]|a.iter().map(|s|s.to_string()).collect::<Vec<_>>();
    let many=|flag:&str,n:usize,port:u16|(0..n).flat_map(|i|[flag.to_string(),format!("81.2.0.{}:{port}",i+1)]).collect::<Vec<_>>();
    let run=|rest:Vec<String>|[v(&["run"]),rest].concat();
    for (args,env,want) in [(run(v(&["--outbound","9"])),None,"--outbound 9: expected auto or 0..=8"),(run(v(&["--outbound","x"])),None,"--outbound x:"),
        (run(vec![]),Some("9"),"POCOL_OUTBOUND 9: expected auto or 0..=8"),(run(many("--seed",8,30300)),None,"--seed: at most 7"),
        (run(v(&["--seed","1.2.3.4:30300","--seed","1.2.3.4:30300"])),None,"--seed 1.2.3.4:30300: given twice"),
        (run(v(&["--seed","1.2.3.4:0"])),None,"--seed 1.2.3.4:0: not a usable address"),(run(v(&["--seed","0.0.0.0:30300"])),None,"--seed 0.0.0.0:30300: not a usable address"),
        (run(v(&["--seed","1.2.3.4:30300","--no-discovery"])),None,"--seed and --bootstrap need peer discovery"),
        (run(v(&["--bootstrap","1.2.3.4:30300","--no-discovery"])),None,"--seed and --bootstrap need peer discovery"),
        (run(v(&["--bootstrap","1.2.3.4:80"])),None,"--bootstrap 1.2.3.4:80: not a usable address"),(run(many("--bootstrap",17,30300)),None,"--bootstrap: at most 16"),
        (run([v(&["--outbound","4"]),many("--peer",14,30300)].concat()),None,"14 configured peers and 5 automatic outbound slots exceed 18"),
        (run(v(&["--outbound","1","--no-discovery"])),None,"needs peer discovery"),(v(&["status","--outbound","9"]),None,"--outbound 9:")]{
        let o=cmd(&args,env);let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(err.contains(want),"{args:?} {env:?}: {err}");
    }
    for (args,env) in [(run(v(&["--outbound","auto"])),None),(run(v(&["--outbound","0"])),None),(run(vec![]),Some(" 2 ")),(run(vec![]),Some(" ")),
        (run(v(&["--outbound","3"])),Some("bad")),(run([many("--seed",7,30300),many("--bootstrap",16,1024)].concat()),None),(run([v(&["--outbound","4"]),many("--peer",13,30300)].concat()),None)]{
        let o=cmd(&args,env);let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?} {env:?}: {err}");
        assert!(err.contains("--network")&&!err.contains("outbound")&&!err.contains("OUTBOUND")&&!err.contains("seed")&&!err.contains("bootstrap"),"{args:?} {env:?}: {err}");
    }
    let d=tmp("outbound");let net=d.join("net");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    for (i,(flag,want)) in [("2",2u64),("auto",2),("0",0)].into_iter().enumerate(){
        let data=d.join(format!("data{i}"));
        let st=cmd(&v(&["status","--network",net.to_str().unwrap(),"--data",data.to_str().unwrap(),"--outbound",flag]),Some("8"));assert_eq!(st.status.code(),Some(0),"{}",text(&st.stderr));let st:serde_json::Value=serde_json::from_slice(&st.stdout).unwrap();
        assert_eq!(st["discovery"]["outbound"]["automatic"].as_u64(),Some(want),"status --outbound {flag} (no listener; the variable is not read)");
    }
    let _=std::fs::remove_dir_all(&d);
}
/// pocol_status from a node's RPC port, None until it answers.
fn rpc_status(rpc:std::net::SocketAddr)->Option<serde_json::Value>{
    use std::io::{Read,Write};
    let body=r#"{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}"#;
    let mut s=std::net::TcpStream::connect(rpc).ok()?;s.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok()?;
    s.write_all(format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).ok()?;
    let mut out=String::new();let _=s.read_to_string(&mut out);
    let v=serde_json::from_str::<serde_json::Value>(out.split("\r\n\r\n").nth(1)?).ok()?["result"].clone();v.is_object().then_some(v)
}

/// D20: only `pocold run` catches SIGINT and SIGTERM; any other command still ends at once. `pocold status` here
/// blocks reading its genesis file, a FIFO: the writer's open returns once pocold opened it (the ordering barrier,
/// past main), then SIGTERM must kill it. With the handlers installed in main it was caught, the read went on, and
/// the command ran until the writer closed.
#[cfg(unix)]
#[test] fn pocold_status_still_dies_on_sigterm(){
    use std::os::unix::process::ExitStatusExt;
    let d=tmp("status-term");let net=d.join("net");std::fs::create_dir_all(&net).unwrap();
    let fifo=net.join("genesis.spec.hex");let c=std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();assert_eq!(unsafe{libc::mkfifo(c.as_ptr(),0o600)},0);
    let mut p=Command::new(env!("CARGO_BIN_EXE_pocold")).args(["status","--network",net.to_str().unwrap(),"--data",d.join("data").to_str().unwrap()])
        .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn().unwrap();
    let w=std::fs::OpenOptions::new().write(true).open(&fifo).unwrap();
    assert_eq!(unsafe{libc::kill(p.id() as i32,libc::SIGTERM)},0);
    let t=std::time::Instant::now();
    let s=loop{if let Some(s)=p.try_wait().unwrap(){break s}if t.elapsed()>std::time::Duration::from_secs(10){drop(w);break p.wait().unwrap()}std::thread::sleep(std::time::Duration::from_millis(20));};
    assert_eq!((s.signal(),s.code()),(Some(libc::SIGTERM),None),"pocold status ends by SIGTERM");
    let _=std::fs::remove_dir_all(&d);
}
/// D20 (VA-SPEC §1a.6): `pocold run` stops cleanly on SIGTERM, as `systemctl stop` sends it: it leaves its run loop at the
/// next event boundary, saves the address book if it changed, prints its final status on stdout and exits 0. Without a
/// handler the signal killed it (no status, no save, exit by signal). (a) A listening node with no peers: a clean book
/// is not written, so only the exit and the status are expected. (b) A node with a `--peer` that answers: once its
/// peer's Hello verified (dirtied) the book, SIGTERM leaves `addrbook` in its data directory.
#[cfg(unix)]
#[test] fn pocold_run_stops_cleanly_on_sigterm(){
    use std::process::{Child,Stdio};
    let d=tmp("sigterm");let net=d.join("net");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let path=|p:&PathBuf|p.to_str().unwrap().to_string();
    // The node's ports stay held until right before it starts (tests/discovery.rs).
    let hold=||{let held=[std::net::TcpListener::bind("127.0.0.1:0").unwrap(),std::net::TcpListener::bind("127.0.0.1:0").unwrap()];let a=held.each_ref().map(|l|l.local_addr().unwrap());(held,a)};
    let start=|data:&PathBuf,extra:&[String]|->(Child,std::net::SocketAddr,std::net::SocketAddr){
        let (held,[p2p,rpc])=hold();
        let mut args=vec!["run".to_string(),"--network".into(),path(&net),"--data".into(),path(data),"--listen".into(),p2p.to_string(),"--rpc".into(),rpc.to_string()];args.extend(extra.iter().cloned());
        drop(held);
        let c=Command::new(env!("CARGO_BIN_EXE_pocold")).args(&args).env_remove("POCOL_DISCOVERY").stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();(c,p2p,rpc)
    };
    // Polls pocol_status until `ready` holds (the ordering barrier), within 60 s.
    let wait=|c:&mut Child,rpc,ready:&dyn Fn(&serde_json::Value)->bool|{let t=std::time::Instant::now();
        loop{if let Some(s)=c.try_wait().unwrap(){panic!("run exited early with {s}")}
            if rpc_status(rpc).is_some_and(|v|ready(&v)){return}
            assert!(t.elapsed()<std::time::Duration::from_secs(60),"not ready within 60 s");std::thread::sleep(std::time::Duration::from_millis(50));}};
    // SIGTERM, then the exit within 10 s: its status and the final status JSON it printed.
    let term=|mut c:Child|->(Option<i32>,serde_json::Value){
        assert_eq!(unsafe{libc::kill(c.id() as i32,libc::SIGTERM)},0);
        let t=std::time::Instant::now();
        let s=loop{if let Some(s)=c.try_wait().unwrap(){break s}if t.elapsed()>std::time::Duration::from_secs(10){let _=c.kill();panic!("no exit within 10 s of SIGTERM")}std::thread::sleep(std::time::Duration::from_millis(20));};
        let mut out=String::new();std::io::Read::read_to_string(c.stdout.as_mut().unwrap(),&mut out).unwrap();
        (s.code(),serde_json::from_str(out.trim()).unwrap_or_else(|e|panic!("no final status on stdout ({e}): {out:?}")))
    };
    // (a)
    let a=d.join("a");let (mut na,_,rpc_a)=start(&a,&[]);wait(&mut na,rpc_a,&|_|true);
    let (code,st)=term(na);
    assert_eq!(code,Some(0),"SIGTERM is a clean stop");assert!(st["discovery"].is_object(),"the final status: {st}");
    assert!(!a.join("addrbook").exists(),"a clean book is not written");
    // (b)
    let b=d.join("b");let (mut nb,p2p_b,rpc_b)=start(&b,&[]);wait(&mut nb,rpc_b,&|_|true);
    let c=d.join("c");let (mut nc,_,rpc_c)=start(&c,&["--peer".into(),p2p_b.to_string()]);
    wait(&mut nc,rpc_c,&|v|v["compatiblePeers"].as_u64()>=Some(1));
    let (code,st)=term(nc);
    assert_eq!((code,st["compatiblePeers"].as_u64()),(Some(0),Some(1)),"a clean stop with its peer: {st}");
    assert!(c.join("addrbook").exists(),"the book the peer's Hello changed is saved at the clean stop");
    let _=term(nb);
    let _=std::fs::remove_dir_all(&d);
}
/// PR7 (T17): `run` takes the A13 memory budget from `--memory-budget-bytes N`, or POCOL_MEMORY_BUDGET_BYTES when the
/// flag is absent (as `pocol start` does), and the eviction depth from `--evict-depth N`. A budget of 1 byte is refused
/// at open by the bound, by flag and by variable; a value that is not a number fails the command, an empty variable
/// too; a depth below the safe minimum is refused naming the term it fails (whole-state and store mode). Before PR7
/// `run` ignored both flags and the variable and ran a node.
#[test] fn memory_budget_and_evict_depth_come_from_the_flags_or_the_environment(){
    let d=tmp("budget");let net=d.join("net");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let run=|i:usize,flags:&[&str],env:Option<&str>|->(Option<i32>,String){
        let data=d.join(format!("data{i}"));let mut c=Command::new(env!("CARGO_BIN_EXE_pocold"));
        c.args([&["run","--network",net.to_str().unwrap(),"--data",data.to_str().unwrap()][..],flags].concat());
        match env{Some(v)=>{c.env("POCOL_MEMORY_BUDGET_BYTES",v);}None=>{c.env_remove("POCOL_MEMORY_BUDGET_BYTES");}}
        let mut c=c.stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().unwrap();let t=std::time::Instant::now();
        while c.try_wait().unwrap().is_none()&&t.elapsed()<std::time::Duration::from_secs(60){std::thread::sleep(std::time::Duration::from_millis(50));}
        let _=c.kill();let o=c.wait_with_output().unwrap();(o.status.code(),text(&o.stderr))};
    for (i,(flags,env,want)) in [(&["--memory-budget-bytes","1"][..],None,"exceeds the node memory budget 1 B"),(&[],Some("1"),"exceeds the node memory budget 1 B"),
        (&["--memory-budget-bytes","1"],Some("x"),"exceeds the node memory budget 1 B"),(&["--memory-budget-bytes","x"],None,"--memory-budget-bytes x:"),
        (&[],Some("x"),"POCOL_MEMORY_BUDGET_BYTES \"x\":"),(&[],Some(""),"POCOL_MEMORY_BUDGET_BYTES \"\":"),(&["--evict-depth","x"],None,"--evict-depth x:"),
        (&["--evict-depth","5"],None,"evict depth 5 is below 2·R_keep + E_win + 258 (4370)"),
        (&["--evict-depth","4370","--state-store"],None,"evict depth 4370 is below the store-mode anchor reach")].into_iter().enumerate(){
        let (code,err)=run(i,flags,env);
        assert_eq!(code,Some(1),"FAIL-ON-ORIGINAL: run {flags:?} {env:?}: {err}");assert!(err.contains(want),"run {flags:?} {env:?}: {err}");
    }
    let _=std::fs::remove_dir_all(&d);
}

/// V3-26 (owner decision 12, 2026-11-13): `--k-safe N` sets the depth of the RPC `safe` tag (local, never finality,
/// default 6) and `--k-alarm N` the depth from which a reorganization is counted (default 60); status reports both with
/// the persisted irreversibility floor (0 on a new data directory); a value that is not a number fails the command, and
/// so does a pair outside 1 <= k_safe <= k_alarm with k_safe < K_hist (32).
#[test] fn k_safe_comes_from_the_flag_and_status_reports_the_floor(){
    let d=tmp("ksafe");let net=d.join("net");let data=d.join("data");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let st=|extra:&[&str]|pocold(&[&["status","--network",net.to_str().unwrap(),"--data",data.to_str().unwrap()][..],extra].concat());
    let o=st(&["--k-safe","7"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let v:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!((v["retention"]["kSafe"].as_u64(),v["retention"]["kAlarm"].as_u64(),v["retention"]["finalized"].as_u64(),v["retention"]["finalityFileIgnored"].as_bool()),(Some(7),Some(60),Some(0),Some(false)));
    assert_eq!(v["retention"]["reorgsGeKAlarm"].as_u64(),Some(0));assert!(v["retention"].get("reorgsGeKSafe").is_none(),"renamed");
    let o=st(&[]);let v:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!((v["retention"]["kSafe"].as_u64(),v["retention"]["kAlarm"].as_u64()),(Some(pocold::node::K_SAFE),Some(pocold::node::K_ALARM)));
    assert_eq!((pocold::node::K_SAFE,pocold::node::K_ALARM),(6,60));
    let o=st(&["--k-alarm","9","--k-safe","9"]);assert!(o.status.success(),"{}",text(&o.stderr));let v:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!((v["retention"]["kSafe"].as_u64(),v["retention"]["kAlarm"].as_u64()),(Some(9),Some(9)));
    for (flags,want) in [(&["--k-safe","x"][..],"--k-safe x:"),(&["--k-alarm","x"][..],"--k-alarm x:"),(&["--k-safe","0"][..],"k_safe 0: the RPC safe depth must be at least 1"),
        (&["--k-safe","7","--k-alarm","6"][..],"k_safe 7 exceeds k_alarm 6"),(&["--k-alarm","5"][..],"k_safe 6 exceeds k_alarm 5"),
        (&["--k-safe","32","--k-alarm","100"][..],"k_safe 32 must be below K_hist 32"),(&["--k-safe","60"][..],"k_safe 60 must be below K_hist 32")]{
        let o=st(flags);assert_eq!(o.status.code(),Some(1),"{flags:?}");assert!(text(&o.stderr).contains(want),"{flags:?}: {}",text(&o.stderr));}
    // `run` also reads POCOL_K_SAFE and POCOL_K_ALARM when the flag is absent (as `pocol start` does): a value that is not
    // a number fails the command before the node opens; with the flag the variable is not read (the open then fails on
    // the 1-byte memory budget instead); a pair the open refuses fails before the budget is checked.
    let run=|flags:&[&str],env:&[(&str,&str)]|{let mut c=Command::new(env!("CARGO_BIN_EXE_pocold"));c.args([&["run","--network",net.to_str().unwrap(),"--data",d.join("run").to_str().unwrap(),"--memory-budget-bytes","1"][..],flags].concat());
        c.env_remove("POCOL_K_SAFE").env_remove("POCOL_K_ALARM");for (k,v) in env{c.env(k,v);}let o=c.output().unwrap();(o.status.code(),text(&o.stderr))};
    let budget="exceeds the node memory budget 1 B";
    let (code,err)=run(&[],&[("POCOL_K_SAFE","x")]);assert_eq!(code,Some(1));assert!(err.contains("POCOL_K_SAFE \"x\":"),"{err}");
    let (code,err)=run(&["--k-safe","7"],&[("POCOL_K_SAFE","x")]);assert_eq!(code,Some(1));assert!(err.contains(budget)&&!err.contains("POCOL_K_SAFE"),"{err}");
    let (code,err)=run(&[],&[("POCOL_K_SAFE","61")]);assert_eq!(code,Some(1));assert!(err.contains("k_safe 61 exceeds k_alarm 60"),"POCOL_K_SAFE alone is used: {err}");
    let (code,err)=run(&[],&[("POCOL_K_ALARM","5")]);assert_eq!(code,Some(1));assert!(err.contains("k_safe 6 exceeds k_alarm 5"),"POCOL_K_ALARM alone is used: {err}");
    let (code,err)=run(&[],&[("POCOL_K_ALARM","x")]);assert_eq!(code,Some(1));assert!(err.contains("POCOL_K_ALARM \"x\":"),"{err}");
    let (code,err)=run(&["--k-alarm","60"],&[("POCOL_K_ALARM","x")]);assert_eq!(code,Some(1));assert!(err.contains(budget)&&!err.contains("POCOL_K_ALARM"),"{err}");
    let (code,err)=run(&[],&[("POCOL_K_SAFE","8"),("POCOL_K_ALARM","7")]);assert_eq!(code,Some(1));assert!(err.contains("k_safe 8 exceeds k_alarm 7"),"the variables are used: {err}");
    let (code,err)=run(&["--k-alarm","8"],&[("POCOL_K_SAFE","8"),("POCOL_K_ALARM","7")]);assert_eq!(code,Some(1));assert!(err.contains(budget),"the flag wins over POCOL_K_ALARM: {err}");
    let (code,err)=run(&[],&[("POCOL_K_SAFE","40"),("POCOL_K_ALARM","60")]);assert_eq!(code,Some(1));assert!(err.contains("k_safe 40 must be below K_hist 32"),"{err}");
    let _=std::fs::remove_dir_all(&d);
}

/// V3-33: the freeze switch comes from `--freeze-user-txs-after S|off`, or POCOL_FREEZE_USER_TXS_AFTER for `run` when the
/// flag is absent; status reports it (and nothing without it); a value that is not a height or `off` fails the command.
/// A running node with an active freeze answers eth_sendRawTransaction with -32016 {reason: frozen}, before decoding.
#[test] fn the_user_transaction_freeze_comes_from_the_flag_or_the_environment_and_is_reported(){
    use std::process::Stdio;
    let d=tmp("freeze");let net=d.join("net");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let status=|i:usize,flags:&[&str]|->Output{let data=d.join(format!("s{i}"));
        Command::new(env!("CARGO_BIN_EXE_pocold")).args([&["status","--network",net.to_str().unwrap(),"--data",data.to_str().unwrap()][..],flags].concat()).output().unwrap()};
    let o=status(0,&[]);assert!(o.status.success(),"{}",text(&o.stderr));
    let v:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();assert!(v.get("userTxFreeze").is_none(),"off: no field");
    let o=status(1,&["--freeze-user-txs-after","5"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let v:serde_json::Value=serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["userTxFreeze"],serde_json::json!({"freezeUserTxsAfter":5,"active":false,"refused":0,"relayDropped":0}));
    let o=status(2,&["--freeze-user-txs-after","off"]);assert!(o.status.success());
    assert!(serde_json::from_slice::<serde_json::Value>(&o.stdout).unwrap().get("userTxFreeze").is_none());
    for bad in ["x","-1","0x5",""]{let o=status(3,&["--freeze-user-txs-after",bad]);
        assert_eq!(o.status.code(),Some(1),"{bad}");assert!(text(&o.stderr).contains(&format!("--freeze-user-txs-after {bad}:")),"{bad}: {}",text(&o.stderr));}
    // run: the flag, else the variable; a bad variable fails before the network starts.
    let hold=||{let held=[std::net::TcpListener::bind("127.0.0.1:0").unwrap(),std::net::TcpListener::bind("127.0.0.1:0").unwrap()];let a=held.each_ref().map(|l|l.local_addr().unwrap());(held,a)};
    let run=|i:usize,flags:&[&str],env:Option<&str>|{let data=d.join(format!("r{i}"));let (held,[p2p,rpc])=hold();
        let mut c=Command::new(env!("CARGO_BIN_EXE_pocold"));
        c.args([&["run","--network",net.to_str().unwrap(),"--data",data.to_str().unwrap(),"--listen",&p2p.to_string(),"--rpc",&rpc.to_string(),"--no-discovery"][..],flags].concat());
        match env{Some(v)=>{c.env("POCOL_FREEZE_USER_TXS_AFTER",v);}None=>{c.env_remove("POCOL_FREEZE_USER_TXS_AFTER");}}
        drop(held);(c.stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap(),rpc)};
    let ready=|c:&mut std::process::Child,rpc|->serde_json::Value{let t=std::time::Instant::now();
        loop{if let Some(s)=c.try_wait().unwrap(){panic!("run exited early with {s}")}
            if let Some(v)=rpc_status(rpc){return v}
            assert!(t.elapsed()<std::time::Duration::from_secs(60),"not ready within 60 s");std::thread::sleep(std::time::Duration::from_millis(50));}};
    let (c,_)=run(0,&[],Some("x"));let o=c.wait_with_output().unwrap();
    assert_eq!(o.status.code(),Some(1));assert!(text(&o.stderr).contains("POCOL_FREEZE_USER_TXS_AFTER x:"),"{}",text(&o.stderr));
    let (mut c,rpc)=run(1,&[],Some("7"));let v=ready(&mut c,rpc);let _=c.kill();let _=c.wait();
    assert_eq!(v["userTxFreeze"]["freezeUserTxsAfter"],7,"from the variable: {v}");
    let (mut c,rpc)=run(2,&["--freeze-user-txs-after","0"],Some("7"));let v=ready(&mut c,rpc);
    assert_eq!((v["userTxFreeze"]["freezeUserTxsAfter"].as_u64(),v["userTxFreeze"]["active"].as_bool()),(Some(0),Some(true)),"the flag wins, active at genesis: {v}");
    let body=r#"{"jsonrpc":"2.0","id":1,"method":"eth_sendRawTransaction","params":["0x02"]}"#;
    let r={use std::io::{Read,Write};let mut s=std::net::TcpStream::connect(rpc).unwrap();s.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        s.write_all(format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).unwrap();
        let mut out=String::new();let _=s.read_to_string(&mut out);serde_json::from_str::<serde_json::Value>(out.split("\r\n\r\n").nth(1).unwrap()).unwrap()};
    assert_eq!(r["error"]["code"],-32016,"{r}");assert_eq!(r["error"]["data"]["reason"],"frozen","{r}");assert_eq!(r["error"]["data"]["freezeUserTxsAfter"],0,"{r}");
    let _=c.kill();let _=c.wait();
    let _=std::fs::remove_dir_all(&d);
}
/// V3-33: export-chain and verify-export refuse bad inputs with status 1 and write nothing.
#[test] fn export_chain_and_verify_export_refuse_bad_inputs(){
    let d=tmp("export");let net=d.join("net");let out=d.join("out");
    let o=pocold(&["init-devnet","--out",net.to_str().unwrap(),"--miners","a,b,c"]);assert!(o.status.success(),"{}",text(&o.stderr));
    let (n,data,o_)=(net.to_str().unwrap(),d.join("data"),out.to_str().unwrap());let data=data.to_str().unwrap();
    for (args,want) in [(vec!["export-chain","--network",n,"--data",data,"--out",o_],"--to-height H"),
        (vec!["export-chain","--network",n,"--data",data,"--to-height","x","--out",o_],"--to-height:"),
        (vec!["export-chain","--network",n,"--data",data,"--to-height","1"],"--out NEW_DIR"),
        (vec!["export-chain","--network",n,"--data",data,"--to-height","0","--out",o_],"at least 1"),
        (vec!["export-chain","--network",n,"--data",data,"--to-height","1","--out",o_],"above the head 0"),
        (vec!["export-chain","--network",d.join("none").to_str().unwrap(),"--data",data,"--to-height","1","--out",o_],"genesis"),
        (vec!["verify-export"],"--dir EXPORT_DIR"),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap()],"manifest.json"),
        // Review finding 3: a pin that is missing its value, misspelt, in `=` form, repeated or malformed is refused,
        // never silently skipped (each would otherwise print verified output with no digest checked).
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--manifest-sha256"],"--manifest-sha256 needs a value"),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--manifest-sha256","--network",n],"--manifest-sha256 needs a value"),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--manifest-sha256=00"],"unknown argument \"--manifest-sha256=00\""),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--manifest-sha","00"],"unknown argument \"--manifest-sha\""),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"extra"],"unknown argument \"extra\""),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--head-hash"],"--head-hash needs a value"),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--head-hash","00","--head-hash","00"],"--head-hash is given twice"),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--manifest-sha256","00"],"--manifest-sha256 must be 64 hex digits"),
        (vec!["verify-export","--dir",d.join("none").to_str().unwrap(),"--head-hash","0x12"],"--head-hash must be 64 hex digits")]{
        let o=pocold(&args);let err=text(&o.stderr);
        assert_eq!(o.status.code(),Some(1),"{args:?}: {err}");assert!(err.starts_with("error: ")&&err.contains(want),"{args:?}: {err}");
        assert!(!out.exists(),"{args:?} wrote nothing");
    }
    let _=std::fs::remove_dir_all(&d);
}
