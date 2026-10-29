//! LP3 S3 pure timing arithmetic; no wall clock, sleeps, scheduler or boot authorization.
//! consensus.md §3 and threat.md timing formulas. Seconds unless a name ends in `_ms`.
//! All derived arithmetic is u128; a block's U(a) must be strictly below 2^63.

pub const TIMESTAMP_LIMIT: u128 = 1u128 << 63;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimingError {
    ZeroAttemptDuration,
    TimestampLimit,
    OutsideWindow,
    NotFirstAttempt,
    MillisecondOverflow,
}

/// The largest wire-format (u32) attempt with p.ts+(a+1)*D_att < 2^63.
/// None means even attempt zero has no valid window. No consensus cap is silently truncated.
pub fn max_attempt(parent_ts: u64, d_att: u32) -> Result<Option<u32>, TimingError> {
    if d_att == 0 {
        return Err(TimingError::ZeroAttemptDuration);
    }
    let parent = parent_ts as u128;
    if parent >= TIMESTAMP_LIMIT {
        return Err(TimingError::TimestampLimit);
    }
    let slots = (TIMESTAMP_LIMIT - 1 - parent) / d_att as u128;
    Ok(if slots == 0 { None } else { Some((slots - 1).min(u32::MAX as u128) as u32) })
}

/// Validated inclusive [L(a), U(a)] with private bounds, so callers cannot forge a window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttemptWindow {
    attempt: u32,
    lower: u64,
    upper: u64,
}

impl AttemptWindow {
    pub fn new(parent_ts: u64, attempt: u32, d_att: u32) -> Result<Self, TimingError> {
        if d_att == 0 {
            return Err(TimingError::ZeroAttemptDuration);
        }
        let parent = parent_ts as u128;
        let a = attempt as u128;
        let d = d_att as u128;
        let lower = parent + a * d + 1;
        let upper = parent + (a + 1) * d;
        if upper >= TIMESTAMP_LIMIT {
            return Err(TimingError::TimestampLimit);
        }
        Ok(Self { attempt, lower: lower as u64, upper: upper as u64 })
    }

    pub fn lower(&self) -> u64 {
        self.lower
    }
    pub fn upper(&self) -> u64 {
        self.upper
    }
    pub fn contains(&self, ts: u64) -> bool {
        self.lower <= ts && ts <= self.upper
    }

    /// Build_fb timestamp: L(0)+Delta_fb_wait at a=0, otherwise exactly L(a).
    /// Reject an incompatible first-attempt wait rather than emitting an invalid timestamp.
    pub fn fallback_timestamp(&self, fb_wait: u32) -> Result<u64, TimingError> {
        let ts = self.lower as u128 + if self.attempt == 0 { fb_wait as u128 } else { 0 };
        if ts > self.upper as u128 {
            return Err(TimingError::OutsideWindow);
        }
        Ok(ts as u64)
    }

    /// The signed first proposer builds at max(L(0), C_P), only if <= U(0).
    pub fn proposer_timestamp(&self, clock: u64) -> Result<u64, TimingError> {
        if self.attempt != 0 {
            return Err(TimingError::NotFirstAttempt);
        }
        let ts = self.lower.max(clock);
        if !self.contains(ts) {
            return Err(TimingError::OutsideWindow);
        }
        Ok(ts)
    }

    /// Transition is strict: C > U(a)+Gamma, not equality. Addition never wraps u64.
    pub fn advance_due(&self, clock: u64, gamma: u64) -> bool {
        clock as u128 > self.upper as u128 + gamma as u128
    }

    /// a>=1 fallback readiness only. The first-attempt WAIT decision uses T_j below.
    pub fn later_fallback_due(&self, clock: u64) -> bool {
        self.attempt >= 1 && clock >= self.lower
    }

    /// T_j=max(b_j+Delta_rel, L(0)+Delta_fb_wait), converted to milliseconds exactly.
    /// b_j is the local parent-receipt time in ms; it may have a fractional second.
    pub fn deadline_ms(&self, parent_received_ms: u128, delta_rel: u64, fb_wait: u32) -> Result<u128, TimingError> {
        if self.attempt != 0 {
            return Err(TimingError::NotFirstAttempt);
        }
        let receipt_bound = parent_received_ms.checked_add(delta_rel as u128 * 1000).ok_or(TimingError::MillisecondOverflow)?;
        let fallback_bound = self.fallback_timestamp(fb_wait)? as u128 * 1000;
        Ok(receipt_bound.max(fallback_bound))
    }
}

/// Timing predicate only: a valid signed template arriving exactly at T_j can lock.
/// Signature/Draw validity and sticky FB state are the caller's responsibility, not asserted here.
pub fn arrives_by_deadline(arrival_ms: u128, deadline_ms: u128) -> bool {
    arrival_ms <= deadline_ms
}

/// ts > C+Phi means future. Phi here is the capital-Phi clock-skew bound, not phi.
pub fn is_future(ts: u64, clock: u64, phi: u64) -> bool {
    ts as u128 > clock as u128 + phi as u128
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingMetrics {
    pub sigma: u128,
    pub x_h: u128,
    pub x_w: u128,
}

/// threat.md timing bounds; these are conditional formula outputs, not measured guarantees.
/// Inputs are Delta, capital Phi, t_exec, Delta_rel, Delta_fb_wait, all in seconds.
pub fn metrics(delta: u64, phi: u64, t_exec: u64, delta_rel: u64, fb_wait: u64) -> TimingMetrics {
    let (d, p, t, r, f) = (delta as u128, phi as u128, t_exec as u128, delta_rel as u128, fb_wait as u128);
    TimingMetrics { sigma: d.max(p + 1) + t, x_h: (d + t).max(p + 1) + d + 2 * t, x_w: (p + 1 + f).max(d + t + r) }
}
