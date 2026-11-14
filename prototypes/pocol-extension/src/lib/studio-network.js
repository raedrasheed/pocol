// Pinned identity of the existing PoCol Mainnet. Public data only.
export const STUDIO_ORIGINS = Object.freeze(['https://studio.pocol.net']);
export const NETWORK = Object.freeze({
  "name": "pocol-public",
  "chainId": 1234567891,
  "genesisHash": "0x3130bfed4d9f4b4f6df657220d94035c21bfe7cc445df9d5572979dc4767f74f",
  "genesisPre": "0xf9018f0284499602d3f8c2846553f1009fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff042801021903200282010004088203e88201f4881bc16d674ec8000089056bc75e2d631000008040018220001040102040102804822000837a1200843b9aca008302000083030000828000832000008401000000824e2082c350824e20830100008208008201008202008405b8d800824e208465ec8780824a388465ec607081a08180028401e187e087de0b6b3a7640008213888398968004288209c4a0acd7f9fa70897180ca0f80cedcde70dc080e6d23a576499abbc046a0e09b28b9f881ea9486a412863a3eb23f4b7a9ffbc69a42fd25dea73f94e590a831d626d1c20da9f3763c97a4cd7a3da9b1ea94eb59c60584bb0fb67132cc4a3f9639c31cdb4935943e381e57ba2c894cd43b25bf592754bd3451c219ea94fcd55660d527e67fabf17f27c4038a3799512ba094b9abf100add97b76683bf6e9ad4372875d897db1a0a1d91673b4b4a2c88ff75d4828dc1932dbaa2831284a66f4b1ea77b4c5e666a2",
  "endpoint": "https://gateway.pocol.net/studio/rpc",
  "trustLevel": "RN"
});
// PLACEHOLDER (decision D1): not a network. The built-in profile above is a spec-2 GenesisSpec made from the
// devnet spec fixture under a chain ID of no network, so the extension pins no retired identity before the new
// genesis exists. deploy/mainnet/set_network_identity.py replaces it with the network at the identity switch, and
// scripts/package-release.py refuses a build that carries it.
export const NETWORK_PLACEHOLDER = true;
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
