//! Pure settlement arithmetic for LP3 SV core vectors; not Escrow/EVM/state execution.
use crate::fixed::{U256, U512};
use crate::genesis_spec::Address;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RewardError {
    Parameters,
    Duplicate,
    Counts,
    Arithmetic,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    Paid,
    PenaltyBurn,
    PayBurn,
}
#[derive(Clone, Debug)]
pub struct ShareInput {
    pub id: Address,
    pub count: u16,
    pub tomb: bool,
    pub account_exists: bool,
}
#[derive(Clone, Debug)]
pub struct ShareRewards {
    pub payments: Vec<(Address, U256, Route)>,
    pub amount: U256,
    pub base_burn: U256,
    pub round_burn: U256,
    pub paid: U256,
    pub penalty_burn: U256,
    pub pay_burn: U256,
}
fn plus(a: &U256, b: &U256) -> Result<U256, RewardError> {
    let mut out = U256::ZERO;
    let mut carry = 0u128;
    for k in 0..4 {
        let n = a.0[k] as u128 + b.0[k] as u128 + carry;
        out.0[k] = n as u64;
        carry = n >> 64;
    }
    if carry != 0 {
        Err(RewardError::Arithmetic)
    } else {
        Ok(out)
    }
}
fn part(n: &U256, m: u64, d: u64) -> Result<U256, RewardError> {
    if d == 0 {
        return Err(RewardError::Parameters);
    }
    U512::mul_u256_u64(n, m).div_rem(&U512::from_u256(&U256::from_u64(d))).and_then(|v| v.0.to_u256()).ok_or(RewardError::Arithmetic)
}
pub fn shares(r: &U256, counts: &[ShareInput], kappa: u8, m: u16) -> Result<ShareRewards, RewardError> {
    if counts.len() > 257 {
        return Err(RewardError::Counts);
    }
    if kappa == 0 || m == 0 {
        return Err(RewardError::Parameters);
    }
    let mut total = 0u64;
    let mut seen = BTreeSet::new();
    for row in counts {
        if !seen.insert(row.id) {
            return Err(RewardError::Duplicate);
        }
        if row.count == 0 {
            return Err(RewardError::Counts);
        }
        total += row.count as u64;
    }
    // A header contributes <=256 explicit shares plus its implicit winner.
    if total > 257 {
        return Err(RewardError::Counts);
    }
    let q = total.max(kappa as u64 * m as u64);
    let amount = part(r, total, q)?;
    let mut result = ShareRewards {
        payments: Vec::new(),
        amount,
        base_burn: r.checked_sub(&amount).ok_or(RewardError::Arithmetic)?,
        round_burn: U256::ZERO,
        paid: U256::ZERO,
        penalty_burn: U256::ZERO,
        pay_burn: U256::ZERO,
    };
    let mut sum = U256::ZERO;
    for row in counts {
        let p = part(r, row.count as u64, q)?;
        sum = plus(&sum, &p)?;
        let route = if row.tomb {
            result.penalty_burn = plus(&result.penalty_burn, &p)?;
            Route::PenaltyBurn
        } else if !row.account_exists {
            result.pay_burn = plus(&result.pay_burn, &p)?;
            Route::PayBurn
        } else {
            result.paid = plus(&result.paid, &p)?;
            Route::Paid
        };
        result.payments.push((row.id, p, route));
    }
    result.round_burn = amount.checked_sub(&sum).ok_or(RewardError::Arithmetic)?;
    Ok(result)
}
#[derive(Clone, Debug)]
pub struct PotSplit {
    pub winner: U256,
    pub proposer: U256,
    pub remainder: U256,
}
pub fn split(pot: &U256, alpha: u16, gamma: u16) -> Result<PotSplit, RewardError> {
    if alpha as u32 + gamma as u32 > 10000 {
        return Err(RewardError::Parameters);
    }
    let winner = part(pot, alpha as u64, 10000)?;
    let proposer = part(pot, gamma as u64, 10000)?;
    let remainder = pot.checked_sub(&winner).and_then(|n| n.checked_sub(&proposer)).ok_or(RewardError::Arithmetic)?;
    Ok(PotSplit { winner, proposer, remainder })
}
