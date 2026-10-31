// Pinned identity of the existing PoCol Mainnet. Public data only.
export const STUDIO_ORIGINS = Object.freeze(['https://studio.pocol.net', 'https://pocol-studio.mystic-gecko-9511.chatgpt.site']);
export const NETWORK = Object.freeze({
  "name": "pocol-public",
  "chainId": 1599846715,
  "genesisHash": "0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca",
  "genesisPre": "0xf9016b01845f5bb93bf89e846ae3a7029e400040004000400040004000400040004000400040004000400040003fff0a82025880023c06200282010004088203e88201f4881bc16d674ec8000089056bc75e2d631000000140018220001040102040102804822000837a1200843b9aca008302000083030000828000832000008401000000824e2082c350824e20830100008208008201008202008405b8d8008080808081a0818002a00952692453fda50223e12b5eb117946dc5402053712d56d5be8b9b338fed9dacf881ea94854e2a71a9d84dc63290804fed3f51e79e1f5d9394854e2a71a9d84dc63290804fed3f51e79e1f5d93ea948f8fff3e07669f41cb2c2e458a0282eeef858b7e948f8fff3e07669f41cb2c2e458a0282eeef858b7eea94a28f9bd53bffdc2651832b238eaa96917f33adca94a28f9bd53bffdc2651832b238eaa96917f33adcaa0a877a16e8e236b8e505e7a752f8e95e26ee59e8ace13156ce8b2307d1ef14c4f",
  "endpoint": "https://gateway.pocol.net/studio/rpc",
  "trustLevel": "RN"
});
export function validateEndpoint(endpoint, trustLevel) {
  if (typeof endpoint !== 'string') throw new Error('endpoint');
  if (/^http:\/\/127\.0\.0\.1:\d{1,5}\/?$/.test(endpoint)) {
    const u = new URL(endpoint); if (Number(u.port || 80) < 1) throw new Error('endpoint port');
    if (trustLevel === 'RN') throw new Error('RN requires the pinned HTTPS endpoint');
    return endpoint;
  }
  // Exact allowlist; no user-controlled URL, credentials, fragment, redirect or path.
  if (endpoint === NETWORK.endpoint && trustLevel === 'RN') return endpoint;
  throw new Error('Use the official PoCol HTTPS endpoint (RN) or a loopback node');
}
export function isStudioOrigin(url) {
  try { const u = new URL(url); return STUDIO_ORIGINS.includes(u.origin); } catch { return false; }
}
export function trustMessage(level) {
  return level === 'RN' ? 'RN: remote PoCol node trusted; file hashes verified' : level === 'LN' ? 'LN: a local node verifies full execution' : 'DEV: development network without verification';
}
