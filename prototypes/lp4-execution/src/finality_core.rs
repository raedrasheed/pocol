//! Approved LP3 CF1–CF6 pure predicates, not execution or a state certificate.
//! Parent facts must come from independently validated state before any future boot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParentFacts {
    pub closing: bool,
    pub close_h: u64,
    pub cursor_done: bool,
    pub is_final: bool,
}
pub fn closing(first_close: Option<u64>, h: u64, ts: u64, h_close: u64, t_close: u64) -> Option<u64> {
    first_close.or_else(|| if h >= h_close || ts >= t_close { Some(h) } else { None })
}
/// Empty cursor is done immediately; a nonempty cursor consumes RET from closeH.
pub fn cursor_bound(close_h: u64, records: u64, ret: u32) -> Result<u64, &'static str> {
    if ret == 0 {
        return Err("ret");
    }
    let rounds = (records as u128 + ret as u128 - 1) / ret as u128;
    u64::try_from(close_h as u128 + rounds.saturating_sub(1)).map_err(|_| "overflow")
}
pub fn gate2(h: u64, ts: u64, h_end: u64, t_end: u64, p2: u32, t_blk: u32) -> Result<bool, &'static str> {
    let height = h_end.checked_sub(p2 as u64).ok_or("threshold")?;
    let time = t_end.checked_sub(p2 as u64 * t_blk as u64).ok_or("threshold")?;
    Ok(h >= height || ts >= time)
}
pub fn final_predicate(p: ParentFacts, h: u64, ts: u64, d: u32, h_end: u64, t_end: u64) -> bool {
    p.closing && p.cursor_done && h as u128 >= p.close_h as u128 + d as u128 && (h >= h_end || ts >= t_end)
}
/// Only the two CF6 ceilings, not all conditions for a valid child.
pub fn child_allowed(parent_final: bool, h: u64, h_end: u64) -> bool {
    !parent_final && h <= h_end
}
