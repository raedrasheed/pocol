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
