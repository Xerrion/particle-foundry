#!/usr/bin/env python3
"""Exercise handbook discovery, offline links, preservation and failure cases."""

from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

SPEC = importlib.util.spec_from_file_location("build_handbook", Path(__file__).with_name("build-handbook.py"))
if SPEC is None or SPEC.loader is None:
    raise RuntimeError("Cannot load the handbook builder")
BUILDER = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = BUILDER
SPEC.loader.exec_module(BUILDER)


def write(root: Path, path: str, content: str | bytes) -> None:
    destination = root / path
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(content.encode() if isinstance(content, str) else content)


def digest_tree(root: Path) -> dict[str, str]:
    return {
        path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in root.rglob("*")
        if path.is_file()
    }


def can_create_file_symlink() -> bool:
    if sys.platform != "win32":
        return True
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        target = root / "target.txt"
        target.write_text("target\n")
        try:
            (root / "alias.txt").symlink_to(target)
        except OSError as error:
            if getattr(error, "winerror", None) == 1314:
                return False
            raise
        return True


FILE_SYMLINKS_AVAILABLE = can_create_file_symlink()


class HandbookTests(unittest.TestCase):
    def setUp(self) -> None:
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name) / "repository"
        self.root.mkdir()
        self.output = Path(self.directory.name) / "output/index.html"
        write(self.root, "docs/README.md", "# Work guide\n\nStart here.\n")

    def build(self) -> tuple[str, dict[str, Any]]:
        report = BUILDER.build(self.root, self.output)
        return self.output.read_text(), report

    def test_discovery_is_complete_and_excludes_dependencies_and_private_files(self) -> None:
        expected = {
            "docs/README.md",
            "docs/new/nested.md",
            "README.md",
            "AGENTS.md",
            "engine/AGENTS.override.md",
            ".github/README.md",
            ".github/runners/README.md",
            "web/new-guide.md",
        }
        for path in expected - {"docs/README.md"}:
            write(self.root, path, "# Guide\n")
        for path in (
            "artifacts/private.md",
            ".venv/package.md",
            "web/node_modules/package/README.md",
            "engine/target/guide.md",
            ".env.notes.md",
        ):
            write(self.root, path, "# Excluded\n")
        _, report = self.build()
        assert {item["path"] for item in report["documents"]} == expected
        assert report["documents"][0]["path"] == "docs/README.md"
        assert not any("node_modules" in item["path"] for item in report["files"])

    def test_links_unicode_duplicates_raw_html_and_generated_plan_alias(self) -> None:
        write(
            self.root,
            "docs/README.md",
            "# Work guide\n\n[Unicode](topic.md#æble) [Duplicate](topic.md#same-1) "
            "[Raw](topic.md#custom) [HTML](topic.html#same)\n",
        )
        write(
            self.root,
            "docs/topic.md",
            '# Topic\n\n## Æble\n\n## Same\n\n## Same\n\n<a id="custom"></a>\n\n[Local](#same-1)\n',
        )
        write(self.root, "docs/topic.html", "<!doctype html><h1>Compatibility</h1>")
        content, report = self.build()
        topic = next(item for item in report["documents"] if item["path"] == "docs/topic.md")
        identity = topic["id"]
        assert f'href="#{identity}__%C3%A6ble"' in content
        assert f'id="{identity}__æble"' in content
        assert f'href="#{identity}__same-1"' in content
        assert f'id="{identity}__custom"' in content
        assert 'href="topic.html' not in content
        assert str(self.root) not in content

    def test_all_docs_data_and_linked_sources_are_copied_byte_for_byte(self) -> None:
        write(
            self.root,
            "docs/README.md",
            "# Work guide\n\n[Code](../engine/src.rs) [Folder](../web/benchmarks/) "
            "[Archive](history/original.zip) ![Image](images/pixel.png)\n",
        )
        original = bytes(range(256))
        write(self.root, "docs/history/original.zip", original)
        write(self.root, "docs/data/unlinked.json", '{"retained":true}\n')
        write(self.root, "docs/images/pixel.png", original)
        write(self.root, "engine/src.rs", "fn main() {}\n")
        write(self.root, "web/benchmarks/example.ts", "export {};\n")
        write(self.root, "web/benchmarks/index.html", "<!doctype html><h1>Original source</h1>\n")
        content, report = self.build()
        for path in (
            "docs/history/original.zip",
            "docs/data/unlinked.json",
            "docs/images/pixel.png",
            "engine/src.rs",
            "web/benchmarks/example.ts",
        ):
            assert (self.output.parent / "files" / path).read_bytes() == (self.root / path).read_bytes()
        assert 'href="files/engine/src.rs"' in content
        assert 'src="files/docs/images/pixel.png"' in content
        assert (self.output.parent / "files/web/benchmarks" / BUILDER.DIRECTORY_INDEX).is_file()
        assert (self.output.parent / "files/web/benchmarks/index.html").read_bytes() == (
            self.root / "web/benchmarks/index.html"
        ).read_bytes()
        assert report["local_links_checked"] > 0

    def test_markdown_tables_code_and_safe_raw_html_remain_available(self) -> None:
        write(
            self.root,
            "docs/README.md",
            '# Work guide\n\n| A | B |\n| --- | --- |\n| 1 | 2 |\n\n```rust\nlet x = "<script>";\n```\n\n'
            '<details id="notes"><summary>Notes</summary><p onclick="bad()">Visible &amp; safe</p></details>\n'
            "<script>bad()</script>\n",
        )
        content, _ = self.build()
        assert '<div class="table-scroll"><table>' in content
        assert '<code class="language-rust">' in content
        assert "&lt;script&gt;" in content
        assert "<summary>Notes</summary>" in content
        assert "Visible &amp; safe" in content
        assert "onclick=" not in content
        assert "<script>bad()" not in content

    def test_missing_file_and_heading_fail_before_output(self) -> None:
        for link, message in (("missing.md", "Missing repository path"), ("#missing", "missing heading")):
            with self.subTest(link=link):
                write(self.root, "docs/README.md", f"# Work guide\n\n[Broken]({link})\n")
                with self.assertRaisesRegex(BUILDER.HandbookError, message):  # noqa: PT027
                    self.build()
                assert not self.output.exists()

    def test_math_delimiters_and_contents_are_preserved_offline(self) -> None:
        write(
            self.root,
            "docs/README.md",
            "# Work guide\n\n"
            r"Inline $x_i < y$ and \(\alpha + \beta\)."
            "\n\n\\[\n\\frac{a_b}{c} < d\n\\]\n\n$$\nx_i^2 = y\n$$\n",
        )
        content, _ = self.build()
        assert '<code class="math">$x_i &lt; y$</code>' in content
        assert r'<code class="math">\(\alpha + \beta\)</code>' in content
        assert '<pre class="math">\\[\n\\frac{a_b}{c} &lt; d\n\\]</pre>' in content
        assert '<pre class="math">$$\nx_i^2 = y\n$$</pre>' in content

    def test_credentials_absolute_links_and_escape_paths_are_rejected(self) -> None:
        write(self.root, ".env", "SECRET=private\n")
        for link in (
            "../.env",
            "/Users/example/private.md",
            "../../outside.md",
            "javascript:alert(1)",
            "javascript://host/%0aalert(1)",
            "file://localhost/Users/example/private.md",
        ):
            with self.subTest(link=link):
                write(self.root, "docs/README.md", f'# Work guide\n\n<a href="{link}">Unsafe</a>\n')
                with self.assertRaises(BUILDER.HandbookError):  # noqa: PT027
                    self.build()
        assert not self.output.exists()

    @unittest.skipUnless(
        FILE_SYMLINKS_AVAILABLE,
        "the current Windows account cannot create file symlinks",
    )
    def test_symbolic_links_cannot_bypass_excluded_paths(self) -> None:
        for private in (".env", "artifacts/private.txt"):
            with self.subTest(private=private):
                write(self.root, private, "private content\n")
                alias = self.root / "alias.txt"
                alias.symlink_to(self.root / private)
                write(self.root, "docs/README.md", "# Work guide\n\n[Alias](../alias.txt)\n")
                with self.assertRaises(BUILDER.HandbookError):  # noqa: PT027
                    self.build()
                alias.unlink()
        assert not self.output.exists()

    def test_svg_urls_and_link_mutation_cannot_bypass_link_policy(self) -> None:
        write(
            self.root,
            "docs/README.md",
            '# Work guide\n\n<svg><a xlink:href="javascript:alert(1)"><text>click</text></a></svg>\n',
        )
        with self.assertRaises(BUILDER.HandbookError):  # noqa: PT027
            self.build()
        write(
            self.root,
            "docs/README.md",
            '# Work guide\n\n<svg><a href="#work-guide"><text>Read</text>'
            '<set attributeName="href" to="javascript:alert(1)"></set></a></svg>\n',
        )
        content, _ = self.build()
        assert "<text>Read</text>" in content
        assert "attributeName=" not in content
        assert "javascript:alert" not in content

    def test_rebuild_removes_stale_owned_files_and_preserves_unowned_files(self) -> None:
        write(self.root, "docs/obsolete.md", "# Obsolete guide\n")
        write(self.root, "docs/evidence/obsolete.json", "{}\n")
        self.build()
        stale = self.output.parent / "files/docs/obsolete.md"
        assert stale.exists()
        write(self.output.parent, "personal-note.txt", "Keep this file.\n")
        (self.root / "docs/obsolete.md").unlink()
        (self.root / "docs/evidence/obsolete.json").unlink()
        self.build()
        assert not stale.exists()
        assert not (self.output.parent / "files/docs/evidence/obsolete.json").exists()
        assert (self.output.parent / "personal-note.txt").read_text() == "Keep this file.\n"

    def test_final_output_validation_rejects_unresolved_links_and_duplicate_ids(self) -> None:
        for markup in (
            '<a href="missing.json">Missing</a>',
            '<p id="same"></p><p id="same"></p>',
            '<a href="#missing">Missing</a>',
        ):
            with self.subTest(markup=markup), self.assertRaises(BUILDER.HandbookError):  # noqa: PT027
                BUILDER.validate_generated({Path("index.html"): markup}, set())

    def test_build_is_deterministic_and_page_ids_do_not_depend_on_order(self) -> None:
        write(self.root, "docs/topic.md", "# A topic\n")
        _, first = self.build()
        hashes = digest_tree(self.output.parent)
        _, second = self.build()
        assert first == second
        assert digest_tree(self.output.parent) == hashes
        assert json.loads((self.output.parent / "manifest.json").read_text()) == first
        topic_id = next(item["id"] for item in first["documents"] if item["path"] == "docs/topic.md")
        write(self.root, "docs/earlier.md", "# A new topic\n")
        _, third = self.build()
        assert topic_id == next(item["id"] for item in third["documents"] if item["path"] == "docs/topic.md")

    def test_real_repository_corpus_is_complete(self) -> None:
        root = Path(__file__).resolve().parents[2]
        handbook = BUILDER.Handbook(root)
        assert set(root.joinpath("docs").rglob("*.md")) <= {root / path for path in handbook.documents}
        for path in ("README.md", "AGENTS.md", "engine/AGENTS.override.md", ".github/README.md"):
            assert Path(path) in handbook.documents


if __name__ == "__main__":
    unittest.main()
