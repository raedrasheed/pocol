#!/usr/bin/env python3
"""Collect read-only inventory of the three existing seeds over the owner's SSH.

Run on the Windows workstation holding the existing SSH identity. No remote
file upload, service change, key read, cutover or consensus write is performed.
This historical collector stays bound to the old network. After the client
identity switch its pinned preflight source changes, so it refuses to run.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import time

import ssh_terminal

HOSTS = ('108.165.128.89', '108.165.128.90', '108.165.128.91')
USER = 'pocol-admin'
PREFLIGHT_SHA256 = 'af8a566627150d720cf5af8d3479c4813d2ec723c6a7702715e1babd38bc0ac1'
CHAIN_ID = 1599846715
GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'


class Blocked(ValueError):
    pass


def unique(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise Blocked('Duplicate JSON field in server report')
        result[key] = value
    return result


def load_program(path=None):
    program = (path or Path(__file__).with_name('preflight.py')).read_bytes()
    # Preserve the approved LF source on Windows too; no other rewriting.
    program = program.replace(b'\r\n', b'\n')
    if hashlib.sha256(program).hexdigest() != PREFLIGHT_SHA256:
        raise Blocked('The source-reviewed read-only preflight program changed')
    return program


def validate_report(value):
    if not isinstance(value, dict) or value.get('schema') != 'pocol-mainnet-preflight-v1':
        raise Blocked('Invalid preflight response schema')
    if (value.get('readOnly') is not True or value.get('deploymentPerformed') is not False
            or value.get('activationHeightSelected') is not False
            or value.get('result') not in ('PASS', 'FAIL')):
        raise Blocked('Unexpected preflight operation or result')
    return value


class SSH:
    def __init__(self, key, runner=subprocess.run, interactive_sudo=False):
        self.program = load_program()
        self.key = Path(key).expanduser()
        if not self.key.is_file():
            raise Blocked('Existing SSH identity file was not found; select it with --ssh-key')
        self.runner = runner
        self.interactive_sudo = interactive_sudo

    def collect(self, host, height=None):
        if host not in HOSTS:
            raise Blocked('SSH destination is outside the three reviewed seeds')
        if height is not None and (type(height) is not int or height < 0):
            raise Blocked('Invalid historical comparison height')
        arguments = ['--rpc', 'http://127.0.0.1:8545', '--layout', 'auto']
        if height is not None:
            arguments += ['--check-height', str(height)]
        if self.interactive_sudo:
            wrapper, begin, end = ssh_terminal.command_program(self.program, ssh_terminal.markers())
            remote = ['sudo', '-p', 'PoCol inventory password for %u on %h: ',
                      'python3', '-B', '-c', wrapper, *arguments]
        else:
            remote = ['sudo', '-n', 'python3', '-B', '-', *arguments]
        command = ['ssh', '-F', 'none', '-tt' if self.interactive_sudo else '-T',
                   '-o', 'BatchMode=yes', '-o', 'IdentitiesOnly=yes',
                   '-o', 'StrictHostKeyChecking=yes', '-o', 'ConnectTimeout=15',
                   '-o', 'ServerAliveInterval=15', '-o', 'ServerAliveCountMax=3',
                   '-i', str(self.key), f'{USER}@{host}', shlex.join(remote)]
        try:
            if self.interactive_sudo:
                print(f'{host}: enter the server account sudo password in this terminal when prompted.', flush=True)
                response = ssh_terminal.run(command, begin, end, timeout=180)
            else:
                response = self.runner(command, input=self.program, capture_output=True, timeout=180)
        except ssh_terminal.TerminalError as error:
            raise Blocked(f'{host}: {error}') from None
        except subprocess.TimeoutExpired:
            raise Blocked(f'SSH inventory timed out for {host}; no remote state was changed') from None
        if response.returncode not in (0, 2):
            error = response.stderr.decode('utf-8', errors='replace').lower()
            if 'host key verification failed' in error or 'remote host identification has changed' in error:
                reason = 'verify the host fingerprint through your existing trusted operator channel'
            elif 'permission denied' in error:
                reason = 'SSH authentication failed; use the existing identity and unlock its agent if needed'
            elif 'password is required' in error:
                reason = 'noninteractive sudo for the read-only preflight is unavailable'
            else:
                reason = 'SSH or noninteractive sudo failed; inspect access from the owner workstation'
            # Do not copy SSH stderr: a user-defined transport could print secrets.
            raise Blocked(f'{host}: {reason}')
        if len(response.stdout) > 256 * 1024:
            raise Blocked(f'{host}: preflight report exceeded its size limit')
        try:
            value = json.loads(response.stdout.decode('utf-8'), object_pairs_hook=unique)
        except (UnicodeError, json.JSONDecodeError):
            raise Blocked(f'{host}: SSH did not return a JSON preflight report') from None
        return validate_report(value)


def same_network(rows):
    if len(rows) != 3:
        raise Blocked('All three seeds must return an inventory')
    identities, miners = set(), set()
    for row in rows:
        validate_report(row)
        if row['result'] != 'PASS':
            raise Blocked('A seed preflight failed; inspect the saved report before any cutover')
        node = row['node']
        if node['chainId'] != CHAIN_ID or node['genesisHash'] != GENESIS:
            raise Blocked('A seed belongs to another network')
        source = node['compatibilitySha256']
        if not isinstance(source, str) or not re.fullmatch(r'[0-9a-f]{64}', source):
            raise Blocked('Invalid running source fingerprint')
        identities.add((source, json.dumps(node['forkSchedule'], separators=(',', ':'))))
        binding = node['binding']
        name, miner = binding['node'], binding['miner']
        if not isinstance(name, str) or not name or not isinstance(miner, str) or not re.fullmatch(r'0x[0-9a-fA-F]{40}', miner):
            raise Blocked('Expected three independent miner identities')
        # The standalone launcher calls every node "pocol". Miner keys, rather
        # than this display name, establish independence between the seeds.
        miners.add(miner.lower())
        value = node['head']['height']
        if not isinstance(value, str) or not re.fullmatch(r'0|[1-9][0-9]*', value) or int(value) >= 2**64:
            raise Blocked('Invalid observed head height')
    if len(identities) != 1:
        raise Blocked('The seeds disagree on running source or fork schedule')
    if len(miners) != 3:
        raise Blocked('Seed miner identities must be independent')
    return next(iter(identities))


def execute(client, report, checkpoint, pause=time.sleep, wait_seconds=15):
    report.update(phase='initial', rounds={'initial': [], 'common': [], 'progress': []})
    def collect_round(name, height=None):
        values = []
        for host in HOSTS:
            report['active_host'] = host
            value = client.collect(host, height)
            report['rounds'][name].append({'host': host, 'report': value})
            checkpoint()
            print(f'{name} {host}: {value["result"]}', flush=True)
            if value['result'] == 'FAIL':
                # Print only fixed stage labels, never remote diagnostic text.
                failed = [stage for stage in ('layout', 'node', 'peers', 'bundle', 'stateBinding', 'service')
                          if value.get('checks', {}).get(stage) is False]
                if failed: print('  failed checks: ' + ', '.join(failed), flush=True)
            values.append(value)
        return values
    def installation(row):
        release = row.get('bundle', {})
        return (row.get('layout'), json.dumps(row.get('paths', {}), sort_keys=True),
                release.get('manifestSha256'), release.get('invitationSha256'))
    def unchanged(first, current):
        if first['node']['binding'] != current['node']['binding']:
            raise Blocked('A seed identity changed during collection')
        if installation(first) != installation(current):
            raise Blocked('A seed installation or public invitation changed during collection')
    initial = collect_round('initial')
    network = same_network(initial)
    common_height = min(int(row['node']['head']['height']) for row in initial) - 60
    if common_height < 1:
        raise Blocked('The old network is too young for a stable historical comparison')
    report.update(phase='common', historical_comparison_height=common_height)
    common = collect_round('common', common_height)
    if same_network(common) != network:
        raise Blocked('The running source or schedule changed during collection')
    for first, current in zip(initial, common): unchanged(first, current)
    blocks = []
    for row in common:
        block = row['comparisonBlock']
        if block.get('height') != str(common_height):
            raise Blocked('A seed did not return the requested historical block')
        for field in ('hash', 'stateRoot', 'receiptsRoot'):
            if not isinstance(block.get(field), str) or not re.fullmatch(r'0x[0-9a-f]{64}', block[field]):
                raise Blocked('Invalid common-block hash or root')
        blocks.append(block)
    if not all(block == blocks[0] for block in blocks):
        raise Blocked('The seeds disagree on the common block hash or state/receipts roots')
    report['phase'] = 'progress'
    checkpoint()
    pause(wait_seconds)
    progress = collect_round('progress')
    if same_network(progress) != network:
        raise Blocked('The running source or schedule changed during progress confirmation')
    if any(int(last['node']['head']['height']) <= int(first['node']['head']['height'])
           for first, last in zip(initial, progress)):
        raise Blocked('A seed did not advance; inspect the observations before any cutover')
    for first, last in zip(initial, progress):
        unchanged(first, last)
    report.update(phase='complete', result='PASS', common_block=blocks[0], all_three_heads_advanced=True,
                  captured_at=datetime.now(timezone.utc).isoformat(timespec='seconds'))
    report.pop('active_host', None)
    checkpoint()


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True, help='A fresh local JSON report filename')
    parser.add_argument('--ssh-key', type=Path, default=Path.home() / '.ssh/id_ed25519_pocol')
    parser.add_argument('--interactive-sudo', action='store_true',
                        help='Use the local SSH terminal for sudo password entry; never store passwords')
    parser.add_argument('--wait-seconds', type=int, default=15, help='Progress observation interval, 1..60 seconds')
    args = parser.parse_args(argv)
    if args.interactive_sudo and not sys.stdin.isatty():
        parser.error('--interactive-sudo requires an interactive terminal for local password entry')
    if not 1 <= args.wait_seconds <= 60:
        parser.error('--wait-seconds must be between 1 and 60')
    if args.out.exists() or not args.out.parent.is_dir():
        parser.error('--out must be a new file in an existing directory')
    report = {'schema': 'pocol-three-seed-preparation-v1', 'read_only': True, 'deployment_performed': False,
              'freeze_height_selected': False, 'new_genesis_selected': False, 'hosts': list(HOSTS),
              'sudo_mode': 'interactive-terminal' if args.interactive_sudo else 'noninteractive',
              'preflight_program_sha256': PREFLIGHT_SHA256, 'phase': 'preflight'}
    def checkpoint():
        args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    try:
        with args.out.open('x', encoding='utf-8') as stream:
            stream.write(json.dumps(report) + '\n')
    except OSError:
        parser.error('Unable to create the fresh report file')
    code = 0
    try:
        execute(SSH(args.ssh_key, interactive_sudo=args.interactive_sudo), report, checkpoint, wait_seconds=args.wait_seconds)
    except (Blocked, OSError, KeyError, TypeError, ValueError) as error:
        report.update(result='BLOCKED', blocked=str(error))
        print('Stopped: ' + str(error), file=sys.stderr)
        code = 1
    checkpoint()
    if not code:
        print(f'PASS: three seeds agree and advance. Read-only report: {args.out}', flush=True)
    return code


if __name__ == '__main__':
    sys.exit(main())
