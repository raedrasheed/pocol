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
        founders = {f'/founders/{i}/{f}': f'"{addresses[i]}"' for i in range(4) for f in ('id', 'reward')}
        for profile, chain_id, _ in networks().values():
            text = (MAINNET / profile).read_text(encoding='utf-8')
            filled = fill(text, {'/genesisTime': '1900000000', '/hashRate/hashesPerSecond': '1000000', **founders})
            self.assertNotIn('PLACEHOLDER', filled)
            data, committed = json.loads(filled), json.loads(text)
            self.assertEqual((data['genesisTime'], data['hashRate']['hashesPerSecond'], data['chainId']), (1900000000, 1000000, chain_id))
            self.assertEqual([(f['id'], f['reward']) for f in data['founders']], [(a, a) for a in addresses])
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
        self.assertIn('for ($i = 0; $i -lt 4; $i++) { $Values["/founders/$i/id"] = \'"\' + $Founders[$i] + \'"\'; $Values["/founders/$i/reward"] = \'"\' + $Founders[$i] + \'"\' }', SCRIPT)
        # The PC's founder key is fresh: an existing wallet home is refused before any server is contacted.
        refusal = SCRIPT.index('if (Test-Path -LiteralPath $DesktopHome)')
        self.assertLess(refusal, SCRIPT.index('& ssh.exe'))
        self.assertIn("$DesktopHome = Join-Path $env:LOCALAPPDATA 'PoCol'", SCRIPT)

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


if __name__ == '__main__':
    unittest.main()
