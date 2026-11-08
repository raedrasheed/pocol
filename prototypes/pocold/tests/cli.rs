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
    assert_eq!(listed,["build-info","cutover","help","init-devnet","init-testnet","run","status"],"{usage}");
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
/// pocol_status from a node's RPC port, None until it answers.
fn rpc_status(rpc:std::net::SocketAddr)->Option<serde_json::Value>{
    use std::io::{Read,Write};
    let body=r#"{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}"#;
    let mut s=std::net::TcpStream::connect(rpc).ok()?;s.set_read_timeout(Some(std::time::Duration::from_secs(5))).ok()?;
    s.write_all(format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes()).ok()?;
    let mut out=String::new();let _=s.read_to_string(&mut out);
    let v=serde_json::from_str::<serde_json::Value>(out.split("\r\n\r\n").nth(1)?).ok()?["result"].clone();v.is_object().then_some(v)
}
