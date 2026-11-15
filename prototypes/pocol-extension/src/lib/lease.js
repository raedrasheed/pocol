// Sites W4: the publication key lease. An unlocked wallet locks LOCK_MS after it was unlocked. Once the user approves a
// job (a Studio publication or an IDE contract transaction), each signature made BY THAT JOB extends the lock time by
// LOCK_MS, never past the job deadline shown in the approval (default 2 h, at most 6 h). The lease ends on completion,
// error or pause (no further extension; the wallet then locks LOCK_MS after the job's last signature at the latest),
// and Lock ends it at once. Nothing else (the IDE page, a site, another job) can extend it. `clock` is injectable.
export const JOB_DEADLINE_DEFAULT_MS = 2 * 60 * 60 * 1000;
export const JOB_DEADLINE_MAX_MS = 6 * 60 * 60 * 1000;
/** The lease rule as a pure function: the lock time after a job's signature at `now`. */
export function extendLease(until, now, lockMs, deadline) {
  if (now >= until || now >= deadline) return until;
  return Math.min(Math.max(until, now + lockMs), deadline);
}
export class KeyLease {
  constructor({lockMs, clock = Date.now} = {}) {
    if (!Number.isFinite(lockMs) || lockMs <= 0) throw new Error('Invalid lock time');
    this.lockMs = lockMs; this.clock = clock; this.until = 0; this.job = null;
  }
  /** The wallet was unlocked: it stays unlocked for LOCK_MS. */
  unlock() { this.until = this.clock() + this.lockMs; this.job = null; return this.until; }
  get unlocked() { return this.clock() < this.until; }
  /** Starts the lease of an approved job; returns its deadline (ms since the epoch). */
  startJob(id, deadlineMs = JOB_DEADLINE_DEFAULT_MS) {
    if (!this.unlocked) throw new Error('Unlock the wallet again');
    if (typeof id !== 'string' || !id) throw new Error('Invalid job');
    if (!Number.isFinite(deadlineMs) || deadlineMs <= 0 || deadlineMs > JOB_DEADLINE_MAX_MS) throw new Error('The job deadline must be between 1 ms and 6 hours');
    const now = this.clock();
    this.job = {id, deadline: now + deadlineMs};
    // Approving starts a fresh LOCK_MS window, as the previous release did, bounded by the deadline.
    this.until = Math.min(Math.max(this.until, now + this.lockMs), this.job.deadline);
    return this.job.deadline;
  }
  /** A signature by job `id`: extends the lock time by LOCK_MS up to the job deadline. Returns false when refused. */
  signed(id) {
    const now = this.clock();
    if (!this.job || this.job.id !== id || now >= this.until || now >= this.job.deadline) return false;
    this.until = extendLease(this.until, now, this.lockMs, this.job.deadline);
    return true;
  }
  /** The job finished, failed or paused: no further extension. */
  endJob(id) { if (this.job && (id === undefined || this.job.id === id)) this.job = null; }
  /** Lock: the wallet locks now and the job lease ends. */
  lock() { this.until = 0; this.job = null; }
}
