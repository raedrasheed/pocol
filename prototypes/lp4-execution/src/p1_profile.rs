//! LP3 S2 schema/identity view. Never a boot authorization or complete ParamGate.
//! Sources: consensus.md:7-34; governance.md R22/R24 and network chainId 777001.
use crate::fixed::U256;
use crate::genesis_spec::{self, GenesisSpec, GsError, Upper, CP_SCHEMA};
use crate::hashes::H256;

/// All 52 consensus parameters in wire order, using their specified unsigned widths.
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
}

impl ConsensusParams {
    /// Checks widths and schema bounds before narrowing; cross-field ParamGate rules are deferred.
    pub fn from_values(v: &[U256; 52]) -> Result<Self, GsError> {
        for (i, field) in CP_SCHEMA.iter().enumerate() {
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
        })
    }

    pub fn to_values(&self) -> [U256; 52] {
        [
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
        ]
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
    UnsupportedVersion(u16),
}

/// Only v1 is implemented. TestFork-v2 remains unsupported, even in simnet.
/// Private entries ensure active_version is called only on a validated schedule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForkSchedule(Vec<(u16, u64)>);
impl ForkSchedule {
    pub fn new(entries: &[(u16, u64)]) -> Result<Self, ProfileError> {
        if entries.first() != Some(&(1, 0)) {
            return Err(ProfileError::ForkStart);
        }
        let mut previous = None;
        for &(version, height) in entries {
            if previous.map_or(false, |h| height <= h) {
                return Err(ProfileError::ForkHeight);
            }
            if version != 1 {
                return Err(ProfileError::UnsupportedVersion(version));
            }
            previous = Some(height);
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
