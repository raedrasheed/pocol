# Chrome Web Store listing: PoCol Studio, IDE & Viewer

English listing text and the answers for the Chrome Web Store developer dashboard. Every statement describes what the
extension in this directory does; `tests/store_listing_test.mjs` checks the parts that the code decides (the permission
list, the hosts, the summary length, the images and the privacy policy page). The owner reviews and submits it
(`store/README.md`); nothing here is uploaded automatically.

## Store listing tab

- **Name** (from the manifest): PoCol Studio, IDE & Viewer
- **Summary** (the manifest `description`, at most 132 characters): Publish websites and smart contracts with your PoCol
  wallet. Local approval, verified files and isolated viewing.
- **Category:** Developer Tools
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

PoCol Studio, IDE & Viewer is the browser companion of the PoCol network. It lets you publish websites and smart
contracts to PoCol with a wallet that stays on your device, and open websites published on PoCol in an isolated viewer.

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
  AES-256-GCM). When you create it, the extension saves an encrypted backup file to your computer.
- The wallet locks after 15 minutes. Web pages never receive your key; they can only ask the extension to open a
  confirmation window.

View PoCol websites safely
- Websites published on PoCol open in a sandboxed viewer that checks every file against the hashes recorded on the
  network. They have no access to your wallet or to the extension, and any signature they request needs your approval.

The extension connects only to the PoCol network gateway (https://explorer.pocol.net) or to a node on your own computer
that you add. It has no analytics and no advertising. It does not mine.

## Privacy practices tab

### Single purpose

Publish websites and smart contracts to the PoCol network with a wallet kept on the user's device, after the user's
approval, and view websites published on PoCol in an isolated viewer.

### Permission justifications

#### `storage`

Keeps the password-encrypted wallet, the network profiles the user adds and the progress of each publication (its
files and signed transactions until it completes) in the extension's local storage, so that an interrupted publication
can resume without signing again.

#### `unlimitedStorage`

A publication can be up to 4 MiB of website files plus its signed transactions; the checkpoint of a large publication
and its files must fit in local storage until the publication completes.

#### `declarativeNetRequest`

Adds session rules for the extension's own viewer tabs only, so that a website opened from the PoCol network cannot
load anything from the internet: its files come only from the verified on-chain copy.

#### Host permission `https://explorer.pocol.net/*`

The PoCol network gateway (`https://explorer.pocol.net/studio/rpc`): the extension reads the network's identity, fees,
the wallet's balance and transaction count, and sends the transactions the user approved.

#### Host permission `http://127.0.0.1/*`

A PoCol node on the user's own computer, used only when the user adds a local network profile in the Settings page.

#### Content scripts on `https://studio.pocol.net/*` and `https://ide.pocol.net/*`

A message bridge in the top frame of PoCol Studio and PoCol IDE only. It forwards the user's "publish" or "deploy"
request to the extension, which opens its own confirmation window. It cannot unlock, export or use the wallet and does
not read page content.

### Remote code

**No, I am not using remote code.** All of the extension's JavaScript is in the package (vendored libraries included);
there is no `eval`, `new Function` or remotely loaded script in extension pages, whose CSP is `script-src 'self'`.
Websites published on PoCol that the user opens in the viewer run only inside the manifest's sandboxed page
(`sandbox.html`, CSP `connect-src 'none'`), which has no extension API access; the Chrome Web Store's Manifest V3
requirements exempt code in sandboxed pages. The owner confirms this answer when submitting (store/README.md).

### Data usage

What the extension sends off the device, and where:

| Data type (dashboard category) | Collected? | What and why |
| --- | --- | --- |
| Personally identifiable information | No | |
| Health information | No | |
| Financial and payment information | Yes | The wallet address (to read its balance and transaction count) and the transactions the user approves, sent to the PoCol network gateway to carry out the user's request. |
| Authentication information | No | The wallet password and private keys are used only inside the extension and are never transmitted; the encrypted backup file is saved to the user's own computer. |
| Personal communications | No | |
| Location | No | The user's IP address reaches the gateway as with any request; the extension does not determine location. |
| Web history | No | |
| User activity | No | |
| Website content | Yes | The website or contract files the user chooses to publish, sent to the PoCol network (public and permanent once published). |

Certifications (all true for this extension):
- I do not sell or transfer user data to third parties, outside of the approved use cases.
- I do not use or transfer user data for purposes that are unrelated to my item's single purpose.
- I do not use or transfer user data to determine creditworthiness or for lending purposes.
