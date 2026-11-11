# Security policy

PoCol is experimental software. The public network is operator-designated **PoCol Mainnet**; that name is a
deployment designation, not a statement of maturity. The engine reports its experimental admission scope,
`specificationCompliant: false` and `publicLaunchReady: false`, and there has been no independent external
security review (see [Current status and limits](README.md#current-status-and-limits)). Do not hold more value
on it than you can afford to lose. Even so, we want to hear about every vulnerability, and we treat reports
seriously.

## How to report a vulnerability

**Report privately. Do not open a public issue, pull request or discussion for a vulnerability.**

Use GitHub's private vulnerability reporting for this repository:

1. Go to <https://github.com/raedrasheed/pocol/security>.
2. Choose **Report a vulnerability** (direct link:
   <https://github.com/raedrasheed/pocol/security/advisories/new>).
3. Fill in the advisory form. Only the maintainers and the people you add can see it.

GitHub's private vulnerability reporting is the only reporting channel. It works only while private
vulnerability reporting is enabled in the repository's settings (Settings → Code security → Private
vulnerability reporting); the maintainers keep it enabled. If the **Report a vulnerability** button is not
shown, it has been switched off. As a last resort, open a minimal public issue that says only that you have a
security report and asks the maintainers for a private contact. Put no details in it: no affected component,
no impact, no steps. Wait for the private contact before you send anything.

## What to include

- The affected component and version: the commit hash, or the `build-info.json` / `pocol build-info` output of
  a release package.
- The impact as you understand it. For example: a consensus split, a crash or stall of a node, a wrong
  validity verdict, key or fund loss, signature or transaction forgery, a peer-to-peer or RPC denial of
  service, or a wallet or extension flaw.
- Steps to reproduce: a minimal test, a script against a **local** devnet, or a sequence of messages or
  transactions. Say what you expected and what happened.
- Whether you believe the issue can be or has been exploited on the public network.
- How you would like to be credited, if at all.

## Scope

In scope: everything in this repository, in particular

- consensus and validation (`prototypes/pocold`, `prototypes/lp4-execution`, `prototypes/lp1-node`, the
  consensus-boundary sources and the vendored EVM crypto patches in `prototypes/third-party/evm-crypto`);
- the node: P2P networking, peer discovery, RPC, storage, mining and the miner registry programs;
- the wallets and their key handling: the Android app (`prototypes/pocol-android`), the Windows wallet
  (`prototypes/pocol-windows`), the `pocol` command, key files and the encrypted key backup
  (`prototypes/pocold/src/keyfile`);
- the browser extension and Studio (`prototypes/pocol-extension`, `prototypes/pocol-studio`);
- release packaging, network invitation and upgrade files, and the deployment scripts under `deploy/`.

Out of scope:

- denial of service that needs only volume (flooding a public seed or gateway) and no protocol or
  implementation flaw;
- findings that only restate the known open conditions in the [README](README.md#current-status-and-limits) or
  the review documents under `development/`, unless you show a concrete new impact;
- social engineering of maintainers or users, and physical attacks on a user's device;
- vulnerabilities in third-party services, operating systems or dependencies that do not affect PoCol in its
  shipped configuration. Report those upstream; tell us too if PoCol is affected.

## Testing rules

- Test on a **local devnet** (see the README and `deploy/standalone`) or on test keys you control. Do not attack
  the public network, its seeds or its gateway, and do not touch other people's accounts or funds.
- Do not publish exploit code, or details that would let someone exploit an issue, before a fix is available
  and we have agreed on disclosure.
- Never send us real private keys, key backups or their passphrases. Use test keys.

## What to expect

- **Acknowledgement:** within 3 business days of your report.
- **First assessment** (severity, affected versions and whether we can reproduce it): within 10 business days.
- **Updates:** at least every 14 days while the report is open.
- **Fix and disclosure:** we aim to fix critical and high-severity issues within 90 days of the report. We agree
  the disclosure date with you, and publish a GitHub security advisory with credit, unless you prefer to stay
  anonymous. A fix to consensus rules can need a coordinated network upgrade, which can take longer. We will
  tell you if it does.

These are targets, not guarantees: PoCol has a small team.

## Safe harbor

If you act in good faith under this policy, we consider your research authorized. We will not take legal action
against you, or ask anyone else to, for that research. Good faith means:

- you follow the testing rules above;
- you do not access, change or destroy data or funds that are not yours, beyond the minimum needed to show the
  issue;
- you do not harm the availability of the public network or other users;
- you report the issue to us privately and promptly, and give us reasonable time to fix it before any
  disclosure.

If someone else brings a legal claim over activity that followed this policy, we will make it known that your
research was authorized. If you are unsure whether something is allowed, ask in your private report first.

## Rewards

There is no bug bounty programme at the moment. A bounty may follow; if one starts, it will be announced in this
file with its terms. Until then we credit reporters in the advisory, unless they ask not to be named.

## Supported versions

Only the latest release, and the `main` branch, receive security fixes. The public network runs experimental
software: a fix can require a new release that every node operator installs, or a coordinated upgrade (see
`deploy/standalone/RELEASE-UPGRADE.md`).
