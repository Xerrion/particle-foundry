# Build and maintain the documentation

Start project work from the [handbook index](README.md). This article explains
how to compile, check and maintain the complete documentation set.

## Build the browser handbook

Run commands from the repository root. Use the pinned Python environment in
`mise.toml`. The existing setup installs `markdown-it-py==4.2.0` in `.venv/`.
If that dependency is missing, install it with `mise run docs:install` before
building. Agents must obtain permission before installing a missing dependency.

```sh
mise run docs:build
```

The build writes `artifacts/docs/index.html` and its linked files under
`artifacts/docs/files/`. Open the HTML directly for offline reading, or serve it:

```sh
mise run docs:serve
```

Open <http://127.0.0.1:8000>. Stop the local server with Ctrl+C.
The server binds only to the loopback interface. To share the complete set,
copy the entire `artifacts/docs/` directory. The HTML contains all chapters.
The `files/` directory contains linked source, data, evidence and archives.
External reference links still need internet access.

The build uses checked-out files, including uncommitted edits. Generated output
stays ignored. Rebuild after documentation changes. Do not commit the handbook.

## What the build includes

The compiler discovers every Markdown file under `docs/` and repository guidance
outside that directory. It includes current topics, architecture, plans,
validation, templates, history and compatibility pages. A new documentation
chapter needs no separate build registration.

Internal chapter links and headings resolve within the single HTML page.
Linked repository files are copied beside it. The build checks the generated
links and rejects missing targets. It does not copy credentials or ignored local
run artifacts. Formula text, tables and code remain available without an external
script or CDN. Browser search filters the chapter navigation and content.

The existing GPU plan HTML remains a compatibility view of its canonical
Markdown. `mise run docs:render` regenerates it. The complete handbook reads
the Markdown directly.

## Check changes

1. Run `mise run docs:render` if the GPU plan or its renderer changed.
2. Run `mise run docs:check` for source links, manifests, preserved evidence and handbook checks.
3. Run `mise run check:markdown` after Markdown edits. Review formatting changes within the assigned scope.
4. Run `mise run check:python` after documentation script edits.
5. Inspect the complete diff and open the compiled handbook. Check navigation, search, tables and narrow screens.

`docs:check` validates documentation claims structurally. It does not execute
application or numerical tests. Run `mise run ci` before delivering application,
engine or build changes. Report hardware checks separately.

Formatters exclude `docs/history/`, `docs/evidence/` and the approved
`docs/elements.md` snapshot. The documentation validator checks their preserved
content and integrity. Keep those records byte-for-byte unchanged.

## Keep one owner per subject

| Subject                                  | Owner                                                                              |
| ---------------------------------------- | ---------------------------------------------------------------------------------- |
| Starting or continuing work              | [Handbook index](README.md)                                                        |
| Setup and running the game               | [Root README](../README.md), with executable commands in [mise.toml](../mise.toml) |
| Selected P/M/E and delivery state        | [Danish status](STATUS.md)                                                         |
| Phase order and engineering mapping      | [Roadmap](roadmap.md), [work reference](remaining-work.md) and their manifests     |
| Behavior, contracts and measured results | The relevant topic, controlling plan or validation article linked from the index   |

`START_HERE.md` and `RESUME.md` retain old links. They direct readers to the
handbook index. The dated recovery record belongs to historical reference.
Do not add another continuation guide.

## Write and update an article

Extend an existing topic before creating another file. Use one subject per
article, descriptive headings and ordinary Markdown links. Follow the
[combustion article](combustion.md) when a topic needs detailed explanation.

1. State purpose, backend, scope and implementation status near the top.
2. Explain behavior, units, invariants and exceptions. Link design rationale beside the relevant rule.
3. Link source symbols and named regression tests. File existence alone does not verify a scientific claim.
4. Separate known defects, unknowns and planned corrections. Link requirements and evidence instead of copying them.
5. Update the index when discovery changes. Run the documentation checks before delivery.

Date a source review or measurement only when it occurred. Record the revision,
dirty state, commands, results and limitations with the [evidence template](templates/phase-evidence.md).
Keep transient logs and screenshots under ignored `artifacts/`.

Update [STATUS.md](STATUS.md) when selected work, completion, merge state,
blockers or deployment evidence changes. Check live Linear and the owning
evidence first. Keep its Danish explanation short. Preserve the distinction
between the selected work track and the next incomplete migration item.
