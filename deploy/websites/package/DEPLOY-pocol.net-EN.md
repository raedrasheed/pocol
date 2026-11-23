# Deploying pocol.net ({{NETWORK}}) on cPanel

| | |
| --- | --- |
| Package | `{{ZIP}}` ({{ZIP_BYTES}} bytes, {{FILE_COUNT}} files) |
| SHA-256 of the ZIP | `{{ZIP_SHA256}}` |
| Built from commit | `{{COMMIT}}` ({{DATE}}) |
| Network | {{NETWORK}}, chain ID `{{CHAIN_ID}}`, genesis hash `{{GENESIS}}` |

This package is the official site pocol.net only (not the explorer, Studio or the IDE). Nothing in it is uploaded,
and no server or DNS record is changed, until you do it by following these steps. The Arabic guide
`DEPLOY-pocol.net-AR.md` has the same steps.

## What the package holds

- `{{ZIP}}`: the contents of the Document Root. The files sit at the top of the ZIP (no enclosing folder), with
  `SHA256SUMS.txt`, the list of every file of the site with its SHA-256.
- `{{ZIP}}.sha256`: the ZIP's checksum line.
- `{{HTACCESS}}`: the site's `.htaccess`. It is **not** in the ZIP, because the host's `.htaccess` holds cPanel's own
  settings and must be merged by hand (step 5), never replaced.
- `MANIFEST.json`: the commit, the network identity and each file with its size and SHA-256.
- `PACKAGE-SHA256SUMS.txt`: the SHA-256 of every file of the package.

Release files kept out of Git that must be uploaded from the signed release: {{RELEASE_FILES}}

GitHub serves the CI artifact `{{ARTIFACT}}` as one ZIP that wraps these files: extract it once on your computer, then
use the files above. The ZIP to upload is `{{ZIP}}`, not GitHub's wrapper.

## 0. Check the package on your computer

    sha256sum -c PACKAGE-SHA256SUMS.txt

Every line must say `OK`. On Windows (PowerShell): `Get-FileHash {{ZIP}} -Algorithm SHA256` must print
`{{ZIP_SHA256}}` (case does not matter).

## 1. Before you start (cPanel)

1. **Document Root.** cPanel > Domains: note the Document Root of pocol.net (usually `public_html`). The commands
   below call it `DOCROOT`; set it first in cPanel > Terminal:

        DOCROOT="$HOME/public_html"

2. **Nested sites.** In the same list, check whether another domain or subdomain (ide.pocol.net, studio.pocol.net)
   has its Document Root *inside* pocol.net's, for example `public_html/ide`. If so, that folder belongs to the other
   site: it must never be deleted. The commands below show where to name it (`ide` in the examples).
3. **PHP.** cPanel > MultiPHP Manager: set pocol.net to PHP {{PHP_RECOMMENDED}} (PHP {{PHP_MINIMUM}} at least;
   `api/network.php` and `api/reward.php` do not run on PHP 8.0). cPanel > Select PHP Version > Extensions (or
   MultiPHP INI Editor): `curl` must be enabled.
4. **Explorer gateway.** The network summary (`/api/network.php`), the reward panel (`/api/reward.php`) and the
   "Get your first fee" claim page read `https://{{GATEWAY}}`. Until the new gateway is installed there and its DNS
   record moved, the summary and the reward panel answer "temporarily unavailable" (HTTP 503) and fee claims fail as
   unreachable. That is
   expected and safe: the pages refuse any data whose chain ID or genesis hash is not {{NETWORK}}'s. This package does
   not change the explorer.

## 2. Back up the current site

Keep the backup outside the Document Root: an archive inside it can be downloaded by anyone and holds
`api/config.php`.

**With Terminal (recommended):**

    STAMP=$(date -u +%Y%m%dT%H%M%SZ)
    mkdir -p ~/pocol-backups && cd "$DOCROOT"
    find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ~/pocol-backups/pocol.net-before-$STAMP-files.txt
    tar -czf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz .
    cp -p .htaccess ~/pocol-backups/htaccess-$STAMP 2>/dev/null || echo "no .htaccess on the host"
    echo "$STAMP"

Write down the printed `STAMP`. For a nested site (step 1.2) add `--exclude=./ide` to `tar` and use
`find . -path ./ide -prune -o -type f -print0` in the first command.

**With File Manager:** Settings > tick "Show Hidden Files (dotfiles)". In your home folder (outside the Document
Root), create the folder `pocol-backups` (+ Folder). Open the Document Root, Select All (deselect a nested site's
folder, step 1.2), Compress > Zip Archive. The dialog proposes an archive path inside the Document Root: replace the
whole path with `/pocol-backups/pocol.net-backup-YYYYMMDD.zip` before you press Compress, so that the archive is never
written where the site serves it (File Manager paths start at your home folder: `/public_html/...` is the Document
Root, `/pocol-backups/...` is outside it). Then Download a copy of the archive and of `.htaccess` to your computer.

Either way, download the backup to your computer before going on.

## 3. Upload and check the ZIP

Upload `{{ZIP}}` and `{{ZIP}}.sha256` to your **home folder** (not the Document Root) with File Manager > Upload.
Then, in Terminal:

    cd ~ && sha256sum -c {{ZIP}}.sha256

It must print `{{ZIP}}: OK`.

## 4. Extract into the Document Root

**Terminal:**

    unzip -o -q ~/{{ZIP}} -d "$DOCROOT"

**File Manager:** select `{{ZIP}}` in the home folder > Extract > enter the Document Root (for example
`/public_html`) > Extract Files. Allow it to overwrite existing files.

The ZIP holds no `.htaccess`, `api/config.php`, `.user.ini`, `php.ini`, `error_log`, `.well-known/` or `cgi-bin/`, so
those host files are left as they are. Keep them.

## 5. Merge `.htaccess`

1. File Manager > Settings > "Show Hidden Files". Open the Document Root and Edit `.htaccess`.
2. Keep cPanel's blocks as they are: the block from `# php -- BEGIN cPanel-generated handler, do not edit` to
   `# php -- END cPanel-generated handler, do not edit` (MultiPHP Manager writes it) and any HTTPS redirect.
3. Paste the whole content of `{{HTACCESS}}`, unchanged, before or after those blocks, as one block. Remove an
   earlier copy of the site's own directives if one is there. Save.
4. If the host has no `.htaccess`, upload `{{HTACCESS}}` into the Document Root and rename it `.htaccess`.
5. Download a copy of the saved file (for step 7): name it `host.htaccess`.

## 6. Permissions

Folders 755, files 644, for the package's paths only:

    cd "$DOCROOT"
    unzip -Z1 ~/{{ZIP}} | grep '/$' | tr '\n' '\0' | xargs -0 -r chmod 755
    unzip -Z1 ~/{{ZIP}} | grep -v '/$' | tr '\n' '\0' | xargs -0 -r chmod 644

## 7. Check the files and delete the old ones

Extracting over the old site leaves on the host every file this version no longer has, and the host keeps serving it.
The retired network's files are such files.

1. Every package file is in place (no output, then `FILES OK`):

        cd "$DOCROOT"
        grep -v '  \.htaccess$' SHA256SUMS.txt | sha256sum -c --quiet - && echo "FILES OK"

2. List the host's files:

        cd "$DOCROOT"
        find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > ~/pocol.net-files.txt

   (with a nested site: `find . -path ./ide -prune -o -type f -print0 | ...`). Download `~/pocol.net-files.txt`.
3. On a computer with a checkout of the repository at commit `{{COMMIT}}` (cloned with `git -c core.autocrlf=false`
   on Windows), run:

        python3 deploy/websites/tools/host_files.py --site pocol.net --listing pocol.net-files.txt --host-copy .htaccess=host.htaccess

   (with a nested site add `--keep-dir ide`). Delete from the host every file it marks `DELETE`, upload again every
   file marked `DIFFERS` or `MISSING` (from the ZIP), fix `.htaccess` if it says `MERGE`, then make the listing again
   and repeat until it prints `OK pocol.net`. Files marked `KEEP` (`api/config.php`, `.user.ini`, `php.ini`,
   `error_log`, `.well-known/`, `cgi-bin/`, the merged `.htaccess`) stay.

   Expected `DELETE` files, left by the retired network's site: `README-AR.txt` (the old upload note),
   `NETWORK-UPDATE-15000.txt`, `RETAINED-NATIVE-DOWNLOADS.json`, `README-MAINNET-UPDATE-AR.txt`,
   `downloads/network.pocol.json` (the retired network's file), `downloads/PoCol-Apps-SHA256SUMS.txt`, the retired
   native packages `downloads/PoCol-Mainnet-0.4.0-Windows.zip` and `downloads/PoCol-Mainnet-0.4.0-Linux.zip`, the
   retired extension packages `downloads/PoCol-Studio-IDE-Viewer-0.4.0.zip`,
   `downloads/PoCol-Studio-IDE-Viewer-0.4.1.zip` and `downloads/PoCol-Studio-Chrome.zip`, and their `.sha256` files.
   The tool's list is the one to follow.

   **Without the repository**, this lists the host files that are not in the package (review it, then delete them,
   except the `KEEP` names above and a nested site's folder):

        cd "$DOCROOT"
        LC_ALL=C comm -23 <(find . -type f | LC_ALL=C sort) \
          <( (sed 's/^[0-9a-f]\{64\}  /.\//' SHA256SUMS.txt; echo ./SHA256SUMS.txt) | LC_ALL=C sort)

## 8. Finish

- Delete `~/{{ZIP}}` and `~/{{ZIP}}.sha256` from the home folder when you no longer need them. Never leave an archive
  in the Document Root.
- If the host or a CDN caches pages (LiteSpeed Cache, Cloudflare), purge the cache.

## 9. Live checks (in your browser)

- `https://pocol.net/` in English and in Arabic (the language selector), and each page of `https://pocol.net/sitemap.xml`:
  no page or image is missing; the browser console (F12) shows no 404 and no Content-Security-Policy error.
- `https://pocol.net/no-such-page` shows the site's 404 page.
- `https://pocol.net/api/config.php` answers 403 (or 404), never a file.
- `https://pocol.net/api/network.php` answers JSON with `"chainId":{{CHAIN_ID}}` and
  `"genesisHash":"{{GENESIS}}"` once the explorer gateway is live; before that it answers 503 with
  "Network data is temporarily unavailable." (step 1.4). `https://pocol.net/api/reward.php` answers JSON, or 503 before
  the gateway is live.
- Developer tools > Network > the page's response headers include `Content-Security-Policy`,
  `X-Content-Type-Options: nosniff` and `X-Frame-Options: DENY` (they come from the merged `.htaccess`).
- `https://pocol.net/SHA256SUMS.txt` is the package's file: its SHA-256 is the one `MANIFEST.json` gives for
  `SHA256SUMS.txt`.

## 10. Rollback (restore the previous site)

**With Terminal**, using the `STAMP` of step 2:

    cd "$DOCROOT"
    tar -xzf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz
    LC_ALL=C comm -13 <(cut -c67- ~/pocol-backups/pocol.net-before-$STAMP-files.txt | LC_ALL=C sort) <(find . -type f | LC_ALL=C sort) > ~/pocol-backups/added-$STAMP.txt
    cat ~/pocol-backups/added-$STAMP.txt

`tar` puts back every earlier file, the earlier `.htaccess` included. The last two commands list, in
`~/pocol-backups/added-$STAMP.txt`, the files that this package added and the earlier site did not have. Review that
list and remove from it every line to keep (a `KEEP` file, such as an `error_log` PHP wrote since). With a nested site
(step 1.2), use `find . -path ./ide -prune -o -type f -print` in the third command, so that no file of that site is
listed. Then delete the listed files, and the folders that this package created and that are now empty (the earlier
site's folders, an empty `cgi-bin/` included, stay):

    tr '\n' '\0' < ~/pocol-backups/added-$STAMP.txt | xargs -0 -r rm -f --
    LC_ALL=C comm -13 <(tar -tzf ~/pocol-backups/pocol.net-docroot-$STAMP.tar.gz | sed -n 's#/$##p' | LC_ALL=C sort) <(find . -mindepth 1 -type d | LC_ALL=C sort) | LC_ALL=C sort -r | tr '\n' '\0' | xargs -0 -r rmdir --ignore-fail-on-non-empty --

(with a nested site: `find . -mindepth 1 -path ./ide -prune -o -type d -print` in the last command). Then
check:

    sha256sum -c --quiet ~/pocol-backups/pocol.net-before-$STAMP-files.txt && echo "RESTORED"

No line other than `RESTORED` (a log file PHP has written since may differ). Purge the cache as in step 8.

**With File Manager:** select the backup archive of step 2 (`/pocol-backups/pocol.net-backup-YYYYMMDD.zip`; upload it
there from your computer if it is gone), Extract it into the Document Root (allow overwrite), put back the saved
`.htaccess` if needed, delete the files and the empty folders this package added (compare with the archive's content;
keep a nested site's folder), and purge the cache.

After a rollback, `host_files.py` does not apply (it compares with this package): the backup's own file list is the
reference.
