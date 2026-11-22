# Chrome Web Store listing: PoCol Studio, IDE & Viewer

English listing text and the answers for the Chrome Web Store developer dashboard. Every statement describes what the
extension in this directory does; `tests/store_listing_test.mjs` checks the parts that the code decides (the permission
list, the hosts, the summary length, the images and the privacy policy page). The owner reviews and submits it
(`store/README.md`); nothing here is uploaded automatically.

## Store listing tab

- **Name** (from the manifest): PoCol Studio, IDE & Viewer
- **Summary** (the manifest `description`, at most 132 characters): PoCol wallet: hold and send PCOL, publish websites
  and contracts from PoCol Studio and IDE. Keys stay on your device; you approve.
- **OWNER DECISION, the item name:** the manifest name above is the side-loaded release's
  (`scripts/package-release.py` RELEASE_NAME, `store/screenshots/RENDERED-FROM.json`). The single purpose below is a
  wallet; "PoCol Wallet" (or "PoCol Wallet: Studio, IDE & Viewer") would say so. A rename changes `build.mjs`,
  RELEASE_NAME and the screenshots together; this repository does not choose it.
- **Category:** Developer Tools (or Productivity; an owner choice: the store has no wallet category)
- **Language:** English
- **Store icon:** `store/icon-128.png` (128x128; the extension's artwork at 96x96 with 16 px transparent padding)
- **Screenshots:** `store/screenshots/01-settings.png`, `02-studio-publisher.png`, `03-studio-wallet.png`,
  `04-ide-contract.png` (1280x800). Upload only renders of the release build: `store/screenshots/RENDERED-FROM.json`
  must say `"placeholderBuild": false` (store/README.md, step 4).
- **Small promo tile:** `store/promo-small-440x280.png` (440x280)
- **Official URL / homepage:** https://pocol.net
- **Support URL:** https://github.com/raedrasheed/pocol/issues
- **Privacy policy URL:** https://pocol.net/privacy.html

### Description

PoCol Studio, IDE & Viewer is the PoCol wallet for Chrome: a wallet for PoCol Mainnet that stays on your device. Hold
and send PCOL, publish websites and smart contracts from PoCol Studio and PoCol IDE with the same wallet, and open
websites published on PoCol in an isolated viewer. Nothing is signed without your approval.

Your PoCol wallet
- Create a wallet, or import one from an encrypted PoCol wallet file, a private key or a recovery phrase (BIP-39,
  first account m/44'/60'/0'/0/0; the phrase itself is not stored).
- The toolbar button shows the network, whether the extension is connected to it (the chain ID and the genesis are
  checked against the network built into the extension, with the latest block and its age), whether the wallet is
  locked, your address and your balance.
- Send PCOL: a confirmation page shows the network, the chain ID, the recipient, the amount, the fee and the total,
  and the transfer is signed in the extension only after you tick the consent box and confirm. It is refused if the
  network does not answer with the expected chain ID and genesis.
- The transfers you sent from this extension are listed with their status (pending, included, failed, dropped,
  rejected, or unknown while the network's answer is unclear); an included transfer is checked again until its block is
  64 blocks deep. On PoCol Mainnet the wallet links to the address on explorer.pocol.net, which shows its balance and
  the activity of recent blocks. Neither is a complete history of the address.
- Back up the wallet at any time (after entering its password): an encrypted file that you can import again, also in
  another browser or after reinstalling.

Publish a website from PoCol Studio
- Design your site at https://studio.pocol.net and choose Publish.
- The extension opens its own confirmation window with the files, their total size, the number of transactions and
  the fee budget read from the network.
- Nothing is signed until you unlock your wallet, tick the consent box and confirm. You approve one fee budget and one
  per-gas ceiling; if the network's fees rise past them, the publication pauses and asks again.
- An interrupted publication resumes from where it stopped, with the same signed transactions.

Deploy a contract from PoCol IDE
- PoCol IDE at https://ide.pocol.net sends the compiled contract to the extension, which shows the operation, the
  destination and the fee before you approve the transaction.

Your keys stay with you
- The wallet is created in your browser and stored encrypted with your password (PBKDF2-SHA256, 600,000 iterations,
  AES-256-GCM). When you create it, the extension saves an encrypted backup file to your computer; Settings makes a
  new one whenever you ask.
- The wallet locks 15 minutes after you unlock it. While a publication or contract job you approved is running, it
  stays unlocked until that job's deadline, shown in the approval (2 hours by default, at most 6 hours); Lock ends it
  at once. Web pages never receive your key; they can only ask the extension to open a confirmation window.

View PoCol websites safely
- Websites published on PoCol open in a sandboxed viewer that checks every file against the hashes recorded on the
  network. They have no access to your wallet or to the extension. On PoCol Mainnet this version gives them no account
  and no signature: such a request is refused without a dialog. Only on a local development network that you add can a
  site ask for the viewer's local wallet, and then only with your approval.

The extension connects only to the PoCol network gateway (https://explorer.pocol.net) or to a node on your own computer
that you add. It has no analytics and no advertising. It does not mine.

## Privacy practices tab

### Single purpose

A wallet for the PoCol blockchain: it keeps the user's PoCol key on the user's device and uses it only after the
user's approval, to send PCOL and to publish websites and contracts from PoCol Studio and PoCol IDE; it also opens the
websites published on PoCol in an isolated viewer that verifies them against the chain. On PoCol Mainnet the viewer
gives websites no account and no signature in this version (`sw.js` seedWalletAllowed); only a local development
network the user adds can use the viewer's local wallet, after the user's approval. Every function holds the user's
PoCol key, asks the user to approve a PoCol transaction, or shows what the PoCol chain holds.

### Permission justifications

#### `storage`

Keeps the password-encrypted wallet, the network profiles the user adds, the list of transfers the user sent from the
extension (hash, recipient, amount, status) and the progress of each publication (its files and signed transactions
until it completes) in the extension's local storage, so that an interrupted publication can resume without signing
again.

#### `unlimitedStorage`

A publication can be up to 4 MiB of website files plus its signed transactions; the checkpoint of a large publication
and its files must fit in local storage until the publication completes.

#### `declarativeNetRequest`

Adds session rules for the extension's own viewer tabs only, so that a website opened from the PoCol network cannot
load anything from the internet: its files come only from the verified on-chain copy.

#### Host permission `https://explorer.pocol.net/*`

The PoCol network gateway (`https://explorer.pocol.net/studio/rpc`): the extension reads the network's identity and
latest block, fees, the wallet's balance and transaction count and the receipts of the transfers it sent, and sends the
transactions the user approved. The wallet's balance, Send and transaction status use this host and no other; the
explorer link opens explorer.pocol.net in a tab only when the user clicks it.

#### Host permission `http://127.0.0.1/*`

A PoCol node on the user's own computer, used only when the user adds a local network profile in the Settings page
(the wallet then reads and sends through that node). A move to `optional_host_permissions`, requested when the user adds
such a profile, would narrow it further; it is not implemented (`tests/store_listing_test.mjs` checks the manifest as
it is).

#### Content scripts on `https://studio.pocol.net/*` and `https://ide.pocol.net/*`

A message bridge in the top frame of PoCol Studio and PoCol IDE only. It forwards the user's "publish" or "deploy"
request to the extension, which opens its own confirmation window. It cannot unlock, export or use the wallet and does
not read page content.

### Remote code

**Answer: Yes, I am using remote code**, with the justification below. The Chrome Web Store program policies (Manifest
V3 requirements, read 2026-11-22) exempt code in sandboxed pages from the remote-code restriction but say it "must
still be possible to determine the full functionality" of the extension, and the dashboard rejects remote code that is
not declared; declaring it is never a violation, while "No" with the viewer running site code risks a "Blue Argon"
rejection. A longer review is expected. The facts the answer rests on:

- All of the extension's own JavaScript is in the package, vendored libraries included: `vendor/ethers.js` and
  `vendor/secp256k1/index.js` are byte-identical to their npm files; the `vendor/hashes/` modules of @noble/hashes
  1.8.0 are its `esm/` files with only the `@noble/hashes/crypto` import made relative (`./crypto.js`) and the
  source-map comment removed (`build.mjs`). Extension pages have no `eval`, `new Function` or remotely loaded script, and their CSP is
  `script-src 'self'`.
- `vendor/ethers.js` is ethers 6.17.0's `dist/ethers.min.js` unchanged (SHA-256
  `b016b0c3898c78fd8156466eb1ff1f42c9df951c2f0d64c9bdf799fe745b0a6c`, from https://registry.npmjs.org/ethers/-/ethers-6.17.0.tgz;
  `tests/wallet_pages_test.mjs` checks it). The extension uses its ABI, transaction, address, hashing and BIP-39
  helpers; it makes no request on import, and no extension code uses its network providers.
- The viewer DOES run JavaScript that is not in the package: the code of a website published on PoCol, which the user
  opens. It runs only inside the manifest's sandboxed page (`sandbox.html`, CSP `connect-src 'none'`, script only from
  the loader's hash and `blob:` URLs of files verified against their on-chain hashes), which has no extension API
  access; per-tab declarativeNetRequest rules block its network access.

Justification for the dashboard: "User-opened websites published on the PoCol blockchain run only in the manifest
sandbox page (no extension APIs, CSP connect-src 'none', network blocked per tab by declarativeNetRequest). Their only
channel is a postMessage bridge to the service worker that checks every message; any signature needs the user's
approval in an extension window. All extension logic is in the package."

Test instructions (dashboard, Test instructions tab): open the toolbar popup and accept the network disclosure; in
Settings, create a wallet (it downloads an encrypted backup) and make a backup with the password; the popup shows the
network status and the balance (0 PCOL for a new wallet; Send then refuses for lack of funds). Open
https://studio.pocol.net, publish a sample site and see the extension's confirmation window (nothing is signed without
the consent box and the wallet's password). A published site opens from the omnibox (`pocol pocol-public/<address>`)
in the viewer; it runs isolated and cannot reach the network or the wallet.

If the review objects to the viewer, the fallback is a store build without it (no sandbox page, viewer, omnibox or
declarativeNetRequest, so the answer becomes "No"); that variant is not built or tested here.

### Data usage

What the extension sends off the device, and where:

| Data type (dashboard category) | Collected? | What and why |
| --- | --- | --- |
| Personally identifiable information | No | |
| Health information | No | |
| Financial and payment information | Yes | The wallet address (to read its balance and transaction count), the transactions the user approves and the hashes of the transfers the user sent (to read their receipts), sent to the PoCol network gateway to carry out the user's request. The list of sent transfers stays on the device. |
| Authentication information | No | The wallet password, private keys and an imported recovery phrase are used only inside the extension and are never transmitted (the phrase is not stored); the encrypted backup file is saved to the user's own computer. |
| Personal communications | No | |
| Location | No | The user's IP address reaches the gateway as with any request; the extension does not determine location. |
| Web history | No | |
| User activity | No | |
| Website content | Yes | The website or contract files the user chooses to publish, sent to the PoCol network (public and permanent once published). |

Certifications (all true for this extension):
- I do not sell or transfer user data to third parties, outside of the approved use cases.
- I do not use or transfer user data for purposes that are unrelated to my item's single purpose.
- I do not use or transfer user data to determine creditworthiness or for lending purposes.

Limited Use: the privacy policy (https://pocol.net/privacy.html, section "Chrome Web Store User Data Policy") states
that the extension's use of user data complies with the Chrome Web Store User Data Policy, including the Limited Use
requirements. The prominent disclosure and consent happen in the extension: before the toolbar wallet's first network
request (network status, balance, Send, transfer status), the popup (or Settings) shows what is sent where and requires
"I understand and agree" (`sw.js` requireDisclosure in netStatus, accountBalance, prepareTransfer and refreshHistory).
Studio publications and IDE contract operations are not behind that agreement: each is a request the user makes on the
site, and it opens the extension's own confirmation window, which reads only the network's identity and fees (and, for
a saved operation, the receipts of its own transactions) until the user unlocks the wallet there and starts the
operation (Publish, or Prepare transaction review); the wallet address and the files are sent only then. The viewer
reads the website the user opens.
