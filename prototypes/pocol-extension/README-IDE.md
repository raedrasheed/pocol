# PoCol Studio, IDE & Viewer 0.5.0

Version 0.5.0 is the extension for the fresh PoCol Mainnet (consensus v3). It combines Studio website publication, IDE contract deployment and the verified Viewer. Network labels use PoCol Mainnet, and Studio and IDE keep their addresses, `https://studio.pocol.net` and `https://ide.pocol.net`. The release pins the network's identity and its fork schedule; there is no activation setting. The Viewer wallet address belongs to the network (see `README-STUDIO.md`). Studio confirmation defaults to English, with Arabic available from the language control.

Contract deployment and contract writes from PoCol IDE use the **same PoCol wallet address** as Studio and the native Windows/Linux wallet. No MetaMask account is needed. Websites send public contract intents; private keys and passwords are accepted only in the extension's trusted confirmation tab.

## Upgrade from 0.4.1 or earlier

This is a development unpacked Chrome extension, not a Chrome Web Store release. Release 0.5.0 has a fixed extension ID, taken from its manifest key, so Chrome treats it as a different extension from 0.4.x: it does not replace 0.4.1, and its storage starts empty. If both stay installed, both run on Studio and IDE. Upgrade in this order:

1. Keep the encrypted wallet export of your native wallet and its password: the `PoCol-Studio-Wallet.json` that `tools/Export-Studio-Wallet.ps1` (Windows) or `tools/export-studio-wallet.py` (Linux, see below) creates. The Studio vault that 0.4.1 saved stays in 0.4.1's storage and is not carried over. The Viewer's seed wallet cannot be exported; its address belongs to the retired network, so nothing carries over from it.
2. Open `chrome://extensions` and click **Remove** on PoCol Studio, IDE & Viewer 0.4.1 (or older). This removes only the extension and its copy of the vault. Do not delete native wallet files.
3. Extract the 0.5.0 package and compare its SHA-256 with the one published with the release. Enable Developer mode, choose **Load unpacked** on the folder containing `manifest.json`, and check that the extension ID Chrome shows is the one published on pocol.net.
4. Reload PoCol IDE or Studio. In the confirmation tab of your first operation, import the wallet JSON. The Studio and IDE address is unchanged, because the imported key is used directly.

A first installation is steps 3 and 4. Later releases keep the same extension ID: to update one, replace its application files in the already-loaded folder and click **Reload**. This keeps the saved vault. The extension never replaces the native wallet identity.

## Deploy a contract

1. Write and compile Solidity in `https://ide.pocol.net`.
2. Select PoCol Mainnet and choose **Review in PoCol Wallet**.
3. In the extension confirmation tab, check the originating site and requested contract.
4. Unlock the saved Studio wallet. Windows users may instead import the encrypted JSON produced by `tools/Export-Studio-Wallet.ps1`, running that script under the Windows account that owns the native wallet. Linux users should create the encrypted JSON with `tools/export-studio-wallet.py` using the instructions below, then select that JSON in the same confirmation tab. No raw key file is created or uploaded by this workflow.
5. Click **Prepare transaction review**. Check the signing address, target or expected deployment address, bytecode/calldata fingerprints, value, and maximum PCOL fee.
6. Select the consent checkbox and click **Confirm & sign transaction**. Keep the confirmation tab open while it checks the receipt.

The original imported key is used directly. The Viewer's seed-derived wallet is a separate mechanism and is not used for IDE or Studio publication; its address belongs to each network (see `README-STUDIO.md`).

## Linux encrypted export

Linux native wallet keys are raw 32-byte files protected by Unix ownership and permissions; they are not Windows DPAPI files. Use the Linux helper under the **same user that owns the existing key**, without changing its ownership or permissions. It requires Python 3 and the `cryptography` package (Debian/Ubuntu package: `python3-cryptography`).

Replace both example paths with the actual existing key and a new writable output file:

```sh
python3 tools/export-studio-wallet.py --key-path /explicit/path/identity.pocol-key --output /writable/path/PoCol-Studio-Wallet.json
```

Enter an export password of at least ten characters twice in the terminal. Passwords are not accepted in command-line arguments, environment variables or pipes. The helper verifies that the source is a regular, single-link file owned by the current user, with mode `0600` or `0400`, and a valid 32-byte key. It refuses symbolic links and existing output files; it does not alter the source. The new JSON is authenticated, password-encrypted and created with mode `0600`.

Select the resulting JSON **only in the extension confirmation tab**. The extension importer verifies the export and retains the exact original wallet address. Do not run the Windows PowerShell DPAPI exporter on a Linux key. Existing raw-key import compatibility remains available inside the trusted tab, but encrypted export is the recommended transfer path.

## Approval and recovery

- Only the exact IDE origins and top-level pages can open contract requests. Jobs/results are bound to their originating tab and origin.
- Supported public methods are `status`, `openContract`, and `jobStatus`. There is no public wallet unlock/export, arbitrary RPC, remote endpoint change, or raw-signing method.
- The extension independently encodes the ABI arguments, checks the pinned chain/genesis, estimates gas, checks the pending nonce and balance, and prepares an expiring two-minute fee review.
- A transaction is signed only after explicit consent. The exact signed transaction is durably stored before any broadcast. A storage failure blocks broadcasting.
- A timeout or connection loss retains the transaction hash and signed checkpoint. **Resume saved transaction** checks the receipt or retransmits those exact bytes; it never silently signs a replacement at another nonce.
- Pending receipts are checked read-only while the tab is open. Inclusion is accepted only if the receipt's block is currently canonical; inclusion is not irreversible finality.
- Studio, IDE and the embedded Viewer share account coordination locks. An unresolved signed job prevents another operation using that address. Open the linked earlier operation or Extension Options history to resolve it.
- Keep native desktop-wallet transactions idle while a browser operation is pending; a browser lock cannot control a separate native application.
- Decrypted wallet bytes remain in the trusted tab's memory, lock after fifteen minutes, and are cleared on locking/page exit. The saved vault is PBKDF2-SHA256/AES-GCM encrypted under the same `studio:wallet` storage key as Studio.

## Network and limits

PoCol Mainnet / pocol-public: chain ID `1599846715`, genesis `0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca`, RPC `https://gateway.pocol.net/studio/rpc`, native currency **PCOL** (18 decimals). Execution and receipts use remote-node trust.

The built-in profile's fork schedule is fixed by this release: `[[1,"0"]]`, version 1 from genesis, as the fresh network's genesis records it. There is no setting for it, and RPC responses cannot choose or override it. A later cap raise (fork version 3) arrives as an extension update, together with the node release. A custom local profile uses the schedule its network operator publishes: omitted means `[[1,"0"]]`, and after a cap raise it is `[[1,"0"],[2,"H2"],[3,"H3"]]` with the operator's heights as quoted decimal strings (version 2 changes no rule; it keeps the versions in order). Until that extension update, a custom RP profile of the PoCol Mainnet chain with the cap-raise schedule conflicts with the built-in profile, and conflicting profiles are read-only: only Viewer dApp signing is blocked on them. Changing a profile invalidates active signing sessions. The built-in RN profile trusts the pinned remote node for consensus and execution.

The release GenesisSpec is a spec-2 (consensus v3) genesis with nonceMode 1, the only kind a node admits: the full nonce range, 0 through 18,446,744,073,709,551,615 inclusive, holds from genesis. This build decodes it at profile and RP decoding with the final CP schema v2 (59 fields), exactly as the release node does. A spec-1 genesis still decodes for compatibility, but no node admits it, and the extension refuses it as a profile with the node's reason, as it refuses a nonceMode other than 1. The decoder and the RP rules are tested against the node's reference decoders (`tests/decoder_diff.mjs`, `tests/v3_decoder_test.mjs`, one verdict file per corpus regenerated from Rust). On a v3 window RP checks the same header items and states that the v3 membership, fork-choice, evidence and reward rules are not evaluated by RP.

Contract intents are limited to 256 KiB, ABI to 512 entries/96 KiB, arguments to 64 KiB, encoded creation bytecode to the Cancun 49,152-byte limit, and call data to 58,000 bytes. EIP-1559 transactions use at most 8,000,000 gas and 200 gwei maximum fee per gas. New requests never choose another network or supply an arbitrary endpoint.

## Source and verification

Baseline: PoCol Studio extension 0.2.1 from `palcertcom/pocol`, branch `work/pocol-studio`, commit `93d7f39fbe1f34890bbe1685a0f95309fc1cb7a5`. The 0.3.1 IDE and icon release and the 0.4.x verifier are carried into 0.5.0; wallet storage is not migrated (see the upgrade above).

Pinned dependencies: Noble hashes 1.8.0, Noble secp256k1 2.3.0, ethers 6.17.0. Dependencies are bundled locally; there are no runtime CDN imports.

```sh
npm ci --ignore-scripts
npm run build
npm test
npm run test:studio
npm run test:publisher
npm run test:contracts
npm run test:contract-ui
python3 -m unittest tests/test_linux_export.py -v
```

`npm run build` makes `dist/`. It carries the manifest key, and with it the fixed extension ID, once the owner's public key is committed in `release/manifest-key.pub`; until then it is a development build, and `npm run package` refuses to package a release. `python3 scripts/package-release.py --extension-id` prints the ID of the committed key. Test-fault builds never carry the key. The source tree keeps `package.json` at 0.4.1 until the signed 0.5.0 release: the version and the website download links move together, because `deploy/websites/tools/site_identity.py --check` requires every download link to name the `package.json` version.

Component checks use mocked Chrome and RPC plus a public test key. They do not access a user's wallet or send live transactions. Real Chrome installation, native wallet file interaction and end-to-end on-chain signing still require device verification.
