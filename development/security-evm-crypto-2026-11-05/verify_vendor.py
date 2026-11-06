#!/usr/bin/env python3
"""Check the reviewed local fork files without network, imports, or code execution."""
import hashlib
import json
from pathlib import Path
import sys


def main():
    root = Path(__file__).resolve().parents[2] / 'prototypes/third-party/evm-crypto'
    problems = []
    count = 0
    try:
        manifest = json.loads((root / 'MANIFEST.json').read_text(encoding='utf-8'))
        for package in manifest['packages']:
            if Path(package['directory']).name != package['directory']:
                raise ValueError('Unsafe package directory')
            directory = root / package['directory']
            if directory.is_symlink() or not directory.is_dir():
                raise ValueError('Library directory must be a regular directory')
            expected = package['current_file_sha256']
            actual = set()
            for path in directory.rglob('*'):
                if path.is_symlink():
                    raise ValueError('Unexpected symlink: ' + str(path.relative_to(root)))
                elif path.is_file():
                    actual.add(path.relative_to(directory).as_posix())
            if actual != set(expected):
                problems.append('Unreviewed/missing files in ' + package['directory'])
            for name, checksum in expected.items():
                relative = Path(name)
                if relative.is_absolute() or '..' in relative.parts:
                    raise ValueError('Unsafe manifest file path')
                path = directory / relative
                if path.is_symlink() or not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != checksum:
                    problems.append('Changed source: ' + str(path.relative_to(root)))
                count += 1
        for name, checksum in manifest['additional_file_sha256'].items():
            path = root / name
            if Path(name).name != name or path.is_symlink() or hashlib.sha256(path.read_bytes()).hexdigest() != checksum:
                problems.append('Changed review/license file: ' + name)
    except (OSError, ValueError, KeyError, TypeError) as error:
        problems.append(str(error))
    print(json.dumps({'ok': not problems, 'reviewed_library_files': count, 'problems': problems}, indent=2))
    return 1 if problems else 0


if __name__ == '__main__':
    sys.exit(main())
