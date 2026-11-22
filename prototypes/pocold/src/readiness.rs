//! Public-launch approval and readiness (V3-28 H1, D-V3-28; OWNER-DECISIONS-2026-11-20 D4). Every value is computed
//! from facts, never written as a label. Launch approval is separated from technical readiness:
//! - `publicLaunchApproved`: the build's hard facts clear (SPEC_VERSION at least 2, consensus sources pinned, no
//!   test-only feature, `V3_LAUNCH_BLOCKERS` empty), the subject is a network with v3 active, (chainId, genesisHash) is
//!   a `NETWORK_PROFILES` pin with an approval reference whose expected admission is `ExpectedAdmission::PUBLIC_MAINNET`
//!   or `ExpectedAdmission::LAUNCH`, and the admission this node booted with (`Admission::Booted`, never devnet.json and
//!   never a pin standing in for a boot) matches that expected admission exactly. It is computed from these facts
//!   directly, not from the blocker list.
//! - `launchApprovalReference`: the approval reference of this release's pin of the network (a recorded fact of the
//!   release, not a verdict), or null.
//! - `launchBlockers`: the hard blockers only, each a stable id with what is missing, in this order: spec-version,
//!   consensus-unpinned, test-features, v3-launch-blocker; no-network or genesis-unreadable; v3-inactive,
//!   network-not-release-pinned, launch-not-approved, admission-unrecorded, admission-not-booted, admission-mismatch.
//!   Every term of the approval has a blocker, so for a network `publicLaunchApproved` holds exactly when the list is
//!   empty, and the list states every reason `publicLaunchReady` is false.
//! - `publicLaunchReady`: `publicLaunchApproved` and no launch blocker.
//! - `technicalReadiness`: `{complete, pending}`, what is still open technically, never claimed passed: every open gate
//!   of the public-network acceptance ledger (`PLAN_GATES_OPEN`, plan-gate-<id>), no-network or genesis-unreadable, and
//!   what the admission reports: admission-scope (not specification), admission-condition-<id>, admission-waiver-<rule>
//!   and profile-kind (not the network profile on a network node). `complete` is true only with nothing pending. These
//!   items are reported, not approval blockers (OWNER-DECISIONS-2026-11-20 D4 and D6 C: the technical gates are reported
//!   and do not block the launch decision).
//! - the network's identity as the facts support it: `networkName` ("PoCol Mainnet" for a network on a public network
//!   chain that this release pins, otherwise left out; `network_display` may still designate the tree's identity, as the
//!   identity switch keeps it equal to the pinned network), `networkEnvironment` ("mainnet"
//!   on a public network chain, `profile::PUBLIC_NETWORK_CHAINS`; "test" on the local test chain; "devnet" on any other
//!   chain) and `networkScope` (the scope of the admission this node booted with; null where nothing was booted).
//!
//! Producers: `pocold build-info` and `pocol build-info` (the build alone: never approved, `no-network`, null identity
//! fields), `pocold status` and `pocol_status` (the admission this node booted with), `init-testnet` (the admission it
//! just made) and the launcher's network description (the invitation's files). The launcher boots nothing, so it always
//! reports `admission-not-booted` and is never approved: what a devnet.json records about its own admission is operator
//! metadata that nothing commits to, never evidence (D-V3-28). Its technical readiness is read from this release's pinned
//! expected admission of the network when there is one, and otherwise from devnet.json's record (bounded and validated);
//! both are informational, and nothing in them is claimed passed.
//!
//! Node-local reporting only: no validity verdict, fork choice or persisted byte reads it. Deployment branding
//! (`network_display`) is never readiness evidence: a designated network reports the same approval and blockers.
use pocol_lp4_execution::{admission::{self,Admitted,Scope},profile::{network_chain,NodeKind,ProfileKind,PUBLIC_NETWORK_CHAINS}};

/// One reason the network is not approved or ready for a public launch, or one open technical item: a stable id and
/// what is missing.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Blocker{pub id:String,pub detail:String}
fn blocker(id:impl Into<String>,detail:impl Into<String>)->Blocker{Blocker{id:id.into(),detail:detail.into()}}

/// The acceptance-ledger gates of development/public-network/PLAN.md that are still open, with what remains. A gate
/// leaves this list only when the ledger marks it closed with its evidence (its status starts with "Closed"), never by
/// a label change here alone; the test `plan_gates_open_match_the_acceptance_ledger` keeps the two equal. They are
/// reported in `technicalReadiness.pending`, never dropped.
pub const PLAN_GATES_OPEN:&[(&str,&str)]=&[
    ("K1","independent identities: the operator key recovery drill (RECOVERY.md), a server run with the encrypted key credential (pocol start --key-credential) and an encrypted keystore or HSM for the other keys remain"),
    ("J1","open miner enrollment: eligibility churn and load, and network-scale enrollment remain (newcomers' first fee: the automated fee faucet, 0.002 PCOL once per new address)"),
    ("I1","stable network identity: the ceremony's genesis and its pin in the clients, the normative digest adoption and an independently reproduced binary release remain"),
    ("A1","honest admission: real worst-case execution calibration, the R18/R21 obligations and the RpcGuard gaps remain"),
    ("S1","bounded historical state: K_eff and historical RPC behavior within the resource budget remain"),
    ("N1","Internet exposure: actual Internet ingress acceptance and the remaining RpcGuard controls remain"),
    ("L1","sustained operation: the launch evidence of REHEARSAL.md (T1-T8, M1-M4, R-off, D1, S1, S2 and the L1 drills), measured on PoCol Mainnet after the launch without a rehearsal (OWNER-DECISIONS-2026-11-20 D1), an actual multi-host soak and an upgrade drill remain"),
    ("E1","network lifetime and economics: the genesis ceremony (it enters the founders' decided reward addresses) and the launch evidence, measured on PoCol Mainnet after the launch, remain"),
    ("Q1","release review: the release candidate's exact-commit full CI and an independent security review (commissioned after the launch) remain; the unresolved findings and public limitations are tracked in development/public-network/FINDINGS.md"),
];
/// V3-28 H2 (1): the admission this release expects a node booting a pinned network to get, every field the
/// readiness of an admission reads.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ExpectedAdmission{
    pub scope:Scope,pub conditions:&'static [&'static str],pub waivers:&'static [&'static str],
    pub profile_kind:ProfileKind,pub node_kind:NodeKind,
}
impl ExpectedAdmission{
    /// Scope specification, no condition, no waiver, the network profile on a network node. No profile reaches it
    /// today (the gate always reports its deferred obligations as conditions).
    pub const LAUNCH:Self=Self{scope:Scope::Specification,conditions:&[],waivers:&[],profile_kind:ProfileKind::Network,node_kind:NodeKind::Node};
    /// The public network's admission (OWNER-DECISIONS-2026-11-20 D5): the network profile on a network node in scope
    /// public-network, with exactly the conditions and the owner's explicit R13, R21 and R26 waivers that the
    /// launcher's boot gives a ceremony genesis of deploy/mainnet/v3-profile.json on its own chain (tests/fn4_profiles.rs
    /// boots one and compares every field). set_network_identity.py pins it, with the approval reference, for a network
    /// on a public network chain. Never specification compliance.
    pub const PUBLIC_MAINNET:Self=Self{scope:Scope::PublicNetwork,
        conditions:&["rpc-absent","continuous-protocol","timing-not-a-bound","gas-monotonicity-assumed","gate-deferred-general-R18","gate-deferred-general-R21","gate-deferred-caps64"],
        waivers:&["R13","R21","R26"],profile_kind:ProfileKind::Network,node_kind:NodeKind::Node};
    /// The experimental admission of every other allowed network (FRESH-NETWORK-CP.md §1 "Launch admission", Q3 = a,
    /// superseded for the mainnet chain by OWNER-DECISIONS-2026-11-20 D5): a Simnet profile on a Simnet node in scope
    /// experimental-devnet, with the conditions and waivers the launcher's boot gives a ceremony genesis of
    /// deploy/mainnet/v3-rehearsal-profile.json (tests/fn4_profiles.rs boots it and checks every field).
    /// set_network_identity.py pins it, without an approval reference, for a switched-to network off the public network
    /// chains. It is never approvable.
    pub const FRESH_NETWORK:Self=Self{scope:Scope::ExperimentalDevnet,conditions:&["rpc-absent","continuous-protocol","timing-not-a-bound","gas-monotonicity-assumed","gate-deferred-general-R18","gate-deferred-general-R21","gate-deferred-caps64","r21-retention-stress"],
        waivers:&["R26"],profile_kind:ProfileKind::Simnet,node_kind:NodeKind::Simnet};
    /// The name this module gives an expected admission in a detail.
    pub fn name(&self)->&'static str{
        if *self==Self::PUBLIC_MAINNET{"PUBLIC_MAINNET"}else if *self==Self::LAUNCH{"LAUNCH"}else if *self==Self::FRESH_NETWORK{"FRESH_NETWORK"}
        else{"an expected admission of its own"}
    }
    /// Whether a pin of a network with this expected admission (and an approval reference) can approve its launch:
    /// `PUBLIC_MAINNET` or `LAUNCH` exactly.
    pub fn approvable(&self)->bool{*self==Self::PUBLIC_MAINNET||*self==Self::LAUNCH}
    /// The fields in which this expected admission differs from `other`, in declaration order.
    pub fn differing(&self,other:&Self)->Vec<&'static str>{
        [(self.scope!=other.scope,"scope"),(self.conditions!=other.conditions,"conditions"),(self.waivers!=other.waivers,"waivers"),
            (self.profile_kind!=other.profile_kind,"profile kind"),(self.node_kind!=other.node_kind,"node kind")]
            .into_iter().filter(|(d,_)|*d).map(|(_,f)|f).collect()
    }
    /// The fields in which the admission facts `f` differ from this expected admission, compared exactly: the scope
    /// name, the conditions and the waivers as ordered lists, and the profile and node kinds (a kind `f` does not know
    /// differs).
    pub fn mismatches(&self,f:&AdmissionFacts)->Vec<&'static str>{
        let list=|want:&[&str],got:&[String]|want.len()==got.len()&&want.iter().zip(got).all(|(w,g)|w==g);
        [(f.scope!=self.scope.name(),"scope"),(!list(self.conditions,&f.conditions),"conditions"),(!list(self.waivers,&f.waivers),"waivers"),
            (f.profile_kind!=Some(self.profile_kind),"profile kind"),(f.node_kind!=Some(self.node_kind),"node kind")]
            .into_iter().filter(|(d,_)|*d).map(|(_,n)|n).collect()
    }
    /// Whether the admission facts `f` are exactly this expected admission (`mismatches` is empty).
    pub fn matches(&self,f:&AdmissionFacts)->bool{self.mismatches(f).is_empty()}
    /// The pinned admission's facts on `chain_id`: as at boot, a network profile counts only on an R22 network chain
    /// (`profile::network_chain`: 777001 or a public network chain).
    pub fn facts(&self,chain_id:u64)->AdmissionFacts{
        AdmissionFacts{scope:self.scope.name().into(),conditions:self.conditions.iter().map(|c|c.to_string()).collect(),
            waivers:self.waivers.iter().map(|w|w.to_string()).collect(),
            network_profile:self.profile_kind==ProfileKind::Network&&self.node_kind==NodeKind::Node&&network_chain(chain_id),
            profile_kind:Some(self.profile_kind),node_kind:Some(self.node_kind)}
    }
}
/// V3-28 H2: one network this release pins, (chainId, genesisHash), with the admission it expects there and the owner's
/// launch approval reference (a decision record and its decision, e.g. "OWNER-DECISIONS-2026-11-20 D1"), if any.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct NetworkProfile{pub chain_id:u64,pub genesis_hash:[u8;32],pub admission:ExpectedAdmission,pub approval:Option<&'static str>}
/// The approval reference set_network_identity.py writes for a network on a public network chain.
pub const PUBLIC_MAINNET_APPROVAL:&str="OWNER-DECISIONS-2026-11-20 D1";
/// V3-28 H2: the networks this release pins. Empty while the tree pins a retired network: no network is pinned yet,
/// whatever its name or designation. deploy/mainnet/set_network_identity.py writes it at the identity switch and its
/// `--check` verifies it: the entry of the network the clients are switched to, unless that is a retired or the
/// rehearsal network, entries separated by `,`, the declaration on one line in that form. On a public network chain
/// (`profile::PUBLIC_NETWORK_CHAINS`) the entry is
/// `NetworkProfile{chain_id:<chainId>,genesis_hash:genesis_hash("<64 hex>"),admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")}`,
/// which approves the launch of that network once a node booted on it reports exactly that admission; on any other
/// chain it is
/// `NetworkProfile{chain_id:<chainId>,genesis_hash:genesis_hash("<64 hex>"),admission:ExpectedAdmission::FRESH_NETWORK,approval:None}`,
/// which removes only `network-not-release-pinned` and approves nothing. The launcher reports either pin's reference
/// and expected admission, and keeps `admission-not-booted`.
pub const NETWORK_PROFILES:&[NetworkProfile]=&[NetworkProfile{chain_id:2656089850,genesis_hash:genesis_hash("664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e"),admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")}];
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
/// At most this many distinct recorded condition ids and waiver rules are reflected each; past it one `overflow`
/// entry stands for the rest.
pub const RECORDED_MAX:usize=32;
/// What an admission says about readiness (a boot's, a pin's, or one recorded in devnet.json): its scope name, its
/// condition ids and waiver rules in order, whether it is the network profile on a network node (R22), and the profile
/// and node kinds where they are known.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct AdmissionFacts{
    pub scope:String,pub conditions:Vec<String>,pub waivers:Vec<String>,pub network_profile:bool,
    pub profile_kind:Option<ProfileKind>,pub node_kind:Option<NodeKind>,
}
/// Where a network's admission facts come from.
#[derive(Clone,Debug,PartialEq,Eq)]
pub enum Admission{
    /// The admission this node booted with (`Admitted`): the only evidence approval reads.
    Booted(AdmissionFacts),
    /// What a devnet.json records about itself (the launcher), None when it records none. Never evidence: the
    /// launcher always reports `admission-not-booted`, and the record is read only for technical readiness, and only
    /// when this release pins no expected admission for the network.
    Recorded(Option<AdmissionFacts>),
}
/// A recorded id kept as written only in the shape the gate writes, else `unreadable`: at most 64 bytes of
/// `-`-separated parts, each `[a-z0-9]+` or a rule `R[0-9]{1,3}` (as in `gate-deferred-general-R18`).
fn recorded_id(v:&serde_json::Value)->String{
    let part=|p:&str|!p.is_empty()&&(p.bytes().all(|b|b.is_ascii_lowercase()||b.is_ascii_digit())
        ||(p.len()<=4&&p.starts_with('R')&&p.len()>1&&p[1..].bytes().all(|b|b.is_ascii_digit())));
    match v.as_str(){Some(s) if (1..=64).contains(&s.len())&&s.split('-').all(part)=>s.into(),_=>"unreadable".into()}
}
/// A recorded waiver rule kept only as `R[0-9]{1,3}`, else `unreadable`.
fn recorded_rule(v:&serde_json::Value)->String{
    match v.as_str(){Some(s) if (2..=4).contains(&s.len())&&s.starts_with('R')&&s[1..].bytes().all(|b|b.is_ascii_digit())=>s.into(),_=>"unreadable".into()}
}
impl AdmissionFacts{
    pub fn of(a:&Admitted)->Self{
        let r=a.report();let (kind,node)=(a.profile().kind(),a.profile().node_kind());
        Self{scope:a.scope().name().into(),conditions:r.conditions.iter().map(|c|c.id.to_string()).collect(),waivers:r.waivers.iter().map(|w|w.rule.to_string()).collect(),
            network_profile:kind==ProfileKind::Network&&node==NodeKind::Node&&network_chain(a.profile().spec().chain_id),
            profile_kind:Some(kind),node_kind:Some(node)}
    }
    /// The admission recorded in a devnet.json (`admission_json`) and its declared profileKind, for the genesis's
    /// `chain_id`; None when there is none. Informational only (`Admission::Recorded`): nothing missing or malformed
    /// reads as cleared, ids outside the gate's shape read `unreadable`, each list is deduplicated and capped at
    /// `RECORDED_MAX` (then `overflow`), a recorded network profile counts only on an R22 network chain
    /// (`profile::network_chain`), and the profile kind is the declared one (network, simnet or synthetic) or unknown.
    /// devnet.json records no node kind.
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
        let declared=info["profileKind"].as_str();
        let profile_kind=[(ProfileKind::Network,"network"),(ProfileKind::Simnet,"simnet"),(ProfileKind::Synthetic,"synthetic")].into_iter()
            .find(|(_,n)|declared.is_some_and(|k|k.eq_ignore_ascii_case(n))).map(|(k,_)|k);
        Some(Self{scope,conditions:ids("conditions","id",recorded_id),waivers:ids("waivers","rule",recorded_rule),
            network_profile:network_chain(chain_id)&&profile_kind==Some(ProfileKind::Network),profile_kind,node_kind:None})
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

/// The network's environment as its chain id supports it: "mainnet" on a public network chain
/// (`profile::PUBLIC_NETWORK_CHAINS`), "test" on the local test chain (`testnet::TEST_CHAIN_ID`), "devnet" on any other.
pub fn network_environment(chain_id:u64)->&'static str{
    if PUBLIC_NETWORK_CHAINS.contains(&chain_id){"mainnet"}else if chain_id==crate::testnet::TEST_CHAIN_ID{"test"}else{"devnet"}
}
/// The wallet's `testCurrency`: whether the chain's PCOL is a test currency. It is not on a public network chain
/// (`profile::PUBLIC_NETWORK_CHAINS`, the mainnet's), whatever its pin or approval; it is on every other chain (the
/// rehearsal, the local test chain, a devnet). A label of the chain, never of launch approval or readiness.
pub fn test_currency(chain_id:u64)->bool{!PUBLIC_NETWORK_CHAINS.contains(&chain_id)}
/// The line `pocol start` prints once its node has booted: the network's identity fields and its approval and readiness
/// as this build computes them for the admission the node booted with (`of_admitted`), and whether its currency is a
/// test currency. Every value is computed; pocol_status carries the reasons.
pub fn start_line(a:&Admitted)->String{
    let r=of_admitted(a);let chain=a.profile().spec().chain_id;
    format!("PoCol node booted: {}, chain {chain}, environment {}, admission scope {} (specification compliant: {}); publicLaunchApproved {}, publicLaunchReady {}, technical readiness complete {} ({} pending); currency PCOL{}. pocol_status lists every launch blocker and pending item. Admission, memory and lifetime limits apply.",
        r.network_name.unwrap_or("network"),r.network_environment.unwrap_or("unknown"),r.network_scope.as_deref().unwrap_or("none"),
        a.specification_compliant(),r.approved,r.ready(),r.technically_complete(),r.pending.len(),if test_currency(chain){" (test currency)"}else{""})
}
/// The public network's display name: the name the ceremony script, the packages and `network_display` give the network
/// of deploy/mainnet/v3-profile.json.
pub const PUBLIC_MAINNET_NAME:&str="PoCol Mainnet";

/// Everything this module computes for one subject.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Readiness{
    /// `publicLaunchApproved`, computed from the facts (module doc), never from `blockers`.
    pub approved:bool,
    /// `launchApprovalReference`: the approval reference of this release's pin of the network, if any.
    pub approval_reference:Option<&'static str>,
    /// `launchBlockers`: the hard blockers.
    pub blockers:Vec<Blocker>,
    /// `technicalReadiness.pending`.
    pub pending:Vec<Blocker>,
    /// `networkName`, `networkEnvironment` and `networkScope`.
    pub network_name:Option<&'static str>,pub network_environment:Option<&'static str>,pub network_scope:Option<String>,
}
impl Readiness{
    /// `publicLaunchReady`: approved and no launch blocker.
    pub fn ready(&self)->bool{self.approved&&self.blockers.is_empty()}
    /// `technicalReadiness.complete`: nothing pending.
    pub fn technically_complete(&self)->bool{self.pending.is_empty()}
}

/// The approval, the hard blockers, the technical readiness and the identity fields of `subject` under build `b`.
pub fn readiness(b:&Build,subject:Subject)->Readiness{
    let mut hard=Vec::new();let mut pending:Vec<Blocker>=Vec::new();
    if b.spec_version<2{hard.push(blocker("spec-version",format!("SPEC_VERSION is {}: this release does not decode the v3 genesis (spec 2)",b.spec_version)))}
    if !b.consensus_pinned{hard.push(blocker("consensus-unpinned","the consensus boundary sources differ from consensus-sources.lock; a release build refuses such a tree"))}
    if !b.test_features.is_empty(){hard.push(blocker("test-features",format!("built with test-only features: {}",b.test_features.join(", "))))}
    // Each V3_LAUNCH_BLOCKERS entry is by definition an open pre-launch defect; the boot gate refuses any non-Simnet
    // admission while the list is non-empty.
    hard.extend(b.v3_launch_blockers.iter().map(|x|blocker("v3-launch-blocker",format!("{x} (V3_LAUNCH_BLOCKERS)"))));
    let build_clear=hard.is_empty();
    pending.extend(b.plan_gates_open.iter().map(|(id,what)|blocker(format!("plan-gate-{id}"),format!("{id} {what} (development/public-network/PLAN.md)"))));
    let none=|hard,pending|Readiness{approved:false,approval_reference:None,blockers:hard,pending,network_name:None,network_environment:None,network_scope:None};
    let n=match subject{
        Subject::Build=>{
            let b=blocker("no-network","readiness is a property of a network this build admits; the build alone is never approved or ready");
            hard.push(b.clone());pending.push(b);return none(hard,pending)
        }
        Subject::Unreadable(why)=>{
            let b=blocker("genesis-unreadable",format!("the network's genesis specification cannot be read: {why}"));
            hard.push(b.clone());pending.push(b);return none(hard,pending)
        }
        Subject::Network(n)=>n,
    };
    if !n.v3_active{hard.push(blocker("v3-inactive","the network's genesis is not a v3 genesis (specVersion below 2): the spec-1 rule set is retired and this release runs only v3 (specVersion 2) networks; the public launch is a v3 genesis"))}
    let pin=b.network_profiles.iter().find(|p|(p.chain_id,p.genesis_hash)==(n.chain_id,n.genesis_hash));
    if pin.is_none(){
        hard.push(blocker("network-not-release-pinned",format!("chain {} genesis {} is not a network pinned by this release (V3-28 H2{})",
            n.chain_id,crate::types::hex(&n.genesis_hash),if b.network_profiles.is_empty(){"; it pins none yet"}else{""})))}
    // The pin that can approve: an approval reference and an approvable expected admission.
    let approving=pin.filter(|p|p.approval.is_some()&&p.admission.approvable());
    if approving.is_none(){
        let why=match pin{
            None=>"this release pins no launch approval for it".to_string(),
            Some(p) if p.approval.is_none()=>format!("this release pins it (expected admission {}) without a launch approval reference",p.admission.name()),
            Some(p)=>format!("this release pins it with approval reference {} but with the expected admission {}, which is neither PUBLIC_MAINNET nor LAUNCH",p.approval.unwrap_or_default(),p.admission.name()),
        };
        hard.push(blocker("launch-not-approved",format!("the public launch of this network is not approved: {why} (OWNER-DECISIONS-2026-11-20 D4)")));
    }
    // The admission facts technical readiness reads: the boot's; in the launcher, the pinned expected admission when
    // there is one, else devnet.json's record. Approval reads only a boot.
    let (facts,booted)=match &n.admission{
        Admission::Booted(f)=>(Some(f.clone()),Some(f)),
        Admission::Recorded(r)=>(pin.map(|p|p.admission.facts(n.chain_id)).or_else(||r.clone()),None),
    };
    if facts.is_none(){
        let b=blocker("admission-unrecorded","no admission is recorded for this network; a node's status reports the admission it booted with");
        hard.push(b.clone());pending.push(b);
    }
    if booted.is_none(){
        hard.push(blocker("admission-not-booted",format!("the launcher boots nothing; approval and readiness are computed only by a node booted on this network (pocol_status). This release pins {} for it, approval reference {}",
            pin.map_or("no expected admission",|p|p.admission.name()),pin.and_then(|p|p.approval).unwrap_or("none"))));
    }
    if let (Some(p),Some(f))=(pin,booted){
        let differ=p.admission.mismatches(f);
        if !differ.is_empty(){
            hard.push(blocker("admission-mismatch",format!("the admission this node booted with differs from the expected admission {} this release pins for the network in {}",
                p.admission.name(),differ.join(", "))));
        }
    }
    if let Some(a)=&facts{
        if a.scope!=Scope::Specification.name(){pending.push(blocker("admission-scope",format!("admitted in scope {}, not specification",a.scope)))}
        pending.extend(a.conditions.iter().map(|c|blocker(format!("admission-condition-{c}"),format!("admission condition {c} (see admission.conditions)"))));
        pending.extend(a.waivers.iter().map(|w|blocker(format!("admission-waiver-{w}"),format!("{w} partly waived (see admission.waivers)"))));
        if !a.network_profile{pending.push(blocker("profile-kind","not admitted as the network profile on a network node (R22)"))}
    }
    let approved=build_clear&&n.v3_active&&approving.zip(booted).is_some_and(|(p,f)|p.admission.matches(f));
    let public=PUBLIC_NETWORK_CHAINS.contains(&n.chain_id);
    let network_name=(public&&pin.is_some()).then_some(PUBLIC_MAINNET_NAME);
    Readiness{approved,approval_reference:pin.and_then(|p|p.approval),blockers:hard,pending,network_name,
        network_environment:Some(network_environment(n.chain_id)),network_scope:booted.map(|f|f.scope.clone())}
}
/// The hard blockers of `subject` (`launchBlockers`).
pub fn blockers(b:&Build,subject:Subject)->Vec<Blocker>{readiness(b,subject).blockers}
fn list(b:&[Blocker])->Vec<serde_json::Value>{b.iter().map(|b|serde_json::json!({"id":b.id,"detail":b.detail})).collect()}
/// `publicLaunchApproved`, `launchApprovalReference`, `publicLaunchReady`, `launchBlockers` [{id, detail}],
/// `technicalReadiness` {complete, pending [{id, detail}]}, `networkEnvironment`, `networkScope` (null where the facts
/// support none) and `networkName`. `networkName` is left out where no name is supported, as `network_display` leaves it
/// out for an undesignated network: the wallets show their own neutral label for a missing name (an Android
/// `optString` would show a JSON null as the text "null").
pub fn json(r:&Readiness)->serde_json::Value{
    let mut v=serde_json::json!({"publicLaunchApproved":r.approved,"launchApprovalReference":r.approval_reference,"publicLaunchReady":r.ready(),
        "launchBlockers":list(&r.blockers),"technicalReadiness":{"complete":r.technically_complete(),"pending":list(&r.pending)},
        "networkEnvironment":r.network_environment,"networkScope":r.network_scope});
    if let Some(name)=r.network_name{v["networkName"]=name.into()}
    v
}
/// Sets the fields of `json` on a JSON object.
pub fn annotate(v:&mut serde_json::Value,r:&Readiness){
    if let (Some(o),serde_json::Value::Object(r))=(v.as_object_mut(),json(r)){o.extend(r)}
}
/// The readiness of the network an admission booted (status, init-testnet).
pub fn of_admitted(a:&Admitted)->Readiness{readiness(&Build::this(),Subject::Network(&Network::admitted(a)))}
/// `build-info`: the build's identity with this build's readiness (the build alone, so never approved or ready).
pub fn build_info()->String{
    let mut info:serde_json::Value=serde_json::from_str(crate::identity::BUILD_INFO).expect("build.rs writes JSON");
    annotate(&mut info,&readiness(&Build::this(),Subject::Build));info.to_string()
}

#[cfg(test)]
mod tests{
    use super::*;
    const PUBLIC:u64=2656089850;
    /// A build with every build fact cleared that pins the public chain's network [9;32] with `PUBLIC_MAINNET` and its
    /// approval reference, and a node booted on it with exactly that admission: approved and ready.
    fn ready()->(Build,Network){
        let n=Network{chain_id:PUBLIC,genesis_hash:[9;32],v3_active:true,admission:Admission::Booted(ExpectedAdmission::PUBLIC_MAINNET.facts(PUBLIC))};
        (Build{spec_version:2,consensus_pinned:true,test_features:vec![],v3_launch_blockers:vec![],plan_gates_open:vec![],
            network_profiles:vec![NetworkProfile{chain_id:PUBLIC,genesis_hash:[9;32],admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some(PUBLIC_MAINNET_APPROVAL)}]},n)
    }
    /// The facts of a specification admission of the network profile on a network node.
    fn cleared()->AdmissionFacts{AdmissionFacts{scope:"specification".into(),conditions:vec![],waivers:vec![],network_profile:true,profile_kind:Some(ProfileKind::Network),node_kind:Some(NodeKind::Node)}}
    fn ids(b:&[Blocker])->Vec<&str>{b.iter().map(|x|x.id.as_str()).collect()}
    fn booted(n:&mut Network)->&mut AdmissionFacts{match &mut n.admission{Admission::Booted(a)=>a,Admission::Recorded(_)=>unreachable!()}}
    fn pinned(b:&mut Build)->&mut NetworkProfile{&mut b.network_profiles[0]}
    /// What a devnet.json records when every fact reads cleared, or when it records an experimental admission.
    fn forged()->Admission{Admission::Recorded(Some(AdmissionFacts{node_kind:None,..cleared()}))}
    fn experimental()->Admission{Admission::Recorded(Some(AdmissionFacts{scope:"experimental-devnet".into(),conditions:vec!["calibration-synthetic".into()],waivers:vec!["R26".into()],
        network_profile:false,profile_kind:Some(ProfileKind::Simnet),node_kind:None}))}
    /// The ids technical readiness reports for the admission facts `a` (after the plan gates).
    fn admission_pending(a:&AdmissionFacts)->Vec<String>{
        let mut want=vec![];
        if a.scope!="specification"{want.push("admission-scope".to_string())}
        want.extend(a.conditions.iter().map(|c|format!("admission-condition-{c}")));
        want.extend(a.waivers.iter().map(|w|format!("admission-waiver-{w}")));
        if !a.network_profile{want.push("profile-kind".into())}
        want
    }
    /// The invariants of OWNER-DECISIONS-2026-11-20 D4 that hold for every subject: ready is approved with no blocker,
    /// a subject that is not ready has a blocker, the technical readiness is complete exactly with nothing pending, and
    /// for a network approval holds exactly when no blocker is left. The JSON says the same.
    fn invariants(r:&Readiness,network:bool){
        assert_eq!(r.ready(),r.approved&&r.blockers.is_empty());
        if !r.ready(){assert!(!r.blockers.is_empty(),"{r:?}")}
        if network{assert_eq!(r.approved,r.blockers.is_empty(),"{r:?}")}else{assert!(!r.approved)}
        let j=json(r);
        assert_eq!((j["publicLaunchApproved"].as_bool(),j["publicLaunchReady"].as_bool()),(Some(r.approved),Some(r.ready())));
        assert_eq!(j["technicalReadiness"]["complete"].as_bool(),Some(r.pending.is_empty()));
        assert_eq!(j["launchBlockers"].as_array().unwrap().len(),r.blockers.len());assert_eq!(j["technicalReadiness"]["pending"].as_array().unwrap().len(),r.pending.len());
        for x in j["launchBlockers"].as_array().unwrap().iter().chain(j["technicalReadiness"]["pending"].as_array().unwrap()){
            assert!(x["id"].is_string()&&x["detail"].as_str().is_some_and(|d|!d.is_empty()),"{x}");
        }
        assert_eq!(j["launchApprovalReference"].as_str(),r.approval_reference);
    }

    #[test]
    fn this_build_is_not_approved_today_with_honest_reasons(){
        let r=readiness(&Build::this(),Subject::Build);invariants(&r,false);
        let got=ids(&r.blockers);
        assert!(!got.contains(&"spec-version"),"SPEC_VERSION 2 is the release: {got:?}");assert!(got.contains(&"no-network"));
        assert_eq!(got.contains(&"consensus-unpinned"),!crate::identity::PINNED);
        assert!(admission::V3_LAUNCH_BLOCKERS.is_empty()&&!got.contains(&"v3-launch-blocker"),"V3_LAUNCH_BLOCKERS is empty today");
        // The open plan gates are reported in technical readiness, not as launch blockers.
        let pending=ids(&r.pending);
        for (g,_) in PLAN_GATES_OPEN{let id=format!("plan-gate-{g}");assert!(pending.contains(&id.as_str())&&!got.contains(&id.as_str()),"{g}")}
        assert!(pending.contains(&"no-network"),"technical readiness is never complete without a network");
        let j=json(&r);
        assert_eq!((j["publicLaunchApproved"].clone(),j["publicLaunchReady"].clone(),j["launchApprovalReference"].clone()),(false.into(),false.into(),serde_json::Value::Null));
        assert_eq!(j["technicalReadiness"]["complete"],false);
        for k in ["networkName","networkEnvironment","networkScope"]{assert!(j[k].is_null(),"{k}: {j}")}
        assert!(j.get("networkName").is_none(),"an unsupported name is left out, never written as null: {j}");
    }
    #[test]
    fn approved_and_ready_only_when_every_term_holds_and_each_reason_alone_blocks(){
        let (b,n)=ready();let r=readiness(&b,Subject::Network(&n));invariants(&r,true);
        assert!(r.approved&&r.ready()&&r.blockers.is_empty(),"{r:?}");
        assert_eq!(r.approval_reference,Some("OWNER-DECISIONS-2026-11-20 D1"));
        // Approved is not technically complete: the public-network admission's scope, conditions and waivers stay pending.
        assert_eq!(ids(&r.pending),admission_pending(&ExpectedAdmission::PUBLIC_MAINNET.facts(PUBLIC)));
        assert!(!r.technically_complete());
        assert_eq!((r.network_name,r.network_environment,r.network_scope.as_deref()),(Some("PoCol Mainnet"),Some("mainnet"),Some("public-network")));
        let j=json(&r);assert_eq!((j["publicLaunchApproved"].clone(),j["publicLaunchReady"].clone()),(true.into(),true.into()));
        // The build alone and an unreadable genesis are never approved.
        let build=readiness(&b,Subject::Build);invariants(&build,false);assert_eq!(ids(&build.blockers),["no-network"]);
        let bad=readiness(&b,Subject::Unreadable("bad".into()));invariants(&bad,false);assert_eq!(ids(&bad.blockers),["genesis-unreadable"]);
        assert_eq!(ids(&bad.pending),["genesis-unreadable"]);assert!(bad.approval_reference.is_none()&&bad.network_environment.is_none());
        type Edit=fn(&mut Build,&mut Network);
        let cases:&[(Edit,&[&str])]=&[
            (|b,_|b.spec_version=1,&["spec-version"]),
            (|b,_|b.consensus_pinned=false,&["consensus-unpinned"]),
            (|b,_|b.test_features=vec!["caps-test","io-fault"],&["test-features"]),
            (|b,_|b.v3_launch_blockers=vec!["D-V3-99 a later pre-launch item"],&["v3-launch-blocker"]),
            (|_,n|n.v3_active=false,&["v3-inactive"]),
            // An unpinned network: another genesis, another chain, the rehearsal chain, the local test chain.
            (|b,_|b.network_profiles.clear(),&["network-not-release-pinned","launch-not-approved"]),
            (|_,n|n.genesis_hash=[8;32],&["network-not-release-pinned","launch-not-approved"]),
            (|_,n|n.chain_id=777002,&["network-not-release-pinned","launch-not-approved"]),
            (|_,n|n.chain_id=1656550731,&["network-not-release-pinned","launch-not-approved"]),
            (|_,n|n.chain_id=777990,&["network-not-release-pinned","launch-not-approved"]),
            // A pin without an approval reference, or with an admission that cannot approve.
            (|b,_|pinned(b).approval=None,&["launch-not-approved"]),
            (|b,n|{pinned(b).admission=ExpectedAdmission::FRESH_NETWORK;n.admission=Admission::Booted(ExpectedAdmission::FRESH_NETWORK.facts(PUBLIC))},&["launch-not-approved"]),
            (|b,n|{pinned(b).admission=ExpectedAdmission::FRESH_NETWORK;pinned(b).approval=None;n.admission=Admission::Booted(ExpectedAdmission::FRESH_NETWORK.facts(PUBLIC))},&["launch-not-approved"]),
            (|b,_|pinned(b).admission=ExpectedAdmission::FRESH_NETWORK,&["launch-not-approved","admission-mismatch"]),
            (|b,_|pinned(b).admission=ExpectedAdmission{waivers:&["R26"],..ExpectedAdmission::PUBLIC_MAINNET},&["launch-not-approved","admission-mismatch"]),
            // The launcher, or any devnet.json facts: never approved, whatever it records.
            (|_,n|n.admission=forged(),&["admission-not-booted"]),
            (|_,n|n.admission=experimental(),&["admission-not-booted"]),
            (|_,n|n.admission=Admission::Recorded(None),&["admission-not-booted"]),
            (|b,n|{b.network_profiles.clear();n.admission=forged()},&["network-not-release-pinned","launch-not-approved","admission-not-booted"]),
            (|b,n|{b.network_profiles.clear();n.admission=Admission::Recorded(None)},&["network-not-release-pinned","launch-not-approved","admission-unrecorded","admission-not-booted"]),
            // A booted admission that differs from the pin in any one field.
            (|_,n|booted(n).scope="experimental-devnet".into(),&["admission-mismatch"]),
            (|_,n|booted(n).conditions.pop().map_or((),|_|()),&["admission-mismatch"]),
            (|_,n|booted(n).waivers.reverse(),&["admission-mismatch"]),
            (|_,n|booted(n).profile_kind=Some(ProfileKind::Simnet),&["admission-mismatch"]),
            (|_,n|booted(n).node_kind=None,&["admission-mismatch"]),
        ];
        for (edit,want) in cases{
            let (mut b,mut n)=ready();edit(&mut b,&mut n);
            let r=readiness(&b,Subject::Network(&n));invariants(&r,true);
            assert_eq!(ids(&r.blockers),*want,"{r:?}");
            assert!(!r.approved&&!r.ready());
            let j=json(&r);assert_eq!(j["launchBlockers"][0]["id"],want[0]);
        }
        // Open plan gates are pending technical items: they do not withhold approval (OWNER-DECISIONS-2026-11-20 D6 C).
        let (mut b,n)=ready();b.plan_gates_open=vec![("Q1","release review")];
        let r=readiness(&b,Subject::Network(&n));invariants(&r,true);
        assert!(r.ready()&&r.pending[0].id=="plan-gate-Q1"&&!r.technically_complete());
        // LAUNCH is the other approvable admission; a specification admission with nothing pending is complete.
        let (mut b,mut n)=ready();pinned(&mut b).admission=ExpectedAdmission::LAUNCH;n.admission=Admission::Booted(cleared());
        let r=readiness(&b,Subject::Network(&n));invariants(&r,true);assert!(r.ready()&&r.technically_complete(),"{r:?}");
        let (mut b,n)=ready();b.test_features=vec!["caps-test","io-fault"];
        assert!(blockers(&b,Subject::Network(&n))[0].detail.ends_with("caps-test, io-fault"));
    }
    #[test]
    fn the_details_name_the_pin_the_approval_and_the_differing_fields(){
        let (mut b,mut n)=ready();booted(&mut n).waivers.reverse();booted(&mut n).node_kind=Some(NodeKind::Simnet);
        let r=readiness(&b,Subject::Network(&n));assert_eq!(ids(&r.blockers),["admission-mismatch"]);
        assert!(r.blockers[0].detail.contains("expected admission PUBLIC_MAINNET")&&r.blockers[0].detail.ends_with("in waivers, node kind"),"{}",r.blockers[0].detail);
        pinned(&mut b).approval=None;
        let r=readiness(&b,Subject::Network(&n));
        assert!(r.blockers[0].detail.contains("without a launch approval reference"),"{}",r.blockers[0].detail);
        let (mut b,n)=ready();pinned(&mut b).admission=ExpectedAdmission::FRESH_NETWORK;
        let r=readiness(&b,Subject::Network(&n));
        assert!(r.blockers[0].detail.contains("expected admission FRESH_NETWORK, which is neither PUBLIC_MAINNET nor LAUNCH"),"{}",r.blockers[0].detail);
        let (mut b,mut n)=ready();n.admission=forged();
        let r=readiness(&b,Subject::Network(&n));
        assert_eq!(r.blockers[0].detail,"the launcher boots nothing; approval and readiness are computed only by a node booted on this network (pocol_status). This release pins PUBLIC_MAINNET for it, approval reference OWNER-DECISIONS-2026-11-20 D1");
        b.network_profiles.clear();
        let r=readiness(&b,Subject::Network(&n));
        assert!(r.blockers.iter().any(|x|x.detail.ends_with("This release pins no expected admission for it, approval reference none")),"{r:?}");
        assert!(r.blockers[0].detail.contains("it pins none yet"));
    }
    /// The launcher boots nothing: a pinned network is never approved there, its approval reference is reported, and its
    /// technical readiness is the pinned expected admission's, which is what a node booted with it reports. Without a pin
    /// the technical readiness reflects devnet.json's record.
    #[test]
    fn the_launcher_reports_the_pin_but_never_approves(){
        let (b,booted_n)=ready();let node=readiness(&b,Subject::Network(&booted_n));
        for recorded in [forged(),experimental(),Admission::Recorded(None)]{
            let n=Network{admission:recorded.clone(),..booted_n.clone()};
            let r=readiness(&b,Subject::Network(&n));invariants(&r,true);
            assert_eq!(ids(&r.blockers),["admission-not-booted"],"{recorded:?}");
            assert!(!r.approved&&!r.ready());
            assert_eq!((r.approval_reference,&r.pending),(node.approval_reference,&node.pending),"{recorded:?}");
            assert_eq!(json(&r)["technicalReadiness"],json(&node)["technicalReadiness"]);
            assert_eq!((r.network_name,r.network_environment,r.network_scope.as_deref()),(Some("PoCol Mainnet"),Some("mainnet"),None),"nothing was booted");
        }
        let unpinned=Build{network_profiles:vec![],..b.clone()};
        let n=Network{admission:experimental(),..booted_n.clone()};
        let r=readiness(&unpinned,Subject::Network(&n));
        assert_eq!(ids(&r.pending),["admission-scope","admission-condition-calibration-synthetic","admission-waiver-R26","profile-kind"]);
        assert_eq!((r.approval_reference,r.network_name),(None,None));
        let n=Network{admission:Admission::Recorded(None),..booted_n};
        let r=readiness(&unpinned,Subject::Network(&n));assert_eq!(ids(&r.pending),["admission-unrecorded"],"nothing missing reads as complete");
    }
    #[test]
    fn expected_admissions_compare_exactly_with_admission_facts(){
        for a in [ExpectedAdmission::PUBLIC_MAINNET,ExpectedAdmission::LAUNCH,ExpectedAdmission::FRESH_NETWORK]{
            assert!(a.matches(&a.facts(PUBLIC))&&a.matches(&a.facts(1656550731)),"{a:?}");
        }
        assert_eq!(ExpectedAdmission::LAUNCH.facts(777001),cleared());
        assert!(ExpectedAdmission::PUBLIC_MAINNET.facts(PUBLIC).network_profile,"R22: a public network chain");
        assert!(!ExpectedAdmission::PUBLIC_MAINNET.facts(1656550731).network_profile&&!ExpectedAdmission::LAUNCH.facts(777990).network_profile,"R22");
        assert_eq!(ExpectedAdmission::PUBLIC_MAINNET.mismatches(&ExpectedAdmission::FRESH_NETWORK.facts(PUBLIC)),["scope","conditions","waivers","profile kind","node kind"]);
        let mut f=ExpectedAdmission::PUBLIC_MAINNET.facts(PUBLIC);f.conditions.swap(0,1);
        assert_eq!(ExpectedAdmission::PUBLIC_MAINNET.mismatches(&f),["conditions"],"ordered lists");
        f=ExpectedAdmission::PUBLIC_MAINNET.facts(PUBLIC);f.profile_kind=None;f.node_kind=None;
        assert_eq!(ExpectedAdmission::PUBLIC_MAINNET.mismatches(&f),["profile kind","node kind"],"an unknown kind differs");
        let mixed=ExpectedAdmission{scope:Scope::ExperimentalDevnet,node_kind:NodeKind::Simnet,..ExpectedAdmission::LAUNCH};
        assert_eq!(mixed.differing(&ExpectedAdmission::LAUNCH),["scope","node kind"]);
        assert!(ExpectedAdmission::LAUNCH.differing(&ExpectedAdmission::LAUNCH).is_empty());
        assert_eq!([ExpectedAdmission::PUBLIC_MAINNET.name(),ExpectedAdmission::LAUNCH.name(),ExpectedAdmission::FRESH_NETWORK.name(),mixed.name()],
            ["PUBLIC_MAINNET","LAUNCH","FRESH_NETWORK","an expected admission of its own"]);
        assert!(ExpectedAdmission::PUBLIC_MAINNET.approvable()&&ExpectedAdmission::LAUNCH.approvable()&&!ExpectedAdmission::FRESH_NETWORK.approvable()&&!mixed.approvable());
        assert_eq!((network_environment(PUBLIC),network_environment(crate::testnet::TEST_CHAIN_ID),network_environment(1656550731),network_environment(777941)),
            ("mainnet","test","devnet","devnet"));
    }
    /// NETWORK_PROFILES holds what set_network_identity.py's `--check` requires of the tree's identity (network_display.rs):
    /// nothing while that is a retired or the rehearsal network (today); on a public network chain exactly its entry
    /// expecting `PUBLIC_MAINNET` with the approval reference; on any other chain its entry expecting `FRESH_NETWORK`
    /// without one. No entry expects `LAUNCH`, and every network keeps `admission-not-booted` in the launcher.
    #[test]
    fn the_release_pins_only_the_tree_network(){
        let retired:serde_json::Value=serde_json::from_str(include_str!("../../../deploy/mainnet/retired-networks.json")).unwrap();
        let rehearsal:serde_json::Value=serde_json::from_str(include_str!("../../../deploy/mainnet/v3-rehearsal-profile.json")).unwrap();
        let (chain,genesis)=(crate::network_display::MAINNET_CHAIN_ID,crate::network_display::MAINNET_GENESIS.to_ascii_lowercase());
        let is_retired=retired["retired"].as_array().unwrap().iter()
            .any(|e|e["chainId"].as_u64()==Some(chain)&&e["genesisHash"].as_str()==Some(format!("0x{genesis}").as_str()));
        let want=if is_retired||rehearsal["chainId"].as_u64()==Some(chain){vec![]}
            else if PUBLIC_NETWORK_CHAINS.contains(&chain){vec![NetworkProfile{chain_id:chain,genesis_hash:genesis_hash(&genesis),admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some(PUBLIC_MAINNET_APPROVAL)}]}
            else{vec![NetworkProfile{chain_id:chain,genesis_hash:genesis_hash(&genesis),admission:ExpectedAdmission::FRESH_NETWORK,approval:None}]};
        assert_eq!(NETWORK_PROFILES,want.as_slice());assert_eq!(Build::this().network_profiles,want);
        assert!(NETWORK_PROFILES.iter().all(|p|p.admission!=ExpectedAdmission::LAUNCH));
        assert!(NETWORK_PROFILES.iter().all(|p|p.approval.is_some()==PUBLIC_NETWORK_CHAINS.contains(&p.chain_id)),"an approval reference only on a public network chain");
        // An unpinned network, and each pinned one, in the launcher with a devnet.json recording a cleared admission.
        let n=Network{chain_id:777001,genesis_hash:[9;32],v3_active:true,admission:forged()};
        let got=blockers(&Build::this(),Subject::Network(&n));let got=ids(&got);
        assert!(got.contains(&"admission-not-booted")&&got.contains(&"network-not-release-pinned")&&got.contains(&"launch-not-approved"),"{got:?}");
        for p in NETWORK_PROFILES{
            let n=Network{chain_id:p.chain_id,genesis_hash:p.genesis_hash,v3_active:true,admission:forged()};
            let r=readiness(&Build::this(),Subject::Network(&n));let got=ids(&r.blockers);
            assert!(got.contains(&"admission-not-booted")&&!got.contains(&"network-not-release-pinned")&&!r.approved,"{got:?}");
        }
    }
    #[test]
    fn recorded_admission_never_reads_as_cleared_from_missing_or_malformed_fields(){
        assert_eq!(AdmissionFacts::recorded(&serde_json::json!({}),777001),None);
        assert_eq!(AdmissionFacts::recorded(&serde_json::json!({"admission":7}),777001),None);
        let a=AdmissionFacts::recorded(&serde_json::json!({"profileKind":"simnet","admission":{"scope":"experimental-devnet",
            "conditions":[{"id":"calibration-synthetic"},{"id":5}],"waivers":[{"rule":"R26"}]}}),777941).unwrap();
        assert_eq!(a,AdmissionFacts{scope:"experimental-devnet".into(),conditions:vec!["calibration-synthetic".into(),"unreadable".into()],waivers:vec!["R26".into()],
            network_profile:false,profile_kind:Some(ProfileKind::Simnet),node_kind:None});
        let a=AdmissionFacts::recorded(&serde_json::json!({"profileKind":"Network","admission":{}}),777001).unwrap();
        assert_eq!(a,AdmissionFacts{scope:"unrecorded".into(),conditions:vec!["unreadable".into()],waivers:vec!["unreadable".into()],network_profile:true,
            profile_kind:Some(ProfileKind::Network),node_kind:None});
        for kind in [serde_json::json!("networks"),serde_json::json!(7),serde_json::Value::Null]{
            let a=AdmissionFacts::recorded(&serde_json::json!({"profileKind":kind,"admission":{}}),PUBLIC).unwrap();
            assert_eq!((a.profile_kind,a.network_profile),(None,false),"{kind}");
        }
        // R22: a recorded network profile on a public network chain is one, on any other chain it is not (the boot
        // refuses it).
        assert!(AdmissionFacts::recorded(&serde_json::json!({"profileKind":"network","admission":{}}),PUBLIC).unwrap().network_profile);
        for chain in [123456789,777002,0,1656550731,777990]{
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
        // The gate's ids that name a rule keep it (they read as the boot reports them); no other capital is kept.
        let ruled=AdmissionFacts::recorded(&serde_json::json!({"admission":{"scope":"experimental-devnet","conditions":[{"id":"gate-deferred-general-R18"},
            {"id":"gate-deferred-general-R21"},{"id":"gate-R"},{"id":"gate-R1234"},{"id":"Gate-r18"},{"id":"gate--x"},{"id":"-gate"}],"waivers":[]}}),777941).unwrap();
        assert_eq!(ruled.conditions,["gate-deferred-general-R18","gate-deferred-general-R21","unreadable"]);
        assert_eq!(a.waivers,["R26","unreadable","R7"]);
        // 60,000 distinct ids reflect as RECORDED_MAX of them and one overflow entry, for each list.
        let many:Vec<serde_json::Value>=(0..60_000).map(|i|serde_json::json!({"id":format!("c{i}"),"rule":format!("R{}",i%1000)})).collect();
        let a=AdmissionFacts::recorded(&serde_json::json!({"admission":{"scope":"experimental-devnet","conditions":many,"waivers":many}}),777941).unwrap();
        assert_eq!(a.conditions.len(),RECORDED_MAX+1);assert_eq!(a.conditions.last().unwrap(),"overflow");assert_eq!(a.conditions[..3],["c0","c1","c2"]);
        assert_eq!(a.waivers.len(),RECORDED_MAX+1);assert_eq!(a.waivers.last().unwrap(),"overflow");
        let n=Network{chain_id:777941,genesis_hash:[1;32],v3_active:false,admission:Admission::Recorded(Some(a))};
        let r=readiness(&Build::this(),Subject::Network(&n));
        assert_eq!(r.pending.iter().filter(|b|b.id.starts_with("admission-condition-")).count(),RECORDED_MAX+1);
        assert!(r.pending.iter().any(|b|b.id=="admission-condition-overflow")&&r.pending.iter().any(|b|b.id=="admission-waiver-overflow"));
        assert!(r.pending.len()<2*RECORDED_MAX+PLAN_GATES_OPEN.len()+8,"{}",r.pending.len());assert!(r.blockers.len()<16,"{}",r.blockers.len());
    }
    /// `AdmissionFacts::of` and `Network::admitted` map a real admission: its scope, every condition id and waiver
    /// rule of its report, the profile and node kinds, and whether the genesis activates v3.
    #[test]
    fn a_booted_admission_maps_its_report_into_technical_readiness(){
        use pocol_lp4_execution::devnet::{self,DevnetConfig};
        let mut c=DevnetConfig::new(777942,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
        let (a,_)=devnet::build(&c).unwrap().admit(&crate::single_host_local()).unwrap();
        let facts=AdmissionFacts::of(&a);
        assert_eq!(facts.scope,"experimental-devnet");assert!(!facts.network_profile,"a simnet profile on a simnet node");
        assert_eq!((facts.profile_kind,facts.node_kind),(Some(ProfileKind::Simnet),Some(NodeKind::Simnet)));
        let r=a.report();
        assert_eq!(facts.conditions,r.conditions.iter().map(|c|c.id.to_string()).collect::<Vec<_>>());
        assert_eq!(facts.waivers,r.waivers.iter().map(|w|w.rule.to_string()).collect::<Vec<_>>());
        assert!(facts.conditions.contains(&"calibration-synthetic".to_string()),"{facts:?}");
        let n=Network::admitted(&a);
        assert_eq!((n.chain_id,n.genesis_hash),(777942,a.genesis_hash()));assert_eq!(n.admission,Admission::Booted(facts.clone()));
        let got=of_admitted(&a);invariants(&got,true);
        // The v3 devnet activates v3 from genesis (no v3-inactive); it is pinned by no release.
        let network:Vec<&str>=ids(&got.blockers).into_iter().skip_while(|id|*id!="v3-inactive"&&*id!="network-not-release-pinned").collect();
        assert_eq!(network,["network-not-release-pinned","launch-not-approved"],"the network blockers of a booted devnet admission");
        let pending:Vec<String>=got.pending.iter().map(|b|b.id.clone()).skip(PLAN_GATES_OPEN.len()).collect();
        assert_eq!(pending,admission_pending(&facts));
        assert_eq!((got.network_name,got.network_environment,got.network_scope.as_deref()),(None,Some("devnet"),Some("experimental-devnet")));
        assert!(n.v3_active);
        // `pocol start`'s line is these computed values, and the devnet's PCOL is a test currency.
        assert_eq!(start_line(&a),format!("PoCol node booted: network, chain 777942, environment devnet, admission scope experimental-devnet (specification compliant: false); publicLaunchApproved false, publicLaunchReady false, technical readiness complete false ({} pending); currency PCOL (test currency). pocol_status lists every launch blocker and pending item. Admission, memory and lifetime limits apply.",got.pending.len()));
    }
    /// The wallet's `testCurrency` is false exactly on the public network chains (the mainnet's, whatever its pin), and
    /// true on the rehearsal chain, the local test chain, 777001 (an R22 network chain that is not a public one), every
    /// retired network and a devnet.
    #[test]
    fn the_currency_is_a_test_currency_off_the_public_network_chains(){
        assert_eq!(PUBLIC_NETWORK_CHAINS,[2656089850]);
        for &chain in PUBLIC_NETWORK_CHAINS{assert!(!test_currency(chain),"{chain}")}
        let retired=include_str!("../../../deploy/mainnet/retired-networks.json");
        let retired:serde_json::Value=serde_json::from_str(retired).unwrap();
        let mut others=vec![1656550731,crate::testnet::TEST_CHAIN_ID,777001,777942,1];
        others.extend(retired["retired"].as_array().unwrap().iter().map(|e|e["chainId"].as_u64().unwrap()));
        for chain in others{assert!(test_currency(chain),"{chain}")}
    }
    /// The entry forms set_network_identity.py writes into NETWORK_PROFILES compile and decode to the hash they name: on
    /// a public network chain the public network's admission with the approval reference, on any other chain the
    /// experimental admission without one (its test reads WRITTEN's line and checks that the tool writes exactly these
    /// forms).
    const WRITTEN:&[NetworkProfile]=&[NetworkProfile{chain_id:2656089850,genesis_hash:genesis_hash("0909090909090909090909090909090909090909090909090909090909090909"),admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")},NetworkProfile{chain_id:777002,genesis_hash:genesis_hash("00112233445566778899aabbccddeeff0123456789abcdef0fedcba987654321"),admission:ExpectedAdmission::FRESH_NETWORK,approval:None}];
    #[test]
    fn network_profile_entries_decode_their_genesis_hash(){
        assert_eq!(WRITTEN[0],ready().0.network_profiles[0]);
        assert_eq!(crate::types::hex(&WRITTEN[1].genesis_hash),"0x00112233445566778899aabbccddeeff0123456789abcdef0fedcba987654321");
        assert_eq!(WRITTEN[0].approval,Some(PUBLIC_MAINNET_APPROVAL));
        assert!(WRITTEN.iter().all(|p|p.admission!=ExpectedAdmission::LAUNCH&&(p.approval.is_some()==PUBLIC_NETWORK_CHAINS.contains(&p.chain_id))));
        let (mut b,n)=ready();b.network_profiles=WRITTEN.to_vec();
        let r=readiness(&b,Subject::Network(&n));assert!(r.ready(),"the public entry approves its network once booted with its admission: {r:?}");
        let mut other=n.clone();other.genesis_hash=WRITTEN[1].genesis_hash;
        assert_eq!(ids(&blockers(&b,Subject::Network(&other))),["network-not-release-pinned","launch-not-approved"],"the pair, not the chain ID alone");
        // The other entry pins its network and approves nothing, booted or in the launcher.
        let fresh=Network{chain_id:WRITTEN[1].chain_id,genesis_hash:WRITTEN[1].genesis_hash,v3_active:true,admission:Admission::Booted(ExpectedAdmission::FRESH_NETWORK.facts(777002))};
        let r=readiness(&b,Subject::Network(&fresh));assert_eq!(ids(&r.blockers),["launch-not-approved"]);
        assert_eq!(ids(&r.pending),admission_pending(&ExpectedAdmission::FRESH_NETWORK.facts(777002)));
        let r=readiness(&b,Subject::Network(&Network{admission:experimental(),..fresh}));assert_eq!(ids(&r.blockers),["launch-not-approved","admission-not-booted"]);
        assert!(r.blockers[1].detail.ends_with("This release pins FRESH_NETWORK for it, approval reference none"),"{}",r.blockers[1].detail);
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
        assert_eq!((info["publicLaunchApproved"].clone(),info["publicLaunchReady"].clone()),(false.into(),false.into()));
        let want=json(&readiness(&Build::this(),Subject::Build));
        for k in ["launchApprovalReference","launchBlockers","technicalReadiness","networkName","networkEnvironment","networkScope"]{assert_eq!(info[k],want[k],"{k}")}
        let raw:serde_json::Value=serde_json::from_str(crate::identity::BUILD_INFO).unwrap();
        for (k,v) in raw.as_object().unwrap(){assert_eq!(&info[k],v,"{k}")}
    }
}
