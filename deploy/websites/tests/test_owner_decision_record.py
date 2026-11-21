#!/usr/bin/env python3
"""The owner decision record of 2026-11-20 (development/decisions/OWNER-DECISIONS-2026-11-20.md) against the code that
cites it. D1 accepts "the documented risks": its section "Accepted risks (D1, D2)" must list every item the node reports
as pending for PoCol Mainnet (prototypes/pocold/src/readiness.rs: each condition and waiver of
ExpectedAdmission::PUBLIC_MAINNET, as admission-condition-<id> and admission-waiver-<rule>, admission-scope, and each
open acceptance-ledger gate of PLAN_GATES_OPEN, as plan-gate-<id>), so no pending item is left without its documented
risk. Every launch approval reference the release can pin (readiness.rs PUBLIC_MAINNET_APPROVAL, every approval of
NETWORK_PROFILES, and the one deploy/mainnet/set_network_identity.py writes) names an existing decision record and a
decision in it."""
import re, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
DECISIONS = ROOT / 'development' / 'decisions'
RECORD = DECISIONS / 'OWNER-DECISIONS-2026-11-20.md'
READINESS = (ROOT / 'prototypes' / 'pocold' / 'src' / 'readiness.rs').read_text(encoding='utf-8')
IDENTITY = (ROOT / 'deploy' / 'mainnet' / 'set_network_identity.py').read_text(encoding='utf-8')
FINDINGS = ROOT / 'development' / 'public-network' / 'FINDINGS.md'
INSTALL = (ROOT / 'deploy' / 'standalone' / 'install-linux.sh').read_text(encoding='utf-8')
GENESIS_RECORD = ROOT / 'development' / 'public-network' / 'GENESIS-RECORD-2026-11-21.md'
# Ledger entries recorded after this decision record, from the owner's post-genesis report of 2026-11-21: D1 could
# not accept them, so this record does not list them; the genesis record cites each one instead.
POST_GENESIS = 'As reported by the owner on 2026-11-21 (GENESIS-RECORD-2026-11-21.md)'

def accepted_risks(text):
    section = text.split('\n## Accepted risks (D1, D2)\n', 1)
    if len(section) != 2:
        raise AssertionError('the record has no section "Accepted risks (D1, D2)"')
    return section[1].split('\n## ', 1)[0]

def public_mainnet(field):
    """The string list `field` of ExpectedAdmission::PUBLIC_MAINNET in readiness.rs."""
    body = re.search(r'pub const PUBLIC_MAINNET:Self=Self\{(.*?)\};', READINESS, re.S).group(1)
    return re.findall(r'"([^"]+)"', re.search(rf'\b{field}:&\[([^\]]*)\]', body).group(1))

def pending_ids():
    """Every pending id the node reports for a booted PoCol Mainnet node with the expected admission."""
    gates = re.findall(r'^\s*\("([A-Z]\d)",', re.search(r'pub const PLAN_GATES_OPEN:&\[\(&str,&str\)\]=&\[(.*?)\n\];', READINESS, re.S).group(1), re.M)
    return (['admission-scope'] + [f'admission-condition-{c}' for c in public_mainnet('conditions')]
            + [f'admission-waiver-{w}' for w in public_mainnet('waivers')] + [f'plan-gate-{g}' for g in gates])

def approval_references():
    refs = {'readiness.rs PUBLIC_MAINNET_APPROVAL': re.search(r'^pub const PUBLIC_MAINNET_APPROVAL:&str="([^"]+)";$', READINESS, re.M).group(1),
            'set_network_identity.py PUBLIC_APPROVAL': re.search(r"^PUBLIC_APPROVAL = '([^']+)'$", IDENTITY, re.M).group(1)}
    profiles = re.search(r'^pub const NETWORK_PROFILES:&\[NetworkProfile\]=&\[(.*)\];$', READINESS, re.M).group(1)
    for n, ref in enumerate(re.findall(r'approval:Some\("([^"]*)"\)', profiles)):
        refs[f'NETWORK_PROFILES entry {n}'] = ref
    return refs

def resolves(ref):
    """Whether "<RECORD NAME> D<n>" names an existing record in development/decisions and a decision D<n> in it."""
    m = re.fullmatch(r'(OWNER-DECISIONS-\d{4}-\d{2}-\d{2}) (D\d+)', ref)
    if not m or not (DECISIONS / f'{m.group(1)}.md').is_file():
        return False
    text = (DECISIONS / f'{m.group(1)}.md').read_text(encoding='utf-8')
    decisions = text.split('\n## Decisions\n', 1)
    return len(decisions) == 2 and re.search(rf'(?m)^\d+\. \*\*{m.group(2)}:', decisions[1].split('\n## ', 1)[0]) is not None

def ledger_rows():
    """FINDINGS.md's table rows as (id, kind, gates, statement)."""
    rows = []
    for line in FINDINGS.read_text(encoding='utf-8').splitlines():
        if line.startswith('| F-'):
            cells = [c.strip() for c in re.split(r'(?<!\\)\|', line)[1:-1]]
            rows.append((cells[0], cells[2], {g.strip() for g in cells[5].split(',')}, cells[4]))
    return rows

def cited_ids(text):
    """Every F-NN that `text` cites, a range written "F-NN to F-MM" included."""
    ids = set(re.findall(r'\bF-\d{2}\b', text))
    for a, b in re.findall(r'\bF-(\d{2}) to F-(\d{2})\b', text):
        ids |= {f'F-{n:02d}' for n in range(int(a), int(b) + 1)}
    return ids

def post_genesis_ids(rows):
    """The ledger entries recorded from the owner's post-genesis report, after this decision record."""
    return {r[0] for r in rows if POST_GENESIS in r[3]}

def unit_setting(name):
    return re.findall(rf'^{name}=(\S+)$', INSTALL, re.M)

def missing(section, ids):
    """The pending ids that `section` does not name (each written as `id`)."""
    return [i for i in ids if f'`{i}`' not in section]

class AcceptedRisks(unittest.TestCase):
    def test_every_pending_item_of_the_public_network_is_listed(self):
        section = accepted_risks(RECORD.read_text(encoding='utf-8'))
        ids = pending_ids()
        self.assertEqual(public_mainnet('waivers'), ['R13', 'R21', 'R26'])
        self.assertGreaterEqual(len(public_mainnet('conditions')), 7)
        self.assertGreaterEqual(len([i for i in ids if i.startswith('plan-gate-')]), 1)
        self.assertEqual(missing(section, ids), [])

    def test_the_check_sees_a_missing_item_or_section(self):
        section = accepted_risks(RECORD.read_text(encoding='utf-8'))
        self.assertEqual(missing(section.replace('`admission-waiver-R21`', 'the R21 waiver'), pending_ids()), ['admission-waiver-R21'])
        self.assertEqual(missing(section.replace('`plan-gate-Q1`', 'Q1'), pending_ids()), ['plan-gate-Q1'])
        self.assertRaises(AssertionError, accepted_risks, '# A record without the section\n')

    def test_the_record_quotes_the_owner_and_claims_no_more_than_d1(self):
        text = RECORD.read_text(encoding='utf-8')
        self.assertIn('Approving a decision is not approval to deploy', text)
        self.assertIn('قبول المخاطر الموثقة المرتبطة به', text)
        self.assertIn('دون تزوير نتائج الجاهزية أو إضعاف قواعد الإجماع', text)
        self.assertIn('Each item is covered by D1 by the owner\'s words; this record claims no more for it.',
                      ' '.join(accepted_risks(text).split()))

    def test_the_record_states_what_the_r13_and_r21_waivers_remove(self):
        # The R13 and R21 waivers drop terms the gate enforces (gate.rs r13_continuous_kind and r21_history; admission.rs
        # Scope::PublicNetwork removes R13 and R21 from the violations), so the record may not say that no waiver removes
        # an enforced check, and it names the terms removed and the terms that stay enforced.
        section = ' '.join(accepted_risks(RECORD.read_text(encoding='utf-8')).split())
        self.assertNotIn('no waiver removes a check that the node enforces', section)
        for text in ('The R26 waiver skips only the predicates that read RpcGuard fields the node does not enforce.',
                     'The R13 and R21 waivers remove terms the parameter gate enforces',
                     "R13's continuous-kind term and R21's history-budget term",
                     'The Specification and ExperimentalDevnet scopes still refuse a network profile on those terms.',
                     "Every other term of R13 and R21 stays enforced: a finite schedule's checks, retained blocks >= min(K_hist, H_END) and the resource facts."):
            self.assertIn(text, section)

    def test_d6_states_the_owners_condition_of_passing_the_tests(self):
        # The owner's words: «بعد اجتياز الاختبارات الفنية والأمنية المطلوبة», after PASSING the required tests.
        text = RECORD.read_text(encoding='utf-8')
        self.assertIn('بعد اجتياز الاختبارات الفنية والأمنية المطلوبة', text)
        d6 = ' '.join(text.split('6. **D6:', 1)[1].split('\n7. ', 1)[0].split())
        self.assertIn('The launch is a full public launch of PoCol Mainnet, after passing the required technical and security tests', d6)
        self.assertIn('a test that cannot run is reported as not run, never as passed', d6)


class AcceptedRisksAgainstTheLedger(unittest.TestCase):
    """The record and the finding ledger agree: every id the record cites is an entry, every entry of an open gate the
    record names is cited, D1, D2 and D5 each have an accepted-risk entry that cites this record, and D2 states the
    unit's memory limits as install-linux.sh writes them."""
    def test_every_cited_entry_exists_and_every_entry_of_an_open_gate_is_cited(self):
        section = accepted_risks(RECORD.read_text(encoding='utf-8'))
        rows = ledger_rows()
        ids = {r[0] for r in rows}
        self.assertNotIn('is not on this branch', section)
        self.assertIn('[FINDINGS.md](../public-network/FINDINGS.md)', section)
        self.assertLessEqual(cited_ids(section), ids, sorted(cited_ids(section) - ids))
        gates = [g[len('plan-gate-'):] for g in pending_ids() if g.startswith('plan-gate-')]
        later = post_genesis_ids(rows)
        for gate in gates:
            want = {r[0] for r in rows if gate in r[2]}
            self.assertTrue(want, gate)
            self.assertEqual(sorted(want - later - cited_ids(section)), [], f'plan-gate-{gate}')
        # An entry recorded after this record is an open finding, which no decision of this record accepts or names.
        self.assertEqual(sorted(later & cited_ids(section)), [])
        self.assertEqual(sorted(r[0] for r in rows if r[0] in later and r[1] != 'open-finding'), [])
        self.assertEqual(sorted(later - cited_ids(GENESIS_RECORD.read_text(encoding='utf-8'))), [])
        self.assertEqual(cited_ids('F-42 to F-44, F-50'), {'F-42', 'F-43', 'F-44', 'F-50'})

    def test_d1_d2_and_d5_each_have_an_accepted_risk_entry_citing_this_record(self):
        rows = {r[0]: r for r in ledger_rows()}
        section = accepted_risks(RECORD.read_text(encoding='utf-8'))
        for decision, entry in (('D1', 'F-92'), ('D2', 'F-93'), ('D5', 'F-10')):
            fid, kind, _, statement = rows[entry]
            self.assertEqual(kind, 'accepted-risk', entry)
            self.assertIn(f'owner decision of 2026-11-20 ({decision})' if entry != 'F-10' else 'owner decision 2026-11-20 D5',
                          statement, entry)
            self.assertIn(entry, section)
        self.assertIn('OWNER-DECISIONS-2026-11-20.md', FINDINGS.read_text(encoding='utf-8'))

    def test_d2_states_the_units_memory_limits(self):
        text = RECORD.read_text(encoding='utf-8')
        d2 = ' '.join(text.split('2. **D2:', 1)[1].split('\n3. ', 1)[0].split())
        (cap,) = unit_setting('MemoryMax')
        self.assertIn(f'`MemoryMax={cap}`', d2)
        self.assertEqual(unit_setting('MemorySwapMax'), ['0'])
        self.assertIn('`MemorySwapMax=0`', d2)
        if not unit_setting('MemoryHigh'):
            self.assertNotIn('MemoryHigh=', text)
            self.assertIn('no `MemoryHigh`', d2)
        self.assertIn('F-23', d2)


class ApprovalReferences(unittest.TestCase):
    def test_every_approval_reference_names_a_decision_of_an_existing_record(self):
        refs = approval_references()
        self.assertGreaterEqual(len(refs), 2)
        for where, ref in refs.items():
            self.assertTrue(resolves(ref), f'{where}: {ref!r}')
        self.assertEqual(refs['readiness.rs PUBLIC_MAINNET_APPROVAL'], 'OWNER-DECISIONS-2026-11-20 D1')
        self.assertEqual(refs['set_network_identity.py PUBLIC_APPROVAL'], refs['readiness.rs PUBLIC_MAINNET_APPROVAL'])

    def test_the_resolver_refuses_what_names_no_decision(self):
        for ref in ('OWNER-DECISIONS-2026-11-20 D9', 'OWNER-DECISIONS-2026-11-21 D1', 'OWNER-DECISIONS-2026-11-20', 'D1',
                    'OWNER-DECISIONS-2026-11-18 D1'):
            self.assertFalse(resolves(ref), ref)
        for ref in ('OWNER-DECISIONS-2026-11-20 D1', 'OWNER-DECISIONS-2026-11-20 D5'):
            self.assertTrue(resolves(ref), ref)

if __name__ == '__main__':
    unittest.main()
