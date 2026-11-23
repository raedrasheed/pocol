#!/usr/bin/env python3
"""Each refusal check of pocol.net/api/reward.php that api_security.php covers is removed alone, in a private copy, and
api_security.php must then fail (2026-11-22 review: vectors refused only by the paid-sum check hid untested checks).

Needs php-cli (run after `php deploy/websites/tests/api_security.php`). Exit status 0 only when every removal is caught.
One check is listed as implied, not removed: projectedBondWei <= projectedPaidWei follows from the per-beneficiary sums
(paid = sum(amount + fill) >= sum(fill) = bond, both compared exactly afterwards), so no vector can reach it alone."""
import shutil, subprocess, sys, tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
FILES = ('deploy/websites/tests/api_security.php', 'deploy/websites/pocol.net/api/network.php',
         'deploy/websites/pocol.net/api/reward.php', 'deploy/rewards/testdata/v3-devnet-tape.json')
# name: (text of reward.php, its replacement without the check)
REMOVALS = {
    'duplicate minerId and rewardAddress': (" || isset($ids[$pair])) pocol_reward_fail();", ") pocol_reward_fail();"),
    'empty beneficiary': ("\n            || ($b->amountWei === '0' && $filled === '0')", "\n           "),
    'explicit zero bond fill': ("(property_exists($b, 'bondAccruedWei') && $filled === '0')\n            || ", ""),
    'final block: settlementHeight is null': (" || $v->settlementHeight !== null\n", "\n"),
    'final block: settlementHeight key present': ("$v->final ? !property_exists($v, 'settlementHeight') || ", "$v->final ? "),
    'closing key present': ("\n        || !property_exists($v, 'closing')) pocol_reward_fail();", ") pocol_reward_fail();"),
    'winnerAccruing is a boolean': (" || !is_bool($v->winnerAccruing ?? null)", ""),
    'final block opens no pot': ("$v->final ? $v->grossRewardWei !== '0' || $v->issuedWei !== '0' || $v->tipShareWei !== '0' || $v->beneficiaryCount !== 0",
                                 "$v->final ? false"),
    'source': ("(property_exists($v, 'source') && $v->source !== 'https://explorer.pocol.net')", "false"),
    'specVersion': ("|| ($v->specVersion ?? null) !== 2\n", "\n"),
    'paid and bond sums': ("if ($paid !== $v->projectedPaidWei || $bond !== $v->projectedBondWei) pocol_reward_fail();", ""),
}

def run(tree):
    return subprocess.run(['php', str(tree / FILES[0])], capture_output=True, text=True, timeout=120)

def main():
    if shutil.which('php') is None:
        print('FAIL php-cli is required', file=sys.stderr)
        return 2
    caught = True
    with tempfile.TemporaryDirectory() as tmp:
        tree = Path(tmp)
        for rel in FILES:
            (tree / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / rel, tree / rel)
        reward = tree / FILES[2]
        original = reward.read_text(encoding='utf-8')
        if run(tree).returncode != 0:
            print('FAIL api_security.php fails on the unchanged copy', file=sys.stderr)
            return 1
        for name, (old, new) in REMOVALS.items():
            if original.count(old) != 1:
                print(f'FAIL {name}: the check text is not in reward.php once (update this list)', file=sys.stderr)
                return 1
            reward.write_text(original.replace(old, new), encoding='utf-8')
            killed = run(tree).returncode != 0
            caught &= killed
            print(f"{'CAUGHT  ' if killed else 'MISSED  '} {name}")
        reward.write_text(original, encoding='utf-8')
    print('PASS' if caught else 'FAIL', f'{len(REMOVALS)} removals')
    return 0 if caught else 1

if __name__ == '__main__':
    sys.exit(main())
