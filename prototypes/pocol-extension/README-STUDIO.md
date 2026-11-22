# PoCol Studio extension — 0.5.0

Built from the existing PoCol extension at `50c2f27ee67961dec7aaf0eb735c0b641810e0ea`, with a trusted browser publishing workflow and the original isolated viewer.

Version 0.5.0 is the extension for the fresh PoCol Mainnet (consensus v3): Studio publication, IDE contract publishing and RP verification, with the network's identity and fork schedule pinned by the release. The network is labeled PoCol Mainnet, and the Viewer wallet address belongs to the network (see below). Fees use **PCOL**. 0.5.0 has a fixed extension ID, so it does not replace 0.4.1: export the wallet, remove 0.4.1, then load 0.5.0. See `README-IDE.md` for the complete release, installation and upgrade instructions.

## Build and checks

```sh
npm ci --ignore-scripts
npm run build
npm test
npm run test:studio
npm run test:publisher
npm run test:contracts
npm run test:contract-ui
```

The build vendors the pinned Noble and ethers dependencies; it downloads no executable code at runtime. Load `dist/` with Chrome's Load unpacked developer option for this development release. Store publication has not been performed or approved. A release has a fixed extension ID: the manifest `key` is the owner's public key in `release/manifest-key.pub`, so every user and every later release has the same ID, `gggcoflmedlhlkcdhebmoepmggdnafmp`, the one published on pocol.net, and the saved vault stays with it. The Studio page still talks to the extension only through an origin-scoped content bridge; it does not need the ID. The published zip is signed with the owner's ssh-ed25519 release key (`deploy/release/README.md`); its `.sha256` is integrity only.

## Network

The built-in `pocol-public` profile pins chain ID `1599846715`, genesis hash `0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca`, and the complete GenesisSpec preimage. It connects only to `https://explorer.pocol.net/studio/rpc` using RN trust: trust the remote node for execution, independently verify site content hashes and the network identity. The relay must be deployed before live publishing/viewing through that profile works. Existing loopback profiles and the original local verification modes remain available.

## The Viewer wallet address

The Viewer's seed-derived wallet ("Local development wallet" in Settings) keys each profile's address with that
profile's network key (netKey, D14), so an address belongs to one network. It has no export, and Chrome deletes the
extension's storage when it is removed, so since 2026-11-22 it serves local networks only: the worker refuses it for
the built-in PoCol Mainnet profile and for any custom profile of the same network (no address, `eth_accounts` empty,
`eth_requestAccounts` and `eth_sendTransaction` refused with 4100). Releases up to 0.4.1 keyed the built-in profile with
a fixed salt (the retired network's netKey); nothing carries over from the retired network. The wallet of PoCol
Mainnet is the PoCol wallet (`README-EN.md`, "The PoCol wallet"), the exportable key Studio and IDE use.

## Existing Windows wallet

`identity.pocol-key` on Windows is DPAPI user-bound ciphertext. A browser cannot decrypt DPAPI directly. Run `tools/Export-Studio-Wallet.ps1` under the Windows account that owns the wallet:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\Export-Studio-Wallet.ps1
```

The script defaults to `%LOCALAPPDATA%\PoCol\identity.pocol-key` and creates `PoCol-Studio-Wallet.json` on the desktop. It asks for an export password twice, refuses to overwrite an existing output, and writes only authenticated encrypted JSON. Import that JSON in the extension's publication confirmation tab, never in the Studio website. The extension stores a PBKDF2-SHA256/AES-GCM encrypted copy; decrypted private bytes remain in the trusted tab's memory and lock after 15 minutes or on closing the tab. Desktop wallet and publisher retain the exact same key/address (no network derivation is applied to this imported key).

The export's PBKDF2-SHA256 + AES-256-CBC + HMAC-SHA256 format is tested against independent Node crypto. Actual Windows DPAPI execution needs verification on Windows; this build environment cannot run that step.

## Web page bridge

The only allowed origin is exactly `https://studio.pocol.net`. Only top-level documents may request these operations:

```js
window.postMessage({
  source: 'pocol-studio', id: crypto.randomUUID(),
  method: 'openPublisher',
  payload: {
    title: 'My website', entry: '/index.html', site: null,
    files: [{path: '/index.html', base64: '...'}]
  }
}, location.origin);
```

Responses use `{source:'pocol-extension', id, ok:true, result}` or `{source:'pocol-extension', id, ok:false, error}`. Methods:

- `status`: version, pinned network, capabilities, and up to 20 public job summaries belonging to the same origin and tab.
- `openPublisher`: validates public file bytes, creates a job, opens a trusted extension confirmation page; returns `{jobId}`. No signing occurs until the person unlocks their wallet, selects the consent checkbox and clicks Confirm & publish.
- `jobStatus`: `{jobId}` returns status/progress/result/error only; the same origin and originating tab must request it.
- `openSite`: `{site:'0x…',path:'/'}` opens the verified viewer for the pinned public profile.

There is no generic RPC, raw signing, private-key, wallet unlock, profile management, or endpoint-setting method in the public bridge. Content scripts are expressly denied the extension-page wallet APIs.

## Publication safety and recovery

The plan includes canonical chunks/manifest, an exact Website runtime, per-transaction fee caps and an aggregate maximum. Updates verify the account's actual `isPublisher(address)` authorization. Each signed transaction is saved before broadcasting. Ambiguous submission resumes the same raw transaction and never silently signs a new one at a replacement nonce. Receipts are checked against their canonical block before accepting/resuming.

Origin-wide Web Locks serialize each job and wallet account across publisher tabs; the embedded dApp wallet observes the same account lock. Jobs with unresolved signed transactions block subsequent jobs from that account and remain retained. After reloading, reopen the operation from extension Options → Publication history and approve a resume. Unsigned expired requests may be cleaned up; pending signed checkpoints are not expired. Cancelling signed work requires every transaction to have a current canonical receipt and explicit acknowledgement that stored files/fees remain.

The extension requests `unlimitedStorage` because a maximum-size publication plus its signed transaction checkpoints can exceed Chrome's normal local quota. Source files and signed transactions are public data; the wallet secret is separately encrypted. Do not send desktop-wallet transactions concurrently while a publication is active.

## Verified scope

- Original extension, fork-schedule and RP verification regressions (`tests/unit.mjs`): 338 passing checks.
- Studio integration, keyfile, pinned-schedule and release-version checks (`tests/studio_test.mjs`): 39 passing checks.
- Publishing engine (`tests/publisher_test.mjs`): 82 passing tests, including M1 vector parity, full mocked publish/fetch/update, wrong-network/ownership refusal, fee bounds, pending recovery (including chunks stored concurrently), reverted receipts and reorganization recovery. Cleanup shares the publisher job lock and rechecks stored state before deleting expired requests.

These are component tests with mocked Chrome/RPC where stated. No real on-chain signing, production RPC deployment or store review was performed by these tests.

Primary protocol source: https://github.com/raedrasheed/pocol/tree/72cf38f6cdb143aeb41431d2702317db1a0d85ba

## JavaScript modules in published sites

The viewer runs every script of a site from a `blob:` URL under a CSP that allows no network, so it resolves module imports itself. A static `import … from './x.js'` or `export … from '../y.mjs'` (a `./`, `../` or `/` path to a file of the site) is rewritten to that file's `blob:` URL, leaf modules first; JSON files load as modules (`with { type: 'json' }`) and CSS files as CSS module scripts. Every `import(…)`, also with a computed specifier, resolves when it runs, so a lazily loaded chunk may import back from the module that loaded it (the default code-split output of Vite and Rollup). A cycle of static imports cannot be expressed with `blob:` URLs and is refused, as are static imports nested more than 8 deep; bare names (`'react'`) and absolute URLs never load. Studio checks the module graph (`checkModuleGraph` in `src/lib/modscan.js`, copied byte for byte to Studio's `dist/modscan.js`) when a folder is imported or a file is replaced, and again before the publisher opens. A project saved before this check still opens, previews and exports, and its files panel lists the errors; only publishing it is refused. Warnings (a computed or external `import()`, `import.meta`) are shown on import and in the publish dialog, never refused. `import.meta.url` is the module's `blob:` URL, so URLs relative to it do not resolve.
