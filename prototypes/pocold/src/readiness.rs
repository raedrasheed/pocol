//! Public-launch readiness (V3-28 H1, D-V3-28). `publicLaunchReady` is computed from facts, never written as a label,
//! and is reported beside `launchBlockers`, every reason it is false: `pocold build-info` and `pocol build-info` (the
//! build alone), `pocold status` and `pocol_status` (the network this node admitted at boot), `init-testnet` (the
//! profile it just admitted) and the launcher's network description (the invitation's files). It is true only when
//! every blocker below has cleared:
//! - build: SPEC_VERSION at least 2 (the v3 release rule set), consensus sources pinned, no test-only
//!   feature, `V3_LAUNCH_BLOCKERS` empty, and every gate of the public-network acceptance ledger closed
//!   (`PLAN_GATES_OPEN`, development/public-network/PLAN.md);
//! - network: v3 active on the genesis, (chainId, genesisHash) one of this release's pinned launch networks
//!   (`NETWORK_PROFILES`, V3-28 H2; none yet), and an admission this node booted with (or, in the launcher, this
//!   release's pin of the network with exactly `ExpectedAdmission::LAUNCH`, V3-28 H2 (1)) in scope specification
//!   with no condition and no waiver, as the network profile on a network node.
//!
//! The launcher admits nothing. What a devnet.json records about its own admission is operator metadata that nothing
//! commits to, never readiness evidence (D-V3-28): it is reflected, bounded and validated, only as further blockers,
//! beside `admission-not-booted`. Each `NETWORK_PROFILES` entry also pins the admission this release expects a node
//! booting that network to get (`ExpectedAdmission`, V3-28 H2 (1)); the launcher compares that pin, never
//! devnet.json, with `ExpectedAdmission::LAUNCH`, and only on an exact match takes the pinned admission in place of
//! the recorded one and drops `admission-not-booted`. `NETWORK_PROFILES` is empty, so today every network keeps
//! `admission-not-booted` in the launcher.
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
    ("K1","independent identities: operator secret-key recovery procedures and an encrypted keystore or HSM remain"),
    ("J1","open miner enrollment: eligibility churn and load, and network-scale enrollment remain (newcomers' first fee: the automated fee faucet, 0.002 PCOL once per new address)"),
    ("I1","stable network identity: the ceremony's genesis and its pin in the clients, the normative digest adoption and an independently reproduced binary release remain"),
    ("A1","honest admission: real worst-case execution calibration, the R18/R21 obligations and the RpcGuard gaps remain"),
    ("S1","bounded historical state: K_eff and historical RPC behavior within the resource budget remain"),
    ("N1","Internet exposure: actual Internet ingress acceptance and the remaining RpcGuard controls remain"),
    ("L1","sustained operation: the rehearsal on the founders' devices, an actual multi-host soak and an upgrade drill remain"),
    ("E1","network lifetime and economics: the genesis ceremony (it enters the founders' decided reward addresses) and the rehearsal evidence remain"),
    ("Q1","release review: an independent security review (commissioned after the launch) and the unresolved-finding ledger remain"),
];
/// V3-28 H2 (1): the admission this release expects a node booting a pinned network to get, every field the
/// readiness of an admission reads, compared exactly (`==`).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ExpectedAdmission{
    pub scope:Scope,pub conditions:&'static [&'static str],pub waivers:&'static [&'static str],
    pub profile_kind:ProfileKind,pub node_kind:NodeKind,
}
impl ExpectedAdmission{
    /// The only pinned admission that lets the launcher drop `admission-not-booted`: scope specification, no
    /// condition, no waiver, the network profile on a network node.
    pub const LAUNCH:Self=Self{scope:Scope::Specification,conditions:&[],waivers:&[],profile_kind:ProfileKind::Network,node_kind:NodeKind::Node};
    /// The fields in which this expected admission differs from `other`, in declaration order.
    pub fn differing(&self,other:&Self)->Vec<&'static str>{
        [(self.scope!=other.scope,"scope"),(self.conditions!=other.conditions,"conditions"),(self.waivers!=other.waivers,"waivers"),
            (self.profile_kind!=other.profile_kind,"profile kind"),(self.node_kind!=other.node_kind,"node kind")]
            .into_iter().filter(|(d,_)|*d).map(|(_,f)|f).collect()
    }
    /// The pinned admission's facts on `chain_id`: as at boot, a network profile counts only on the R22 chain.
    pub fn facts(&self,chain_id:u64)->AdmissionFacts{
        AdmissionFacts{scope:self.scope.name().into(),conditions:self.conditions.iter().map(|c|c.to_string()).collect(),
            waivers:self.waivers.iter().map(|w|w.to_string()).collect(),
            network_profile:self.profile_kind==ProfileKind::Network&&self.node_kind==NodeKind::Node&&chain_id==R22_NETWORK_CHAIN_ID}
    }
}
/// V3-28 H2: one launch network this release pins, (chainId, genesisHash), with the admission it expects there.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct NetworkProfile{pub chain_id:u64,pub genesis_hash:[u8;32],pub admission:ExpectedAdmission}
/// V3-28 H2: the launch networks this release pins. Empty: no network is a launch network yet, whatever its name or
/// designation, and the launcher keeps `admission-not-booted` for every network. deploy/mainnet/set_network_identity.py
/// writes it at the identity switch and its `--check` verifies it: the entry
/// `NetworkProfile{chain_id:<chainId>,genesis_hash:genesis_hash("<64 hex>"),admission:ExpectedAdmission::LAUNCH}` of
/// the network the clients are switched to, unless that is a retired or the rehearsal network, entries separated by
/// `,`; the declaration stays on one line in that form. A launch network expects the launch admission; a node that
/// boots it with any other admission (experimental-devnet today) still reports its own blockers.
pub const NETWORK_PROFILES:&[NetworkProfile]=&[];
/// A genesis hash written as 64 lower-case hex digits, as `pocol_status` reports it without `0x`, decoded at compile
/// time in a `NETWORK_PROFILES` entry (a malformed one does not compile).
pub const fn genesis_hash(hex:&str)->[u8;32]{
    const fn nibble(c:u8)->u8{match c{b'0'..=b'9'=>c-b'0',b'a'..=b'f'=>c-b'a'+10,_=>panic!("a genesis hash is lower-case hex")}}
    let b=hex.as_bytes();
    assert!(b.len()==64,"a genesis hash is 64 hex digits");
    let mut out=[0u8;32];let mut i=0;
    while i<32{out[i]=nibble(b[2*i])<<4|nibble(b[2*i+1]);i+=1}
    out
}

/// The build's facts. `this()` reads this binary's; tests change one at a time.
#[derive(Clone,Debug)]
pub struct Build{
    pub spec_version:u64,pub consensus_pinned:bool,pub test_features:Vec<&'static str>,
    pub v3_launch_blockers:Vec<&'static str>,pub plan_gates_open:Vec<(&'static str,&'static str)>,pub network_profiles:Vec<NetworkProfile>,
}
impl Build{
    pub fn this()->Self{
        let test_features=[(pocol_lp4_execution::CAPS_TEST,"caps-test"),(cfg!(feature="crash-test"),"crash-test"),(cfg!(feature="io-fault"),"io-fault")]
            .into_iter().filter(|(on,_)|*on).map(|(_,n)|n).collect();
        Self{spec_version:u64::from(crate::genesis_spec::SPEC_VERSION),consensus_pinned:crate::identity::PINNED,
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
    /// The admission this node booted with (`Admitted`): the readiness evidence of a booted node. In the launcher,
    /// which boots nothing, only a release pin of exactly `ExpectedAdmission::LAUNCH` stands in for it (H2 (1)).
    Booted(AdmissionFacts),
    /// What a devnet.json records about itself (the launcher), None when it records none. Never evidence: its facts
    /// can only add blockers, beside `admission-not-booted`. Only a `NETWORK_PROFILES` pin of this network whose
    /// expected admission is exactly `ExpectedAdmission::LAUNCH` replaces it (and drops `admission-not-booted`).
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
    /// `chain_id`; None when there is none. Informational only (`Admission::Recorded` keeps `admission-not-booted`
    /// unless a launch pin of the network matches, and is then not read at all): nothing missing
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
    if b.spec_version<2{out.push(blocker("spec-version",format!("SPEC_VERSION is {}: this release does not decode the v3 genesis (spec 2)",b.spec_version)))}
    if !b.consensus_pinned{out.push(blocker("consensus-unpinned","the consensus boundary sources differ from consensus-sources.lock; a release build refuses such a tree"))}
    if !b.test_features.is_empty(){out.push(blocker("test-features",format!("built with test-only features: {}",b.test_features.join(", "))))}
    out.extend(b.v3_launch_blockers.iter().map(|x|blocker("v3-launch-blocker",format!("{x} (V3_LAUNCH_BLOCKERS)"))));
    out.extend(b.plan_gates_open.iter().map(|(id,what)|blocker(format!("plan-gate-{id}"),format!("{id} {what} (development/public-network/PLAN.md)"))));
    let n=match subject{
        Subject::Build=>{out.push(blocker("no-network","readiness is a property of a network this build admits; the build alone is never ready"));return out}
        Subject::Unreadable(why)=>{out.push(blocker("genesis-unreadable",format!("the network's genesis specification cannot be read: {why}")));return out}
        Subject::Network(n)=>n,
    };
    if !n.v3_active{out.push(blocker("v3-inactive","the network's genesis is not a v3 genesis (specVersion below 2): the spec-1 rule set is retired and this release runs only v3 (specVersion 2) networks; the public launch is a v3 genesis"))}
    let pin=b.network_profiles.iter().find(|p|(p.chain_id,p.genesis_hash)==(n.chain_id,n.genesis_hash));
    if pin.is_none(){
        out.push(blocker("network-not-release-pinned",format!("chain {} genesis {} is not a launch network pinned by this release (V3-28 H2{})",
            n.chain_id,crate::types::hex(&n.genesis_hash),if b.network_profiles.is_empty(){"; it pins none yet"}else{""})))}
    // V3-28 H2 (1): the release's pin of this network, compared exactly; devnet.json is never compared.
    let launch_pin=pin.filter(|p|p.admission==ExpectedAdmission::LAUNCH);
    let facts=match (&n.admission,launch_pin){
        (Admission::Booted(a),_)=>Some(a.clone()),
        (Admission::Recorded(_),Some(p))=>Some(p.admission.facts(n.chain_id)),
        (Admission::Recorded(a),None)=>{
            let why=match pin{
                None=>"this release pins no expected admission for this network".to_string(),
                Some(p)=>format!("the expected admission this release pins for this network is not the launch admission (it differs in {})",
                    p.admission.differing(&ExpectedAdmission::LAUNCH).join(", ")),
            };
            out.push(blocker("admission-not-booted",format!("the launcher admits nothing, devnet.json's recorded admission is not evidence and {why} (V3-28 H2); a booted node's status reports the admission readiness is computed from")));
            a.clone()
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
        (Build{spec_version:2,consensus_pinned:true,test_features:vec![],v3_launch_blockers:vec![],plan_gates_open:vec![],
            network_profiles:vec![NetworkProfile{chain_id:777001,genesis_hash:[9;32],admission:ExpectedAdmission::LAUNCH}]},n)
    }
    fn cleared()->AdmissionFacts{AdmissionFacts{scope:"specification".into(),conditions:vec![],waivers:vec![],network_profile:true}}
    fn ids(b:&[Blocker])->Vec<&str>{b.iter().map(|x|x.id.as_str()).collect()}
    fn booted(n:&mut Network)->&mut AdmissionFacts{match &mut n.admission{Admission::Booted(a)=>a,Admission::Recorded(_)=>unreachable!()}}
    fn pinned(b:&mut Build)->&mut ExpectedAdmission{&mut b.network_profiles[0].admission}
    /// What a devnet.json records when every fact reads cleared, or when it records an experimental admission.
    fn forged()->Admission{Admission::Recorded(Some(cleared()))}
    fn experimental()->Admission{Admission::Recorded(Some(AdmissionFacts{scope:"experimental-devnet".into(),conditions:vec!["calibration-synthetic".into()],waivers:vec!["R26".into()],network_profile:false}))}

    #[test]
    fn this_build_is_not_ready_today_with_honest_reasons(){
        let b=blockers(&Build::this(),Subject::Build);let got=ids(&b);
        assert!(!got.contains(&"spec-version"),"SPEC_VERSION 2 is the release: {got:?}");assert!(got.contains(&"no-network"));
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
            (|b,_|b.spec_version=1,&["spec-version"]),
            (|b,_|b.consensus_pinned=false,&["consensus-unpinned"]),
            (|b,_|b.test_features=vec!["caps-test","io-fault"],&["test-features"]),
            (|b,_|b.v3_launch_blockers=vec!["D-V3-99 a later pre-launch item"],&["v3-launch-blocker"]),
            (|b,_|b.plan_gates_open=vec![("Q1","release review")],&["plan-gate-Q1"]),
            (|_,n|n.v3_active=false,&["v3-inactive"]),
            (|b,_|b.network_profiles.clear(),&["network-not-release-pinned"]),
            (|_,n|n.genesis_hash=[8;32],&["network-not-release-pinned"]),
            (|_,n|n.chain_id=777002,&["network-not-release-pinned"]),
            // A recorded admission never clears, even one recording every fact cleared (a forged devnet.json): without
            // a pin of the network, or with a pin whose expected admission is not exactly the launch admission.
            (|b,n|{b.network_profiles.clear();n.admission=forged()},&["network-not-release-pinned","admission-not-booted"]),
            (|b,n|{pinned(b).scope=Scope::ExperimentalDevnet;n.admission=forged()},&["admission-not-booted"]),
            (|b,n|{pinned(b).conditions=&["timing-not-a-bound"];n.admission=forged()},&["admission-not-booted"]),
            (|b,n|{pinned(b).waivers=&["R26"];n.admission=forged()},&["admission-not-booted"]),
            (|b,n|{pinned(b).profile_kind=ProfileKind::Simnet;n.admission=forged()},&["admission-not-booted"]),
            (|b,n|{pinned(b).node_kind=NodeKind::Simnet;n.admission=forged()},&["admission-not-booted"]),
            (|b,n|{pinned(b).scope=Scope::ExperimentalDevnet;n.admission=Admission::Recorded(None)},&["admission-not-booted","admission-unrecorded"]),
            // Without a matching pin the recorded facts still add their blockers.
            (|b,n|{pinned(b).scope=Scope::ExperimentalDevnet;n.admission=experimental()},
                &["admission-not-booted","admission-scope","admission-condition-calibration-synthetic","admission-waiver-R26","profile-kind"]),
            // A pinned launch admission on another chain than R22's still is not the network profile there.
            (|b,n|{n.chain_id=1656550731;b.network_profiles[0].chain_id=1656550731;n.admission=forged()},&["profile-kind"]),
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
    /// V3-28 H2 (1): the launcher drops `admission-not-booted` only when the release pins the network with exactly the
    /// launch admission, and then the pin, never devnet.json, is what it reads: a recorded admission neither clears nor
    /// adds anything, and recording none is no `admission-unrecorded`.
    #[test]
    fn a_pinned_launch_admission_replaces_the_recorded_one_only_on_an_exact_match(){
        let (b,mut n)=ready();
        for recorded in [forged(),experimental(),Admission::Recorded(None)]{
            n.admission=recorded.clone();
            assert_eq!(blockers(&b,Subject::Network(&n)),[],"{recorded:?}");
        }
        assert_eq!(ExpectedAdmission::LAUNCH.facts(R22_NETWORK_CHAIN_ID),cleared());
        assert!(!ExpectedAdmission::LAUNCH.facts(1656550731).network_profile,"R22");
        // The same pinned network with any one field of its expected admission changed keeps the blocker, and
        // its detail names the pin.
        let (mut b,mut n)=ready();pinned(&mut b).waivers=&["R26"];n.admission=forged();
        let got=blockers(&b,Subject::Network(&n));assert_eq!(ids(&got),["admission-not-booted"]);
        assert!(got[0].detail.contains("is not the launch admission (it differs in waivers)"),"{}",got[0].detail);
        let mixed=ExpectedAdmission{scope:Scope::ExperimentalDevnet,node_kind:NodeKind::Simnet,..ExpectedAdmission::LAUNCH};
        assert_eq!(mixed.differing(&ExpectedAdmission::LAUNCH),["scope","node kind"]);
        assert!(ExpectedAdmission::LAUNCH.differing(&ExpectedAdmission::LAUNCH).is_empty());
        b.network_profiles.clear();
        assert!(blockers(&b,Subject::Network(&n))[1].detail.contains("pins no expected admission for this network"));
        // A booted admission is read as booted, whatever the pin expects.
        let (mut b,mut n)=ready();pinned(&mut b).scope=Scope::ExperimentalDevnet;
        assert_eq!(blockers(&b,Subject::Network(&n)),[]);
        booted(&mut n).scope="experimental-devnet".into();b.network_profiles[0].admission=ExpectedAdmission::LAUNCH;
        assert_eq!(ids(&blockers(&b,Subject::Network(&n))),["admission-scope"]);
    }
    /// This release pins no network, so the launcher keeps `admission-not-booted` for every network today.
    #[test]
    fn no_network_is_pinned_today(){
        assert!(NETWORK_PROFILES.is_empty()&&Build::this().network_profiles.is_empty());
        let n=Network{chain_id:R22_NETWORK_CHAIN_ID,genesis_hash:[9;32],v3_active:true,admission:forged()};
        let got=blockers(&Build::this(),Subject::Network(&n));let got=ids(&got);
        assert!(got.contains(&"admission-not-booted")&&got.contains(&"network-not-release-pinned"),"{got:?}");
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
        // The v3 devnet activates v3 from genesis: no v3-inactive blocker.
        let network:Vec<&str>=got.iter().copied().skip_while(|id|*id!="v3-inactive"&&*id!="network-not-release-pinned").collect();
        let mut expect=vec!["network-not-release-pinned"];expect.extend(want.iter().map(String::as_str));
        assert_eq!(network,expect,"the network blockers of a booted devnet admission");
        assert!(!got.contains(&"admission-not-booted")&&!got.contains(&"admission-unrecorded"));
        assert!(n.v3_active);
    }
    /// The entry form set_network_identity.py writes into NETWORK_PROFILES compiles, decodes to the hash it names and
    /// pins the launch admission (its test reads WRITTEN's line and checks that the tool writes exactly this form).
    const WRITTEN:&[NetworkProfile]=&[NetworkProfile{chain_id:777001,genesis_hash:genesis_hash("0909090909090909090909090909090909090909090909090909090909090909"),admission:ExpectedAdmission::LAUNCH},NetworkProfile{chain_id:777002,genesis_hash:genesis_hash("00112233445566778899aabbccddeeff0123456789abcdef0fedcba987654321"),admission:ExpectedAdmission::LAUNCH}];
    #[test]
    fn network_profile_entries_decode_their_genesis_hash(){
        assert_eq!(WRITTEN[0],NetworkProfile{chain_id:777001,genesis_hash:[9;32],admission:ExpectedAdmission::LAUNCH});
        assert_eq!(crate::types::hex(&WRITTEN[1].genesis_hash),"0x00112233445566778899aabbccddeeff0123456789abcdef0fedcba987654321");
        assert!(WRITTEN.iter().all(|p|p.admission==ExpectedAdmission::LAUNCH),"the tool pins the launch admission");
        let (mut b,mut n)=ready();b.network_profiles=WRITTEN.to_vec();(n.chain_id,n.genesis_hash)=(WRITTEN[1].chain_id,WRITTEN[1].genesis_hash);
        assert!(blockers(&b,Subject::Network(&n)).is_empty(),"an entry in this form pins its network");
        n.genesis_hash=WRITTEN[0].genesis_hash;
        assert_eq!(ids(&blockers(&b,Subject::Network(&n))),["network-not-release-pinned"],"the pair, not the chain ID alone");
        // In the launcher (a recorded admission) the written entry's launch admission drops admission-not-booted (on
        // the R22 chain; elsewhere the pinned launch admission still is no network profile, R22).
        let mut n=Network{chain_id:WRITTEN[0].chain_id,genesis_hash:WRITTEN[0].genesis_hash,v3_active:true,admission:forged()};
        assert!(blockers(&b,Subject::Network(&n)).is_empty(),"the written entry pins the launch admission");
        (n.chain_id,n.genesis_hash)=(WRITTEN[1].chain_id,WRITTEN[1].genesis_hash);
        assert_eq!(ids(&blockers(&b,Subject::Network(&n))),["profile-kind"]);
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
