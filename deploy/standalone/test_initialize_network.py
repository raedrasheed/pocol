#!/usr/bin/env python3
"""FN-7 (FRESH-NETWORK-DESIGN C8): Initialize-PoColNetwork.ps1 fills the committed v3 profile that pocold init-testnet
accepts, instead of building a spec-1 config.

These are text checks of the script against the profiles (deploy/mainnet), testnet.rs's Config, readiness.rs's
blocker ids and expected admissions, the profile code's public network chains, install-linux.sh and FRESH-NETWORK-CP.md,
so they run on any host. RealRunExpectations also admits a ceremony-filled profile with this tree's release pocold
(POCOLD_BIN_DIR, default prototypes/pocold/target/release, as test_launcher.py) and checks the script's real-run
expectations against its summary, since a real run needs the servers. The script itself runs, with the real pocold, in
test_initialize_network.ps1 (its -DryRun into a temporary package, then the refusals) on the Windows job of
five-device-build.yml.
"""
import json
import os
import re
import subprocess
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SCRIPT = (HERE / 'Initialize-PoColNetwork.ps1').read_text(encoding='utf-8')
MAINNET = ROOT / 'deploy/mainnet'
TESTNET = (ROOT / 'prototypes/pocold/src/testnet.rs').read_text(encoding='utf-8')
READINESS = (ROOT / 'prototypes/pocold/src/readiness.rs').read_text(encoding='utf-8')
RECORD = (ROOT / 'development/decisions/FRESH-NETWORK-CP.md').read_text(encoding='utf-8')
PROFILE_CODE = (ROOT / 'prototypes/lp4-execution/src/p1_profile.rs').read_text(encoding='utf-8')
BIN = Path(os.environ.get('POCOLD_BIN_DIR', ROOT / 'prototypes/pocold/target/release'))
EXE = '.exe' if os.name == 'nt' else ''


def placeholders(value, path=''):
    if isinstance(value, str):
        return [path or '/'] if value.startswith('PLACEHOLDER') else []
    if isinstance(value, list):
        return [p for i, v in enumerate(value) for p in placeholders(v, f'{path}/{i}')]
    if isinstance(value, dict):
        return [p for k, v in value.items() for p in placeholders(v, f'{path}/{k}')]
    return []


def pointer(value, path):
    for part in path.strip('/').split('/'):
        value = value[int(part)] if part.isdigit() else value[part]
    return value


def ps_list(name):
    """The quoted strings of a `$Name = @(...)` list in the script."""
    body = re.search(r'^\$' + name + r' = @\((.*?)\)$', SCRIPT, re.M | re.S)
    assert body, name
    return re.findall(r"'([^']*)'", body.group(1))


def ps_chains(name):
    """The chain ids of a `$Name = @([uint64]N, ...)` list in the script."""
    body = re.search(r'^\$' + name + r' = @\((.*?)\)$', SCRIPT, re.M | re.S)
    assert body, name
    return [int(n) for n in re.findall(r'\[uint64\](\d+)', body.group(1))]


def public_network_chains():
    """PUBLIC_NETWORK_CHAINS of the profile code (R22)."""
    lists = re.findall(r'^pub const PUBLIC_NETWORK_CHAINS: &\[u64\] = &\[([0-9, ]*)\];$', PROFILE_CODE, re.M)
    assert len(lists) == 1, lists
    return [int(n) for n in lists[0].replace(' ', '').split(',') if n]


def public_mainnet_items():
    """readiness.rs ExpectedAdmission::PUBLIC_MAINNET as technical-readiness ids: each condition, then each waiver."""
    m = re.search(r'pub const PUBLIC_MAINNET:Self=Self\{scope:Scope::PublicNetwork,\s*conditions:&\[(.*?)\],\s*waivers:&\[(.*?)\],'
                  r'profile_kind:ProfileKind::Network,node_kind:NodeKind::Node\};', READINESS, re.S)
    assert m, 'PUBLIC_MAINNET'
    return ([f'admission-condition-{c}' for c in re.findall(r'"([^"]+)"', m.group(1))]
            + [f'admission-waiver-{w}' for w in re.findall(r'"([^"]+)"', m.group(2))])


def networks():
    """{name: (profile file, chain id, display name)} of the script's $Networks table."""
    return {m[0]: (m[1], int(m[2]), m[3]) for m in re.findall(
        r"^\s+(\w+)\s+= @\{ Profile = '([^']+)';\s+ChainId = \[uint64\](\d+); Name = '([^']+)' \}$", SCRIPT, re.M)}


def config_fields():
    """testnet.rs `struct Config`: {camelCase field: has a serde default}."""
    body = re.search(r'struct Config \{\n(.*?)\n\}', TESTNET, re.S).group(1)
    fields, default = {}, False
    for line in body.splitlines():
        line = line.strip()
        if line.startswith('#[serde(default'):
            default = True
        m = re.match(r'([a-z_0-9]+): ', line)
        if m:
            head, *rest = m.group(1).split('_')
            fields[head + ''.join(w.title() for w in rest)] = default
            default = False
    return fields


def fill(text, values):
    """The script's Set-Ceremony rule: each value replaces the one quoted occurrence of its placeholder string."""
    parsed = json.loads(text)
    for path, value in values.items():
        placeholder = pointer(parsed, path)
        assert isinstance(placeholder, str) and placeholder.startswith('PLACEHOLDER') and not set('"\\') & set(placeholder), path
        quoted = '"' + placeholder + '"'
        assert text.count(quoted) == 1, path
        text = text.replace(quoted, value)
    return text


class InitializeNetwork(unittest.TestCase):
    def test_the_networks_are_the_committed_profiles_with_the_cp_section_5_ids(self):
        table = networks()
        self.assertEqual(set(table), {'Rehearsal', 'Mainnet'})
        # OWNER-DECISIONS-2026-11-20 D1: Mainnet is the default; Rehearsal stays a devnet option.
        self.assertIn("    [ValidateSet('Rehearsal','Mainnet')][string]$Network = 'Mainnet',", SCRIPT)
        self.assertNotIn("Mandatory=$true)][ValidateSet('Rehearsal','Mainnet')]", SCRIPT)
        self.assertEqual(table['Rehearsal'][:2], ('v3-rehearsal-profile.json', 1656550731))
        self.assertEqual(table['Mainnet'], ('v3-profile.json', 2656089850, 'PoCol Mainnet'))
        for name, (profile, chain_id, _) in table.items():
            self.assertEqual(json.loads((MAINNET / profile).read_text(encoding='utf-8'))['chainId'], chain_id, name)
            self.assertIn(f'| {name} | **{chain_id}** |', RECORD)

    def test_the_filled_pointers_are_exactly_the_profiles_placeholders(self):
        ceremony = ps_list('Ceremony')
        want = ['/genesisTime', '/hashRate/hashesPerSecond'] + [f'/founders/{i}/{f}' for i in range(4) for f in ('id', 'reward')]
        self.assertEqual(ceremony, want)
        for profile, _, _ in networks().values():
            self.assertEqual(sorted(placeholders(json.loads((MAINNET / profile).read_text(encoding='utf-8')))), sorted(ceremony), profile)
        # testnet.rs's --test-placeholders fills the launch second the dry run leaves (and only ceremony inputs).
        self.assertIn('"/genesisTime" =>', TESTNET)
        self.assertIn("if (!$DryRun) { $Values['/genesisTime'] = $GenesisTime }", SCRIPT)

    def test_the_profile_is_a_config_init_testnet_accepts(self):
        fields = config_fields()
        self.assertIn('founders', fields)
        required = {k for k, has_default in fields.items() if not has_default}
        self.assertEqual(required, {'chainId', 'genesisTime', 'registrationPerBlock'})
        retired = set(re.search(r'RETIRED_OPTIONS: \[&str; 3\] = \[(.*?)\]', TESTNET).group(1).replace('"', '').replace(' ', '').split(','))
        for profile, _, _ in networks().values():
            data = json.loads((MAINNET / profile).read_text(encoding='utf-8'))
            self.assertLessEqual(set(data), set(fields), profile)
            self.assertLessEqual(required, set(data), profile)
            self.assertFalse({'miners', 'accounts'} & set(data), 'a profile has founders, not miners or accounts')
            self.assertFalse(retired & set(data))
            self.assertIs(data['v3'], True)
            self.assertEqual(data['registrationPerBlock'], data['cp']['R_max'])
            self.assertEqual(len(data['founders']), 4)
            self.assertEqual(data['hashRate']['class'], 'measurement')

    def test_the_script_rule_fills_typed_values_and_leaves_no_placeholder(self):
        addresses = [f'0x{i:040x}' for i in (0xa1, 0xa2, 0xa3, 0xa4)]
        # Founders 1-3 are paid to owner-held addresses (owner decision 2026-11-16); founder 4 is its own reward address.
        rewards = [f'0x{i:040x}' for i in (0xb1, 0xb2, 0xb3)] + addresses[3:]
        founders = {**{f'/founders/{i}/id': f'"{addresses[i]}"' for i in range(4)},
                    **{f'/founders/{i}/reward': f'"{rewards[i]}"' for i in range(4)}}
        for profile, chain_id, _ in networks().values():
            text = (MAINNET / profile).read_text(encoding='utf-8')
            filled = fill(text, {'/genesisTime': '1900000000', '/hashRate/hashesPerSecond': '1000000', **founders})
            self.assertNotIn('PLACEHOLDER', filled)
            data, committed = json.loads(filled), json.loads(text)
            self.assertEqual((data['genesisTime'], data['hashRate']['hashesPerSecond'], data['chainId']), (1900000000, 1000000, chain_id))
            self.assertEqual([(f['id'], f['reward']) for f in data['founders']], list(zip(addresses, rewards)))
            self.assertEqual({k: v for k, v in data.items() if k not in ('genesisTime', 'hashRate', 'founders')},
                             {k: v for k, v in committed.items() if k not in ('genesisTime', 'hashRate', 'founders')})
            # The dry run: everything but the launch second, which --test-placeholders fills.
            dry = json.loads(fill(text, {'/hashRate/hashesPerSecond': '1000000', **founders}))
            self.assertEqual(placeholders(dry), ['/genesisTime'])

    def test_no_spec_1_config_is_built(self):
        self.assertNotIn('Get-Random', SCRIPT)
        self.assertNotIn('ConvertTo-Json', SCRIPT)
        for key in ('miners', 'accounts', 'registrationPerBlock='):
            self.assertEqual(re.findall(r'\b' + re.escape(key) + r'\s*=', SCRIPT), [], key)
        self.assertEqual(re.findall(r'(?<![\d.])\d{1,3}(?:\.\d{1,3}){3}(?![\d.])', SCRIPT), [], 'no IPv4 address is written into the script')
        retired = {e['chainId'] for e in json.loads((MAINNET / 'retired-networks.json').read_text(encoding='utf-8'))['retired']}
        for chain_id in retired | {261597237, 777001, 1234567891}:
            self.assertNotIn(str(chain_id), SCRIPT)

    def test_the_test_flag_is_only_the_dry_runs(self):
        lines = [line for line in SCRIPT.splitlines() if '--test-placeholders' in line and not line.lstrip().startswith('#')]
        self.assertEqual(lines, ["$TestFlag = @(); if ($DryRun) { $TestFlag = @('--test-placeholders') }"])
        self.assertIn('& $Daemon init-testnet --config $ConfigFile --out $Genesis @TestFlag', SCRIPT)
        # A real run refuses a test genesis and any placeholder left in its config.
        self.assertIn("} elseif ($IsTest -or [uint64]$Admitted.chainId -ne $Selected.ChainId) {", SCRIPT)
        self.assertIn("if (!$DryRun -and ($Config.Contains('PLACEHOLDER')", SCRIPT)
        # The dry run stops before anything is published or started.
        dry_return = SCRIPT.index("Write-Host \"DRY RUN complete.")
        self.assertLess(dry_return, SCRIPT.index('& scp.exe @SshArgs $Invitation'))
        self.assertLess(dry_return, SCRIPT.index('Start-Process'))

    def test_servers_are_parameters_on_the_installers_port(self):
        self.assertIn('[Parameter(Mandatory=$true)][ValidateCount(3,3)][string[]]$Servers', SCRIPT)
        self.assertIn('[Net.IPAddress]::TryParse($Server', SCRIPT)
        self.assertNotIn('P2pPort]', SCRIPT, 'the port is not a parameter')
        port = int(re.search(r'^\$P2pPort = (\d+)$', SCRIPT, re.M).group(1))
        self.assertIn(f'--listen 0.0.0.0:{port} ', (HERE / 'install-linux.sh').read_text(encoding='utf-8'))
        self.assertIn('$PackArgs += @(\'--seed\', "${Server}:$P2pPort")', SCRIPT)
        self.assertIn('sudo ufw allow $P2pPort/tcp', SCRIPT)
        for value in ('Mandatory=$true)][uint64]$HashesPerSecond', '[uint64]$GenesisTime = 0', '[int]$LaunchDelaySeconds = 0'):
            self.assertIn(value, SCRIPT)

    def test_four_founders_three_servers_and_this_pc(self):
        self.assertIn("$Founders += Assert-Address $Address $Server", SCRIPT)
        self.assertIn("$Founders += Assert-Address $Desktop.address 'this PC'", SCRIPT)
        self.assertIn('for ($i = 0; $i -lt 4; $i++) { $Values["/founders/$i/id"] = \'"\' + $Founders[$i] + \'"\'; $Values["/founders/$i/reward"] = \'"\' + $Rewards[$i] + \'"\' }', SCRIPT)
        # The PC's founder key is fresh: an existing wallet home is refused before any server is contacted.
        refusal = SCRIPT.index('if (Test-Path -LiteralPath $DesktopHome)')
        self.assertLess(refusal, SCRIPT.index('& ssh.exe'))
        self.assertIn("$DesktopHome = Join-Path $env:LOCALAPPDATA 'PoCol'", SCRIPT)

    def test_founders_1_to_3_are_paid_to_owner_held_reward_addresses(self):
        # Owner decision 2026-11-16: a required parameter for a real run, three distinct addresses, checked before any
        # server is contacted; founder 4 (this PC) keeps its own address.
        self.assertIn('[ValidateCount(3,3)][string[]]$ServerRewardAddresses', SCRIPT)
        self.assertNotIn('Mandatory=$true)][ValidateCount(3,3)][string[]]$ServerRewardAddresses', SCRIPT, 'a dry run runs without it')
        required = "if (!$DryRun -and !$ServerRewardAddresses) { throw 'ServerRewardAddresses is required"
        self.assertIn(required, SCRIPT)
        checks = SCRIPT.index('Assert-RewardAddress $ServerRewardAddresses[$i]')
        for later in ('& ssh.exe', '& scp.exe', 'New-Item -ItemType Directory -Path $Work'):
            self.assertLess(SCRIPT.index(required), SCRIPT.index(later), later)
            self.assertLess(checks, SCRIPT.index(later), later)
        self.assertIn("if (@($ServerRewardAddresses | Sort-Object -Unique).Count -ne 3)", SCRIPT)
        self.assertIn("$Rewards += $Founders[3]", SCRIPT)
        self.assertIn("if ($Founders -ccontains $Rewards[$i])", SCRIPT)
        self.assertIn("$Filled.founders[$i].reward -cne $Rewards[$i]", SCRIPT)
        self.assertIn('pocol-sign --nonce 1', SCRIPT)
        # The refused set mirrors pocold testnet.rs allowed(): zero, 0x...c0c001-0x...c0c0ff, 0xff..fe.
        allowed = re.search(r'pub\(crate\) fn allowed\(address: Address\) -> bool \{(.*?)\n\}', TESTNET, re.S).group(1)
        self.assertIn('0xc0c001u64', allowed); self.assertIn('0xc0c0ffu64', allowed); self.assertIn('U256::from(2)', allowed)
        for text in ("('0x' + ('0' * 40))", "('0x' + ('f' * 38) + 'fe')", '$Low -ge 0xc0c001 -and $Low -le 0xc0c0ff',
                     "$Address -cnotmatch '^0x[0-9a-f]{40}$'"):
            self.assertIn(text, SCRIPT)
        # No real address is committed: the README names placeholders.
        readme = (HERE / 'README.md').read_text(encoding='utf-8')
        self.assertIn('-ServerRewardAddresses OWNER_ADDR1,OWNER_ADDR2,OWNER_ADDR3', readme)

    def test_the_readiness_summary_is_parsed_checked_and_printed(self):
        self.assertIn('$Admitted = $Summary | ConvertFrom-Json', SCRIPT)
        # OWNER-DECISIONS-2026-11-20 D5: the admission is checked against the genesis's chain, both ways.
        self.assertIn('$IsPublic = $PublicNetworkChains -contains [uint64]$Admitted.chainId', SCRIPT)
        self.assertIn("if ($Admitted.scope -cne 'public-network' -or $Admitted.profileKind -cne 'network' -or $Admitted.nodeKind -cne 'node') {", SCRIPT)
        self.assertIn("} elseif ($Admitted.scope -cne 'experimental-devnet' -or $Admitted.profileKind -cne 'simnet' -or $Admitted.nodeKind -cne 'simnet') {", SCRIPT)
        self.assertIn('if ($Admitted.specificationCompliant -isnot [bool] -or $Admitted.specificationCompliant) { throw', SCRIPT)
        self.assertIn('if ($Admitted.publicLaunchApproved -isnot [bool] -or $Admitted.publicLaunchApproved -or $Admitted.publicLaunchReady -isnot [bool] -or $Admitted.publicLaunchReady) {', SCRIPT)
        self.assertIn('foreach ($Blocker in $Blockers) { Write-Host "  $($Blocker.id): $($Blocker.detail)" }', SCRIPT)
        self.assertIn('foreach ($Item in $Pending) { Write-Host "  $($Item.id): $($Item.detail)" }', SCRIPT)
        self.assertIn('Write-Host "Public launch approved: $($Admitted.publicLaunchApproved). Public launch ready: $($Admitted.publicLaunchReady). Launch blockers, verbatim:"', SCRIPT)
        self.assertNotIn("Write-Host 'Public launch ready: false", SCRIPT, 'the computed values are printed, not a literal')
        for literal in ('Experimental admission', 'ExperimentalDevnet scope', 'not public-launch-ready'):
            self.assertNotIn(literal, SCRIPT, 'no literal admission or readiness claim (D6 D)')
        # OWNER-DECISIONS-2026-11-20 D4: the admission's items are technical readiness, not launch blockers.
        self.assertIn('$Admitted.technicalReadiness.pending', SCRIPT)
        required, refused = ps_list('RequiredBlockers'), ps_list('RefusedBlockers')
        self.assertEqual(required, ['network-not-release-pinned', 'launch-not-approved'])
        self.assertEqual(refused, ['consensus-unpinned', 'test-features', 'spec-version', 'v3-inactive', 'v3-launch-blocker'])
        experimental, public = ps_list('ExperimentalPending'), ps_list('PublicPending')
        self.assertEqual(experimental, ['admission-scope', 'profile-kind'])
        self.assertEqual(public, ['admission-scope'])
        for blocker in required + refused + experimental + public:
            self.assertIn(f'blocker("{blocker}"', READINESS, 'a blocker id readiness.rs reports')
        # Exactly PUBLIC_MAINNET's conditions and waivers, in its order, compared as a list.
        self.assertEqual(ps_list('PublicAdmissionItems'), public_mainnet_items())
        self.assertIn("if (($Items -join ',') -cne ($PublicAdmissionItems -join ',')) {", SCRIPT)
        self.assertIn("if ($PendingIds -ccontains 'profile-kind') { throw", SCRIPT)
        self.assertIn('### Launch disclosures (DRAFT: wording for owner review)', (HERE / 'README.md').read_text(encoding='utf-8'))

    def test_the_public_network_chains_are_the_profile_codes_and_the_mainnets(self):
        mainnet = json.loads((MAINNET / 'v3-profile.json').read_text(encoding='utf-8'))['chainId']
        self.assertEqual(ps_chains('PublicNetworkChains'), public_network_chains())
        self.assertEqual(public_network_chains(), [mainnet])
        self.assertEqual(networks()['Mainnet'][1], mainnet)
        self.assertNotIn(networks()['Rehearsal'][1], ps_chains('PublicNetworkChains'), 'the rehearsal is a devnet option')
        self.assertNotIn(int(re.search(r'^\$TestChainId = \[uint64\](\d+)$', SCRIPT, re.M).group(1)), ps_chains('PublicNetworkChains'))

    def test_the_runbook_is_the_mainnet_ceremony_on_the_three_founder_servers(self):
        # OWNER-DECISIONS-2026-11-20 D1 and D3: the README's ceremony runs Mainnet on the three servers (also the seeds);
        # Rehearsal is only a devnet option; the hash rate is measured on the four founder devices.
        readme = (HERE / 'README.md').read_text(encoding='utf-8')
        commands = re.findall(r'^powershell .*Initialize-PoColNetwork\.ps1 .*$', readme, re.M)
        self.assertEqual(len(commands), 1, commands)
        self.assertIn('-Network Mainnet -Servers 108.165.128.89,108.165.128.90,108.165.128.91 ', commands[0])
        self.assertIn('-ServerRewardAddresses OWNER_ADDR1,OWNER_ADDR2,OWNER_ADDR3', commands[0])
        self.assertNotIn('-Network Rehearsal -Servers', readme)
        self.assertIn('`pocol hashrate` on each of the four founder', readme)
        self.assertNotIn('networkHashrateEst', readme)
        for gone in ('first the **rehearsal**', '**Rehearsal, then mainnet.**', 'experimental admission\nconditions',
                     'admitted as an experimental network', 'not in experimental-devnet scope'):
            self.assertNotIn(gone, readme)

    def test_the_scripts_are_ascii(self):
        # Windows PowerShell 5.1 reads a script without a byte-order mark in the ANSI code page.
        for name in ('Initialize-PoColNetwork.ps1', 'test_initialize_network.ps1'):
            (HERE / name).read_bytes().decode('ascii')

    def test_the_safety_checks_are_kept(self):
        for kept in ("Refusing to replace an existing network.", 'sudo test ! -e /opt/pocol-standalone/network.pocol.json',
                     "-cne $Expected) { throw \"Checksum mismatch: $File\" }", "is not covered by SHA256SUMS",
                     "'-F','none','-o','IdentitiesOnly=yes','-i',$SshKey", 'sudo bash install-linux.sh'):
            self.assertIn(kept, SCRIPT)
        # scp only uploads (the remote is always the destination), and the only file read back is the public address.
        for line in [l for l in SCRIPT.splitlines() if '& scp.exe' in l]:
            self.assertRegex(line, r'"\$\{Remote\}:\$Tag/[a-z.]+"$')
        self.assertEqual(re.findall(r"'cat [^']+'", SCRIPT), ["'cat /opt/pocol-standalone/identity.address'"])
        self.assertNotIn('StrictHostKeyChecking', SCRIPT)

    def test_the_windows_package_and_ci_carry_the_ceremony(self):
        package = (HERE / 'package.py').read_text(encoding='utf-8')
        self.assertIn("for name in ['v3-rehearsal-profile.json','v3-profile.json']: files[name]=root/'deploy/mainnet'/name", package)
        workflow = (ROOT / '.github/workflows/five-device-build.yml').read_text(encoding='utf-8')
        self.assertIn('./deploy/standalone/test_initialize_network.ps1', workflow)
        self.assertIn('python deploy/standalone/test_initialize_network.py', workflow)
        # Every file these tests read triggers the workflow that runs them, on pull requests and on pushes.
        filters = re.findall(r'^    paths: \[(.*)\]$', workflow, re.M)
        self.assertEqual(len(filters), 2)
        for read in ('deploy/standalone/**', 'prototypes/**', 'deploy/mainnet/v3-*profile.json',
                     'deploy/mainnet/retired-networks.json', 'development/decisions/FRESH-NETWORK-CP.md'):
            for paths in filters:
                self.assertIn(f"'{read}'", paths)

    def test_this_pcs_clock_is_checked_like_the_servers(self):
        # A running Windows Time service is not a synchronized clock: the source and the measured offset are checked.
        self.assertIn('& w32tm.exe /query /source', SCRIPT)
        self.assertIn("-match 'Local CMOS Clock|Free-running System Clock'", SCRIPT)
        self.assertIn('& w32tm.exe /stripchart "/computer:$TimePeer" /samples:1 /dataonly', SCRIPT)
        self.assertIn('if ([Math]::Abs($OffsetSeconds) -gt 0.5) { throw', SCRIPT)
        self.assertNotIn('/query /status', SCRIPT)
        # Before anything is created on a server.
        self.assertLess(SCRIPT.index('[Math]::Abs($OffsetSeconds)'), SCRIPT.index('& ssh.exe'))

    def test_the_shipped_readme_describes_the_v3_network(self):
        readme = (HERE / 'README.md').read_text(encoding='utf-8')
        profile = json.loads((MAINNET / 'v3-profile.json').read_text(encoding='utf-8'))
        cp = profile['cp']
        self.assertIn(f"On Mainnet that is blocks exit+{cp['W_act']} to exit+{cp['U'] - 1}.", readme)
        self.assertIn(f"(`R_max = {cp['R_max']}`)", readme)
        self.assertIn(f"({cp['W_act']} on PoCol Mainnet)", readme)
        retired = {e['chainId'] for e in json.loads((MAINNET / 'retired-networks.json').read_text(encoding='utf-8'))['retired']}
        for gone in [str(c) for c in retired | {261597237}] + ['60,480', 'cutover', 'Probation seats']:
            self.assertNotIn(gone, readme)


class RealRunExpectations(unittest.TestCase):
    """A real run's checks (no test flag, the profile's own chain id) need the servers, so they run here against a real
    pocold init-testnet of each committed profile filled the way the script fills it (throwaway addresses, a temp dir
    only, never a launch genesis): the summary must pass exactly the checks the script makes."""

    def admit(self, profile):
        founders = [f'0x{i:040x}' for i in (0xa1, 0xa2, 0xa3, 0xa4)]
        rewards = [f'0x{i:040x}' for i in (0xb1, 0xb2, 0xb3)] + founders[3:]
        values = {'/genesisTime': '1900000000', '/hashRate/hashesPerSecond': '1000000',
                  **{f'/founders/{i}/id': f'"{founders[i]}"' for i in range(4)},
                  **{f'/founders/{i}/reward': f'"{rewards[i]}"' for i in range(4)}}
        with tempfile.TemporaryDirectory(prefix='pocol-fn7-real-') as tmp:
            config = Path(tmp) / profile
            config.write_text(fill((MAINNET / profile).read_text(encoding='utf-8'), values), encoding='utf-8')
            p = subprocess.run([str(BIN / ('pocold' + EXE)), 'init-testnet', '--config', str(config), '--out', str(Path(tmp) / 'genesis')],
                               capture_output=True, text=True, encoding='utf-8', timeout=600)
            self.assertEqual(p.returncode, 0, p.stderr)
            return json.loads(p.stdout)

    def check_common(self, summary, chain_id):
        self.assertNotIn('testPlaceholders', summary)
        self.assertEqual(summary['chainId'], chain_id)
        self.assertIs(summary['specificationCompliant'], False)
        self.assertIs(summary['publicLaunchApproved'], False)
        self.assertIs(summary['publicLaunchReady'], False)
        ids = [b['id'] for b in summary['launchBlockers']]
        for required in ps_list('RequiredBlockers'):
            self.assertIn(required, ids)
        for refused in ps_list('RefusedBlockers'):
            self.assertNotIn(refused, ids, 'a release build: the script refuses a genesis with this blocker')
        self.assertIs(summary['technicalReadiness']['complete'], False)
        return [b['id'] for b in summary['technicalReadiness']['pending']]

    def test_the_mainnet_real_run_meets_the_public_network_expectation(self):
        chain_id = networks()['Mainnet'][1]
        self.assertIn(chain_id, ps_chains('PublicNetworkChains'))
        summary = self.admit('v3-profile.json')
        pending = self.check_common(summary, chain_id)
        self.assertEqual((summary['scope'], summary['profileKind'], summary['nodeKind']), ('public-network', 'network', 'node'))
        for item in ps_list('PublicPending'):
            self.assertIn(item, pending)
        self.assertNotIn('profile-kind', pending)
        self.assertEqual([i for i in pending if re.match(r'admission-(condition|waiver)-', i)], ps_list('PublicAdmissionItems'))
        # The identity fields the script prints: the facts', before any pin.
        self.assertEqual((summary['networkEnvironment'], summary['networkScope']), ('mainnet', 'public-network'))
        self.assertNotIn('networkName', summary)
        self.assertIsNone(summary['launchApprovalReference'])

    def test_the_rehearsal_devnet_option_meets_the_experimental_expectation(self):
        chain_id = networks()['Rehearsal'][1]
        self.assertNotIn(chain_id, ps_chains('PublicNetworkChains'))
        summary = self.admit('v3-rehearsal-profile.json')
        pending = self.check_common(summary, chain_id)
        self.assertEqual((summary['scope'], summary['profileKind'], summary['nodeKind']), ('experimental-devnet', 'simnet', 'simnet'))
        for item in ps_list('ExperimentalPending'):
            self.assertIn(item, pending)
        self.assertEqual((summary['networkEnvironment'], summary['networkScope']), ('devnet', 'experimental-devnet'))


if __name__ == '__main__':
    unittest.main()
