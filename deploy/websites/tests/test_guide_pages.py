#!/usr/bin/env python3
"""pocol.net's guide pages (about.html ... pocol-status.html) are static English HTML that about.js replaces with its
render of guide-content.js (English or Arabic). The static text must be that render's English: the description meta,
the intro, every section (title, paragraphs, bullets, links) and the table of contents, so a reader without JavaScript,
a search engine and the link checks see what the script shows. Only the ?v= content versions that site_identity.py
writes into a static link may differ."""
import html, json, re, unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
MAIN = ROOT / 'deploy' / 'websites' / 'pocol.net'

def guide():
    text = (MAIN / 'guide-content.js').read_text(encoding='utf-8')
    return json.loads(text[len('window.POCOL_GUIDE = '):].rstrip().rstrip(';'))

def esc(value):
    # about.js escape() escapes & < > " and '; the static pages write the apostrophe as &#x27;.
    return html.escape(str(value), quote=True)

def sections(topic):
    out = ''
    for s in topic['sections']:
        out += f'<section class="doc-section" id="{esc(s["id"])}"><h2>{esc(s["title"]["en"])}</h2>'
        out += ''.join(f'<p>{esc(p["en"])}</p>' for p in s.get('paragraphs') or [])
        if s.get('bullets'):
            out += '<ul>' + ''.join(f'<li>{esc(b["en"])}</li>' for b in s['bullets']) + '</ul>'
        if s.get('links'):
            out += '<div class="actions">' + ''.join(
                f'<a class="button outline" href="{esc(l["url"])}">{esc(l["label"]["en"])}</a>' for l in s['links']) + '</div>'
        out += '</section>'
    return out

def unversioned(text):
    return re.sub(r'\?v=[0-9a-f]{12}', '', text)

class GuidePages(unittest.TestCase):
    def test_every_guide_page_is_the_english_render_of_the_guide(self):
        g = guide()
        self.assertEqual(len(g['topics']), 8)
        for topic in g['topics']:
            page = unversioned((MAIN / g['files'][topic['id']]).read_text(encoding='utf-8'))
            with self.subTest(topic=topic['id']):
                start = page.index('<section class="doc-section"')
                self.assertEqual(page[start:page.index('</article>', start)], unversioned(sections(topic)))
                self.assertIn(f'<meta name="description" content="{esc(topic["intro"]["en"])}">', page)
                self.assertIn(f'<p>{esc(topic["intro"]["en"])}</p>', page)
                toc = re.search(r'<nav class="toc" aria-label="On this page">(.*?)</nav>', page).group(1)
                expected = ''.join(f'<a href="#{esc(s["id"])}">{esc(s["title"]["en"])}</a>' for s in topic['sections'])
                self.assertEqual(toc, expected)

    def test_every_in_page_link_has_its_target(self):
        g = guide()
        ids = {t['id']: {s['id'] for s in t['sections']} for t in g['topics']}
        for topic in g['topics']:
            for s in topic['sections']:
                for link in s.get('links') or []:
                    url = link['url']
                    if url.startswith('#'):
                        self.assertIn(url[1:], ids[topic['id']], (topic['id'], url))
                    elif '#' in url and not url.startswith('https://'):
                        page, anchor = url.split('#', 1)
                        target = next((t for t in g['topics'] if g['files'][t['id']] == page), None)
                        if target is None:
                            # A page outside the guide (the home page): the id is in its static HTML.
                            self.assertIn(f'id="{anchor}"', (MAIN / page).read_text(encoding='utf-8'), (topic['id'], url))
                        else:
                            self.assertIn(anchor, ids[target['id']] | {'mining-lab', 'chapters'}, (topic['id'], url))

if __name__ == '__main__':
    unittest.main()
