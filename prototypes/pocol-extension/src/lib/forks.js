// Consensus activation is a local trust decision. Never learn this schedule from
// an RPC response: an endpoint must not choose the rules used to verify itself.
const U64_MAX = (1n << 64n) - 1n;
const invalid = () => { throw new Error('forkSchedule must be [[1,"0"]] or [[1,"0"],[2,"<activation height>"]]'); };
export function normalizeForkSchedule(value = undefined) {
  if (value === undefined) value = [[1, '0']];
  if (!Array.isArray(value) || value.length < 1 || value.length > 2) invalid();
  const schedule = value.map((entry, i) => {
    if (!Array.isArray(entry) || entry.length !== 2 || entry[0] !== i + 1) invalid();
    const height = entry[1];
    if (!(typeof height === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(height)) && !(Number.isSafeInteger(height) && height >= 0)) invalid();
    const h = BigInt(height);
    if (h > U64_MAX || (i === 0 ? h !== 0n : h === 0n)) invalid();
    return Object.freeze([entry[0], h.toString()]);
  });
  return Object.freeze(schedule);
}
/** The caller supplies a normalized, locally approved schedule. */
export function activeVersion(schedule, height) {
  return schedule.length === 2 && BigInt(height) >= BigInt(schedule[1][1]) ? 2n : 1n;
}
