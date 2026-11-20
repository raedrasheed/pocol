#!/usr/bin/env python3
"""FN-7 (FRESH-NETWORK-DESIGN C8): Initialize-PoColNetwork.ps1 fills the committed v3 profile that pocold init-testnet
accepts, instead of building a spec-1 config.

These are text checks of the script against the profiles (deploy/mainnet), testnet.rs's Config, readiness.rs's
blocker ids, install-linux.sh and FRESH-NETWORK-CP.md, so they run on any host. The script itself runs, with the real
pocold, in test_initialize_network.ps1 (its -DryRun into a temporary package, then the refusals) on the Windows job of
five-device-build.yml.
"""
import json
import re
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SCRIPT = (HERE / 'Initialize-PoColNetwork.ps1').read_text(encoding='utf-8')
MAINNET = ROOT / 'deploy/mainnet'
TESTNET = (ROOT / 'prototypes/pocold/src/testnet.rs').read_text(encoding='utf-8')
READINESS = (ROOT / 'prototypes/pocold/src/readiness.rs').read_text(encoding='utf-8')
RECORD = (ROOT / 'development/decisions/FRESH-NETWORK-CP.md').read_text(encoding='utf-8')
INSTALL = (HERE / 'install-linux.sh').read_text(encoding='utf-8')
README = (HERE / 'README.md').read_text(encoding='utf-8')
RUNBOOK = (ROOT / 'deploy/mainnet/README.md').read_text(encoding='utf-8')
REHEARSAL = (ROOT / 'development/public-network/REHEARSAL.md').read_text(encoding='utf-8')
NODE = (ROOT / 'prototypes/pocold/src/node.rs').read_text(encoding='utf-8')
POCOL = (ROOT / 'prototypes/pocold/src/bin/pocol.rs').read_text(encoding='utf-8')
GIB = 1 << 30


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
        self.assertIn("[ValidateSet('Rehearsal','Mainnet')][string]$Network", SCRIPT)
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
        self.assertIn("$Admitted.scope -cne 'experimental-devnet'", SCRIPT)
        self.assertIn('$Admitted.publicLaunchReady', SCRIPT)
        self.assertIn('foreach ($Blocker in $Blockers) { Write-Host "  $($Blocker.id): $($Blocker.detail)" }', SCRIPT)
        required, refused = ps_list('RequiredBlockers'), ps_list('RefusedBlockers')
        self.assertEqual(required, ['admission-scope', 'profile-kind'])
        for blocker in required + refused:
            self.assertIn(f'blocker("{blocker}"', READINESS, 'a blocker id readiness.rs reports')
        self.assertIn('### Launch disclosures (DRAFT: wording for owner review)', (HERE / 'README.md').read_text(encoding='utf-8'))

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
                     'deploy/mainnet/retired-networks.json', 'deploy/mainnet/README.md',
                     'development/decisions/FRESH-NETWORK-CP.md', 'development/public-network/REHEARSAL.md',
                     'development/public-network/OPERATIONS.md'):
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


def runbook_steps():
    """deploy/mainnet/README.md's numbered runbook steps, as test_release_signing.py reads them."""
    return dict(re.findall(r'(?ms)^(\d)\. \*\*(.*?)(?=^\d\. \*\*|^## )', RUNBOOK))


def unit_settings():
    """install-linux.sh's systemd unit (the UNIT heredoc): {key: [values]}."""
    body = re.search(r"<<'UNIT'\n(.*?)\nUNIT\n", INSTALL, re.S).group(1)
    settings = {}
    for line in body.splitlines():
        m = re.match(r'^([A-Za-z]+)=(.*)$', line)
        if m:
            settings.setdefault(m.group(1), []).append(m.group(2))
    return settings


def systemd_bytes(value):
    """A systemd memory size (bytes, or K/M/G/T with base 1024)."""
    m = re.fullmatch(r'(\d+)([KMGT]?)', value)
    assert m, value
    return int(m.group(1)) << {'': 0, 'K': 10, 'M': 20, 'G': 30, 'T': 40}[m.group(2)]


def memory_budget(settings):
    """The node's memory budget under the unit: its POCOL_MEMORY_BUDGET_BYTES, else node.rs MEM_BUDGET."""
    for env in settings.get('Environment', []):
        m = re.fullmatch(r'"?POCOL_MEMORY_BUDGET_BYTES=(\d+)"?', env)
        if m:
            return int(m.group(1))
    m = re.search(r'^pub const MEM_BUDGET:u64=(\d+)<<(\d+);$', NODE, re.M)
    assert m, 'node.rs MEM_BUDGET'
    return int(m.group(1)) << int(m.group(2))


class Rehearsal(unittest.TestCase):
    """The rehearsal's server unit, hash rate measurement and runbook texts agree with the code they describe."""

    def test_the_server_unit_leaves_the_node_its_memory_budget(self):
        unit = unit_settings()
        budget = memory_budget(unit)
        self.assertEqual(budget, 6 * GIB, 'the default budget the docs quote')
        self.assertEqual(len(unit.get('MemoryMax', [])), 1, unit)
        cap = systemd_bytes(unit['MemoryMax'][0])
        # The A13 model leaves out allocator overhead: the cap is the budget plus 1 GiB the budget does not count.
        self.assertGreaterEqual(cap, budget + GIB, 'MemoryMax below the budget would OOM-kill a node its gate admits')
        for high in unit.get('MemoryHigh', []):
            self.assertGreaterEqual(systemd_bytes(high), budget, 'MemoryHigh below the budget throttles the node')
        self.assertEqual(unit.get('MemorySwapMax'), ['0'])
        # The README's thread inventory fits the unit's task limit (the node also refuses to start otherwise).
        tasks = max(int(n) for n in re.findall(r'(\d+) tasks with', README))
        self.assertGreaterEqual(int(unit['TasksMax'][0]), tasks)

    def test_the_docs_quote_the_units_memory_limits(self):
        cap = unit_settings()['MemoryMax'][0]
        host = systemd_bytes(cap) // GIB + 1
        for name, text in (('deploy/standalone/README.md', README), ('REHEARSAL.md', REHEARSAL)):
            self.assertIn(f'`MemoryMax={cap}`', text, name)
            self.assertIn(f'at least {host} GiB of RAM', text, name)
            for gone in ('MemoryMax=3G', 'MemoryHigh=2560M', '4 GiB server', '4 GiB host'):
                self.assertNotIn(gone, text, name)
        self.assertNotIn('4 GiB', INSTALL)
        self.assertIn(f'at least {host} GiB of RAM', INSTALL.replace('\n# ', ' '))

    def test_the_docs_quote_the_pinned_store_model(self):
        # The A13 store-mode total the unit comment and the docs quote is the one network_budget.rs pins (run by
        # full-nonce-upgrade.yml), so a moved model fails here as well as there.
        budget = (ROOT / 'prototypes/pocold/tests/network_budget.rs').read_text(encoding='utf-8')
        total = int(re.search(r'^const V3_MAINNET_STORE_TOTAL:u128=([0-9_]+);$', budget, re.M).group(1).replace('_', ''))
        quoted = f'{total:,} B'
        operations = (ROOT / 'development/public-network/OPERATIONS.md').read_text(encoding='utf-8')
        for name, text in (('install-linux.sh', INSTALL), ('deploy/standalone/README.md', README),
                           ('REHEARSAL.md', REHEARSAL), ('OPERATIONS.md', operations)):
            self.assertIn(quoted, text, name)

    def test_the_late_start_check_on_the_filled_profile_is_a_record(self):
        # The ceremony fills the profile and admits the genesis in one run (no pause between them, on either network),
        # so the ignored launch_delay_asert test on the filled profile runs after the genesis: the runbook says so.
        asert = (ROOT / 'prototypes/pocold/tests/launch_delay_asert.rs').read_text(encoding='utf-8')
        self.assertIn('#[ignore = "ceremony record: POCOL_FILLED_PROFILE=<path>', asert)
        rates = re.search(r'let rates: \[u64; (\d+)\]', asert).group(1)
        fill = SCRIPT.index('$Config = Set-Ceremony $ProfileText $Values')
        admit = SCRIPT.index('& $Daemon init-testnet --config $ConfigFile')
        self.assertNotRegex(SCRIPT[fill:admit], r'Read-Host|PromptForChoice|launch_delay_asert')
        step = ' '.join(runbook_steps()['4'].split())
        self.assertIn('--test launch_delay_asert -- --ignored --nocapture', step)
        self.assertIn('for the mainnet as for the rehearsal', step)
        self.assertIn('it is a record, not a gate', step)
        # CI checks sampled rates, not every rate: the docs give the count the test has.
        words = {'8': 'eight'}[rates]
        for name, text in (('runbook step 4', step), ('launch_delay_asert.rs', ' '.join(asert.replace('//!', '').replace('///', '').split()))):
            self.assertIn(f'{words} rates spanning', text, name)
            self.assertNotIn('every admissible', text, name)
            self.assertNotIn('whole admissible range', text, name)

    def test_the_hash_rate_is_measured_on_each_device_and_summed(self):
        # pocol hashrate exists, before any home is opened, and the ceremony and both READMEs point to it.
        self.assertIn('"hashrate" => &["--seconds"]', POCOL)
        self.assertLess(POCOL.index('if command == "hashrate"'), POCOL.index('let home_path = get("--home")'))
        self.assertIn('hashrate [--seconds 1..600]', POCOL)
        param = SCRIPT[:SCRIPT.index('Mandatory=$true)][uint64]$HashesPerSecond')]
        self.assertIn('`pocol hashrate`', param[param.rindex('param('):], 'the parameter comment names the command')
        steps = runbook_steps()
        for name, text in (('deploy/mainnet/README.md step 4', steps['4']), ('deploy/standalone/README.md', README)):
            self.assertIn('pocol hashrate --seconds 60', text, name)
            self.assertIn('pocol.exe hashrate --seconds 60', text, name)
            self.assertRegex(text, r'sum of the four|add the four medians', name)
            self.assertIn('networkHashrateEst', text, name)
        self.assertIn('`pocol hashrate`', REHEARSAL)

    def test_the_miner_key_comes_from_pocol_identity_on_each_device(self):
        self.assertIn('runuser -u pocol-standalone -- /opt/pocol-standalone/pocol identity --home /var/lib/pocol-standalone', INSTALL)
        self.assertIn('$Desktop = (& $Node identity | ConvertFrom-Json)', SCRIPT)
        step = runbook_steps()['3']
        self.assertIn('pocol identity --home /var/lib/pocol-standalone', step)
        self.assertIn('pocol.exe identity', step)
        self.assertNotIn('independent-key flow', step)
        miner = step[:step.index('Only the public miner ID leaves the device.')]
        self.assertNotIn('pocol-key', miner, 'pocol-key makes the reward keys, not the miner keys')

    def test_the_rehearsal_runs_the_four_founders_the_ceremony_makes(self):
        step = runbook_steps()['6']
        self.assertIn('four founding devices', step)
        self.assertIn("owner's PC (founder 4)", step)
        self.assertNotIn('three US servers only', RUNBOOK)
        self.assertIn("Write-Host \"Creates the $($Selected.Name) genesis (chain id $($Selected.ChainId)) with four founders: the three servers and this PC.\"", SCRIPT)
        self.assertNotIn('The host set is the owner', REHEARSAL)

    def test_the_compared_invitation_value_is_the_file_sha256(self):
        self.assertIn('$NetworkHash = (Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash.ToLowerInvariant()', SCRIPT)
        self.assertIn('Write-Host "Invitation: $Target (SHA-256 $NetworkHash)"', SCRIPT)
        self.assertIn("printf '%s  %s\\n' '$NetworkHash' network.pocol.json | sha256sum -c -", SCRIPT)
        for name, text in (('deploy/mainnet/README.md step 5', runbook_steps()['5']), ('deploy/standalone/README.md', README)):
            self.assertIn('file SHA-256', text, name)
            self.assertIn('`networkFingerprint`, a different', text, name)
        self.assertNotIn('Publish the invitation fingerprint', README)

    def test_the_server_and_clock_preparation_matches_the_script(self):
        # Every requirement the README lists is one the script or the installer applies.
        for in_script in ("$ServerUser = 'pocol-admin'", "'.ssh\\id_ed25519_pocol'", 'timedatectl show -p NTPSynchronized --value',
                          'sudo ufw allow $P2pPort/tcp', "'-F','none','-o','IdentitiesOnly=yes'",
                          '& w32tm.exe /stripchart "/computer:$TimePeer" /samples:1 /dataonly', "($TimeSource -split ',')[0]"):
            self.assertIn(in_script, SCRIPT)
        self.assertNotIn('ufw enable', SCRIPT, 'the script never enables ufw')
        for in_readme in ('`pocol-admin`', 'id_ed25519_pocol', 'SSH on port 22', 'timedatectl show -p NTPSynchronized --value',
                          'sudo ufw allow 30300/tcp', 'never enables ufw', 'sudo ufw allow OpenSSH',
                          'w32tm /stripchart /computer:NTP_HOST /samples:1 /dataonly', 'w32tm /query /source',
                          'Local CMOS Clock', 'systemd\n    250 or later'):
            self.assertIn(in_readme, README)
        self.assertNotIn('tested on 24.04', README)


if __name__ == '__main__':
    unittest.main()
