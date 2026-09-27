#!/usr/bin/env python3
"""Regenerate the self-contained GPU plan HTML from its canonical Markdown.

Run from the repository through mise, which installs markdown-it-py:
    mise run docs:render
The game and its runtime dependencies are not modified.
"""
from __future__ import annotations

import hashlib
import html
import re
import sys
from pathlib import Path


def slugify(text: str) -> str:
    """Match the heading-anchor convention used by validate-docs.py."""
    text = re.sub(r"<[^>]+>", "", text).replace("`", "")
    text = re.sub(r"[^\w\s-]", "", text.lower(), flags=re.UNICODE)
    return re.sub(r"\s", "-", text.strip())


def main() -> int:
    try:
        from markdown_it import MarkdownIt
    except ImportError:
        print("Missing documentation dependency: run mise run docs:install.", file=sys.stderr)
        return 2
    root = Path(__file__).resolve().parents[1]
    source = root / "plans/fluid-gpu-redesign/plan.md"
    destination = source.with_suffix(".html")
    raw = source.read_bytes()
    text = raw.decode("utf-8")
    digest = hashlib.sha256(raw).hexdigest()
    parser = MarkdownIt("commonmark", {"html": True}).enable("table")
    tokens = parser.parse(text)
    seen: dict[str, int] = {}
    nav: list[str] = []
    for index, token in enumerate(tokens):
        if token.type != "heading_open":
            continue
        heading = tokens[index + 1].content
        base = slugify(heading)
        duplicate = seen.get(base, 0)
        seen[base] = duplicate + 1
        anchor = base if duplicate == 0 else f"{base}-{duplicate}"
        token.attrSet("id", anchor)
        if token.tag == "h2":
            nav.append(f'<a href="#{html.escape(anchor, quote=True)}">{html.escape(heading)}</a>')
    body = parser.renderer.render(tokens, parser.options, {})
    body = body.replace("<table>", '<div class="table-scroll"><table>').replace("</table>", "</table></div>")
    stylesheet = """
:root { color-scheme: light; --ink:#142331; --muted:#526574; --line:#d9e3e8; --accent:#176572; }
* { box-sizing:border-box; }
html { scroll-behavior:smooth; }
body { margin:0; background:#f4f7f8; color:var(--ink); font:16px/1.65 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }
header { background:#142d3d; color:#fff; padding:2.5rem max(1.5rem,calc((100vw - 1400px)/2)); }
header .eyebrow { font-size:.8rem; letter-spacing:.1em; text-transform:uppercase; color:#bce3e5; }
header h1 { font-size:clamp(1.8rem,4vw,2.9rem); line-height:1.18; margin:.7rem 0; color:#fff; }
header p { max-width:860px; margin:.6rem 0; }
header a { color:#d4f1f4; }
.layout { max-width:1400px; margin:0 auto; display:grid; grid-template-columns:245px minmax(0,1fr); gap:2.5rem; padding:2rem 1.5rem 4rem; }
nav { position:sticky; top:1.25rem; align-self:start; font-size:.86rem; border:1px solid var(--line); background:white; padding:1.1rem; border-radius:8px; }
nav strong { display:block; margin:0 0 .6rem; }
nav a { display:block; line-height:1.4; padding:.48rem 0; text-decoration:none; }
main { min-width:0; background:white; border:1px solid var(--line); border-radius:8px; padding:clamp(1.2rem,3vw,2.8rem); }
main > h1 { font-size:2rem; line-height:1.2; margin-top:0; }
h2 { font-size:1.55rem; line-height:1.3; margin-top:2.8rem; padding-top:.5rem; border-top:2px solid var(--line); }
h3 { font-size:1.15rem; line-height:1.4; margin-top:2rem; }
h1,h2,h3 { scroll-margin-top:1rem; }
a { color:var(--accent); text-underline-offset:3px; overflow-wrap:anywhere; }
p { margin:.85rem 0; }
li { margin:.35rem 0; }
code { font-size:.88em; background:#eff4f6; border-radius:3px; padding:.12rem .25rem; overflow-wrap:anywhere; }
pre { padding:1.1rem; background:#edf3f6; border:1px solid var(--line); border-radius:6px; overflow-x:auto; font-size:.87rem; line-height:1.55; }
pre code { background:none; padding:0; overflow-wrap:normal; }
.table-scroll { overflow-x:auto; margin:1.2rem 0; }
table { border-collapse:collapse; font-size:.88rem; line-height:1.5; width:100%; }
th,td { border:1px solid var(--line); vertical-align:top; padding:.65rem .75rem; min-width:100px; }
th { text-align:left; background:#e6f0f3; }
tr:nth-child(even) td { background:#f8fafb; }
footer { border-top:1px solid var(--line); padding:1.5rem; max-width:1400px; margin:auto; color:var(--muted); font-size:.85rem; }
@media(max-width:900px) { .layout { grid-template-columns:1fr; gap:1rem; } nav { position:static; } nav a { display:inline-block; padding:.4rem .75rem .4rem 0; } }
@media print { header { background:white; color:var(--ink); padding:1rem 0; } header h1,header a,header .eyebrow { color:var(--ink); } .layout { display:block; padding:0; } nav { display:none; } main { border:0; padding:0; } h2,h3 { break-after:avoid; } tr { break-inside:avoid; } pre { white-space:pre-wrap; overflow-wrap:anywhere; } .table-scroll { overflow:visible; } }
"""
    output = f'''<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="source-sha256" content="{digest}">
<meta name="description" content="Particle Foundry P1 implementation plan: Rust reference, WASM delivery, wgpu/WGSL execution, and future-ready composition.">
<title>Fluid and GPU redesign | Particle Foundry</title>
<style>{stylesheet}</style>
</head>
<body>
<header>
<div class="eyebrow">Particle Foundry / P1 / Planning revision 21 September 2026</div>
<h1>Rust reference. WASM delivery. wgpu execution.</h1>
<p>Keep the TypeScript frontend. Redesign the engine around conservative transport, one authoritative state, and direct GPU rendering. Validate each milestone before promotion.</p>
<p><a href="../../START_HERE.md">Developer entrypoint</a> / <a href="../../roadmap.md">All phases</a> / <a href="plan.md">Canonical Markdown</a></p>
</header>
<div class="layout">
<nav aria-label="Plan sections"><strong>Implementation plan</strong>{''.join(nav)}</nav>
<main>{body}</main>
</div>
<footer>Generated from plan.md. Historical audit evidence is preserved; this documentation revision does not implement or benchmark the simulation.</footer>
</body>
</html>
'''
    destination.write_text(output, encoding="utf-8")
    print(f"Generated {destination.relative_to(root)} from SHA-256 {digest}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
