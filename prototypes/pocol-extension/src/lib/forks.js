// Consensus activation is a local trust decision. Never learn this schedule from
// an RPC response: an endpoint must not choose the rules used to verify itself.
const U64_MAX = (1n << 64n) - 1n;
// The highest consensus version this wallet verifies (pocold profile MAX_FORK_VERSION). Raised only together
// with the node release that implements the next version's rules.
export const MAX_FORK_VERSION = 2;
const invalid = () => { throw new Error('forkSchedule must be [[1,"0"]] followed by later activations [[2,"<height>"], …] at increasing heights, up to version ' + MAX_FORK_VERSION); };
/** `max` exists for tests of later versions; callers use the default. Versions are 1, 2, … in order. */
export function normalizeForkSchedule(value = undefined, max = MAX_FORK_VERSION) {
  if (value === undefined) value = [[1, '0']];
  if (!Array.isArray(value) || value.length < 1 || value.length > 16) invalid();
  let previous = -1n;
  const schedule = value.map((entry, i) => {
    if (!Array.isArray(entry) || entry.length !== 2 || entry[0] !== i + 1 || entry[0] > max) invalid();
    const height = entry[1];
    if (!(typeof height === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(height)) && !(Number.isSafeInteger(height) && height >= 0)) invalid();
    const h = BigInt(height);
    if (h > U64_MAX || (i === 0 ? h !== 0n : h <= previous)) invalid();
    previous = h;
    return Object.freeze([entry[0], h.toString()]);
  });
  return Object.freeze(schedule);
}
/** The caller supplies a normalized, locally approved schedule: the version of the last entry at or below `height`. */
export function activeVersion(schedule, height) {
  const h = BigInt(height);
  let version = 1n;
  for (const [v, start] of schedule) if (BigInt(start) <= h) version = BigInt(v);
  return version;
}
