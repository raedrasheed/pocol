# PoCol Studio, IDE & Viewer 0.4.0

Version 0.4.0 combines Studio website publication, IDE contract deployment, the supplied PoCol artwork, and support for an explicitly configured full 64-bit mining range. Network labels use PoCol Mainnet. Existing wallet storage, addresses, chain ID and genesis remain unchanged. Studio confirmation defaults to English, with Arabic available from the language control.

The 0.3 series adds contract deployment and contract writes from PoCol IDE using the **same PoCol wallet address** as Studio and the native Windows/Linux wallet. No MetaMask account is needed. Websites send public contract intents; private keys and passwords are accepted only in the extension's trusted confirmation tab.

## Install or update

This is a development unpacked Chrome extension, not a Chrome Web Store release.

1. Keep your existing encrypted wallet export and password available.
2. To update an existing unpacked installation, replace its application files in the **same already-loaded extension folder**, then open `chrome://extensions` and click **Reload** on that existing extension. **Do not click Remove or load the update from a different folder:** this can change Chrome's extension ID/storage and make the saved wallet unavailable. Keep that extension installed to retain its encrypted Studio vault. Do not delete native wallet files as part of an update.
3. For a first installation, extract this package, enable Chrome Developer mode and choose **Load unpacked** on the folder containing `manifest.json`.
4. Reload PoCol IDE or Studio after updating the extension.

A new unpacked folder can have a different extension ID/storage. In that case import the same encrypted wallet export in the confirmation tab; this preserves the original address. The extension never replaces the native wallet identity.

## Deploy a contract

1. Write and compile Solidity in `https://ide.pocol.net` (or the owner preview).
2. Select PoCol Mainnet and choose **Review in PoCol Wallet**.
3. In the extension confirmation tab, check the originating site and requested contract.
4. Unlock the saved Studio wallet. Windows users may instead import the encrypted JSON produced by `tools/Export-Studio-Wallet.ps1`, running that script under the Windows account that owns the native wallet. Linux users should create the encrypted JSON with `tools/export-studio-wallet.py` using the instructions below, then select that JSON in the same confirmation tab. No raw key file is created or uploaded by this workflow.
5. Click **Prepare transaction review**. Check the signing address, target or expected deployment address, bytecode/calldata fingerprints, value, and maximum PCOL fee.
6. Select the consent checkbox and click **Confirm & sign transaction**. Keep the confirmation tab open while it checks the receipt.

The original imported key is used directly. The older Viewer seed-derived wallet is a separate mechanism and is not used for IDE or Studio publication.

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

RP verification uses only the saved local `forkSchedule`: omitted means version 1; `[[1,"0"],[2,"<activation height>"]]` enables version 2 at the exact locally configured height. RPC responses cannot choose or override that schedule. The full nonce range is 0 through 18,446,744,073,709,551,615, inclusive. Historical version-1 headers retain their original range. Changing a profile invalidates active signing sessions; conflicting network identities or schedules are read-only. The built-in RN profile continues to trust the pinned remote node for consensus and execution, and this package alone does not activate a network upgrade.

To configure the reserved `pocol-public` profile, open the extension's **Options → PoCol Mainnet activation**, paste the exact operator-approved schedule, then click **Save trusted schedule**. For example, if the operator approves height H, replace `ACTIVATION_HEIGHT` with H in `[[1,"0"],[2,"ACTIVATION_HEIGHT"]]`; keep both heights quoted to preserve all 64 bits. Reopen Viewer tabs after saving. Use the same schedule for any custom local profile of this chain. Saving `[[1,"0"]]` explicitly restores version-1-only configuration. The control changes only the locally trusted schedule, never the endpoint, chain ID, genesis, encrypted wallet, or address. Website content scripts and RPC responses cannot call this settings-only operation. No activation height is preselected in this release; coordinate the native-node upgrade separately.

Contract intents are limited to 256 KiB, ABI to 512 entries/96 KiB, arguments to 64 KiB, encoded creation bytecode to the Cancun 49,152-byte limit, and call data to 58,000 bytes. EIP-1559 transactions use at most 8,000,000 gas and 200 gwei maximum fee per gas. New requests never choose another network or supply an arbitrary endpoint.

## Source and verification

Baseline: PoCol Studio extension 0.2.1 from `palcertcom/pocol`, branch `work/pocol-studio`, commit `93d7f39fbe1f34890bbe1685a0f95309fc1cb7a5`. The published 0.3.1 IDE and icon release is reconciled with the full-range verifier in 0.4.0; wallet storage is not migrated.

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

Component checks use mocked Chrome and RPC plus a public test key. They do not access a user's wallet or send live transactions. Real Chrome installation, native wallet file interaction and end-to-end on-chain signing still require device verification.
