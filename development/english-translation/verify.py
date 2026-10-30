#!/usr/bin/env python3
"""Verify English presentation integrity without rewriting historical attestations."""
import collections
import hashlib
import json
from pathlib import Path
import re
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]
HERE = Path(__file__).resolve().parent
ARABIC = re.compile(r'[\u0600-\u06ff\u0750-\u077f\u08a0-\u08ff\ufb50-\ufdff\ufe70-\ufefc]')
NUM = re.compile(r'\d+(?:\.\d+)?')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def original(revision, path):
    return subprocess.check_output(['git', '-C', str(ROOT), 'show', revision + ':' + path])


def main():
    manifest = json.loads((HERE / 'MANIFEST.json').read_text())
    revision = manifest['sourceRevision']
    errors = []
    for entry in manifest['files']:
        p = ROOT / entry['path']
        if not p.is_file() or digest(p.read_bytes()) != entry['sha256']:
            errors.append('Current digest: ' + entry['path'])
        if entry.get('originalPath') and digest(original(revision, entry['originalPath'])) != entry['originalSha256']:
            errors.append('Original digest: ' + entry['originalPath'])
    paths = set(subprocess.check_output(
        ['git', '-C', str(ROOT), 'ls-files', '--cached', '--others', '--exclude-standard'], text=True
    ).splitlines())
    raw_hits = []
    for name in sorted(paths):
        p = ROOT / name
        if not p.is_file() or name == manifest['excludedThesis']['path']:
            continue
        try:
            text = p.read_text(encoding='utf-8')
        except UnicodeError:
            continue
        if ARABIC.search(text):
            raw_hits.append(name)
    errors.extend('Raw Arabic text: ' + name for name in raw_hits)
    thesis = manifest['excludedThesis']
    if digest((ROOT / thesis['path']).read_bytes()) != thesis['originalSha256']:
        errors.append('Excluded thesis changed')
    ref = ROOT / 'development/m1/reference'
    aggregate = (ref / 'FINAL_DESIGN.md').read_text()
    for name in manifest['referenceSections']:
        p = ref / (name + '.md')
        before = original(revision, p.relative_to(ROOT).as_posix()).decode().splitlines()
        after = p.read_text().splitlines()
        if len(before) != len(after):
            errors.append('Reference line count: ' + name)
        for line, (a, b) in enumerate(zip(before, after), 1):
            if collections.Counter(NUM.findall(a)) != collections.Counter(NUM.findall(b)):
                errors.append('Reference numerical tokens: %s:%d' % (name, line))
        if '#' + p.read_text() + '\n' not in aggregate:
            errors.append('Aggregate section differs: ' + name)
    zip_count = 0
    for entry in manifest['files']:
        if not entry['path'].endswith('.zip'):
            continue
        with zipfile.ZipFile(ROOT / entry['path']) as archive:
            if archive.testzip() is not None:
                errors.append('ZIP integrity: ' + entry['path'])
            for item in archive.namelist():
                try:
                    text = archive.read(item).decode('utf-8')
                except UnicodeError:
                    continue
                if ARABIC.search(text):
                    errors.append('Arabic ZIP text: ' + entry['path'] + ':' + item)
        zip_count += 1
    print(json.dumps({'result': 'PASS' if not errors else 'FAIL',
                      'manifestFiles': len(manifest['files']),
                      'referenceSections': len(manifest['referenceSections']),
                      'translatedArchives': zip_count,
                      'rawArabicFiles': raw_hits,
                      'thesisUnchanged': not any('thesis' in e for e in errors),
                      'errors': errors}, indent=2))
    raise SystemExit(bool(errors))


if __name__ == '__main__':
    main()
