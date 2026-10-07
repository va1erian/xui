"""Saves real pages for the engine comparison as single files: their style
sheets inlined, scripts dropped, images left pointing at the web (no probe has
a fetcher, so none loads them: every engine is on equal terms).

Usage: python fetch_live.py <out-dir>

The pages are not committed (their content is theirs); this re-fetches them.
File names are fixed and free of ':' (Windows refuses it in a name, and wiki
titles such as Special:Random carry one).
"""

import html
import re
import sys
import urllib.parse
import urllib.request
from pathlib import Path

UA = "Mozilla/5.0 (xui engine-compare fixture fetcher)"

PAGES = {
    "wikipedia-rust.html": "https://en.wikipedia.org/wiki/Rust_(programming_language)",
    "hacker-news.html": "https://news.ycombinator.com/",
}


def get(url: str) -> str:
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=30) as r:
        charset = r.headers.get_content_charset() or "utf-8"
        return r.read().decode(charset, "replace")


def flatten(url: str) -> str:
    page = get(url)

    def inline(match: re.Match) -> str:
        tag = match.group(0)
        if not re.search(r'rel=["\']?stylesheet', tag, re.I):
            return tag
        href = re.search(r'href=["\']([^"\']+)', tag, re.I)
        if not href:
            return tag
        css_url = urllib.parse.urljoin(url, html.unescape(href.group(1)))
        try:
            css = get(css_url)
        except OSError as e:
            print(f"  {css_url}: {e}", file=sys.stderr)
            return ""
        return f"<style>/* {css_url} */\n{css}\n</style>"

    page = re.sub(r"<link\b[^>]*>", inline, page, flags=re.I)
    page = re.sub(r"<script\b.*?</script>", "", page, flags=re.I | re.S)
    page = re.sub(r"<noscript\b.*?</noscript>", "", page, flags=re.I | re.S)
    # Protocol-relative and root-relative links would resolve to file:.
    page = re.sub(r'(src|href)="//', r'\1="https://', page)
    origin = "{0.scheme}://{0.netloc}".format(urllib.parse.urlsplit(url))
    page = re.sub(r'(src|href)="/(?!/)', rf'\1="{origin}/', page)
    return page


def main() -> None:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "live")
    out.mkdir(parents=True, exist_ok=True)
    for name, url in PAGES.items():
        assert ":" not in name
        print(f"{name} <- {url}")
        (out / name).write_text(flatten(url), encoding="utf-8")


if __name__ == "__main__":
    main()
