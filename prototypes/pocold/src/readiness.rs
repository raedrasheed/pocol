//! Public-launch readiness (V3-28 H1, D-V3-28). `publicLaunchReady` is computed from facts, never written as a label,
//! and is reported beside `launchBlockers`, every reason it is false: `pocold build-info` and `pocol build-info` (the
//! build alone), `pocold status` and `pocol_status` (the network this node admitted at boot), `init-testnet` (the
//! profile it just admitted) and the launcher's network description (the invitation's files). It is true only when
//! every blocker below has cleared:
//! - build: not a v3-preview build, SPEC_VERSION at least 2 (V3-PR-I), consensus sources pinned, no test-only
//!   feature, `V3_LAUNCH_BLOCKERS` empty, and every gate of the public-network acceptance ledger closed
//!   (`PLAN_GATES_OPEN`, development/public-network/PLAN.md);
//! - network: v3 active on the genesis, (chainId, genesisHash) one of this release's pinned launch networks
//!   (`NETWORK_PROFILES`, V3-28 H2; none yet), and an admission this node booted with in scope specification with
//!   no condition and no waiver, as the network profile on a network node.
//!
//! The launcher admits nothing, so its description always keeps `admission-not-booted`: what a devnet.json records
//! about its own admission is operator metadata that nothing commits to, never readiness evidence (D-V3-28). It is
//! reflected, bounded and validated, only as further blockers. Letting the launcher report ready would need H2 to pin
//! the expected admission with each `NETWORK_PROFILES` entry and compare against that, not against devnet.json.
//!
//! Node-local reporting only: no validity verdict, fork choice or persisted byte reads it. Deployment branding
//! (`network_display`) is never readiness evidence: a designated network reports the same blockers.
use pocol_lp4_execution::{admission::{self,Admitted,Scope},profile::{NodeKind,ProfileKind}};

/// One reason the network is not ready for a public launch: a stable id and what is missing.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Blocker{pub id:String,pub detail:String}
fn blocker(id:impl Into<String>,detail:impl Into<String>)->Blocker{Blocker{id:id.into(),detail:detail.into()}}

/// The acceptance-ledger gates of development/public-network/PLAN.md that are still open, with what remains. A gate
/// leaves this list only when the ledger marks it closed with its evidence (its status starts with "Closed"), never by
/// a label change here alone; the test `plan_gates_open_match_the_acceptance_ledger` keeps the two equal.
pub const PLAN_GATES_OPEN:&[(&str,&str)]=&[
    ("K1","independent identities: operator secret-key recovery and an encrypted keystore or HSM remain"),
    ("J1","open miner enrollment: a public faucet, eligibility churn and load, and network-scale enrollment remain"),
    ("I1","stable network identity: the normative digest adoption and an independently reproduced binary release remain"),
    ("A1","honest admission: an approved profile, measured H_cal, real worst-case calibration, the R18/R21 obligations and the RpcGuard gaps remain"),
    ("S1","bounded historical state: K_eff and historical RPC behavior within the resource budget remain"),
    ("N1","Internet exposure: actual Internet ingress acceptance and the remaining RpcGuard controls remain"),
    ("L1","sustained operation: physical servers, an actual multi-host soak and an upgrade drill remain"),
    ("E1","network lifetime and economics: approved allocation, issuance, bonds, H_CLOSE/H_END and cumulative caps remain"),
    ("Q1","release review: an independent security review and the unresolved-finding ledger remain"),
];
/// V3-28 H2: the launch networks (chainId, genesisHash) this release pins. Empty: no network is a launch network yet,
/// whatever its name or designation.
pub const NETWORK_PROFILES:&[(u64,[u8;32])]=&[];

/// The build's facts. `this()` reads this binary's; tests change one at a time.
#[derive(Clone,Debug)]
pub struct Build{
    pub v3_preview:bool,pub spec_version:u64,pub consensus_pinned:bool,pub test_features:Vec<&'static str>,
    pub v3_launch_blockers:Vec<&'static str>,pub plan_gates_open:Vec<(&'static str,&'static str)>,pub network_profiles:Vec<(u64,[u8;32])>,
}
impl Build{
    pub fn this()->Self{
        let test_features=[(pocol_lp4_execution::CAPS_TEST,"caps-test"),(cfg!(feature="crash-test"),"crash-test"),(cfg!(feature="io-fault"),"io-fault")]
            .into_iter().filter(|(on,_)|*on).map(|(_,n)|n).collect();
        Self{v3_preview:pocol_lp4_execution::V3_PREVIEW,spec_version:u64::from(crate::genesis_spec::SPEC_VERSION),consensus_pinned:crate::identity::PINNED,
            test_features,v3_launch_blockers:admission::V3_LAUNCH_BLOCKERS.to_vec(),plan_gates_open:PLAN_GATES_OPEN.to_vec(),network_profiles:NETWORK_PROFILES.to_vec()}
    }
}
/// R22 (p1_profile.rs `ProfileIdentity::validate`): the network profile exists only on this chain id.
pub const R22_NETWORK_CHAIN_ID:u64=777001;
/// At most this many distinct recorded condition ids and waiver rules are reflected each; past it one `overflow`
/// entry stands for the rest.
pub const RECORDED_MAX:usize=32;
/// What an admission says about readiness (a boot's, or one recorded in devnet.json).
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AdmissionFacts{pub scope:String,pub conditions:Vec<String>,pub waivers:Vec<String>,pub network_profile:bool}
/// Where a network's admission facts come from.
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Admission{
    /// The admission this node booted with (`Admitted`): the only readiness evidence.
    Booted(AdmissionFacts),
    /// What a devnet.json records about itself (the launcher), None when it records none. Never evidence: it always
    /// keeps `admission-not-booted`, and its facts can only add blockers.
    Recorded(Option<AdmissionFacts>),
}
/// A recorded id kept as written only in the shape the gate writes (`[a-z0-9-]{1,64}`), else `unreadable`.
fn recorded_id(v:&serde_json::Value)->String{
    match v.as_str(){Some(s) if (1..=64).contains(&s.len())&&s.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit()||b==b'-')=>s.into(),_=>"unreadable".into()}
}
/// A recorded waiver rule kept only as `R[0-9]{1,3}`, else `unreadable`.
fn recorded_rule(v:&serde_json::Value)->String{
    match v.as_str(){Some(s) if (2..=4).contains(&s.len())&&s.starts_with('R')&&s[1..].bytes().all(|b|b.is_ascii_digit())=>s.into(),_=>"unreadable".into()}
}
impl AdmissionFacts{
    pub fn of(a:&Admitted)->Self{
        let r=a.report();
        Self{scope:a.scope().name().into(),conditions:r.conditions.iter().map(|c|c.id.to_string()).collect(),waivers:r.waivers.iter().map(|w|w.rule.to_string()).collect(),
            network_profile:a.profile().kind()==ProfileKind::Network&&a.profile().node_kind()==NodeKind::Node}
    }
    /// The admission recorded in a devnet.json (`admission_json`) and its declared profileKind, for the genesis's
    /// `chain_id`; None when there is none. Informational only (`Admission::Recorded` always blocks): nothing missing
    /// or malformed reads as cleared, ids outside the gate's shape read `unreadable`, each list is deduplicated and
    /// capped at `RECORDED_MAX` (then `overflow`), and a recorded network profile counts only on the R22 chain.
    pub fn recorded(info:&serde_json::Value,chain_id:u64)->Option<Self>{
        let a=info.get("admission")?.as_object()?;
        let ids=|key:&str,field:&str,read:fn(&serde_json::Value)->String|->Vec<String>{
            let Some(list)=a.get(key).and_then(|v|v.as_array()) else{return vec!["unreadable".into()]};
            let mut out:Vec<String>=Vec::new();
            for x in list{
                let id=read(&x[field]);if out.contains(&id){continue}
                if out.len()==RECORDED_MAX{out.push("overflow".into());break}
                out.push(id);
            }
            out
        };
        let scope=match a.get("scope"){None=>"unrecorded".into(),Some(s)=>recorded_id(s)};
        Some(Self{scope,conditions:ids("conditions","id",recorded_id),waivers:ids("waivers","rule",recorded_rule),
            network_profile:chain_id==R22_NETWORK_CHAIN_ID&&info["profileKind"].as_str().is_some_and(|k|k.eq_ignore_ascii_case("network"))})
    }
}
/// The network's facts.
#[derive(Clone,Debug)]
pub struct Network{pub chain_id:u64,pub genesis_hash:[u8;32],pub v3_active:bool,pub admission:Admission}
impl Network{
    pub fn admitted(a:&Admitted)->Self{
        Self{chain_id:a.profile().spec().chain_id,genesis_hash:a.genesis_hash(),v3_active:a.profile().v3_active(u64::MAX),admission:Admission::Booted(AdmissionFacts::of(a))}
    }
}
/// What readiness is asked of: the build alone, a network, or a network whose genesis cannot be read.
pub enum Subject<'a>{Build,Network(&'a Network),Unreadable(String)}

/// Every blocker, build facts first. Empty exactly when a public launch is ready.
pub fn blockers(b:&Build,subject:Subject)->Vec<Blocker>{
    let mut out=Vec::new();
    if b.v3_preview{out.push(blocker("v3-preview-build","this is a v3-preview build: the dormant v3 rules run for simnet tests only and peer only with byte-identical trees"))}
    if b.spec_version<2{out.push(blocker("spec-version",format!("SPEC_VERSION is {}: this release has not activated the v3 rules (V3-PR-I raises it to 2)",b.spec_version)))}
    if !b.consensus_pinned{out.push(blocker("consensus-unpinned","the consensus boundary sources differ from consensus-sources.lock; a release build refuses such a tree"))}
    if !b.test_features.is_empty(){out.push(blocker("test-features",format!("built with test-only features: {}",b.test_features.join(", "))))}
    out.extend(b.v3_launch_blockers.iter().map(|x|blocker("v3-launch-blocker",format!("{x} (V3_LAUNCH_BLOCKERS)"))));
    out.extend(b.plan_gates_open.iter().map(|(id,what)|blocker(format!("plan-gate-{id}"),format!("{id} {what} (development/public-network/PLAN.md)"))));
    let n=match subject{
        Subject::Build=>{out.push(blocker("no-network","readiness is a property of a network this build admits; the build alone is never ready"));return out}
        Subject::Unreadable(why)=>{out.push(blocker("genesis-unreadable",format!("the network's genesis specification cannot be read: {why}")));return out}
        Subject::Network(n)=>n,
    };
    if !n.v3_active{out.push(blocker("v3-inactive","the network's genesis never activates the v3 rules (spec version 1 and no fork to version 3); the public launch is a v3 genesis"))}
    if !b.network_profiles.contains(&(n.chain_id,n.genesis_hash)){
        out.push(blocker("network-not-release-pinned",format!("chain {} genesis {} is not a launch network pinned by this release (V3-28 H2{})",
            n.chain_id,crate::types::hex(&n.genesis_hash),if b.network_profiles.is_empty(){"; it pins none yet"}else{""})))}
    let facts=match &n.admission{
        Admission::Booted(a)=>Some(a),
        Admission::Recorded(a)=>{
            out.push(blocker("admission-not-booted","the launcher admits nothing and devnet.json's recorded admission is not evidence; a booted node's status reports the admission readiness is computed from"));
            a.as_ref()
        }
    };
    match facts{
        None=>out.push(blocker("admission-unrecorded","no admission is recorded for this network; a node's status reports the admission it booted with")),
        Some(a)=>{
            if a.scope!=Scope::Specification.name(){out.push(blocker("admission-scope",format!("admitted in scope {}, not specification",a.scope)))}
            out.extend(a.conditions.iter().map(|c|blocker(format!("admission-condition-{c}"),format!("admission condition {c} (see admission.conditions)"))));
            out.extend(a.waivers.iter().map(|w|blocker(format!("admission-waiver-{w}"),format!("{w} partly waived (see admission.waivers)"))));
            if !a.network_profile{out.push(blocker("profile-kind","not admitted as the network profile on a network node (R22)"))}
        }
    }
    out
}
/// `publicLaunchReady` (true only with no blocker) and `launchBlockers` [{id, detail}].
pub fn json(blockers:&[Blocker])->serde_json::Value{
    serde_json::json!({"publicLaunchReady":blockers.is_empty(),"launchBlockers":blockers.iter().map(|b|serde_json::json!({"id":b.id,"detail":b.detail})).collect::<Vec<_>>()})
}
/// Sets `publicLaunchReady` and `launchBlockers` on a JSON object.
pub fn annotate(v:&mut serde_json::Value,blockers:&[Blocker]){
    if let (Some(o),serde_json::Value::Object(r))=(v.as_object_mut(),json(blockers)){o.extend(r)}
}
/// The readiness of the network an admission booted (status, init-testnet).
pub fn of_admitted(a:&Admitted)->Vec<Blocker>{blockers(&Build::this(),Subject::Network(&Network::admitted(a)))}
/// `build-info`: the build's identity with this build's readiness (the build alone, so never ready).
pub fn build_info()->String{
    let mut info:serde_json::Value=serde_json::from_str(crate::identity::BUILD_INFO).expect("build.rs writes JSON");
    annotate(&mut info,&blockers(&Build::this(),Subject::Build));info.to_string()
}

#[cfg(test)]
mod tests{
    use super::*;
    /// A build and a network with every blocker cleared.
    fn ready()->(Build,Network){
        let n=Network{chain_id:777001,genesis_hash:[9;32],v3_active:true,admission:Admission::Booted(cleared())};
        (Build{v3_preview:false,spec_version:2,consensus_pinned:true,test_features:vec![],v3_launch_blockers:vec![],plan_gates_open:vec![],network_profiles:vec![(777001,[9;32])]},n)
    }
    fn cleared()->AdmissionFacts{AdmissionFacts{scope:"specification".into(),conditions:vec![],waivers:vec![],network_profile:true}}
    fn ids(b:&[Blocker])->Vec<&str>{b.iter().map(|x|x.id.as_str()).collect()}
    fn booted(n:&mut Network)->&mut AdmissionFacts{match &mut n.admission{Admission::Booted(a)=>a,Admission::Recorded(_)=>unreachable!()}}

    #[test]
    fn this_build_is_not_ready_today_with_honest_reasons(){
        let b=blockers(&Build::this(),Subject::Build);let got=ids(&b);
        assert!(got.contains(&"spec-version"),"{got:?}");assert!(got.contains(&"no-network"));
        assert_eq!(got.contains(&"v3-preview-build"),cfg!(feature="v3-preview"));
        assert_eq!(got.contains(&"consensus-unpinned"),!crate::identity::PINNED);
        for (g,_) in PLAN_GATES_OPEN{assert!(got.contains(&format!("plan-gate-{g}").as_str()),"{g}")}
        assert!(admission::V3_LAUNCH_BLOCKERS.is_empty()&&!got.contains(&"v3-launch-blocker"),"V3_LAUNCH_BLOCKERS is empty today");
        let j=json(&b);assert_eq!(j["publicLaunchReady"],false);assert_eq!(j["launchBlockers"].as_array().unwrap().len(),b.len());
        assert!(j["launchBlockers"].as_array().unwrap().iter().all(|x|x["id"].is_string()&&x["detail"].as_str().is_some_and(|d|!d.is_empty())));
    }
    #[test]
    fn ready_only_when_every_blocker_clears_and_each_reason_alone_blocks(){
        let (b,n)=ready();
        assert!(blockers(&b,Subject::Network(&n)).is_empty());assert_eq!(json(&[])["publicLaunchReady"],true);
        assert_eq!(ids(&blockers(&b,Subject::Build)),["no-network"],"the build alone is never ready");
        assert_eq!(ids(&blockers(&b,Subject::Unreadable("bad".into()))),["genesis-unreadable"]);
        type Edit=fn(&mut Build,&mut Network);
        let cases:&[(Edit,&[&str])]=&[
            (|b,_|b.v3_preview=true,&["v3-preview-build"]),
            (|b,_|b.spec_version=1,&["spec-version"]),
            (|b,_|b.consensus_pinned=false,&["consensus-unpinned"]),
            (|b,_|b.test_features=vec!["caps-test","io-fault"],&["test-features"]),
            (|b,_|b.v3_launch_blockers=vec!["D-V3-99 a later pre-launch item"],&["v3-launch-blocker"]),
            (|b,_|b.plan_gates_open=vec![("Q1","release review")],&["plan-gate-Q1"]),
            (|_,n|n.v3_active=false,&["v3-inactive"]),
            (|b,_|b.network_profiles.clear(),&["network-not-release-pinned"]),
            (|_,n|n.genesis_hash=[8;32],&["network-not-release-pinned"]),
            (|_,n|n.chain_id=777002,&["network-not-release-pinned"]),
            // A recorded admission never clears, even one recording every fact cleared (a forged devnet.json).
            (|_,n|n.admission=Admission::Recorded(Some(cleared())),&["admission-not-booted"]),
            (|_,n|n.admission=Admission::Recorded(None),&["admission-not-booted","admission-unrecorded"]),
            (|_,n|booted(n).scope="experimental-devnet".into(),&["admission-scope"]),
            (|_,n|booted(n).conditions=vec!["timing-not-a-bound".into()],&["admission-condition-timing-not-a-bound"]),
            (|_,n|booted(n).waivers=vec!["R26".into()],&["admission-waiver-R26"]),
            (|_,n|booted(n).network_profile=false,&["profile-kind"]),
        ];
        for (edit,want) in cases{
            let (mut b,mut n)=ready();edit(&mut b,&mut n);
            let got=blockers(&b,Subject::Network(&n));assert_eq!(ids(&got),*want);
            let j=json(&got);assert_eq!(j["publicLaunchReady"],false);assert_eq!(j["launchBlockers"][0]["id"],want[0]);
        }
        let (mut b,n)=ready();b.test_features=vec!["caps-test","io-fault"];
        assert!(blockers(&b,Subject::Network(&n))[0].detail.ends_with("caps-test, io-fault"));
    }
    #[test]
    fn recorded_admission_never_reads_as_cleared_from_missing_or_malformed_fields(){
        assert_eq!(AdmissionFacts::recorded(&serde_json::json!({}),777001),None);
        assert_eq!(AdmissionFacts::recorded(&serde_json::json!({"admission":7}),777001),None);
        let a=AdmissionFacts::recorded(&serde_json::json!({"profileKind":"simnet","admission":{"scope":"experimental-devnet",
            "conditions":[{"id":"calibration-synthetic"},{"id":5}],"waivers":[{"rule":"R26"}]}}),777941).unwrap();
        assert_eq!(a,AdmissionFacts{scope:"experimental-devnet".into(),conditions:vec!["calibration-synthetic".into(),"unreadable".into()],waivers:vec!["R26".into()],network_profile:false});
        let a=AdmissionFacts::recorded(&serde_json::json!({"profileKind":"Network","admission":{}}),777001).unwrap();
        assert_eq!(a,AdmissionFacts{scope:"unrecorded".into(),conditions:vec!["unreadable".into()],waivers:vec!["unreadable".into()],network_profile:true});
        // R22: a recorded network profile on any other chain is not one (the boot refuses it).
        for chain in [123456789,777002,0]{
            assert!(!AdmissionFacts::recorded(&serde_json::json!({"profileKind":"network","admission":{}}),chain).unwrap().network_profile,"{chain}");
        }
    }
    /// Recorded ids are reflected only in the gate's shape, deduplicated and capped, so a large or hand-written
    /// devnet.json cannot inflate the description or put free text into a stable id.
    #[test]
    fn recorded_ids_are_validated_deduplicated_and_bounded(){
        let a=AdmissionFacts::recorded(&serde_json::json!({"admission":{"scope":"specification; ready",
            "conditions":[{"id":"cleared by the owner; publicLaunchReady"},{"id":"rpc-absent"},{"id":"rpc-absent"},{"id":"A"},{"id":""},{"id":"x".repeat(65)},{"id":"x".repeat(64)}],
            "waivers":[{"rule":"R26"},{"rule":"R1234"},{"rule":"r26"},{"rule":"R"},{"rule":"R2x"},{"rule":"R7"}]}}),777941).unwrap();
        assert_eq!(a.scope,"unreadable");
        assert_eq!(a.conditions,["unreadable".to_string(),"rpc-absent".into(),"x".repeat(64)]);
        assert_eq!(a.waivers,["R26","unreadable","R7"]);
        // 60,000 distinct ids reflect as RECORDED_MAX of them and one overflow entry, for each list.
        let many:Vec<serde_json::Value>=(0..60_000).map(|i|serde_json::json!({"id":format!("c{i}"),"rule":format!("R{}",i%1000)})).collect();
        let a=AdmissionFacts::recorded(&serde_json::json!({"admission":{"scope":"experimental-devnet","conditions":many,"waivers":many}}),777941).unwrap();
        assert_eq!(a.conditions.len(),RECORDED_MAX+1);assert_eq!(a.conditions.last().unwrap(),"overflow");assert_eq!(a.conditions[..3],["c0","c1","c2"]);
        assert_eq!(a.waivers.len(),RECORDED_MAX+1);assert_eq!(a.waivers.last().unwrap(),"overflow");
        let n=Network{chain_id:777941,genesis_hash:[1;32],v3_active:false,admission:Admission::Recorded(Some(a))};
        let got=blockers(&Build::this(),Subject::Network(&n));
        assert_eq!(got.iter().filter(|b|b.id.starts_with("admission-condition-")).count(),RECORDED_MAX+1);
        assert!(got.iter().any(|b|b.id=="admission-condition-overflow")&&got.iter().any(|b|b.id=="admission-waiver-overflow"));
        assert!(got.len()<2*RECORDED_MAX+PLAN_GATES_OPEN.len()+16,"{}",got.len());
    }
    /// `AdmissionFacts::of` and `Network::admitted` map a real admission: its scope, every condition id and waiver
    /// rule of its report, the profile and node kinds, and whether the genesis activates v3.
    #[test]
    fn a_booted_admission_maps_its_report_into_blockers(){
        use pocol_lp4_execution::devnet::{self,DevnetConfig};
        let mut c=DevnetConfig::new(777942,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
        let (a,_)=devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap();
        let facts=AdmissionFacts::of(&a);
        assert_eq!(facts.scope,"experimental-devnet");assert!(!facts.network_profile,"a simnet profile on a simnet node");
        let r=a.report();
        assert_eq!(facts.conditions,r.conditions.iter().map(|c|c.id.to_string()).collect::<Vec<_>>());
        assert_eq!(facts.waivers,r.waivers.iter().map(|w|w.rule.to_string()).collect::<Vec<_>>());
        assert!(facts.conditions.contains(&"calibration-synthetic".to_string()),"{facts:?}");
        let n=Network::admitted(&a);
        assert_eq!((n.chain_id,n.genesis_hash),(777942,a.genesis_hash()));assert_eq!(n.admission,Admission::Booted(facts.clone()));
        let got=of_admitted(&a);let got=ids(&got);
        let mut want=vec!["admission-scope".to_string()];
        want.extend(facts.conditions.iter().map(|c|format!("admission-condition-{c}")));
        want.extend(facts.waivers.iter().map(|w|format!("admission-waiver-{w}")));
        want.push("profile-kind".into());
        let network:Vec<&str>=got.iter().copied().skip_while(|id|*id!="v3-inactive"&&*id!="network-not-release-pinned").collect();
        let mut expect=vec!["v3-inactive","network-not-release-pinned"];expect.extend(want.iter().map(String::as_str));
        assert_eq!(network,expect,"the network blockers of a booted devnet admission");
        assert!(!got.contains(&"admission-not-booted")&&!got.contains(&"admission-unrecorded"));
        // A spec-1 genesis with the schedule [(1,0)] never activates v3. No admissible schedule activates it later
        // (ForkSchedule's bound MAX_FORK_VERSION is 2), so the height Network::admitted asks at is not observable.
        assert!(!n.v3_active);
    }
    /// PLAN_GATES_OPEN lists exactly the acceptance-ledger gates whose status is not "Closed".
    #[test]
    fn plan_gates_open_match_the_acceptance_ledger(){
        let plan=std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../development/public-network/PLAN.md")).unwrap();
        let ledger=plan.split("## Acceptance ledger").nth(1).unwrap().split("\n## ").next().unwrap();
        let mut open=Vec::new();let mut all=0;
        for row in ledger.lines().filter(|l|l.starts_with("| ")&&!l.starts_with("| Gate")){
            let cells:Vec<&str>=row.split('|').map(str::trim).collect();
            let Some((id,_))=cells[1].split_once(':') else{continue};all+=1;
            if !cells[3].starts_with("Closed"){open.push(id.to_string())}
        }
        assert_eq!(all,9,"{ledger}");
        assert_eq!(open,PLAN_GATES_OPEN.iter().map(|(g,_)|g.to_string()).collect::<Vec<_>>());
    }
    #[test]
    fn build_info_reports_the_build_readiness(){
        let info:serde_json::Value=serde_json::from_str(&build_info()).unwrap();
        assert_eq!(info["publicLaunchReady"],false);
        assert_eq!(info["launchBlockers"],json(&blockers(&Build::this(),Subject::Build))["launchBlockers"]);
        let raw:serde_json::Value=serde_json::from_str(crate::identity::BUILD_INFO).unwrap();
        for (k,v) in raw.as_object().unwrap(){assert_eq!(&info[k],v,"{k}")}
    }
}
