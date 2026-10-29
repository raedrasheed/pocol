//! S5 work in progress: only the unambiguous R16/R20/R26 arithmetic.
//! This module is NOT a complete ParamGate and cannot authorize boot.
//! General R18/R21 derivations require the separately recorded design decision.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeriveError {
    Overflow,
    ZeroSendRate,
    ZeroRetirementRate,
}

fn add(a: u128, b: u128) -> Result<u128, DeriveError> {
    a.checked_add(b).ok_or(DeriveError::Overflow)
}
fn mul(a: u128, b: u128) -> Result<u128, DeriveError> {
    a.checked_mul(b).ok_or(DeriveError::Overflow)
}

/// Exact nonnegative rational milliseconds. Fields are private and normalized.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Milliseconds {
    numerator: u128,
    denominator: u128,
}
impl Milliseconds {
    fn new(numerator: u128, denominator: u128) -> Self {
        debug_assert!(denominator > 0);
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            let r = a % b;
            a = b;
            b = r;
        }
        Self { numerator: numerator / a, denominator: denominator / a }
    }
    pub fn ratio(&self) -> (u128, u128) {
        (self.numerator, self.denominator)
    }
    /// Avoid cross multiplication, so even u64::MAX input cannot wrap.
    pub fn at_most_ms(&self, limit: u64) -> bool {
        let q = self.numerator / self.denominator;
        q < limit as u128 || (q == limit as u128 && self.numerator % self.denominator == 0)
    }
    /// Exact decimal seconds, or None for a repeating decimal. No float or rounding.
    pub fn exact_seconds(&self) -> Option<String> {
        let d = self.denominator.checked_mul(1000)?;
        let mut factor = d;
        for p in [2, 5] {
            while factor % p == 0 {
                factor /= p;
            }
        }
        if factor != 1 {
            return None;
        }
        let mut text = (self.numerator / d).to_string();
        let mut rem = self.numerator % d;
        if rem != 0 {
            text.push('.');
        }
        while rem != 0 {
            rem = rem.checked_mul(10)?;
            text.push(char::from(b'0' + (rem / d) as u8));
            rem %= d;
        }
        Some(text)
    }
}

/// governance.md R16/R20, threat.md Lmax and the immutable D86 LAG=2048.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryMetrics {
    pub l_max: u128,
    pub records_required: u128,
    pub close_blocks_required: u128,
}
pub fn registry_metrics(u: u32, e_win: u32, d: u16, m_max: u16, r_max: u8, records: u32, ret: u32, p2: u32) -> Result<RegistryMetrics, DeriveError> {
    if ret == 0 {
        return Err(DeriveError::ZeroRetirementRate);
    }
    let l_max = (u as u128).max(2 * e_win as u128 + d as u128 + 1);
    let records_required = m_max as u128 + r_max as u128 * (l_max + 2048);
    let close_blocks_required = (records as u128 + ret as u128 - 1) / ret as u128 + p2 as u128 + d as u128 + 1;
    Ok(RegistryMetrics { l_max, records_required, close_blocks_required })
}

/// Local RpcGuard configuration; all times explicitly in integer milliseconds.
/// No configurable SEND_MAX/BATCH_SEND_MAX: they are derived by formula alone.
#[derive(Clone, Debug)]
pub struct RpcLimits {
    pub conn_max: u64,
    pub req_buf: u64,
    pub conn_buf: u64,
    pub send_sock_buf: u64,
    pub heavy_active: u64,
    pub light_active: u64,
    pub light_queue: u64,
    pub heavy_resp_mem: u64,
    pub light_resp_mem: u64,
    pub resp_max: u64,
    pub light_resp_max: u64,
    pub batch_resp_max: u64,
    pub scratch_heavy: u64,
    pub scratch_light: u64,
    pub mem_total: u64,
    pub batch_max: u64,
    pub err_item_max: u64,
    pub logs_count_max: u64,
    pub logs_scan_bytes: u64,
    pub rate: u64,
    pub bucket_cap: u64,
    pub queue_wait_ms: u64,
    pub compute_heavy_ms: u64,
    pub cancel_max_ms: u64,
    pub grace_ms: u64,
    pub min_send_rate: u64,
    pub batch_wall_ms: u64,
    pub enforce_slack_ms: u64,
    pub timer_eps_ms: u64,
}
impl Default for RpcLimits {
    fn default() -> Self {
        const M: u64 = 1024 * 1024;
        Self {
            conn_max: 32,
            req_buf: M,
            conn_buf: 64 * 1024,
            send_sock_buf: 32 * 1024,
            heavy_active: 4,
            light_active: 16,
            light_queue: 32,
            heavy_resp_mem: 48 * M,
            light_resp_mem: 16 * M,
            resp_max: 8 * M,
            light_resp_max: 512 * 1024,
            batch_resp_max: 16 * M,
            scratch_heavy: 16 * M,
            scratch_light: M,
            mem_total: 192 * M,
            batch_max: 20,
            err_item_max: 256,
            logs_count_max: 10000,
            logs_scan_bytes: 32 * M,
            rate: 20,
            bucket_cap: 20,
            queue_wait_ms: 2000,
            compute_heavy_ms: 5000,
            cancel_max_ms: 100,
            grace_ms: 2000,
            min_send_rate: 65536,
            batch_wall_ms: 420000,
            enforce_slack_ms: 200,
            timer_eps_ms: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RpcMetrics {
    pub memory_required: u128,
    pub batch_bytes_hard: u128,
    pub batch_send_max: Milliseconds,
    pub batch_send_watchdog_max: Milliseconds,
    pub batch_wall_min: Milliseconds,
}
impl RpcLimits {
    /// SEND_MAX(size)=GRACE+size/R+ENFORCE_SLACK, in exact milliseconds.
    pub fn send_max(&self, size: u128) -> Result<Milliseconds, DeriveError> {
        if self.min_send_rate == 0 {
            return Err(DeriveError::ZeroSendRate);
        }
        let rate = self.min_send_rate as u128;
        let n = add(mul(add(self.grace_ms as u128, self.enforce_slack_ms as u128)?, rate)?, mul(size, 1000)?)?;
        Ok(Milliseconds::new(n, rate))
    }
    pub fn derive(&self) -> Result<RpcMetrics, DeriveError> {
        let pair = add(self.req_buf as u128, self.conn_buf as u128)?;
        let memory = add(add(self.heavy_resp_mem as u128, self.light_resp_mem as u128)?, mul(self.heavy_active as u128, self.scratch_heavy as u128)?)?;
        let memory = add(add(memory, mul(self.light_active as u128, self.scratch_light as u128)?)?, mul(self.conn_max as u128, pair)?)?;
        let errors = add(mul(self.batch_max as u128, self.err_item_max as u128)?, 64)?;
        let hard = add(self.batch_resp_max as u128, errors)?;
        if self.min_send_rate == 0 {
            return Err(DeriveError::ZeroSendRate);
        }
        let rate = self.min_send_rate as u128;
        // network.md distinguishes the batch formula from its slack-inclusive watchdog.
        let send_n = add(mul(self.grace_ms as u128, rate)?, mul(hard, 1000)?)?;
        let send = Milliseconds::new(send_n, rate);
        let watched = add(send_n, mul(self.enforce_slack_ms as u128, rate)?)?;
        let item = add(add(self.queue_wait_ms as u128, self.enforce_slack_ms as u128)?, add(self.compute_heavy_ms as u128, self.cancel_max_ms as u128)?)?;
        let wall = add(mul(mul(self.batch_max as u128, item)?, rate)?, watched)?;
        Ok(RpcMetrics {
            memory_required: memory,
            batch_bytes_hard: hard,
            batch_send_max: send,
            batch_send_watchdog_max: Milliseconds::new(watched, rate),
            batch_wall_min: Milliseconds::new(wall, rate),
        })
    }
    /// Only R26. A true result is never a complete ParamGate result or boot permit.
    pub fn satisfies_r26(&self) -> bool {
        let v = match self.derive() {
            Ok(v) => v,
            Err(_) => return false,
        };
        self.heavy_resp_mem >= self.resp_max
            && self.light_resp_mem as u128 >= self.conn_max as u128 * self.light_resp_max as u128
            && v.memory_required <= self.mem_total as u128
            && self.batch_resp_max >= self.resp_max
            && self.light_queue >= self.conn_max
            && self.batch_max as u128 * self.err_item_max as u128 + 64 <= self.conn_buf as u128
            && self.resp_max >= 2 * 32768 + 546 * 512
            && self.logs_count_max >= 546
            && self.logs_scan_bytes >= 32768
            && self.light_resp_max >= 385 * 1024
            && v.batch_wall_min.at_most_ms(self.batch_wall_ms)
            && self.rate >= 1
            && self.bucket_cap >= self.batch_max
            && self.timer_eps_ms > 0
            && self.timer_eps_ms < self.enforce_slack_ms
            && self.enforce_slack_ms <= 500
            && self.send_sock_buf <= 64 * 1024
    }
}
