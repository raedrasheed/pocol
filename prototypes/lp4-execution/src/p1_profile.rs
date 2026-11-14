//! LP3 S2 schema/identity view. Never a boot authorization or complete ParamGate.
//! Sources: consensus.md:7-34; governance.md R22/R24 and network chainId 777001.
use crate::fixed::U256;
use crate::genesis_spec::{self, GenesisSpec, GsError, Upper, CP_LEN, CP_V2_LEN};
use crate::hashes::H256;

/// Named parameters with validated widths. V1 has 52 wire values; appended v2 fields are zero on v1.
#[allow(non_snake_case)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsensusParams {
    pub g_ts: u64,
    pub target_g: U256,
    pub T_blk: u32,
    pub tau: u32,
    pub nonceMode: u8,
    pub c: u8,
    pub D_att: u32,
    pub dFbWait: u32,
    pub m: u16,
    pub kappa: u8,
    pub S_max: u16,
    pub D: u16,
    pub W_w: u16,
    pub alpha_bp: u16,
    pub gamma_bp: u16,
    pub subsidy: U256,
    pub B_reg: U256,
    pub R_max: u8,
    pub M_max: u16,
    pub M_min: u16,
    pub REG_RECORDS_MAX: u32,
    pub SWEEP_MAX: u16,
    pub SWEEP_SCAN: u16,
    pub REG_OPS: u16,
    pub W_act: u32,
    pub Theta_inact: u32,
    pub E_win: u32,
    pub U: u32,
    pub E_max: u8,
    pub EVID_BYTES_MAX: u32,
    pub GAS_LIMIT: u64,
    pub baseFee0: U256,
    pub TX_MAX: u32,
    pub BODY_MAX: u32,
    pub B_code_max: u32,
    pub CODE_CAP: u64,
    pub TXBYTES_CAP: u64,
    pub TX_CAP: u64,
    pub SLOT_CAP: u64,
    pub ACCT_CAP: u64,
    pub SYS_SLOT_MAX: u32,
    pub SYS_KEYS_BLOCK_MAX: u32,
    pub SYS_KEYS_USER_MAX: u32,
    pub SYS_ACCT_BLOCK_MAX: u32,
    pub G_SYS: u64,
    pub H_END: u64,
    pub T_END: u64,
    pub H_CLOSE: u64,
    pub T_CLOSE: u64,
    pub Z_close: u32,
    pub RET: u32,
    pub P2: u32,
    pub H_half: u64,
    pub S_tail: U256,
    pub P_MAX: u16,
    pub T_PROB: u32,
    pub F0_acct: U256,
    pub F0_slot: U256,
    pub F0_code: U256,
    pub gamma_tip_bp: u16,
    pub baseFeeMin: U256,
    pub R_react: u8,
    pub C_react: u32,
    pub E_epoch: u32,
    pub theta_bp: u16,
    pub k_sigma: u16,
    pub rho_bp: u16,
    schema_version: u8,
}

impl ConsensusParams {
    /// Checks widths and schema bounds before narrowing; cross-field ParamGate rules are deferred.
    pub fn from_values(v: &[U256]) -> Result<Self, GsError> {
        let schema_version = match v.len() {
            CP_LEN => 1,
            CP_V2_LEN => 2,
            n => return Err(GsError { code: "gsCount", detail: format!("CP {n}") }),
        };
        let schema = genesis_spec::cp_schema(schema_version).unwrap();
        for (i, field) in schema.iter().enumerate() {
            let high = match field.hi {
                Upper::None => None,
                Upper::Value(n) => Some(n),
                Upper::Gamma => Some(10000u64.saturating_sub(v[13].to_u64().unwrap_or(u64::MAX))),
            };
            if v[i].bits() > field.width || v[i] < U256::from_u64(field.lo) || high.map_or(false, |n| v[i] > U256::from_u64(n)) {
                return Err(GsError { code: "gsRange", detail: field.name.into() });
            }
        }
        Ok(Self {
            g_ts: v[0].to_u64().unwrap() as u64,
            target_g: v[1],
            T_blk: v[2].to_u64().unwrap() as u32,
            tau: v[3].to_u64().unwrap() as u32,
            nonceMode: v[4].to_u64().unwrap() as u8,
            c: v[5].to_u64().unwrap() as u8,
            D_att: v[6].to_u64().unwrap() as u32,
            dFbWait: v[7].to_u64().unwrap() as u32,
            m: v[8].to_u64().unwrap() as u16,
            kappa: v[9].to_u64().unwrap() as u8,
            S_max: v[10].to_u64().unwrap() as u16,
            D: v[11].to_u64().unwrap() as u16,
            W_w: v[12].to_u64().unwrap() as u16,
            alpha_bp: v[13].to_u64().unwrap() as u16,
            gamma_bp: v[14].to_u64().unwrap() as u16,
            subsidy: v[15],
            B_reg: v[16],
            R_max: v[17].to_u64().unwrap() as u8,
            M_max: v[18].to_u64().unwrap() as u16,
            M_min: v[19].to_u64().unwrap() as u16,
            REG_RECORDS_MAX: v[20].to_u64().unwrap() as u32,
            SWEEP_MAX: v[21].to_u64().unwrap() as u16,
            SWEEP_SCAN: v[22].to_u64().unwrap() as u16,
            REG_OPS: v[23].to_u64().unwrap() as u16,
            W_act: v[24].to_u64().unwrap() as u32,
            Theta_inact: v[25].to_u64().unwrap() as u32,
            E_win: v[26].to_u64().unwrap() as u32,
            U: v[27].to_u64().unwrap() as u32,
            E_max: v[28].to_u64().unwrap() as u8,
            EVID_BYTES_MAX: v[29].to_u64().unwrap() as u32,
            GAS_LIMIT: v[30].to_u64().unwrap() as u64,
            baseFee0: v[31],
            TX_MAX: v[32].to_u64().unwrap() as u32,
            BODY_MAX: v[33].to_u64().unwrap() as u32,
            B_code_max: v[34].to_u64().unwrap() as u32,
            CODE_CAP: v[35].to_u64().unwrap() as u64,
            TXBYTES_CAP: v[36].to_u64().unwrap() as u64,
            TX_CAP: v[37].to_u64().unwrap() as u64,
            SLOT_CAP: v[38].to_u64().unwrap() as u64,
            ACCT_CAP: v[39].to_u64().unwrap() as u64,
            SYS_SLOT_MAX: v[40].to_u64().unwrap() as u32,
            SYS_KEYS_BLOCK_MAX: v[41].to_u64().unwrap() as u32,
            SYS_KEYS_USER_MAX: v[42].to_u64().unwrap() as u32,
            SYS_ACCT_BLOCK_MAX: v[43].to_u64().unwrap() as u32,
            G_SYS: v[44].to_u64().unwrap() as u64,
            H_END: v[45].to_u64().unwrap() as u64,
            T_END: v[46].to_u64().unwrap() as u64,
            H_CLOSE: v[47].to_u64().unwrap() as u64,
            T_CLOSE: v[48].to_u64().unwrap() as u64,
            Z_close: v[49].to_u64().unwrap() as u32,
            RET: v[50].to_u64().unwrap() as u32,
            P2: v[51].to_u64().unwrap() as u32,
            H_half: v.get(52).and_then(U256::to_u64).unwrap_or(0) as u64,
            S_tail: v.get(53).copied().unwrap_or(U256::ZERO),
            P_MAX: v.get(54).and_then(U256::to_u64).unwrap_or(0) as u16,
            T_PROB: v.get(55).and_then(U256::to_u64).unwrap_or(0) as u32,
            F0_acct: v.get(56).copied().unwrap_or(U256::ZERO),
            F0_slot: v.get(57).copied().unwrap_or(U256::ZERO),
            F0_code: v.get(58).copied().unwrap_or(U256::ZERO),
            gamma_tip_bp: v.get(59).and_then(U256::to_u64).unwrap_or(0) as u16,
            baseFeeMin: v.get(60).copied().unwrap_or(U256::ZERO),
            R_react: v.get(61).and_then(U256::to_u64).unwrap_or(0) as u8,
            C_react: v.get(62).and_then(U256::to_u64).unwrap_or(0) as u32,
            E_epoch: v.get(63).and_then(U256::to_u64).unwrap_or(0) as u32,
            theta_bp: v.get(64).and_then(U256::to_u64).unwrap_or(0) as u16,
            k_sigma: v.get(65).and_then(U256::to_u64).unwrap_or(0) as u16,
            rho_bp: v.get(66).and_then(U256::to_u64).unwrap_or(0) as u16,
            schema_version,
        })
    }

    pub fn schema_version(&self) -> u8 { self.schema_version }
    pub fn cp_len(&self) -> usize { if self.schema_version == 1 { CP_LEN } else { CP_V2_LEN } }
    pub fn to_values(&self) -> Vec<U256> {
        let mut values = vec![
            U256::from_u64(self.g_ts as u64),
            self.target_g,
            U256::from_u64(self.T_blk as u64),
            U256::from_u64(self.tau as u64),
            U256::from_u64(self.nonceMode as u64),
            U256::from_u64(self.c as u64),
            U256::from_u64(self.D_att as u64),
            U256::from_u64(self.dFbWait as u64),
            U256::from_u64(self.m as u64),
            U256::from_u64(self.kappa as u64),
            U256::from_u64(self.S_max as u64),
            U256::from_u64(self.D as u64),
            U256::from_u64(self.W_w as u64),
            U256::from_u64(self.alpha_bp as u64),
            U256::from_u64(self.gamma_bp as u64),
            self.subsidy,
            self.B_reg,
            U256::from_u64(self.R_max as u64),
            U256::from_u64(self.M_max as u64),
            U256::from_u64(self.M_min as u64),
            U256::from_u64(self.REG_RECORDS_MAX as u64),
            U256::from_u64(self.SWEEP_MAX as u64),
            U256::from_u64(self.SWEEP_SCAN as u64),
            U256::from_u64(self.REG_OPS as u64),
            U256::from_u64(self.W_act as u64),
            U256::from_u64(self.Theta_inact as u64),
            U256::from_u64(self.E_win as u64),
            U256::from_u64(self.U as u64),
            U256::from_u64(self.E_max as u64),
            U256::from_u64(self.EVID_BYTES_MAX as u64),
            U256::from_u64(self.GAS_LIMIT as u64),
            self.baseFee0,
            U256::from_u64(self.TX_MAX as u64),
            U256::from_u64(self.BODY_MAX as u64),
            U256::from_u64(self.B_code_max as u64),
            U256::from_u64(self.CODE_CAP as u64),
            U256::from_u64(self.TXBYTES_CAP as u64),
            U256::from_u64(self.TX_CAP as u64),
            U256::from_u64(self.SLOT_CAP as u64),
            U256::from_u64(self.ACCT_CAP as u64),
            U256::from_u64(self.SYS_SLOT_MAX as u64),
            U256::from_u64(self.SYS_KEYS_BLOCK_MAX as u64),
            U256::from_u64(self.SYS_KEYS_USER_MAX as u64),
            U256::from_u64(self.SYS_ACCT_BLOCK_MAX as u64),
            U256::from_u64(self.G_SYS as u64),
            U256::from_u64(self.H_END as u64),
            U256::from_u64(self.T_END as u64),
            U256::from_u64(self.H_CLOSE as u64),
            U256::from_u64(self.T_CLOSE as u64),
            U256::from_u64(self.Z_close as u64),
            U256::from_u64(self.RET as u64),
            U256::from_u64(self.P2 as u64),
        ];
        if self.schema_version == 2 {
            values.extend([
                U256::from_u64(self.H_half as u64),
                self.S_tail,
                U256::from_u64(self.P_MAX as u64),
                U256::from_u64(self.T_PROB as u64),
                self.F0_acct,
                self.F0_slot,
                self.F0_code,
                U256::from_u64(self.gamma_tip_bp as u64),
                self.baseFeeMin,
                U256::from_u64(self.R_react as u64),
                U256::from_u64(self.C_react as u64),
                U256::from_u64(self.E_epoch as u64),
                U256::from_u64(self.theta_bp as u64),
                U256::from_u64(self.k_sigma as u64),
                U256::from_u64(self.rho_bp as u64),
            ]);
        }
        values
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileKind {
    Network,
    Simnet,
    Synthetic,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Node,
    Simnet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProfileError {
    Genesis(GsError),
    InputTooLarge,
    ChainId,
    R22,
    ForkStart,
    ForkHeight,
    ForkVersionOrder,
    UnsupportedVersion(u16),
}

/// The highest fork version this build implements. Raised only by the PR that adds that version's rules
/// (V3-DESIGN.md §3.4); everything else about a schedule is checked generically below.
pub const MAX_FORK_VERSION: u16 = 2;

/// Optional v2 activation enables the full nonce domain; v1 remains the default.
/// Private entries ensure active_version is called only on a validated schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkSchedule(Vec<(u16, u64)>);
impl ForkSchedule {
    pub fn new(entries: &[(u16, u64)]) -> Result<Self, ProfileError> {
        Self::new_bounded(entries, MAX_FORK_VERSION)
    }
    /// `new` with an explicit highest version, so tests can exercise schedules beyond this build's rules.
    /// Errors keep their historical precedence per entry: height, then version, then order. Version 1 may
    /// repeat (a no-op entry); any later version may appear once, and versions never decrease.
    #[doc(hidden)]
    pub fn new_bounded(entries: &[(u16, u64)], max: u16) -> Result<Self, ProfileError> {
        if entries.first() != Some(&(1, 0)) {
            return Err(ProfileError::ForkStart);
        }
        let mut previous: Option<(u16, u64)> = None;
        for &(version, height) in entries {
            if previous.map_or(false, |(_, h)| height <= h) {
                return Err(ProfileError::ForkHeight);
            }
            if version == 0 || version > max {
                return Err(ProfileError::UnsupportedVersion(version));
            }
            if previous.map_or(false, |(p, _)| version < p || (version == p && version >= 2)) {
                return Err(ProfileError::ForkVersionOrder);
            }
            previous = Some((version, height));
        }
        Ok(Self(entries.to_vec()))
    }
    pub fn entries(&self) -> &[(u16, u64)] {
        &self.0
    }
    pub fn active_version(&self, height: u64) -> u16 {
        // First activation is guaranteed at zero; exact activation heights select the new entry.
        self.0.iter().rev().find(|(_, h)| *h <= height).unwrap().0
    }
}

/// Schema-checked identity only. No constructor can produce a bootable profile in LP3.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileIdentity {
    spec: GenesisSpec,
    cp: ConsensusParams,
    hash: H256,
    forks: ForkSchedule,
    kind: ProfileKind,
    node_kind: NodeKind,
}
impl ProfileIdentity {
    /// Bounded bytes, full decode/hash binding, advertised chainId, R22, then R24 schedule.
    /// AllocRoot/sysCodeHash are identities, not verified builder outputs. R1–R26 is NOT complete.
    pub fn validate(
        bytes: &[u8],
        expected_hash: &H256,
        chain_id: u64,
        kind: ProfileKind,
        node_kind: NodeKind,
        forks: &[(u16, u64)],
    ) -> Result<Self, ProfileError> {
        if bytes.len() > genesis_spec::MAX_SPEC_BYTES {
            return Err(ProfileError::InputTooLarge);
        }
        let spec = genesis_spec::verify_hash(bytes, expected_hash).map_err(ProfileError::Genesis)?;
        if chain_id != spec.chain_id {
            return Err(ProfileError::ChainId);
        }
        if (node_kind == NodeKind::Node && kind != ProfileKind::Network) || ((kind == ProfileKind::Network) != (chain_id == 777001)) {
            return Err(ProfileError::R22);
        }
        let forks = ForkSchedule::new(forks)?;
        let cp = ConsensusParams::from_values(&spec.cp).map_err(ProfileError::Genesis)?;
        Ok(Self { spec, cp, hash: *expected_hash, forks, kind, node_kind })
    }
    pub fn spec_version(&self) -> u8 { self.spec.spec_version }
    /// The sole host-rule predicate: the genesis route and the future scheduled-fork route.
    pub fn v3_active(&self, height: u64) -> bool {
        self.spec_version() >= 2 || self.forks.active_version(height) >= 3
    }
    pub fn spec(&self) -> &GenesisSpec {
        &self.spec
    }
    pub fn cp(&self) -> &ConsensusParams {
        &self.cp
    }
    pub fn genesis_hash(&self) -> H256 {
        self.hash
    }
    pub fn forks(&self) -> &ForkSchedule {
        &self.forks
    }
    pub fn kind(&self) -> ProfileKind {
        self.kind
    }
    pub fn node_kind(&self) -> NodeKind {
        self.node_kind
    }
    pub fn bootable(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod v3_tests {
    use super::*;
    fn identity(version:u8,nonce_mode:u8,forks:&[(u16,u64)],max_fork:u16)->ProfileIdentity{
        let mut values:Vec<U256>=genesis_spec::cp_schema(version).unwrap().iter().map(|f|U256::from_u64(f.lo)).collect();
        values[4]=U256::from_u64(nonce_mode.into());
        let raw=genesis_spec::encode(&GenesisSpec{spec_version:version,chain_id:7,cp:values,alloc_root:[0;32],
            m0_list:vec![genesis_spec::Member{id:[1;20],reward_addr:[2;20]}],sys_code_hash:[0;32]});
        // Schema-only test identity. This never constructs an Admitted or bootable profile.
        let spec=genesis_spec::decode_bounded(&raw,2).unwrap();
        ProfileIdentity{cp:ConsensusParams::from_values(&spec.cp).unwrap(),spec,hash:genesis_spec::genesis_hash(&raw),
            forks:ForkSchedule::new_bounded(forks,max_fork).unwrap(),kind:ProfileKind::Simnet,node_kind:NodeKind::Simnet}
    }
    #[test]
    fn v3_predicate_keeps_v1_and_earnin_dormant_at_every_height_boundary(){
        for nonce_mode in [0,1]{
            let p=identity(1,nonce_mode,&[(1,0),(2,1000)],2);
            for h in [0,1,999,1000,1001,u64::MAX]{assert!(!p.v3_active(h));}
            assert_eq!(p.spec_version(),1);assert_eq!(p.cp().cp_len(),52);
            assert_eq!((p.cp().H_half,p.cp().P_MAX,p.cp().T_PROB,p.cp().rho_bp),(0,0,0,0));
            assert_eq!(p.cp().F0_acct,U256::ZERO);assert!(!p.bootable());
        }
    }
    #[test]
    fn v3_predicate_covers_genesis_and_future_sequential_schedule(){
        let p=identity(1,0,&[(1,0),(2,10),(3,20)],3);
        for h in [0,9,10,19]{assert!(!p.v3_active(h));}
        for h in [20,21,u64::MAX]{assert!(p.v3_active(h));}
        assert_eq!(p.forks().active_version(20),3);
        let p=identity(2,1,&[(1,0)],2);
        for h in [0,1,u64::MAX]{assert!(p.v3_active(h));}
        assert_eq!(p.spec_version(),2);assert_eq!(p.cp().cp_len(),67);
        assert_eq!(p.cp().to_values(),p.spec().cp);assert!(!p.bootable());
    }
}
