#!/usr/bin/env python3
"""deploy/websites/tools/host_files.py (SITE-02): a host listing is compared with a site's snapshot. A file the
snapshot no longer has (the retired network's invitation and extension package) is to delete, a changed file differs,
a snapshot file the host lacks is missing (a kept-out release file is named as such), and host configuration
(api/config.php, .well-known/, cgi-bin/) is kept. Synthetic listings only; no host is contacted."""
import contextlib, hashlib, io, sys, tempfile, unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'tools'))
import host_files as hf  # noqa: E402

def listing_of(site):
    """The listing of a host that serves exactly the snapshot (the kept-out release files with their recorded digest)."""
    want, _ = hf.expected(site)
    return want

def text(listing, star=False):
    return ''.join(f'{d} {"*" if star else " "}./{p}\n' for p, d in sorted(listing.items()))

class HostFiles(unittest.TestCase):
    def run_main(self, site, listing_text, *extra):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'listing.txt'
            path.write_text(listing_text, encoding='utf-8')
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                rc = hf.main(['--site', site, '--listing', str(path), *extra])
        return rc, out.getvalue(), err.getvalue()

    def test_a_host_that_serves_the_snapshot_is_in_step(self):
        for site in hf.SITES:
            with self.subTest(site=site):
                rc, out, err = self.run_main(site, text(listing_of(site), star=site == 'ide.pocol.net'))
                self.assertEqual(rc, 0, out + err)
                self.assertIn(f'OK {site}: delete 0, differs 0, missing 0', out)

    def test_retired_files_left_by_an_overwrite_upload_are_to_delete(self):
        listing = listing_of('pocol.net')
        # What the 2026-11-01 host still serves after an overwrite upload of the current snapshot.
        stale = {'downloads/network.pocol.json': 'f4ed1373' + '0' * 56, 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip': '85' * 32,
                 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip.sha256': 'ef' * 32, 'downloads/PoCol-Apps-SHA256SUMS.txt': '1' * 64,
                 'api/config.php': '2' * 64, '.well-known/acme-challenge/x': '3' * 64}
        rc, out, _ = self.run_main('pocol.net', text({**listing, **stale}))
        self.assertEqual(rc, 1)
        for path in ('downloads/network.pocol.json', 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip',
                     'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip.sha256', 'downloads/PoCol-Apps-SHA256SUMS.txt'):
            self.assertIn(f'DELETE   {path}  (not in the snapshot)', out)
        self.assertIn('KEEP     api/config.php', out)
        self.assertIn('KEEP     .well-known/acme-challenge/x', out)
        self.assertIn('delete 4, differs 0, missing 0, keep 2', out)
        rc, out, _ = self.run_main('pocol.net', text({**listing, 'notes.txt': '4' * 64}), '--keep', 'notes.txt')
        self.assertEqual(rc, 0, out)

    def test_changed_and_missing_files_are_named(self):
        listing = listing_of('pocol.net')
        changed = dict(listing, **{'index.html': '0' * 64})
        del changed['style.css']
        rc, out, _ = self.run_main('pocol.net', text(changed))
        self.assertEqual(rc, 1)
        self.assertRegex(out, r'DIFFERS  index\.html  \(host 000000000000, snapshot [0-9a-f]{12}\)')
        self.assertIn('MISSING  style.css  (upload it)', out)

    def test_a_kept_out_release_file_is_checked_by_its_recorded_digest(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp) / 'deploy/websites/pocol.net'
            base.mkdir(parents=True)
            (base / 'index.html').write_bytes(b'<p>x</p>')
            digest = hashlib.sha256(b'<p>x</p>').hexdigest()
            (base / 'SHA256SUMS.txt').write_text(f'{"5" * 64}  downloads/PoCol-Mainnet-9.9.9-arm64.apk\n{digest}  index.html\n', encoding='utf-8')
            want, kept = hf.expected('pocol.net', tmp)
            self.assertEqual(kept, {'downloads/PoCol-Mainnet-9.9.9-arm64.apk'})
            rows = hf.compare('pocol.net', {'index.html': digest, 'SHA256SUMS.txt': want['SHA256SUMS.txt']}, root=tmp)
            self.assertEqual(rows, [('MISSING', 'downloads/PoCol-Mainnet-9.9.9-arm64.apk',
                                     'a release file kept out of Git: upload it from the signed release')])
            # A stale SHA256SUMS.txt (a line that is not the tracked file, or a file it does not list) is refused.
            (base / 'index.html').write_bytes(b'<p>y</p>')
            with self.assertRaises(hf.Refused):
                hf.expected('pocol.net', tmp)
            (base / 'index.html').write_bytes(b'<p>x</p>')
            (base / 'extra.js').write_bytes(b'1')
            with self.assertRaises(hf.Refused):
                hf.expected('pocol.net', tmp)

    def test_malformed_listings_are_refused(self):
        good = 'a' * 64
        for bad in (f'{good}  /etc/passwd\n', f'{good}  ../x\n', f'{good}  a/../../x\n', 'not a line\n',
                    f'{good}  a\n{good}  ./a\n'):
            with self.subTest(bad=bad):
                rc, out, err = self.run_main('pocol.net', bad)
                self.assertEqual(rc, 2)
                self.assertIn('FAIL', err)

if __name__ == '__main__':
    unittest.main()
