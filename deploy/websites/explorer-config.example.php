<?php
// Template for explorer.pocol.net/api/config.php, the host-only operator file (never committed: .gitignore ignores
// **/api/config.php, deploy/websites/tools/site_identity.py refuses one in a snapshot, and api/.htaccess denies it).
// It is kept outside the document root, so it is neither in SHA256SUMS.txt nor uploaded. The keys are the ones the
// tracked readers use: api/rpc.php (site_origin), api/lib.php pocol_forward (gateway_url), api/health.php,
// api/miner-locations.php and api/transactions.php (chain_id, genesis_hash). The values below are placeholders;
// deploy/websites/tests/explorer_smoke.mjs writes a test config.php from this file for a local devnet only.
return [
    // The explorer gateway's /rpc (deploy/rewards/gateway.py) the PHP relay forwards to: http(s), no credentials or fragment.
    'gateway_url' => 'http://127.0.0.1:8088/rpc',
    // The only Origin api/rpc.php accepts on a browser request.
    'site_origin' => 'https://explorer.pocol.net',
    // The network the gateway must report: the chain ID as an integer, the genesis hash in lowercase hex.
    'chain_id' => 0,
    'genesis_hash' => '0x0000000000000000000000000000000000000000000000000000000000000000',
];
