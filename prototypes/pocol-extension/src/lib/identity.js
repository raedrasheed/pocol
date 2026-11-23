// Network identity of an endpoint against the profile a page or the worker signs for (2026-11-22): the chain ID
// (eth_chainId) and the genesis hash (pocol_status.genesisHash) must both equal the profile's. Every signing path (the
// wallet's Send, the Studio publisher, the IDE contract page, viewer-site signatures) refuses a mismatch; this module
// words the refusal so that the user sees the difference: what the profile pins and what the endpoint answered.
// Pure: it neither stores nor sends anything.

const shortHash = (h) => (typeof h === 'string' && /^0x[0-9a-fA-F]{64}$/.test(h) ? h.toLowerCase() : null);
/** The decimal chain ID of an eth_chainId answer (a 0x quantity) or a number, or null when it is neither. */
export function chainIdOf(v) {
  try {
    if (typeof v === 'number' && Number.isSafeInteger(v) && v >= 0) return BigInt(v).toString();
    if (typeof v === 'bigint' && v >= 0n) return v.toString();
    if (typeof v === 'string' && /^0x[0-9a-fA-F]{1,16}$/.test(v)) return BigInt(v).toString();
  } catch { /* not a chain ID */ }
  return null;
}
/** null when the endpoint's identity equals the expected one; otherwise the list of differing fields, each
 *  {field: 'chainId'|'genesisHash', expected, got} (`got` is null for a missing or malformed answer). */
export function identityDifference(expected, endpointChainId, endpointGenesis) {
  const out = [];
  const wantChain = chainIdOf(expected.chainId), gotChain = chainIdOf(endpointChainId);
  if (wantChain === null || gotChain !== wantChain) out.push({ field: 'chainId', expected: wantChain, got: gotChain });
  const wantGenesis = shortHash(expected.genesisHash), gotGenesis = shortHash(endpointGenesis);
  if (wantGenesis === null || gotGenesis !== wantGenesis) out.push({ field: 'genesisHash', expected: wantGenesis, got: gotGenesis });
  return out.length ? out : null;
}
/** The words of a difference: "chain ID: expected 1, the endpoint answered 2; genesis hash: expected 0x…, the
 *  endpoint answered 0x…". */
export function describeDifference(diff) {
  const label = { chainId: 'chain ID', genesisHash: 'genesis hash' };
  return (diff || []).map((d) => `${label[d.field]}: expected ${d.expected ?? '(none)'}, the endpoint answered ${d.got ?? 'no valid value'}`).join('; ');
}
/** Throws `prefix + the difference` (with .code and .difference) when the endpoint's identity differs. */
export function assertIdentity(expected, endpointChainId, endpointGenesis, prefix = 'The endpoint is another network', code = 4901) {
  const diff = identityDifference(expected, endpointChainId, endpointGenesis);
  if (diff) throw Object.assign(new Error(`${prefix} (${describeDifference(diff)})`), { code, difference: diff });
}
