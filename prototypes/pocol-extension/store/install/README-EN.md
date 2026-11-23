# PoCol wallet for Chrome

This is the Chrome Web Store version of the PoCol extension: a wallet for the PoCol blockchain. It holds one PoCol
account per browser profile, shows the network status, the address and its balance, sends PCOL after you review and
confirm each transfer, and signs the transactions of PoCol Studio (website publication, `https://studio.pocol.net`) and
PoCol IDE (contract deployment, `https://ide.pocol.net`) in its own confirmation windows. It does not mine.

## Install and start

1. Install it from its Chrome Web Store listing. Check that the publisher and the link come from `https://pocol.net`.
2. Open the PoCol wallet from the toolbar. Before its first network request it shows what it sends where; read it and agree.
3. In Settings, "PoCol wallet", create a wallet (its encrypted backup is downloaded at once) or import one: an encrypted
   PoCol wallet file, a private key, or a BIP-39 English recovery phrase (its first account, `m/44'/60'/0'/0/0`).

## Keep your wallet

The wallet lives only in this browser profile, encrypted with your password. Chrome deletes it when the extension is
removed, and the Web Store item is a separate installation from any side-loaded copy, with its own empty storage.
Before you remove the extension, move to another browser, or move from a side-loaded copy to this one, download the
encrypted backup (Settings, "PoCol wallet", "Download encrypted backup") and keep it with its password. Import it in the new installation
under "An encrypted PoCol wallet file". Nobody can recover a wallet whose backup or password is lost.

A Studio or IDE window never replaces the saved wallet: a wallet file of another address chosen there is used in that
tab only. Only Settings replaces the saved wallet, after you confirm that you backed it up.

## Native wallet users

To use the address of the PoCol Windows or Linux wallet, export it once as an encrypted wallet file with the export
helper that comes with the signed native release (`Export-Studio-Wallet.ps1` on Windows, `export-studio-wallet.py` on
Linux), and import that file. The Web Store version does not include these helpers.

## Privacy and source

The privacy policy is `https://pocol.net/privacy.html`. The source code is at `https://github.com/raedrasheed/pocol`
(`prototypes/pocol-extension`). Licence: Apache-2.0 (`LICENSE`, `NOTICE`).
