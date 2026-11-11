#!/usr/bin/env python3
"""Checks the relative links of the documentation: README.md and docs/**/*.md.

    tools/check_links.py [files...]

Each Markdown link `[text](target)` or `<a href="target">` whose target is relative (no scheme, not `mailto:`)
must name an existing file or folder, relative to the file that holds it. A target with an anchor (`file.md#title`
or `#title`) must also match a heading of the target Markdown file, with GitHub's rule for heading anchors
(lower case, punctuation removed, spaces turned into hyphens, `-1`, `-2`... for repeated titles).
Links inside code (fenced blocks and `inline code`) are ignored. A target in a folder that is never versioned
(`GENERATED`: images that the tools draw from the user's ROMs, docs/creatures.md, docs/niveaux.md) is only counted:
it exists only after its generation. Exit status 1 if a link is broken.
"""

import re
import sys
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent.parent

LINK = re.compile(r"!?\[(?:[^\[\]]|\[[^\]]*\])*\]\(\s*<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\s*\)")
HREF = re.compile(r"<a\s[^>]*href=\"([^\"]+)\"")
FENCE = re.compile(r"^\s*(```|~~~)")
INLINE_CODE = re.compile(r"(`+)(?:(?!\1).)+?\1")
# folders produced from the ROMs, ignored by git (.gitignore)
GENERATED = [ROOT / "assets", ROOT / "hd"]
HEADING = re.compile(r"^(#{1,6})\s+(.*?)\s*#*\s*$")


def strip_code(text: str):
    """Lines of `text` without fenced code blocks and inline code, with their numbers."""
    fenced = False
    for n, line in enumerate(text.splitlines(), 1):
        if FENCE.match(line):
            fenced = not fenced
            continue
        if not fenced:
            yield n, INLINE_CODE.sub("", line)


def slug(title: str) -> str:
    """GitHub's anchor of a heading."""
    title = re.sub(r"<[^>]+>", "", title)  # HTML tags
    title = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", title)  # links: their text
    title = title.lower().replace("`", "")
    title = re.sub(r"[^\w\- ]", "", title)  # \w keeps accented letters, as GitHub does
    return title.replace(" ", "-")


def anchors(path: Path, cache={}) -> set:
    if path not in cache:
        seen = {}
        found = set()
        fenced = False
        for line in path.read_text(encoding="utf-8").splitlines():
            if FENCE.match(line):
                fenced = not fenced
                continue
            m = None if fenced else HEADING.match(line)
            if m:
                s = slug(m.group(2))
                k = seen.get(s, 0)
                seen[s] = k + 1
                found.add(s if k == 0 else f"{s}-{k}")
        # explicit anchors: <a name="..."> or id="..."
        found |= set(re.findall(r"<a\s+(?:name|id)=\"([^\"]+)\"", path.read_text(encoding="utf-8")))
        cache[path] = found
    return cache[path]


def check(md: Path, generated: list) -> list:
    errors = []
    for n, line in strip_code(md.read_text(encoding="utf-8")):
        for target in LINK.findall(line) + HREF.findall(line):
            if re.match(r"^[a-z][a-z0-9+.-]*:", target, re.I):
                continue  # http:, https:, mailto:...
            file, _, anchor = target.partition("#")
            dest = (md.parent / unquote(file)).resolve() if file else md
            where = f"{md.relative_to(ROOT)}:{n}"
            if any(dest.is_relative_to(g) for g in GENERATED):
                generated.append(where)
                continue
            if not dest.exists():
                errors.append(f"{where}: missing target {target}")
            elif anchor and dest.suffix == ".md" and dest.is_file():
                if unquote(anchor).lower() not in anchors(dest):
                    errors.append(f"{where}: no heading for anchor {target}")
    return errors


def main() -> int:
    files = [Path(f).resolve() for f in sys.argv[1:]] or [ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md"))]
    generated = []
    errors = [e for f in files for e in check(f, generated)]
    for e in errors:
        print(e)
    print(f"{len(files)} files checked, {len(errors)} broken link(s), {len(generated)} link(s) to generated files "
          f"(not versioned) skipped")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
