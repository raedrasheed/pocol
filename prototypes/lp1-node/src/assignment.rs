//! LP3 S4 pure membership/nonce/Draw computations, consensus.md §§2,5,6.
//! ParentView contains caller-supplied facts from an already validated sealed parent.
//! Neither this view nor a computed assignment is a state proof, signature, or boot permission.
use std::collections::BTreeSet;

use crate::fixed::{work, U256, U512};
use crate::genesis_spec::{Address, RESERVED_HIGH, RESERVED_LOW, SYSTEM_ADDRESS};
use crate::hashes::{sha256, H256};

pub const NONCE_SPACE: u128 = 1u128 << 64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssignmentError {
    ParentHeight,
    MemberBound,
    DuplicateMember,
    ReservedMember,
    EmptyMembership,
    ZeroTargetOrC,
    TicketCollision,
    DuplicateCount,
    WeightOverflow,
    ZeroWeight,
    DrawCounterExhausted,
    IndexOutOfRange,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParentMember {
    pub id: Address,
    pub tomb: bool,
    pub last_share: u64,
    pub grace_start: u64,
}

/// act is the parent's ActList, not the entire registry and not the child state.
pub struct ParentView<'a> {
    pub height: u64,
    pub seal: &'a H256,
    pub act: &'a [ParentMember],
}

fn reserved(id: &Address) -> bool {
    let low = (id[17] as u32) << 16 | (id[18] as u32) << 8 | id[19] as u32;
    *id == [0; 20] || *id == SYSTEM_ADDRESS || (id[..17] == [0; 17] && (RESERVED_LOW..=RESERVED_HIGH).contains(&low))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Membership {
    height: u64,
    seal: H256,
    ids: Vec<Address>,
    /// Probationers excluded by the host's bond rule. Sorted, a subset of `ids`.
    accruing: Vec<Address>,
}
impl Membership {
    /// Computes A_h*, excluding tombs. `all_live` is selected by the host only for v3
    /// unbounded ranges; false preserves activity gating and the empty-active fallback.
    /// Input-shape checks reject inconsistent views, not new consensus validity rules.
    pub fn from_parent(parent: &ParentView<'_>, height: u64, w_act: u32, m_max: u16, all_live: bool) -> Result<Self, AssignmentError> {
        if height.checked_sub(1) != Some(parent.height) {
            return Err(AssignmentError::ParentHeight);
        }
        if m_max == 0 || parent.act.len() > m_max as usize {
            return Err(AssignmentError::MemberBound);
        }
        let mut seen = BTreeSet::new();
        let mut live = Vec::new();
        let mut active = Vec::new();
        let cutoff = height.saturating_sub(w_act as u64);
        for member in parent.act {
            if !seen.insert(member.id) {
                return Err(AssignmentError::DuplicateMember);
            }
            if reserved(&member.id) {
                return Err(AssignmentError::ReservedMember);
            }
            if !member.tomb {
                live.push(member.id);
                if member.last_share >= cutoff || member.grace_start >= cutoff {
                    active.push(member.id);
                }
            }
        }
        let mut ids = if all_live || active.is_empty() { live } else { active };
        ids.sort();
        Ok(Self { height, seal: *parent.seal, ids, accruing: Vec::new() })
    }
    pub fn ids(&self) -> &[Address] {
        &self.ids
    }
    /// Marks probationers excluded by the host's bond rule. They keep their nonce range;
    /// when every member is marked, the historical all-probationer draw exception applies.
    /// Ids outside the membership are ignored.
    pub fn with_accruing(mut self, ids: impl IntoIterator<Item = Address>) -> Self {
        let mut accruing: Vec<Address> = ids.into_iter().filter(|id| self.ids.binary_search(id).is_ok()).collect();
        accruing.sort();
        accruing.dedup();
        self.accruing = accruing;
        self
    }
    pub fn accruing(&self) -> &[Address] {
        &self.accruing
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NonceMode {
    Bounded,
    Unbounded,
}

pub fn nonce_max(target: &U256, mode: NonceMode, c: u8) -> Result<u128, AssignmentError> {
    if target.is_zero() || c == 0 {
        return Err(AssignmentError::ZeroTargetOrC);
    }
    if mode == NonceMode::Unbounded {
        return Ok(NONCE_SPACE);
    }
    // target>0 means the existing work() result is exact, not its target=0 clamp.
    let product = U512::mul_u256_u64(&work(target), c as u64);
    Ok(if product.bits() > 64 { NONCE_SPACE } else { product.0[0] as u128 })
}

pub fn nonce_seed(seal: &H256, height: u64, attempt: u32) -> H256 {
    let mut bytes = b"PoColNonce".to_vec();
    bytes.extend_from_slice(seal);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&attempt.to_be_bytes());
    sha256(&bytes)
}

pub fn nonce_ticket(seed: &H256, id: &Address) -> H256 {
    let mut bytes = [0u8; 52];
    bytes[..32].copy_from_slice(seed);
    bytes[32..].copy_from_slice(id);
    sha256(&bytes)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NonceRange {
    id: Address,
    ticket: H256,
    start: u128,
    end_exclusive: u128,
}
impl NonceRange {
    pub fn id(&self) -> Address {
        self.id
    }
    pub fn ticket(&self) -> H256 {
        self.ticket
    }
    pub fn start(&self) -> u128 {
        self.start
    }
    pub fn end_exclusive(&self) -> u128 {
        self.end_exclusive
    }
    pub fn inclusive_end(&self) -> Option<u64> {
        if self.start == self.end_exclusive {
            None
        } else {
            Some((self.end_exclusive - 1) as u64)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assignment {
    seed: H256,
    nonce_max: u128,
    ranges: Vec<NonceRange>,
}
impl Assignment {
    pub fn new(members: &Membership, attempt: u32, target: &U256, mode: NonceMode, c: u8) -> Result<Self, AssignmentError> {
        if members.ids.is_empty() {
            return Err(AssignmentError::EmptyMembership);
        }
        let nonce_max = nonce_max(target, mode, c)?;
        let seed = nonce_seed(&members.seal, members.height, attempt);
        let mut tickets: Vec<_> = members.ids.iter().map(|id| (nonce_ticket(&seed, id), *id)).collect();
        tickets.sort();
        if tickets.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            // M1 does not specify a tie policy for distinct identities with equal SHA-256 tickets.
            return Err(AssignmentError::TicketCollision);
        }
        let n = tickets.len() as u128;
        let ranges = tickets
            .into_iter()
            .enumerate()
            .map(|(i, (ticket, id))| {
                let i = i as u128;
                NonceRange { id, ticket, start: i * nonce_max / n, end_exclusive: (i + 1) * nonce_max / n }
            })
            .collect();
        Ok(Self { seed, nonce_max, ranges })
    }
    pub fn seed(&self) -> H256 {
        self.seed
    }
    pub fn nonce_max(&self) -> u128 {
        self.nonce_max
    }
    pub fn ranges(&self) -> &[NonceRange] {
        &self.ranges
    }
    /// The member whose range holds `nonce` (consensus.md §5). The ranges are contiguous and sorted:
    /// range i is [i·nMax/n, (i+1)·nMax/n), so both bounds are non-decreasing in i and an empty range
    /// (nMax < n) holds no nonce. A binary search for the first range ending above `nonce` finds the
    /// same range the linear scan of the first range holding it found (FN-1, part of C6): O(log M)
    /// instead of O(M) per nonce at M_max 10,000. `owner_linear` (tests) is the oracle.
    pub fn owner(&self, nonce: u64) -> Option<Address> {
        let nonce = nonce as u128;
        if nonce >= self.nonce_max {
            return None;
        }
        let i = self.ranges.partition_point(|r| r.end_exclusive <= nonce);
        self.ranges.get(i).filter(|r| r.start <= nonce && nonce < r.end_exclusive).map(|r| r.id)
    }
    /// The pre-FN-1 linear scan, kept as the oracle of `owner`.
    #[cfg(test)]
    fn owner_linear(&self, nonce: u64) -> Option<Address> {
        let nonce = nonce as u128;
        if nonce >= self.nonce_max {
            return None;
        }
        self.ranges.iter().find(|r| r.start <= nonce && nonce < r.end_exclusive).map(|r| r.id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareCount {
    pub height: u64,
    pub id: Address,
    pub count: u64,
}

/// Exact 256-bit rejection sampler: accept x < floor(2^256/W)*W, then return x mod W.
#[derive(Clone, Debug)]
pub struct UnbiasedSampler {
    total: u128,
    divisor: U512,
    last: U256,
}
impl UnbiasedSampler {
    pub fn new(total: u128) -> Result<Self, AssignmentError> {
        if total == 0 {
            return Err(AssignmentError::ZeroWeight);
        }
        let mut divisor = U512::ZERO;
        divisor.0[0] = total as u64;
        divisor.0[1] = (total >> 64) as u64;
        let mut space = U512::ZERO;
        space.0[4] = 1;
        let remainder = space.div_rem(&divisor).unwrap().1.to_u256().unwrap();
        let last = U256::MAX.checked_sub(&remainder).unwrap();
        Ok(Self { total, divisor, last })
    }
    pub fn sample(&self, digest: &H256) -> Option<u128> {
        let x = U256::from_be_slice(digest).unwrap();
        if x > self.last {
            return None;
        }
        let rem = U512::from_u256(&x).div_rem(&self.divisor).unwrap().1;
        Some(rem.0[0] as u128 | (rem.0[1] as u128) << 64)
    }
}

pub fn proposer_digest(seal: &H256, height: u64, counter: u32) -> H256 {
    let mut bytes = b"PoColProposer".to_vec();
    bytes.extend_from_slice(seal);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes.extend_from_slice(&counter.to_be_bytes());
    sha256(&bytes)
}

#[derive(Clone, Debug)]
pub struct DrawWeights {
    height: u64,
    seal: H256,
    weights: Vec<(Address, u128)>,
    sampler: UnbiasedSampler,
}
impl DrawWeights {
    pub fn new(members: &Membership, w_w: u16, history: &[ShareCount]) -> Result<Self, AssignmentError> {
        if members.ids.is_empty() {
            return Err(AssignmentError::EmptyMembership);
        }
        let mut weights: Vec<_> = members.ids.iter().map(|id| (*id, 0u128)).collect();
        // D1 / V3-13: the host marks excluded probationers (all ACCRUING on legacy chains,
        // only those below B_reg/4 on v3). Retain the owner-approved exception when all
        // members are marked; with none marked, every member is a candidate as before.
        let all_accruing = members.accruing.len() == members.ids.len();
        let candidate = |id: &Address| all_accruing || members.accruing.binary_search(id).is_err();
        let mut seen = BTreeSet::new();
        let lower = members.height.saturating_sub(1).saturating_sub(w_w as u64);
        let upper = members.height.checked_sub(2);
        for row in history {
            if !seen.insert((row.height, row.id)) {
                return Err(AssignmentError::DuplicateCount);
            }
            if upper.map_or(false, |end| lower <= row.height && row.height <= end) {
                if let Ok(i) = members.ids.binary_search(&row.id) {
                    if candidate(&row.id) {
                        weights[i].1 = weights[i].1.checked_add(row.count as u128).ok_or(AssignmentError::WeightOverflow)?;
                    }
                }
            }
        }
        let mut total = 0u128;
        for (_, w) in &weights {
            total = total.checked_add(*w).ok_or(AssignmentError::WeightOverflow)?;
        }
        if total == 0 {
            // D2: every candidate weighs 1; a non-candidate stays at 0.
            for (id, w) in &mut weights {
                *w = u128::from(candidate(id));
            }
            total = weights.iter().map(|(_, w)| *w).sum();
        }
        Ok(Self { height: members.height, seal: members.seal, weights, sampler: UnbiasedSampler::new(total)? })
    }
    pub fn weights(&self) -> &[(Address, u128)] {
        &self.weights
    }
    pub fn total(&self) -> u128 {
        self.sampler.total
    }
    pub fn choose(&self, index: u128) -> Result<Address, AssignmentError> {
        if index >= self.total() {
            return Err(AssignmentError::IndexOutOfRange);
        }
        let mut sum = 0;
        for (id, w) in &self.weights {
            sum += *w;
            if sum > index {
                return Ok(*id);
            }
        }
        Err(AssignmentError::IndexOutOfRange)
    }
    pub fn draw(&self) -> Result<(Address, u32), AssignmentError> {
        self.draw_using(0, |k| proposer_digest(&self.seal, self.height, k))
    }
    fn draw_using<F: FnMut(u32) -> H256>(&self, mut counter: u32, mut digest: F) -> Result<(Address, u32), AssignmentError> {
        loop {
            if let Some(index) = self.sampler.sample(&digest(counter)) {
                return Ok((self.choose(index)?, counter));
            }
            counter = counter.checked_add(1).ok_or(AssignmentError::DrawCounterExhausted)?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// A deterministic xorshift64* generator (no dependency; the crate builds offline on 1.58).
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }
    }
    fn id_of(i: u64) -> Address {
        let mut id = [0x11; 20];
        id[12..].copy_from_slice(&i.to_be_bytes());
        id
    }
    /// An assignment of `n` members over `nonce_max`, its ranges built as `Assignment::new` builds them.
    fn assignment_of(n: u64, nonce_max: u128) -> Assignment {
        let n128 = n as u128;
        let ranges = (0..n)
            .map(|i| {
                let k = i as u128;
                NonceRange { id: id_of(i), ticket: [0; 32], start: k * nonce_max / n128, end_exclusive: (k + 1) * nonce_max / n128 }
            })
            .collect();
        Assignment { seed: [0; 32], nonce_max, ranges }
    }
    /// The nonces where an answer can change: every range bound, one below and one above it, and the u64 edges.
    fn edges(a: &Assignment) -> Vec<u64> {
        let mut out = vec![0, 1, u64::MAX - 1, u64::MAX];
        let mut add = |x: u128| {
            for d in [x.wrapping_sub(1), x, x + 1] {
                if d <= u64::MAX as u128 {
                    out.push(d as u64);
                }
            }
        };
        add(a.nonce_max);
        // Every bound of a small set; of a large one (the linear oracle is O(M) per nonce) the first and last 64
        // ranges and every 37th.
        let m = a.ranges.len();
        for (i, r) in a.ranges.iter().enumerate() {
            if m <= 1024 || i < 64 || i + 64 >= m || i % 37 == 0 {
                add(r.start);
                add(r.end_exclusive);
            }
        }
        out
    }
    #[test]
    fn binary_search_owner_equals_the_linear_scan_on_edge_and_random_assignments() {
        let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
        let mut cases: Vec<(u64, u128)> = Vec::new();
        // Empty set, one member, members above nMax (empty ranges), the full u64 domain, a 2^64 - 1 domain,
        // and M_max 10,000 (owner decision 2026-11-13).
        for n in [0u64, 1, 2, 3, 7, 64, 255, 256, 1000, 10_000] {
            for nonce_max in [1u128, 2, 3, 5, 63, 64, 1000, 9_999, 10_000, 10_001, 1 << 32, (1 << 64) - 1, NONCE_SPACE] {
                cases.push((n, nonce_max));
            }
        }
        for _ in 0..200 {
            let n = rng.next() % 2048;
            let nonce_max = match rng.next() % 4 {
                0 => (rng.next() % 4096) as u128 + 1,
                1 => (rng.next() as u128) + 1,
                2 => NONCE_SPACE,
                _ => (rng.next() >> (rng.next() % 64)) as u128 + 1,
            };
            cases.push((n, nonce_max));
        }
        let mut checked = 0u64;
        for (n, nonce_max) in cases {
            let a = assignment_of(n, nonce_max);
            let mut nonces = edges(&a);
            for _ in 0..256 {
                let x = rng.next();
                nonces.push(x);
                if nonce_max <= u64::MAX as u128 {
                    nonces.push(x % nonce_max as u64);
                }
            }
            for nonce in nonces {
                assert_eq!(a.owner(nonce), a.owner_linear(nonce), "n={} nMax={} nonce={}", n, nonce_max, nonce);
                checked += 1;
            }
            // The last range ends at nMax: its last nonce has an owner, nMax itself none.
            if n > 0 && nonce_max <= NONCE_SPACE {
                let last = (nonce_max - 1) as u64;
                assert_eq!(a.owner(last), Some(a.ranges.last().unwrap().id));
            }
        }
        assert!(checked > 100_000);
    }
    #[test]
    fn binary_search_owner_is_exhaustive_on_small_domains() {
        for n in 0u64..=12 {
            for nonce_max in 1u128..=40 {
                let a = assignment_of(n, nonce_max);
                for nonce in 0u64..=45 {
                    assert_eq!(a.owner(nonce), a.owner_linear(nonce), "n={} nMax={} nonce={}", n, nonce_max, nonce);
                }
            }
        }
    }
    #[test]
    fn binary_search_owner_matches_on_a_computed_assignment() {
        let ids: Vec<Address> = (1..=300u64).map(id_of).collect();
        let members = Membership { height: 9, seal: [7; 32], ids, accruing: vec![] };
        for (mode, c) in [(NonceMode::Unbounded, 1u8), (NonceMode::Bounded, 1), (NonceMode::Bounded, 16)] {
            let a = Assignment::new(&members, 2, &U256::MAX, mode, c).unwrap();
            let mut rng = Rng(42);
            for nonce in edges(&a).into_iter().chain((0..4096).map(|_| rng.next())) {
                assert_eq!(a.owner(nonce), a.owner_linear(nonce));
            }
        }
    }
    #[test]
    fn rejected_hash_retries_without_wrap_or_modulo_fallback() {
        let mut id = [1; 20];
        id[19] = 2;
        let member = Membership { height: 10, seal: [0; 32], ids: vec![id], accruing: vec![] };
        let weights = DrawWeights::new(&member, 1, &[ShareCount { height: 8, id, count: 3 }]).unwrap();
        let mut counters = Vec::new();
        let got = weights
            .draw_using(0, |k| {
                counters.push(k);
                if k == 0 {
                    [255; 32]
                } else {
                    [0; 32]
                }
            })
            .unwrap();
        assert_eq!(counters, [0, 1]);
        assert_eq!(got, (id, 1));
        assert_eq!(weights.draw_using(u32::MAX, |_| [255; 32]), Err(AssignmentError::DrawCounterExhausted));
    }
}
