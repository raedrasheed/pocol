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
