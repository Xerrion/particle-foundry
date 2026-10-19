#!/usr/bin/env python3
"""Build the complete offline handbook from the current repository documentation.

Run through mise to use the existing markdown-it-py dependency. The optional output
argument names an HTML file or a directory. Generated files contain no build time
or absolute checkout paths, so identical inputs produce identical output.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
import shutil
import sys
from dataclasses import dataclass, field
from html.parser import HTMLParser
from pathlib import Path
from typing import TYPE_CHECKING, Any
from urllib.parse import quote, unquote, urlsplit

if TYPE_CHECKING:
    from markdown_it.rules_block import StateBlock
    from markdown_it.rules_inline import StateInline

EXCLUDED_DIRECTORIES = {
    ".git",
    ".venv",
    ".ruff_cache",
    ".pytest_cache",
    "__pycache__",
    "node_modules",
    "target",
    "artifacts",
    "dist",
    "generated",
}
GROUPS = (
    "Continue work",
    "Current system",
    "Architecture",
    "Plans",
    "Validation",
    "Evidence and history",
    "Repository guidance",
)
START_ORDER = ("docs/README.md", "docs/STATUS.md", "docs/PHASES.md")
BLOCKED_TAGS = {
    "script",
    "style",
    "iframe",
    "object",
    "embed",
    "base",
    "meta",
    "link",
    "form",
    "animate",
    "animatemotion",
    "animatetransform",
    "set",
}
VOID_TAGS = {
    "area",
    "base",
    "br",
    "col",
    "embed",
    "hr",
    "img",
    "input",
    "link",
    "meta",
    "param",
    "source",
    "track",
    "wbr",
}
EXTERNAL_SCHEMES = {"http", "https", "mailto", "tel"}
DIRECTORY_INDEX = "__handbook-directory__.html"


class HandbookError(ValueError):
    """A source document cannot be included in a complete offline handbook."""


def slugify(text: str) -> str:
    """Use the same Unicode heading convention as the existing docs tools."""
    text = re.sub(r"<[^>]+>", "", text).replace("`", "")
    text = re.sub(r"[^\w\s-]", "", text.lower(), flags=re.UNICODE)
    return re.sub(r"\s", "-", text.strip())


def page_id(path: Path) -> str:
    """Keep page identities stable when discovery order or titles change."""
    source = path.as_posix()
    digest = hashlib.sha256(source.encode()).hexdigest()[:10]
    return f"page-{slugify(source.replace('/', '-'))}-{digest}"


def permitted_path(path: Path) -> bool:
    """Exclude build output, dependency trees and credential files."""
    return not any(
        part in EXCLUDED_DIRECTORIES
        or (part.startswith(".env") and part not in {".env.example", ".env.sample", ".env.template"})
        or part.lower() in {"credentials", "credentials.json", "id_rsa", "id_ed25519"}
        for part in path.parts
    )


def discover_documents(root: Path) -> list[Path]:
    """Include every authored Markdown file without traversing build output."""
    documents: list[Path] = []
    for directory, names, files in root.walk():
        names[:] = sorted(
            name for name in names if permitted_path(Path(name)) and (not name.startswith(".") or name == ".github")
        )
        for name in sorted(files):
            path = directory / name
            relative = path.relative_to(root)
            if path.suffix.lower() == ".md" and permitted_path(relative):
                documents.append(relative)
    if Path("docs/README.md") not in documents:
        raise HandbookError("Missing canonical handbook entry: docs/README.md")
    order = {name: index for index, name in enumerate(START_ORDER)}
    return sorted(
        documents, key=lambda path: (GROUPS.index(group_for(path)), order.get(path.as_posix(), 99), path.as_posix())
    )


def group_for(path: Path) -> str:
    """Use broad chapter groups while discovering individual pages automatically."""
    source = path.as_posix()
    if source in START_ORDER:
        return GROUPS[0]
    if source.startswith("docs/architecture/"):
        return GROUPS[2]
    if source.startswith("docs/plans/") or source in {"docs/remaining-work.md", "docs/roadmap.md"}:
        return GROUPS[3]
    if source.startswith(("docs/validation/", "docs/templates/")):
        return GROUPS[4]
    if source.startswith(("docs/history/", "docs/evidence/")) or source in {
        "docs/CHANGELOG.md",
        "docs/START_HERE.md",
        "docs/RESUME.md",
    }:
        return GROUPS[5]
    if source.startswith("docs/reference/recovery"):
        return GROUPS[5]
    return GROUPS[1] if source.startswith("docs/") else GROUPS[6]


def preserve_inline_math(state: StateInline, silent: bool) -> bool:
    """Keep TeX delimiters and formulas readable without an online renderer."""
    for opening, closing in ((r"\(", r"\)"), ("$$", "$$"), ("$", "$")):
        if not state.src.startswith(opening, state.pos):
            continue
        end = state.src.find(closing, state.pos + len(opening))
        if end == -1 or "\n" in state.src[state.pos : end]:
            continue
        if not silent:
            token = state.push("html_inline", "", 0)
            formula = state.src[state.pos : end + len(closing)]
            token.content = f'<code class="math">{html.escape(formula)}</code>'
        state.pos = end + len(closing)
        return True
    return False


def preserve_block_math(state: StateBlock, start: int, end: int, silent: bool) -> bool:
    """Preserve multiline TeX before CommonMark consumes backslash escapes."""
    line = state.src[state.bMarks[start] + state.tShift[start] : state.eMarks[start]].strip()
    if line not in {"$$", r"\["}:
        return False
    closing = "$$" if line == "$$" else r"\]"
    for cursor in range(start + 1, end):
        candidate = state.src[state.bMarks[cursor] + state.tShift[cursor] : state.eMarks[cursor]].strip()
        if candidate != closing:
            continue
        if not silent:
            formula = state.src[state.bMarks[start] : state.eMarks[cursor]]
            token = state.push("html_block", "", 0)
            token.content = f'<pre class="math">{html.escape(formula)}</pre>\n'
            token.map = [start, cursor + 1]
            state.line = cursor + 1
        return True
    return False


@dataclass
class Document:
    path: Path
    source: str
    tokens: list[Any]
    title: str
    identity: str
    anchors: dict[str, str] = field(default_factory=dict)
    headings: list[tuple[str, str]] = field(default_factory=list)


class HTMLInventory(HTMLParser):
    """Collect actual browser IDs and local resource links for final checks."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.ids: set[str] = set()
        self.duplicates: set[str] = set()
        self.links: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        identity = values.get("id") or (values.get("name") if tag == "a" else None)
        if identity:
            if identity in self.ids:
                self.duplicates.add(identity)
            self.ids.add(identity)
        for name in ("href", "xlink:href", "src", "poster"):
            if values.get(name):
                self.links.append(str(values[name]))

    handle_startendtag = handle_starttag


class BodyRewriter(HTMLParser):
    """Retain HTML formatting while removing executable raw HTML."""

    def __init__(self, handbook: Handbook, document: Document) -> None:
        super().__init__(convert_charrefs=False)
        self.handbook = handbook
        self.document = document
        self.output: list[str] = []
        self.blocked: list[str] = []

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        if tag in BLOCKED_TAGS:
            if tag not in VOID_TAGS:
                self.blocked.append(tag)
            return
        if self.blocked:
            return
        safe: list[str] = []
        for name, original in attrs:
            value = original
            if name.startswith("on") or name in {"style", "srcdoc", "formaction", "action", "srcset"}:
                continue
            if value is None:
                safe.append(name)
                continue
            if name in {"href", "xlink:href", "src", "poster"}:
                value = self.handbook.rewrite_url(self.document, value, image=name in {"src", "poster"})
            elif name == "id" or (tag == "a" and name == "name"):
                value = self.document.anchors.get(value, value)
            elif name in {"for", "aria-labelledby", "aria-describedby"}:
                value = " ".join(self.document.anchors.get(part, part) for part in value.split())
            safe.append(f'{name}="{html.escape(value, quote=True)}"')
        attributes = " " + " ".join(safe) if safe else ""
        self.output.append(f"<{tag}{attributes}>")

    def handle_endtag(self, tag: str) -> None:
        if self.blocked:
            if tag == self.blocked[-1]:
                self.blocked.pop()
            return
        if tag not in BLOCKED_TAGS and tag not in VOID_TAGS:
            self.output.append(f"</{tag}>")

    def handle_startendtag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        self.handle_starttag(tag, attrs)
        if tag not in VOID_TAGS:
            self.handle_endtag(tag)

    def handle_data(self, data: str) -> None:
        if not self.blocked:
            self.output.append(data)

    def handle_entityref(self, name: str) -> None:
        if not self.blocked:
            self.output.append(f"&{name};")

    def handle_charref(self, name: str) -> None:
        if not self.blocked:
            self.output.append(f"&#{name};")

    def handle_comment(self, data: str) -> None:
        if not self.blocked:
            self.output.append(f"<!--{data}-->")


class Handbook:
    """Own document identities, rewritten links and the copied file set."""

    def __init__(self, root: Path) -> None:
        try:
            from markdown_it import MarkdownIt
        except ImportError as error:
            raise HandbookError(
                "Missing documentation dependency. Use the existing docs:install task after approval."
            ) from error
        self.root = root.resolve()
        self.parser = MarkdownIt("commonmark", {"html": True}).enable("table")
        self.parser.inline.ruler.before("escape", "preserved_math", preserve_inline_math)
        self.parser.block.ruler.before("fence", "preserved_math", preserve_block_math)
        self.documents: dict[Path, Document] = {}
        self.attachments: set[Path] = set()
        self.directories: set[Path] = set()
        for path in discover_documents(self.root):
            absolute = self.resolve_source(path)
            source = absolute.read_text(encoding="utf-8")
            tokens = self.parser.parse(source)
            headings = [(index, token) for index, token in enumerate(tokens) if token.type == "heading_open"]
            title = tokens[headings[0][0] + 1].content if headings else path.stem
            title = html.unescape(re.sub(r"<[^>]+>|[*`_]", "", title))
            document = Document(path, source, tokens, title, page_id(path))
            raw = HTMLInventory()
            raw.feed(self.parser.renderer.render(tokens, self.parser.options, {}))
            used: set[str] = set()
            for index, token in headings:
                heading = tokens[index + 1].content
                base = slugify(heading) or "section"
                anchor = base
                duplicate = 0
                while anchor in used:
                    duplicate += 1
                    anchor = f"{base}-{duplicate}"
                used.add(anchor)
                document.anchors[anchor] = f"{document.identity}__{anchor}"
                token.attrSet("id", anchor)
                token.tag = f"h{min(int(token.tag[1]) + 1, 6)}"
                tokens[index + 2].tag = token.tag
                if int(token.tag[1]) <= 3:
                    label = html.unescape(re.sub(r"<[^>]+>|[*`_]", "", heading))
                    document.headings.append((document.anchors[anchor], label))
            for identity in raw.ids:
                if identity in document.anchors:
                    raise HandbookError(f"{path}: raw HTML ID duplicates a heading: {identity}")
                document.anchors[identity] = f"{document.identity}__{identity}"
            if raw.duplicates:
                raise HandbookError(f"{path}: duplicate raw HTML IDs: {', '.join(sorted(raw.duplicates))}")
            self.documents[path] = document
        for path in sorted((self.root / "docs").rglob("*")):
            relative = path.relative_to(self.root)
            if path.is_file() and permitted_path(relative):
                self.add_attachment(relative)

    def resolve_source(self, path: Path) -> Path:
        resolved = (self.root / path).resolve()
        if (
            not resolved.is_relative_to(self.root)
            or not permitted_path(path)
            or not permitted_path(resolved.relative_to(self.root))
        ):
            raise HandbookError(f"Excluded repository path: {path.as_posix()}")
        if not resolved.exists():
            raise HandbookError(f"Missing repository path: {path.as_posix()}")
        return resolved

    def add_attachment(self, path: Path) -> None:
        source = self.resolve_source(path)
        if source.is_dir():
            self.directories.add(path)
            for child in sorted(source.iterdir()):
                relative = child.relative_to(self.root)
                if permitted_path(relative) and not child.name.startswith("."):
                    self.add_attachment(relative)
        else:
            self.attachments.add(path)

    def rewrite_url(self, document: Document, url: str, *, image: bool = False) -> str:
        parsed = urlsplit(html.unescape(url))
        if parsed.scheme in EXTERNAL_SCHEMES or (not parsed.scheme and parsed.netloc):
            return url
        if image and re.match(r"^data:image/(png|jpeg|gif|webp);base64,", url, re.IGNORECASE):
            return url
        if parsed.scheme or parsed.path.startswith(("/", "\\")):
            raise HandbookError(f"{document.path}: unsupported or absolute link: {url}")
        if not parsed.path:
            if not parsed.fragment:
                return f"#{document.identity}"
            target = document.path
        else:
            target = document.path.parent / unquote(parsed.path)
            resolved = self.resolve_source(target)
            target = resolved.relative_to(self.root)
        if target.suffix.lower() == ".html" and target.with_suffix(".md") in self.documents:
            target = target.with_suffix(".md")
        if target in self.documents:
            owner = self.documents[target]
            fragment = unquote(parsed.fragment)
            if fragment and fragment not in owner.anchors:
                raise HandbookError(f"{document.path}: missing heading in {target}: #{fragment}")
            anchor = owner.anchors[fragment] if fragment else owner.identity
            return f"#{quote(anchor, safe='_-')}"
        self.add_attachment(target)
        suffix = "/" + DIRECTORY_INDEX if target in self.directories else ""
        result = "files/" + quote(target.as_posix(), safe="/") + suffix
        if parsed.query:
            result += "?" + parsed.query
        if parsed.fragment:
            if target.suffix.lower() in {".html", ".htm"}:
                inventory = HTMLInventory()
                inventory.feed(self.resolve_source(target).read_text(encoding="utf-8"))
                if unquote(parsed.fragment) not in inventory.ids:
                    raise HandbookError(f"{document.path}: missing HTML anchor in {target}: #{parsed.fragment}")
            result += "#" + parsed.fragment
        return result

    def render(self) -> str:
        chapters: list[str] = []
        navigation: list[str] = []
        for group in GROUPS:
            links: list[str] = []
            for document in self.documents.values():
                if group_for(document.path) != group:
                    continue
                rendered = self.parser.renderer.render(document.tokens, self.parser.options, {})
                body = BodyRewriter(self, document)
                body.feed(rendered)
                body.close()
                contents = (
                    "".join(body.output)
                    .replace("<table>", '<div class="table-scroll"><table>')
                    .replace("</table>", "</table></div>")
                )
                self.add_attachment(document.path)
                source_link = "files/" + quote(document.path.as_posix(), safe="/")
                title = html.escape(document.title)
                headings = "".join(
                    f'<li><a href="#{quote(anchor, safe="_-")}">{html.escape(label)}</a></li>'
                    for anchor, label in document.headings
                )
                chapters.append(
                    f'<article id="{document.identity}" '
                    f'data-document="{html.escape(document.path.as_posix(), quote=True)}">'
                    f'<div class="chapter-meta"><span>{html.escape(group)}</span>'
                    f'<a href="{source_link}" download>Markdown source</a></div>'
                    f'<p class="source-path">{html.escape(document.path.as_posix())}</p>'
                    f'<details class="page-contents"><summary>On this page</summary><ul>{headings}</ul></details>'
                    f'{contents}<p class="chapter-end"><a href="#handbook-start">Back to start</a></p></article>'
                )
                links.append(
                    f'<li><a href="#{document.identity}" data-page="{document.identity}"><span>{title}</span>'
                    f"<small>{html.escape(document.path.as_posix())}</small></a></li>"
                )
            if links:
                navigation.append(
                    f'<details class="nav-group" open><summary>{html.escape(group)}</summary>'
                    f"<ul>{''.join(links)}</ul></details>"
                )
        base = Path(__file__).resolve().parent
        css = (base / "handbook.css").read_text(encoding="utf-8")
        script = (base / "handbook.js").read_text(encoding="utf-8")
        return f"""<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="description" content="Complete offline Particle Foundry documentation, plans, guidance and evidence.">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src 'self' data:;
style-src 'unsafe-inline'; script-src 'unsafe-inline'; base-uri 'none'; form-action 'none'">
<title>Particle Foundry handbook</title><style>{css}</style></head>
<body><a class="skip-link" href="#handbook-main">Skip navigation</a>
<header id="handbook-start"><p class="eyebrow">Particle Foundry</p><h1>Project handbook</h1>
<p>Start with the work guide. Browse current behavior, plans and recorded evidence in one offline document.</p>
<p class="build-info">{len(self.documents)} documents. <a href="manifest.json">Build inventory</a>.
<a href="#" id="print-handbook">Print complete handbook</a>.</p></header>
<div class="layout"><aside><nav aria-label="Handbook chapters"><label for="handbook-search">Find a page or topic</label>
<input type="search" id="handbook-search" placeholder="Search all document text"
aria-describedby="search-help" autocomplete="off">
<p id="search-help">Press / to search. Escape clears the search.</p>
<p id="search-status" role="status" aria-live="polite"></p>
<button id="clear-search" type="button" hidden>Show all pages</button>{"".join(navigation)}</nav></aside>
<main id="handbook-main" tabindex="-1">{"".join(chapters)}</main></div>
<noscript><p class="no-script">All chapters are available below. Use your browser's Find command to search.
</p></noscript>
<script>{script}</script></body></html>
"""

    def directory_pages(self) -> dict[Path, str]:
        pages: dict[Path, str] = {}
        for directory in sorted(self.directories):
            children = {path for path in self.attachments | self.directories if path.parent == directory}
            links = []
            for child in sorted(children):
                suffix = "/" + DIRECTORY_INDEX if child in self.directories else ""
                links.append(
                    f'<li><a href="{quote(child.name)}{suffix}">'
                    f"{html.escape(child.name)}{'/' if suffix else ''}</a></li>"
                )
            pages[Path("files") / directory / DIRECTORY_INDEX] = (
                '<!doctype html><html lang="en"><meta charset="utf-8"><title>'
                + html.escape(directory.as_posix())
                + "</title><h1>"
                + html.escape(directory.as_posix())
                + "</h1><ul>"
                + "".join(links)
                + "</ul></html>"
            )
        return pages


def validate_generated(pages: dict[Path, str], attachments: set[Path]) -> int:
    """Reject duplicate IDs and unresolved paths or fragments in generated HTML."""
    inventories: dict[Path, HTMLInventory] = {}
    targets = attachments | pages.keys()
    count = 0
    for path, content in pages.items():
        inventory = HTMLInventory()
        inventory.feed(content)
        if inventory.duplicates:
            raise HandbookError(f"{path}: duplicate output IDs: {', '.join(sorted(inventory.duplicates))}")
        inventories[path] = inventory
    for path, inventory in inventories.items():
        for url in inventory.links:
            parsed = urlsplit(url)
            if parsed.scheme or parsed.netloc:
                continue
            count += 1
            if parsed.path.startswith("/"):
                raise HandbookError(f"{path}: absolute output link: {url}")
            target = Path(path.parent, unquote(parsed.path)) if parsed.path else path
            # Normalize relative directory-listing links without using checkout paths.
            parts: list[str] = []
            for part in target.parts:
                if part == "..":
                    if not parts:
                        raise HandbookError(f"{path}: output link escapes the handbook: {url}")
                    parts.pop()
                elif part != ".":
                    parts.append(part)
            target = Path(*parts)
            if target not in targets:
                raise HandbookError(f"{path}: unresolved output link: {url}")
            if parsed.fragment and target in inventories and unquote(parsed.fragment) not in inventories[target].ids:
                raise HandbookError(f"{path}: unresolved output anchor: {url}")
    return count


def stale_owned_files(destination: Path, current: set[Path]) -> list[Path]:
    """Remove only unchanged files recorded by an earlier handbook build."""
    manifest = destination.parent / "manifest.json"
    if not manifest.is_file():
        return []
    previous = json.loads(manifest.read_text(encoding="utf-8"))
    if previous.get("generator") != "particle-foundry-handbook":
        return []
    records = [
        {"path": "files/" + record["path"], "sha256": record["sha256"]} for record in previous["files"]
    ] + previous.get("generated", [])
    stale: list[Path] = []
    for record in records:
        relative = Path(record["path"])
        if relative.is_absolute() or ".." in relative.parts:
            raise HandbookError("Previous handbook inventory contains an unsafe output path")
        if relative in current:
            continue
        output = destination.parent / relative
        if not output.exists():
            continue
        if output.is_symlink() or not output.resolve().is_relative_to(destination.parent.resolve()):
            raise HandbookError(f"Previous handbook output is a symbolic link: {relative}")
        if not output.is_file() or hashlib.sha256(output.read_bytes()).hexdigest() != record["sha256"]:
            raise HandbookError(f"Previous handbook output was modified. Preserve it before rebuilding: {relative}")
        stale.append(output)
    return stale


def build(root: Path, destination: Path) -> dict[str, Any]:
    """Validate the complete artifact before writing the selected output files."""
    handbook = Handbook(root)
    content = handbook.render()
    pages = {Path(destination.name): content, **handbook.directory_pages()}
    copied = {Path("files") / path for path in handbook.attachments}
    links = validate_generated(pages, copied | {Path("manifest.json")})
    report: dict[str, Any] = {
        "format": 1,
        "generator": "particle-foundry-handbook",
        "entry": destination.name,
        "documents": [
            {
                "path": document.path.as_posix(),
                "id": document.identity,
                "title": document.title,
                "group": group_for(document.path),
                "sha256": hashlib.sha256(document.source.encode()).hexdigest(),
            }
            for document in handbook.documents.values()
        ],
        "files": [
            {"path": path.as_posix(), "sha256": hashlib.sha256(handbook.resolve_source(path).read_bytes()).hexdigest()}
            for path in sorted(handbook.attachments)
        ],
        "local_links_checked": links,
        "generated": [
            {"path": path.as_posix(), "sha256": hashlib.sha256(body.encode()).hexdigest()}
            for path, body in sorted(pages.items())
        ],
    }
    stale = stale_owned_files(destination, copied | pages.keys() | {Path("manifest.json")})
    destination.parent.mkdir(parents=True, exist_ok=True)
    for path in sorted(handbook.attachments):
        output = destination.parent / "files" / path
        output.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(handbook.resolve_source(path), output)
    for path, body in pages.items():
        output = destination.parent / path
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(body, encoding="utf-8")
    (destination.parent / "manifest.json").write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )
    for path in stale:
        path.unlink()
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "output", nargs="?", type=Path, help="HTML file or output directory (default: artifacts/docs/index.html)"
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    destination = args.output or root / "artifacts/docs/index.html"
    if destination.suffix.lower() != ".html":
        destination /= "index.html"
    try:
        report = build(root, destination)
    except (HandbookError, OSError, UnicodeError) as error:
        sys.stderr.write(f"Handbook build failed: {error}\n")
        return 1
    label = destination.relative_to(root) if destination.is_relative_to(root) else destination.name
    sys.stdout.write(
        f"Built {label}: {len(report['documents'])} documents, {len(report['files'])} copied files, "
        f"{report['local_links_checked']} checked local links.\n"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
