"""Draft 0.7 corrections to the 0.3 reference model (R7-02, C20). Specification tooling only.

Imports m1-draft-0.3/tools/m1model.py and replaces two functions in place:

1. _semantic: the path rules are evaluated as an ordered list of ALL violations
   (path.length, leading-slash path.grammar, then per segment path.dotSegment or
   path.grammar, left to right). With every rule enabled the first violation is
   reported exactly as before. When the mutation harness disables one rule, only
   that rule is skipped and the next violation in the same path is still
   reported (0.3 stopped after the first violation, so a disabled rule hid
   later ones: Codex rule-disable-probe.json).

2. Fetcher.fetch: a chunk is inserted into the session ChunkCache only when no
   rule of its fetch stage (missing, prefix, length, origin) was disabled while
   checking it. A response checked under a disabled rule is never cached, so it
   cannot later satisfy the same (address, length) key at a stage whose rule is
   enabled. With no rule disabled the behaviour is unchanged.
"""

import m1model as M

FETCH_RULES = ('missing', 'prefix', 'length', 'origin')


def path_violations(path):
    """All B05 violations of a manifest path, in precedence order."""
    out = []
    if len(path) == 0 or len(path) > M.PATH_MAX:
        out.append('path.length')
    if path[:1] != b'/':
        out.append('path.grammar')
    body = path[1:] if path[:1] == b'/' else path
    for seg in body.split(b'/'):
        if seg in (b'.', b'..'):
            out.append('path.dotSegment')
        elif not M.SEG_RE.fullmatch(seg):
            out.append('path.grammar')
    return out


def _semantic(version, entry, files, disabled):
    def chk(cond, rule, detail=''):
        if cond and rule not in disabled:
            raise M.Reject(rule, 'semantic', detail)

    chk(version != 1, 'manifest.version', str(version))
    chk(len(files) == 0 or len(files) > M.FILES_MAX, 'manifest.fileCount', str(len(files)))
    chk(entry >= len(files), 'manifest.entryIndex', str(entry))
    prev, total = None, 0
    for i, f in enumerate(files):
        for rule in path_violations(f['path']):
            chk(True, rule, repr(f['path'][:40]))
        chk(prev is not None and not f['path'] > prev, 'path.order', 'file %d' % i)
        prev = f['path']
        chk(not 1 <= f['mime'] <= M.MIME_MAX, 'file.mime', 'file %d' % i)
        chk(i == entry and f['mime'] != M.MIME_HTML, 'entry.mime', 'file %d' % i)
        chk(f['size'] > M.FILE_MAX, 'file.size', 'file %d' % i)
        s = 0
        for j, (_, ln) in enumerate(f['chunks']):
            chk(not 1 <= ln <= M.CHUNK_MAX, 'chunk.len', 'file %d chunk %d' % (i, j))
            s += ln
        chk(s != f['size'], 'file.sum', 'file %d' % i)
        total += f['size']
    chk(total > M.SITE_MAX, 'site.size', str(total))


def _fetch(self, addr, ln, factory, disabled, stage_prefix):
    key = (bytes(addr), ln)
    self.references += 1
    self.keys.add(key)
    cache = None if self.mode == 'none' else self.caches['manifest' if stage_prefix == 'mfetch' else 'content']
    if cache is not None and key in cache:
        return cache[key]
    data = M.check_chunk(addr, ln, self.prov.get(addr), factory, disabled, stage_prefix)
    attested = not any('%s.%s' % (stage_prefix, r) in disabled for r in FETCH_RULES)
    if cache is not None and attested:
        cache[key] = data
    elif cache is not None:
        self.uncachedMutant = getattr(self, 'uncachedMutant', 0) + 1
    return data


ORIGINAL_SEMANTIC = M._semantic          # kept only to demonstrate the 0.3 defects in fixtures
ORIGINAL_FETCH = M.Fetcher.fetch
M._semantic = _semantic
M.Fetcher.fetch = _fetch
model = M
