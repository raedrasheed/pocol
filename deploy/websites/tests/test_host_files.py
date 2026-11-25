#!/usr/bin/env python3
"""deploy/websites/tools/host_files.py (SITE-02): a host listing is compared with a site's snapshot. A file the
snapshot no longer has (the retired network's invitation and extension package) is to delete, a changed file differs,
a snapshot file the host lacks is missing (a kept-out release file is named as such), and host configuration
(api/config.php, cPanel's .user.ini and php.ini, error_log, .well-known/, cgi-bin/) is kept. A host .htaccess that
differs is to merge, not to replace (cPanel writes its PHP handler there), and is kept once a copy of it holds the
snapshot's directives unchanged. Synthetic listings only; no host is contacted."""
import contextlib, hashlib, io, sys, tempfile, unittest
from unittest import mock
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
                self.assertIn(f'OK {site}: delete 0, differs 0, merge 0, missing 0', out)

    def test_retired_files_left_by_an_overwrite_upload_are_to_delete(self):
        listing = listing_of('pocol.net')
        # What the 2026-11-01 host still serves after an overwrite upload of the current snapshot.
        stale = {'downloads/network.pocol.json': 'f4ed1373' + '0' * 56, 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip': '85' * 32,
                 'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip.sha256': 'ef' * 32, 'downloads/PoCol-Apps-SHA256SUMS.txt': '1' * 64,
                 'api/config.php': '2' * 64, '.well-known/acme-challenge/x': '3' * 64}
        rc, out, _ = self.run_main('pocol.net', text({**listing, **stale}))
        self.assertEqual(rc, 1)
        for path in ('downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip',
                     'downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip.sha256', 'downloads/PoCol-Apps-SHA256SUMS.txt'):
            self.assertIn(f'DELETE   {path}  (not in the snapshot)', out)
        self.assertIn('DIFFERS  downloads/network.pocol.json', out)
        self.assertNotIn('DELETE   downloads/network.pocol.json', out)
        self.assertIn('KEEP     api/config.php', out)
        self.assertIn('KEEP     .well-known/acme-challenge/x', out)
        # The retired Windows package was kept (KEEP) while the pages linked it under the LEGACY_NATIVE exemption. The
        # owner's request of 2026-11-23 removed the links and the exemption, so the host's copy is a file to delete, like
        # the retired Linux package; a name that LEGACY_NATIVE lists is still kept.
        _, legacy, _ = self.run_main('pocol.net', text({**listing, 'downloads/PoCol-Mainnet-0.4.0-Windows.zip': '6' * 64,
                                                       'downloads/PoCol-Mainnet-0.4.0-Linux.zip': '7' * 64}))
        self.assertIn('DELETE   downloads/PoCol-Mainnet-0.4.0-Windows.zip  (not in the snapshot)', legacy)
        self.assertIn('DELETE   downloads/PoCol-Mainnet-0.4.0-Linux.zip', legacy)
        with mock.patch.object(hf.si, 'LEGACY_NATIVE', frozenset({'PoCol-Mainnet-0.4.0-Windows.zip'})):
            _, exempt, _ = self.run_main('pocol.net', text({**listing, 'downloads/PoCol-Mainnet-0.4.0-Windows.zip': '6' * 64}))
        self.assertIn('KEEP     downloads/PoCol-Mainnet-0.4.0-Windows.zip', exempt)
        self.assertIn('delete 3, differs 1, merge 0, missing 0, keep 2', out)
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

    def test_cpanel_php_settings_and_error_logs_are_kept(self):
        # 2026-11-22 review: cPanel's MultiPHP INI Editor writes .user.ini or php.ini, and PHP writes error_log, into the
        # document root (and api/); deleting them would drop the host's PHP settings.
        listing = listing_of('pocol.net')
        host = {'.user.ini': '1' * 64, 'php.ini': '2' * 64, 'error_log': '3' * 64, 'api/error_log': '4' * 64,
                'api/.user.ini': '5' * 64, 'api/config.php': '6' * 64}
        rc, out, _ = self.run_main('pocol.net', text({**listing, **host}))
        self.assertEqual(rc, 0, out)
        for path in host:
            self.assertIn(f'KEEP     {path}  (host configuration, not in a snapshot)', out)
        # Only those names: a file that merely contains them is still to delete.
        rc, out, _ = self.run_main('pocol.net', text({**listing, 'php.ini.bak': '7' * 64, 'old_error_log.txt': '8' * 64}))
        self.assertEqual(rc, 1)
        self.assertIn('DELETE   php.ini.bak', out)
        self.assertIn('DELETE   old_error_log.txt', out)

    def test_a_differing_htaccess_is_merged_not_replaced(self):
        snapshot = (hf.SITES['pocol.net'] / '.htaccess').read_bytes()
        cpanel = (b'# php -- BEGIN cPanel-generated handler, do not edit\n'
                  b'<IfModule mime_module>\n  AddHandler application/x-httpd-ea-php82 .php .php8 .phtml\n</IfModule>\n'
                  b'# php -- END cPanel-generated handler, do not edit\n')
        host = cpanel + b'\n' + snapshot
        listing = dict(listing_of('pocol.net'), **{'.htaccess': hashlib.sha256(host).hexdigest()})
        rc, out, _ = self.run_main('pocol.net', text(listing))
        self.assertEqual(rc, 1)
        self.assertIn('MERGE    .htaccess  (host ', out)
        self.assertIn("keep its cPanel-generated block and any redirect, add the snapshot's .htaccess unchanged", out)
        self.assertNotIn('DIFFERS  .htaccess', out)
        self.assertIn('merge 1,', out)
        with tempfile.TemporaryDirectory() as tmp:
            copy = Path(tmp) / 'host.htaccess'
            copy.write_bytes(host)
            rc, out, _ = self.run_main('pocol.net', text(listing), '--host-copy', f'.htaccess={copy}')
            self.assertEqual(rc, 0, out)
            self.assertIn("KEEP     .htaccess  (merged: the host's own directives and the snapshot's, unchanged)", out)
            # The snapshot's directives before the host's, with a comment of the owner's among them, are merged as well.
            later = snapshot.replace(b'Options -Indexes', b'# kept by the owner\nOptions -Indexes') + b'\n' + cpanel
            copy.write_bytes(later)
            rc, out, _ = self.run_main('pocol.net', text(dict(listing, **{'.htaccess': hashlib.sha256(later).hexdigest()})),
                                       '--host-copy', f'.htaccess={copy}')
            self.assertEqual(rc, 0, out)
            # A copy that is not the listed file, or that lacks, changes or splits a snapshot directive, is still to merge.
            copy.write_bytes(host + b'# edited after the listing\n')
            rc, out, _ = self.run_main('pocol.net', text(listing), '--host-copy', f'.htaccess={copy}')
            self.assertEqual(rc, 1)
            self.assertIn('MERGE    .htaccess  (the --host-copy file is not the listed host file)', out)
            for bad in (cpanel + snapshot.replace(b'  Header always set X-Frame-Options "DENY"\n', b''),
                        cpanel + snapshot.replace(b'Options -Indexes', b'Options +Indexes'),
                        cpanel + snapshot.replace(b'ErrorDocument 404 /404.html\n', b'') + b'ErrorDocument 404 /404.html\n'):
                self.assertNotEqual(bad, cpanel + snapshot)
                copy.write_bytes(bad)
                rc, out, _ = self.run_main('pocol.net', text(dict(listing, **{'.htaccess': hashlib.sha256(bad).hexdigest()})),
                                           '--host-copy', f'.htaccess={copy}')
                self.assertEqual(rc, 1, out)
                self.assertIn("does not hold the snapshot's directives as one unchanged block", out)
            # A copy of a file the listing does not have is refused.
            rc, _, err = self.run_main('pocol.net', text(listing), '--host-copy', f'api/.htaccess={copy}')
            self.assertEqual(rc, 2)
            self.assertIn('the listing has no such file', err)
        # Where the snapshot has no .htaccess (ide.pocol.net), the host's own is kept, not deleted.
        _, out, _ = self.run_main('ide.pocol.net', text(dict(listing_of('ide.pocol.net'), **{'.htaccess': '9' * 64})))
        self.assertIn("KEEP     .htaccess  (the host's .htaccess; the snapshot has none here", out)
        # Any other file that differs is still uploaded again.
        rc, out, _ = self.run_main('pocol.net', text(dict(listing_of('pocol.net'), **{'robots.txt': '0' * 64})))
        self.assertIn('DIFFERS  robots.txt', out)

    def test_a_nested_document_root_is_kept_whole_with_keep_dir(self):
        # 2026-11-23 review: cPanel lets a subdomain's document root sit inside pocol.net's (the IDE's guide suggests
        # public_html/ide). Without --keep-dir every file of that site is DELETE, and deleting them would take it down.
        listing = dict(listing_of('pocol.net'), **{'ide/index.html': '1' * 64, 'ide/assets/a.js': '2' * 64,
                                                   'studio/index.html': '3' * 64, 'idea.html': '4' * 64})
        rc, out, _ = self.run_main('pocol.net', text(listing))
        self.assertEqual(rc, 1)
        self.assertIn('DELETE   ide/index.html  (not in the snapshot)', out)
        rc, out, _ = self.run_main('pocol.net', text(listing), '--keep-dir', 'ide', '--keep-dir', 'studio/')
        self.assertEqual(rc, 1, out)
        for path in ('ide/index.html', 'ide/assets/a.js', 'studio/index.html'):
            self.assertIn(f'KEEP     {path}  (another document root nested here (--keep-dir ', out)
        # Only that folder: a file whose name merely begins with it is still to delete.
        self.assertIn('DELETE   idea.html  (not in the snapshot)', out)
        self.assertIn('delete 1, differs 0, merge 0, missing 0, keep 3', out)
        # A folder that holds snapshot files, or the root itself, is refused.
        for bad in ('api', 'assets', '.', '../x', '/home/user/public_html/ide'):
            with self.subTest(bad=bad):
                rc, _, err = self.run_main('pocol.net', text(listing), '--keep-dir', bad)
                self.assertEqual(rc, 2, bad)
                self.assertIn('FAIL', err)

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
