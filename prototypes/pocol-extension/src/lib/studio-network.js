// The built-in network that deploy/mainnet/set_network_identity.py pins: PoCol Mainnet, the public network, from the
// identity switch on. Public data only.
export const STUDIO_ORIGINS = Object.freeze(['https://studio.pocol.net']);
/*@fault testGateway*/export const NETWORK/*@else*/const PINNED_NETWORK/*@end*/ = Object.freeze({
  "name": "pocol-public",
  "chainId": 2656089850,
  "genesisHash": "0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e",
  "genesisPre": "0xf901bc02849e50b6faf8c4846b01fd289d331c14db73ff371e10c5b505558d8473ed9318cdd6f79aab68f276a6ae0a82025801025a08200282010004088203e88201f4883782dace9d9000008901158e460913d0000004822710018260001040102082016882016882017c04822000837a1200843b9aca00830200008303000082800084400000008401000000824e20833d0900830f4240830240008210008201008202008405b8d8008080808082010081800283c09cc08801bc16d674ec8000821388839896800482017c8209c4a03e8d8110695bcc118f6c63032c868154d596aa2651ed9a78bd1fd4bcaa8295fff8acea9415df38aaba05b83fb8c58d2dd9d804654278a96f9415df38aaba05b83fb8c58d2dd9d804654278a96fea944479d23cb5580b912a8402230071b8a7f0a701ec94bf335311d1a3ffc8c0efd1dc422f727f48e6de11ea94c319eeced588afe03515508bab5f27805534b3079456bd8b1a42a6a3b09c04ebdd91e6fa3cc944b6bdea94e45343ea8d6d54fb693a6ad424fac11d85624b6e944ce8b7625efc6898e59d7436c3a5a7569e91b3fca0a1d91673b4b4a2c88ff75d4828dc1932dbaa2831284a66f4b1ea77b4c5e666a2",
  "endpoint": "https://explorer.pocol.net/studio/rpc",
  "trustLevel": "RN"
});
// Sites W11 test build only (build.mjs `testGateway`, a "[TEST FAULT testGateway]" build in dist-fault-testGateway/,
// refused by scripts/package-release.py): the built-in RN profile is the devnet given to the build, served by a local
// Studio gateway at http://127.0.0.1:<port>/studio/rpc, so the release RN path (validateEndpoint's exact endpoint
// match) runs against a devnet. The release keeps the pinned NETWORK above.
/*@fault testGateway*//*@else*/export const NETWORK = Object.freeze({ ...PINNED_NETWORK, .../*@testGatewayNetwork*/null });
/*@end*/export function validateEndpoint(endpoint, trustLevel) {
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
