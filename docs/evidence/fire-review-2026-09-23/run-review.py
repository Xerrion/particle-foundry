#!/usr/bin/env python3
"""Run the characterization checks against an unchanged source ZIP.

Requires Python 3.9+, Node.js and TypeScript's tsc on PATH.
Usage: python run-review.py /path/to/src.zip --output ./fire-review-run
No packages are installed and the original ZIP is never written.
"""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import zipfile


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source_zip', type=Path)
    parser.add_argument('--output', type=Path, default=Path('fire-review-run'))
    args = parser.parse_args()
    archive = args.source_zip.resolve(strict=True)
    output = args.output.resolve()
    if output.exists():
        parser.error('Output path already exists. Choose a new directory to preserve earlier evidence.')
    tools = {name: shutil.which(name) for name in ('node', 'tsc')}
    missing = [name for name, binary in tools.items() if not binary]
    if missing:
        parser.error('Missing tools on PATH: ' + ', '.join(missing))
    output.mkdir(parents=True)
    source = output / 'source'
    source.mkdir()
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            target = (source / info.filename).resolve()
            if not target.is_relative_to(source.resolve()):
                raise ValueError(f'Unsafe archive path: {info.filename}')
        z.extractall(source)
    root = source / 'src'
    if not (root / 'simulation' / 'physics.ts').is_file():
        parser.error('Expected src/simulation/physics.ts in the source ZIP.')
    build = output / 'build'
    subprocess.run([
        tools['tsc'], '--target', 'ES2022', '--module', 'commonjs',
        '--moduleResolution', 'node', '--lib', 'ES2022,DOM', '--skipLibCheck',
        '--outDir', str(build), '--rootDir', str(root),
        str(root / 'simulation' / 'physics.ts'), str(root / 'tools' / 'brush.ts'),
    ], check=True)
    harness = output / 'audit-fire.cjs'
    shutil.copyfile(Path(__file__).with_name('audit-fire.cjs'), harness)
    env = dict(os.environ, FIRE_REVIEW_BUILD=str(build))
    run = subprocess.run([tools['node'], str(harness)], env=env, text=True,
                         stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    (output / 'test-output.txt').write_text(run.stdout, encoding='utf-8')
    print(run.stdout)
    with zipfile.ZipFile(archive) as z:
        for info in z.infolist():
            if not info.is_dir() and (source / info.filename).read_bytes() != z.read(info.filename):
                raise RuntimeError(f'Extracted source changed: {info.filename}')
    print(f'Evidence: {output}')
    if run.returncode:
        raise SystemExit(run.returncode)
    data = json.loads((output / 'fire-audit-results.json').read_text())
    print(f"Confirmed {len(data['results'])} characterization observations; this is not a corrected-spec pass.")


if __name__ == '__main__':
    main()
