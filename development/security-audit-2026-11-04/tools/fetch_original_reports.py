#!/usr/bin/env python3
"""Read original PoCol Actions artifacts and CodeQL through the official gh API.

Requires normal GitHub API access and existing gh authentication. This does not
change network policy, credentials, workflows, alert state or production data.
Every API call is GET. Artifacts stay outside the checkout in a new directory.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import tempfile
import zipfile
from datetime import datetime, timezone
from urllib.parse import urlencode, urlsplit

REPO = 'palcertcom/pocol'
ARTIFACTS = (
    {'label': 'osv', 'run': 41875907642, 'id': 13345977967,
     'name': 'OSV Scanner SARIF file', 'head_prefix': '04a7e0d',
     'sha256': '3dc96f2ff2d8b883d01249f64a57466f82c0ad7e844fb2a738eae8c0b9fdb300'},
    {'label': 'semgrep', 'run': 41889848835, 'id': 13351764891,
     'name': 'semgrep-pocol-findings', 'head_prefix': '01e88da',
     'sha256': 'e165aa022f4540335499beead40fd02d6b0a0966fdc20073d6ef0a0924972fa0'},
)


def gh_get(endpoint, target, accept='application/vnd.github+json'):
    with target.open('xb') as out:
        subprocess.run(['gh', 'api', '--hostname', 'github.com', '--method', 'GET',
                        '-H', 'X-GitHub-Api-Version: 2022-11-28',
                        '-H', 'Accept: '+accept, endpoint],
                       stdout=out, stderr=subprocess.PIPE, check=True, timeout=90)


def get_json(endpoint, target):
    gh_get(endpoint, target)
    return json.loads(target.read_text(encoding='utf-8'))


def validate_metadata(expected, run, artifact):
    if run.get('id') != expected['run'] or run.get('repository', {}).get('full_name') != REPO:
        raise ValueError('Run identity does not match the original repository/run')
    if not str(run.get('head_sha', '')).startswith(expected['head_prefix']):
        raise ValueError('Run SHA does not match the original scan')
    if (artifact.get('id') != expected['id'] or artifact.get('name') != expected['name']
            or artifact.get('workflow_run', {}).get('id') != expected['run']):
        raise ValueError('Artifact identity does not match the original scan')
    if artifact.get('expired') is not False:
        raise ValueError('Artifact expired or expiration state was not supplied')
    if artifact.get('digest') != 'sha256:'+expected['sha256']:
        raise ValueError('Artifact metadata digest differs from the public run page')
    if not 0 < artifact.get('size_in_bytes', 0) <= 3_000_000:
        raise ValueError('Unexpected artifact size')


def extract_verified(archive, expected_digest, destination):
    with archive.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    if digest != expected_digest:
        raise ValueError('Downloaded ZIP SHA-256 differs from the original artifact digest')
    with zipfile.ZipFile(archive) as bundle:
        members = bundle.infolist()
        if len(members) > 100 or sum(i.file_size for i in members) > 32 * 1024 * 1024:
            raise ValueError('Unexpected expanded archive size')
        paths = set()
        # Validate every name before writing any archive member.
        for item in members:
            name = PurePosixPath(item.filename)
            if (not name.parts or name.is_absolute() or '..' in name.parts
                    or '\\' in item.filename or any(':' in part for part in name.parts)
                    or any(part.rstrip(' .') != part for part in name.parts)
                    or stat.S_ISLNK(item.external_attr >> 16) or str(name) in paths):
                raise ValueError('Unsafe or duplicate ZIP member')
            if item.file_size > 8 * 1024 * 1024:
                raise ValueError('Unexpected archive member size')
            paths.add(str(name))
        destination.mkdir(mode=0o700)
        for item in members:
            target = destination.joinpath(*PurePosixPath(item.filename).parts)
            if item.is_dir():
                target.mkdir(mode=0o700, parents=True, exist_ok=True)
            else:
                target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
                with bundle.open(item) as source, target.open('xb') as out:
                    data = source.read(8 * 1024 * 1024 + 1)
                    if len(data) != item.file_size:
                        raise ValueError('Archive member length mismatch')
                    out.write(data)
    return sorted(str(path.relative_to(destination)) for path in destination.rglob('*') if path.is_file())


def collect(output):
    os.umask(0o077)
    for parent in Path(__file__).resolve().parents:
        if (parent/'.git').exists():
            if output.resolve().is_relative_to(parent):
                raise ValueError('Choose a new output directory outside the repository')
            break
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    report = {'repository': REPO, 'captured_at_utc': datetime.now(timezone.utc).isoformat(),
              'read_only': True, 'results': [], 'errors': []}
    base = 'repos/'+REPO

    def attempt(label, operation):
        try:
            detail = operation()
            report['results'].append({'label': label, 'detail': detail})
            print('OK '+label)
        except (OSError, ValueError, KeyError, TypeError, zipfile.BadZipFile,
                subprocess.SubprocessError) as error:
            # Do not print subprocess stderr or credential-bearing environment values.
            detail = {'label': label, 'error_type': type(error).__name__,
                      'message': str(error) if isinstance(error, ValueError)
                      else 'Read failed; check network, gh permissions, or file validity.'}
            if isinstance(error, subprocess.CalledProcessError):
                # Redirect errors may contain signed URLs: preserve host names only.
                stderr = (error.stderr or b'').decode('utf-8', errors='replace')
                hosts = set()
                for url in re.findall(r'https?://[^\s\x22\x27]+', stderr):
                    try: host = urlsplit(url).hostname
                    except ValueError: continue
                    if host and re.fullmatch(r'[a-zA-Z0-9.-]+', host): hosts.add(host)
                detail['request_hosts'] = sorted(hosts)
                code = re.search(r'HTTP (\d{3})', stderr)
                if code: detail['http_status'] = int(code[1])
            report['errors'].append(detail)
            print('FAILED '+label+' ('+type(error).__name__+')')

    for item in ARTIFACTS:
        def artifact_job(item=item):
            label = item['label']
            run = get_json(base+'/actions/runs/'+str(item['run']), output/(label+'-run.json'))
            metadata = get_json(base+'/actions/artifacts/'+str(item['id']), output/(label+'-artifact.json'))
            validate_metadata(item, run, metadata)
            archive = output/(label+'-original.zip')
            gh_get(base+'/actions/artifacts/'+str(item['id'])+'/zip', archive)
            return {'run': item['run'], 'artifact': item['id'], 'sha256': item['sha256'],
                    'files': extract_verified(archive, item['sha256'], output/label)}
        attempt(item['label'], artifact_job)

    def alert_job():
        data = get_json(base+'/code-scanning/alerts/7', output/'codeql-alert-7.json')
        if data.get('number') != 7:
            raise ValueError('Unexpected alert number')
        return {'number': 7, 'state': data.get('state'), 'tool': data.get('tool', {}).get('name')}
    attempt('codeql-alert-7', alert_job)

    def instances_job():
        data = get_json(base+'/code-scanning/alerts/7/instances?per_page=100',
                        output/'codeql-alert-7-instances.json')
        if not isinstance(data, list):
            raise ValueError('Unexpected alert instances response')
        return {'instances': len(data), 'possibly_truncated': len(data) == 100}
    attempt('codeql-alert-7-instances', instances_job)

    def analyses_job():
        query = urlencode({'tool_name': 'CodeQL', 'ref': 'refs/heads/main', 'per_page': 100})
        data = get_json(base+'/code-scanning/analyses?'+query, output/'codeql-main-analyses.json')
        if not isinstance(data, list):
            raise ValueError('Unexpected analysis list response')
        latest = {}
        for analysis in sorted(data, key=lambda a: a.get('created_at', ''), reverse=True):
            if analysis.get('tool', {}).get('name') == 'CodeQL':
                latest.setdefault((analysis.get('category'), analysis.get('analysis_key')), analysis)
        if not latest:
            raise ValueError('No CodeQL analysis was returned for main')
        if len(latest) > 10:
            raise ValueError('More than 10 analysis categories; review metadata before continuing')
        for analysis in latest.values():
            number = analysis['id']
            if not isinstance(number, int) or number <= 0:
                raise ValueError('Invalid analysis identifier')
            sarif = output/('codeql-main-'+str(number)+'.sarif.json')
            gh_get(base+'/code-scanning/analyses/'+str(number), sarif, 'application/sarif+json')
            if not isinstance(json.loads(sarif.read_text()).get('runs'), list):
                raise ValueError('Unexpected CodeQL SARIF structure')
        return {'analyses_returned': len(data), 'possibly_truncated': len(data) == 100,
                'latest_categories_downloaded': len(latest)}
    attempt('codeql-main-analyses', analyses_job)

    report['files'] = {str(p.relative_to(output)): hashlib.file_digest(p.open('rb'), 'sha256').hexdigest()
                       for p in sorted(output.rglob('*')) if p.is_file()}
    (output/'manifest.json').write_text(json.dumps(report, indent=2)+'\n', encoding='utf-8')
    return 1 if report['errors'] else 0


def self_test():
    import unittest
    from unittest.mock import patch
    class Validation(unittest.TestCase):
        def setUp(self):
            self.temp = tempfile.TemporaryDirectory(); self.addCleanup(self.temp.cleanup)
            self.root = Path(self.temp.name)
        def archive(self, name, symlink=False):
            path = self.root/'input.zip'
            with zipfile.ZipFile(path, 'w') as bundle:
                entry = zipfile.ZipInfo(name)
                if symlink: entry.external_attr = (stat.S_IFLNK | 0o777) << 16
                bundle.writestr(entry, '{"results":[]}')
            return path, hashlib.sha256(path.read_bytes()).hexdigest()
        def test_valid_archive_preserves_json(self):
            path, digest = self.archive('semgrep-results.json')
            self.assertEqual(extract_verified(path, digest, self.root/'out'), ['semgrep-results.json'])
            self.assertEqual(json.loads((self.root/'out/semgrep-results.json').read_text()), {'results': []})
        def test_bad_digest_is_refused_before_extraction(self):
            path, _ = self.archive('result.json')
            with self.assertRaises(ValueError): extract_verified(path, '0'*64, self.root/'out')
            self.assertFalse((self.root/'out').exists())
        def test_traversal_absolute_windows_and_symlink_are_refused(self):
            for name, link in [('../outside.json',False),('/outside.json',False),('C:/outside.json',False),('x/result.json:stream',False),('.. /outside.json',False),('x\\outside.json',False),('link.json',True)]:
                with self.subTest(name=name):
                    path, digest = self.archive(name,link)
                    with self.assertRaises(ValueError):extract_verified(path,digest,self.root/'out')
                    self.assertFalse((self.root/'out').exists())
        def test_wrong_run_expiry_and_changed_digest_are_refused(self):
            item = ARTIFACTS[0]
            run = {'id':item['run'],'repository':{'full_name':REPO},'head_sha':item['head_prefix']+'0'*33}
            good = {'id':item['id'],'name':item['name'],'workflow_run':{'id':item['run']},'expired':False,'digest':'sha256:'+item['sha256'],'size_in_bytes':16000}
            validate_metadata(item,run,good)
            for change in [{'expired':True},{'workflow_run':{'id':1}},{'digest':'sha256:'+'0'*64}]:
                with self.subTest(change=change):
                    with self.assertRaises(ValueError):validate_metadata(item,run,{**good,**change})
        def test_full_collection_and_independent_permission_failure(self):
            import io
            import contextlib
            import copy
            fixtures = copy.deepcopy(ARTIFACTS)
            payload = {}
            for item in fixtures:
                data = io.BytesIO()
                with zipfile.ZipFile(data,'w') as bundle:
                    bundle.writestr(item['label']+'-results.json','{"results":[]}')
                item['sha256'] = hashlib.sha256(data.getvalue()).hexdigest()
                payload[item['id']] = data.getvalue()
            fail_alert = False
            def fake_get(endpoint,target,accept='application/vnd.github+json'):
                for item in fixtures:
                    if endpoint.endswith('/actions/runs/'+str(item['run'])):
                        target.write_text(json.dumps({'id':item['run'],'repository':{'full_name':REPO},'head_sha':item['head_prefix']+'0'*33}));return
                    if endpoint.endswith('/actions/artifacts/'+str(item['id'])):
                        target.write_text(json.dumps({'id':item['id'],'name':item['name'],'workflow_run':{'id':item['run']},'expired':False,'digest':'sha256:'+item['sha256'],'size_in_bytes':len(payload[item['id']])}));return
                    if endpoint.endswith('/actions/artifacts/'+str(item['id'])+'/zip'):
                        target.write_bytes(payload[item['id']]);return
                if endpoint.endswith('/code-scanning/alerts/7'):
                    if fail_alert:raise subprocess.CalledProcessError(1,['gh','api'],stderr=b'Get "https://storage.example.invalid/archive?sig=DO-NOT-PRESERVE": Forbidden')
                    value={'number':7,'state':'open','tool':{'name':'CodeQL'}}
                elif '/instances?' in endpoint:value=[{'ref':'refs/heads/main'}]
                elif '/analyses?' in endpoint:value=[{'id':3,'category':'python','analysis_key':'test','created_at':'2026-11-05','tool':{'name':'CodeQL'}}]
                elif endpoint.endswith('/analyses/3'):
                    self.assertEqual(accept,'application/sarif+json');value={'version':'2.1.0','runs':[{'tool':{'driver':{'name':'CodeQL'}}}]}
                else:raise AssertionError('Unexpected endpoint')
                target.write_text(json.dumps(value))
            previous = os.umask(0o077)
            try:
                with patch(__name__+'.ARTIFACTS',fixtures),patch(__name__+'.gh_get',side_effect=fake_get),contextlib.redirect_stdout(io.StringIO()):
                    target=self.root/'complete';self.assertEqual(collect(target),0)
                    manifest=json.loads((target/'manifest.json').read_text());self.assertEqual(len(manifest['results']),5);self.assertFalse(manifest['errors'])
                    fail_alert=True;target=self.root/'partial';self.assertEqual(collect(target),1)
                    manifest=json.loads((target/'manifest.json').read_text());self.assertEqual([e['label'] for e in manifest['errors']],['codeql-alert-7'])
                    self.assertEqual(manifest['errors'][0]['request_hosts'],['storage.example.invalid'])
                    self.assertNotIn('DO-NOT-PRESERVE',(target/'manifest.json').read_text())
                    self.assertTrue((target/'osv/osv-results.json').is_file());self.assertTrue((target/'semgrep/semgrep-results.json').is_file())
            finally:os.umask(previous)
    with patch('subprocess.run', side_effect=AssertionError('Self-test must stay offline')):
        return 0 if unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(Validation)).wasSuccessful() else 1


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', nargs='?', type=Path, help='New private directory outside the repository')
    parser.add_argument('--self-test', action='store_true', help='Validate fixtures offline; no API requests')
    args = parser.parse_args()
    if args.self_test: raise SystemExit(self_test())
    if args.output is None: parser.error('An output directory is required')
    raise SystemExit(collect(args.output))
