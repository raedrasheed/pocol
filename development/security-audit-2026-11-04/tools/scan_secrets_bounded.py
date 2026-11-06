"""Offline detect-secrets scan with per-file deadlines and hash-only results.

Run using the isolated audit venv from the repository root. No secret is verified
against a service. Timeout rows are coverage gaps, never clean results.
"""
import argparse
import json
import multiprocessing
from pathlib import Path
import secrets
import signal
import subprocess


def expired(signum, frame):
    raise TimeoutError('per-file deadline')


def initialize():
    from detect_secrets.main import parse_args
    from detect_secrets.settings import get_settings
    parse_args(['scan', '--no-verify'])
    # Review inline-allowlisted candidates too; add no project exclusions.
    get_settings().disable_filters('detect_secrets.filters.allowlist.is_line_allowlisted')
    signal.signal(signal.SIGALRM, expired)


def scan(job):
    from detect_secrets.core.scan import scan_file
    path, timeout = job
    candidates = set()
    status = 'completed'
    signal.setitimer(signal.ITIMER_REAL, timeout)
    try:
        candidates.update(scan_file(path))
    except TimeoutError:
        status = 'timeout-partial'
    except Exception as error:
        # Record only the exception type, not an error containing source text.
        status = 'error-' + type(error).__name__
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
    return {'file': path, 'status': status,
            'results': sorted((s.json() for s in candidates), key=lambda s:(s.get('line_number',0),s['type'],s['hashed_secret']))}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', type=Path)
    parser.add_argument('--seconds-per-file', type=float, default=5)
    parser.add_argument('--jobs', type=int, default=2)
    args = parser.parse_args()
    if not 0 < args.seconds_per_file <= 60 or not 1 <= args.jobs <= 4:
        parser.error('Use 1-4 jobs and a per-file deadline of at most 60 seconds')
    paths = subprocess.check_output(['git','ls-files','-z']).decode().split('\0')
    paths = [path for path in paths if path and Path(path).is_file()]
    initialize()
    from detect_secrets.core.scan import scan_line
    from detect_secrets.settings import get_settings
    control = len(list(scan_line('api_key = "'+secrets.token_urlsafe(32)+'"')))
    if not control:
        raise RuntimeError('Positive control was not detected; scan is invalid')
    summary = {'tool':'detect-secrets 1.5.0', 'networkVerification':False,
               'positiveControlMatches':control, 'filesRequested':len(paths),
               'secondsPerFile':args.seconds_per_file, 'jobs':args.jobs,
               'settings':get_settings().json()}
    with args.output.open('w') as out:
        out.write(json.dumps({'configuration':summary})+'\n');out.flush()
        with multiprocessing.Pool(args.jobs, initializer=initialize) as pool:
            for result in pool.imap_unordered(scan,((path,args.seconds_per_file) for path in paths)):
                out.write(json.dumps(result)+'\n');out.flush()
