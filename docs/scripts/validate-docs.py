#!/usr/bin/env python3
"""Validate the documentation package without third-party dependencies.

Checks planning manifests, table synchronization, local links/anchors, generated
HTML freshness, migration/source manifest consistency, and original evidence integrity. Does not execute the supplied
audit harness or run application, numerical, browser, or GPU tests.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import unquote, urlsplit
from zipfile import BadZipFile, ZipFile

ROOT = Path(__file__).resolve().parents[1]
ORIGINAL_SHA256 = "f0c353534cca10736d98c449fa96b5e2ed07c241a3ba7cd4463f897ab9860eb5"
EXPECTED_SYMBOLS = "H He Li Be B C N O F Ne Na Mg Al Si P S Cl Ar K Ca Sc Ti V Cr Mn Fe Co Ni Cu Zn Ga Ge As Se Br Kr Rb Sr Y Zr Nb Mo Tc Ru Rh Pd Ag Cd In Sn Sb Te I Xe Cs Ba La Ce Pr Nd Pm Sm Eu Gd Tb Dy Ho Er Tm Yb Lu Hf Ta W Re Os Ir Pt Au Hg Tl Pb Bi Po At Rn Fr Ra Ac Th Pa U Np Pu Am Cm Bk Cf Es Fm Md No Lr Rf Db Sg Bh Hs Mt Ds Rg Cn Nh Fl Mc Lv Ts Og".split()
EXPECTED_COUNTS = {"core":40, "extended":43, "nuclear":20, "exotic":15}
TIER_PHASE = {"core":"P4", "extended":"P5", "nuclear":"P6", "exotic":"P8"}


def slugify(text: str) -> str:
    text = re.sub(r"<[^>]+>", "", text).replace("`", "")
    text = re.sub(r"[^\w\s-]", "", text.lower(), flags=re.UNICODE)
    return re.sub(r"\s", "-", text.strip())


def without_fences(text: str) -> str:
    return re.sub(r"(?ms)^(`{3,}|~{3,})[^\n]*\n.*?^\1\s*$", "", text)


def markdown_anchors(text: str) -> set[str]:
    seen: dict[str, int] = {}
    anchors: set[str] = set()
    for match in re.finditer(r"(?m)^#{1,6}\s+(.+?)\s*#*\s*$", without_fences(text)):
        base = slugify(match.group(1))
        duplicate = seen.get(base, 0)
        seen[base] = duplicate + 1
        anchors.add(base if duplicate == 0 else f"{base}-{duplicate}")
    anchors.update(re.findall(r'\bid=["\']([^"\']+)["\']', text))
    return anchors


class HTMLLinks(HTMLParser):
    def __init__(self) -> None:
        super().__init__()
        self.links: list[str] = []
        self.anchors: set[str] = set()
        self.source_hash: str | None = None

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        values = dict(attrs)
        if values.get("id"):
            self.anchors.add(str(values["id"]))
        if tag == "a" and values.get("href"):
            self.links.append(str(values["href"]))
        if tag == "meta" and values.get("name") == "source-sha256":
            self.source_hash = values.get("content")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source-archive", type=Path,
        help="Optionally verify the inspected source ZIP against the static-review manifest; never execute it.",
    )
    args = parser.parse_args()
    errors: list[str] = []
    checked: list[str] = []

    def require(condition: bool, message: str) -> None:
        if not condition:
            errors.append(message)

    try:
        roster = json.loads((ROOT / "data/element-roadmap.json").read_text(encoding="utf-8"))
        records = roster["elements"]
        require(roster["kind"] == "planning-manifest-not-runtime-properties", "Roster is not clearly labelled as planning data")
        require(len(records) == 118, "Roster must contain exactly 118 records")
        require([e["atomicNumber"] for e in records] == list(range(1,119)), "Atomic numbers must be unique, complete and sorted")
        require([e["symbol"] for e in records] == EXPECTED_SYMBOLS, "Element symbols do not match atomic-number order")
        require(len({e["name"] for e in records}) == 118, "Duplicate or missing element names")
        require(Counter(e["tier"] for e in records) == EXPECTED_COUNTS, "Cohort counts differ from 40/43/20/15")
        require(roster["tierCounts"] == EXPECTED_COUNTS, "Declared cohort counts do not match policy")
        require(roster["cumulativeTargets"] == [40,83,103,118], "Wrong cumulative roster targets")
        require({e["symbol"] for e in records if e["legacyModelDocumented"]} == set("H He C N O S Fe Cu".split()), "Historical eight-element status changed")
        for e in records:
            require(e["targetPhase"] == TIER_PHASE[e["tier"]], f"Wrong target phase for {e['symbol']}")
            require(e["newBackendStatus"] in {"planned","experimental","validated"}, f"Bad support status for {e['symbol']}")
            require(bool(e["sourceIds"]), f"No identity source for {e['symbol']}")
        schema = json.loads((ROOT / "data/element-roadmap.schema.json").read_text(encoding="utf-8"))
        require(schema["properties"]["elements"]["minItems"] == 118, "Schema lost its roster cardinality contract")
        checked.append("118 element identities, tier counts, target phases and historical status")

        elements_text = (ROOT / "elements.md").read_text(encoding="utf-8")
        block = elements_text.split("<!-- BEGIN ELEMENT ROSTER -->",1)[1].split("<!-- END ELEMENT ROSTER -->",1)[0]
        table_rows = re.findall(r"(?m)^\| (\d+) \| ([A-Za-z]+) \| ([^|]+) \| (P\d+) \| (Yes|No) \|$", block)
        expected_rows = [(str(e["atomicNumber"]),e["symbol"],e["name"],e["targetPhase"],"Yes" if e["legacyModelDocumented"] else "No") for tier in EXPECTED_COUNTS for e in records if e["tier"]==tier]
        require([tuple(v.strip() for v in row) for row in table_rows] == expected_rows, "Element Markdown roster is out of sync with JSON")
        checked.append("Markdown element roster matches the JSON manifest")

        phase_manifest = json.loads((ROOT / "data/development-phases.json").read_text(encoding="utf-8"))
        phases = phase_manifest["phases"]
        require([p["id"] for p in phases] == [f"P{i}" for i in range(1,10)], "Phases must be ordered P1-P9")
        require(phases[0]["milestones"] == [f"M{i}" for i in range(8)], "GPU M0-M7 must be the first phase")
        seen: set[str] = set()
        for p in phases:
            require(set(p["dependsOn"]).issubset(seen), f"Phase dependency cycle/order problem in {p['id']}")
            require((ROOT / p["document"]).is_file(), f"Missing controlling plan for {p['id']}")
            require(p["status"] in {"planned","in-progress","implemented","validated","blocked"}, f"Invalid phase status {p['id']}")
            if p["status"] == "validated":
                require(bool(p["evidence"]), f"Validated phase lacks evidence: {p['id']}")
            seen.add(p["id"])
        require(phase_manifest["nextPhase"] in seen, "Next phase does not exist")
        next_phase = next(p for p in phases if p["id"]==phase_manifest["nextPhase"])
        require(phase_manifest["nextMilestone"] in next_phase["milestones"], "Next milestone does not belong to next phase")
        roadmap=(ROOT/"roadmap.md").read_text(encoding="utf-8")
        block=roadmap.split("<!-- BEGIN PHASE TABLE -->",1)[1].split("<!-- END PHASE TABLE -->",1)[0]
        for p in phases:
            expected=f"| {p['id']} | {p['title']} | {', '.join(p['milestones'])} | {p['gate']} |"
            require(expected in block, f"Roadmap phase table out of sync for {p['id']}")
        checked.append("P1-P9 dependency order and synchronized roadmap table")

        source_review = json.loads((ROOT / "data/source-review-manifest.json").read_text(encoding="utf-8"))
        require(source_review["kind"] == "static-source-review-not-benchmark", "Source manifest lost its static-review qualification")
        require(not any(source_review[k] for k in ("sourceModified", "applicationChecksRun", "benchmarksRun", "gpuTestsRun")), "This static review must not claim source edits or implementation checks")
        source_files = source_review["files"]
        require(len(source_files) == source_review["sourceArchive"]["fileCount"] == 43, "Inspected source file count changed")
        for name, metadata in source_files.items():
            require(name.startswith("src/") and ".." not in Path(name).parts, f"Invalid source-manifest path: {name}")
            require(bool(re.fullmatch(r"[0-9a-f]{64}", metadata["sha256"])), f"Invalid source hash: {name}")
            require(metadata["bytes"] > 0 and metadata["lines"] > 0, f"Invalid source metadata: {name}")
        observations = source_review["observations"]
        observation_ids = [o["id"] for o in observations]
        require(observation_ids == [f"SRC-{i:02d}" for i in range(1, 13)], "Static source observations must be unique SRC-01..SRC-12")
        for observation in observations:
            require(bool(observation["locations"]), f"Source observation lacks a location: {observation['id']}")
            for location in observation["locations"]:
                name = location["path"]
                require(name in source_files, f"Unknown source location: {name}")
                if name in source_files:
                    require(1 <= location["startLine"] <= location["endLine"] <= source_files[name]["lines"], f"Source range out of bounds: {observation['id']} {name}")
        checked.append("43-file static source manifest and 12 bounded source observations; not runtime verification")

        work = json.loads((ROOT / "data/engine-migration-work.json").read_text(encoding="utf-8"))
        require(work["kind"] == "planning-work-packages-not-implementation-status", "Work manifest is not labelled as a plan")
        require((ROOT / work["decision"]).is_file() and (ROOT / work["plan"]).is_file(), "Missing selected ADR or migration work plan")
        require(phase_manifest["architectureDecision"] == work["decision"], "Phase/work architecture decisions disagree")
        require(phase_manifest["engineMigrationWork"] == "data/engine-migration-work.json", "Phase manifest lacks the canonical migration-work link")
        items = work["workItems"]
        require([i["id"] for i in items] == [f"E{i:02d}" for i in range(15)], "Work packages must be unique E00..E14")
        previous_items: set[str] = set()
        migration_text = (ROOT / work["plan"]).read_text(encoding="utf-8")
        allowed_statuses = {"planned", "in-progress", "implemented", "validated", "blocked"}
        for item in items:
            identity = item["id"]
            require(item["phase"] == "P1", f"Migration work moved outside P1: {identity}")
            require(item["milestone"] in phases[0]["milestones"], f"Unknown P1 milestone: {identity}")
            require(set(item["dependsOn"]).issubset(previous_items), f"Work dependency cycle/order problem: {identity}")
            require(set(item["sourceObservations"]).issubset(set(observation_ids)), f"Unknown source observation in {identity}")
            require(item["status"] in allowed_statuses, f"Invalid work status: {identity}")
            require(isinstance(item["evidence"], list), f"Evidence must be a list: {identity}")
            if item["status"] == "validated":
                require(bool(item["evidence"]), f"Validated work lacks evidence: {identity}")
            require(f"### {identity}: {item['title']}" in migration_text, f"Work title out of sync with Markdown: {identity}")
            require(item["work"] in migration_text and item["acceptance"] in migration_text, f"Work/acceptance out of sync with Markdown: {identity}")
            previous_items.add(identity)
        require(work["nextWorkItem"] in previous_items, "Unknown next work package")
        next_work = next(i for i in items if i["id"] == work["nextWorkItem"])
        require(next_work["phase"] == phase_manifest["nextPhase"] and next_work["milestone"] == phase_manifest["nextMilestone"], "Entrypoint next-work/phase/milestone disagree")
        work_by_id = {i["id"]: i for i in items}
        require("E04" not in work_by_id["E05"]["dependsOn"] and "E04" in work_by_id["E11"]["dependsOn"], "Independent circuit branch dependency was lost")
        execution = work["phaseExecution"]
        require([e["phase"] for e in execution] == [p["id"] for p in phases], "Cross-phase engine ownership must cover P1-P9 exactly once")
        for entry, phase in zip(execution, phases):
            require(entry["milestones"] == f"{phase['milestones'][0]}-{phase['milestones'][-1]}", f"Wrong engine execution milestones: {entry['phase']}")
            require(bool(entry["ownership"]), f"Missing runtime ownership: {entry['phase']}")
        checked.append("E00-E14 DAG, source traceability, Markdown work/acceptance synchronization and P1-P9 runtime ownership")

        if args.source_archive is not None:
            archive_bytes = args.source_archive.read_bytes()
            require(hashlib.sha256(archive_bytes).hexdigest() == source_review["sourceArchive"]["sha256"], "Provided source archive differs from the inspected snapshot")
            with ZipFile(args.source_archive) as archive:
                require(archive.testzip() is None, "CRC error in provided source archive")
                names = [i.filename for i in archive.infolist() if not i.is_dir()]
                require(len(names) == len(set(names)), "Duplicate files in provided source archive")
                require(set(names) == set(source_files), "Provided source entries differ from the static manifest")
                for name, metadata in source_files.items():
                    content = archive.read(name)
                    require(hashlib.sha256(content).hexdigest() == metadata["sha256"], f"Source file hash changed: {name}")
                    require(len(content) == metadata["bytes"], f"Source size mismatch: {name}")
                    require(len(content.decode("utf-8").splitlines()) == metadata["lines"], f"Source line count mismatch: {name}")
            checked.append("Provided source ZIP CRC/SHA-256 and every source file hash/size/line count; source was not executed")
        else:
            print("NOTE: source archive not supplied; static manifest structure checked, source bytes not reverified.")

        docs = sorted([*ROOT.rglob("*.md"), *ROOT.rglob("*.html")])
        anchor_cache: dict[Path,set[str]] = {}
        link_count = 0
        for p in docs:
            text = p.read_text(encoding="utf-8")
            require("\ufffd" not in text, f"Replacement character in {p.relative_to(ROOT)}")
            require("\u2013" not in text and "\u2014" not in text, f"Long dash in authored document {p.relative_to(ROOT)}")
            require(not re.search(r"turn\d+(?:search|view|file)\d+", text), f"Unresolved tool citation in {p.relative_to(ROOT)}")
            if p.suffix == ".html":
                parsed = HTMLLinks(); parsed.feed(text)
                links = parsed.links
                anchor_cache[p.resolve()] = parsed.anchors
            else:
                links = re.findall(r"\[[^\]\n]+\]\(([^\s)]+)(?:\s+\"[^\"]*\")?\)", without_fences(text))
                anchor_cache[p.resolve()] = markdown_anchors(text)
            for target in links:
                url = urlsplit(target)
                if url.scheme or url.netloc:
                    continue
                resolved = (p.parent / unquote(url.path)).resolve() if url.path else p.resolve()
                link_count += 1
                require(resolved.is_relative_to(ROOT.resolve()), f"Local link escapes docs: {p.relative_to(ROOT)} -> {target}")
                require(resolved.exists(), f"Missing local target: {p.relative_to(ROOT)} -> {target}")
                if not resolved.exists() or not url.fragment or resolved.suffix not in {".md", ".html"}:
                    continue
                if resolved not in anchor_cache:
                    t=resolved.read_text(encoding="utf-8")
                    if resolved.suffix==".html":
                        q=HTMLLinks(); q.feed(t); anchor_cache[resolved]=q.anchors
                    else:
                        anchor_cache[resolved]=markdown_anchors(t)
                require(unquote(url.fragment) in anchor_cache[resolved], f"Missing local anchor: {p.relative_to(ROOT)} -> {target}")
        checked.append(f"{len(docs)} authored Markdown/HTML documents and {link_count} local links/anchors")

        source=(ROOT/"plans/fluid-gpu-redesign/plan.md").read_bytes()
        rendered=HTMLLinks(); rendered.feed((ROOT/"plans/fluid-gpu-redesign/plan.html").read_text(encoding="utf-8"))
        require(rendered.source_hash == hashlib.sha256(source).hexdigest(), "GPU HTML is stale; run render-plan.py")
        checked.append("Generated GPU HTML matches the current Markdown source hash")

        original=ROOT/"history/original-docs-2026-09-21.zip"
        hashes=json.loads((ROOT/"history/original-file-hashes.json").read_text(encoding="utf-8"))
        require(hashlib.sha256(original.read_bytes()).hexdigest() == ORIGINAL_SHA256, "Original input archive was modified")
        with ZipFile(original) as z:
            require(z.testzip() is None, "CRC error in preserved input archive")
            for rel, expected_hash in hashes.items():
                require(hashlib.sha256(z.read("docs/"+rel)).hexdigest()==expected_hash, f"Historical input hash mismatch: {rel}")
                require((ROOT/rel).is_file(), f"Original file path removed from updated package: {rel}")
        for name in ["audit.cjs","current-audit-output.txt","current-audit-results.json"]:
            rel="plans/fluid-gpu-redesign/"+name
            require(hashlib.sha256((ROOT/rel).read_bytes()).hexdigest()==hashes[rel], f"Historical audit evidence modified: {name}")
        audit=json.loads((ROOT/"plans/fluid-gpu-redesign/current-audit-results.json").read_text(encoding="utf-8"))
        passes=sum(1 for r in audit["results"] if r["passed"])
        require(len(audit["results"])==35 and passes==24, "Historical audit summary no longer matches retained records")
        checked.append("Input archive CRC/SHA-256, all original paths, and three unchanged audit files")
        prior_meta = json.loads((ROOT / "history/previous-revision.json").read_text(encoding="utf-8"))
        prior = ROOT / "history" / prior_meta["filename"]
        require(hashlib.sha256(prior.read_bytes()).hexdigest() == prior_meta["sha256"], "Prior documentation archive changed")
        with ZipFile(prior) as archive:
            require(archive.testzip() is None, "CRC error in prior documentation archive")
            for rel in ("data/element-roadmap.json", "data/element-roadmap.schema.json", "elements.md"):
                require(archive.read("docs/" + rel) == (ROOT / rel).read_bytes(), f"Approved element roster/schema/document changed: {rel}")
        checked.append("Prior documentation ZIP CRC/SHA-256 and byte-preserved element roster, schema and element guide")

    except (OSError, ValueError, KeyError, IndexError, StopIteration, TypeError, BadZipFile) as exc:
        errors.append(f"Validation could not complete: {type(exc).__name__}: {exc}")

    for message in checked:
        print("CHECKED:", message)
    if errors:
        for error in errors:
            print("FAIL:", error, file=sys.stderr)
        return 1
    print("PASS: documentation package checks. No application, numerical, browser or GPU tests were run.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
