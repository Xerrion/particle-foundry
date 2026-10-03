"""Cache selected build paths through Forgejo Runner's native cache service."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import tempfile

from runtime import add_query, json_request, request, require_same_origin

VERSION = "particle-foundry-tar-zstd-v2"
CHUNK_SIZE = 32 * 1024 * 1024


def digest(paths):
    result = hashlib.sha256()
    for path in sorted(paths):
        result.update(str(path).encode() + b"\0")
        result.update(path.read_bytes())
    return result.hexdigest()[:24]


def configuration(job):
    workspace = Path.cwd()
    cargo = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    paths = [workspace / "web/node_modules", Path.home() / ".bun/install/cache"]
    if job in ("rust", "browser"):
        paths += [workspace / "engine/target", cargo / "registry", cargo / "git"]
    image = digest([Path(".forgejo/ci/Dockerfile"), Path(".forgejo/ci/chrome-ci"), Path("mise.toml")])
    toolchain = digest([Path("engine/rust-toolchain.toml")])
    locks = digest([Path("engine/Cargo.lock"), Path("web/bun.lock")])
    source = digest([Path("engine/Cargo.toml"), *[path for path in Path("engine/crates").rglob("*") if path.is_file()]])
    prefix = f"particle-foundry-v2-{job}-{image}-{toolchain}-"
    key = f"{prefix}{locks}-{source}"
    return paths, key, [f"{prefix}{locks}-", prefix]


def api_base():
    base = os.environ.get("ACTIONS_CACHE_URL")
    if not base:
        raise RuntimeError("Forgejo Runner cache is disabled")
    return base.rstrip("/") + "/_apis/artifactcache/"


def extract(archive, paths):
    allowed = [path.resolve() for path in paths]

    def filter_member(member, destination):
        member = tarfile.data_filter(member, destination)
        target = (Path(destination) / member.name).resolve()
        if not any(target == path or target.is_relative_to(path) for path in allowed):
            raise RuntimeError("Cache archive contains an unexpected path")
        if member.issym() or member.islnk():
            link = (target.parent / member.linkname).resolve() if member.issym() else Path("/") / member.linkname
            if not any(link == path or link.is_relative_to(path) for path in allowed):
                raise RuntimeError("Cache archive contains an unexpected link")
        return member

    process = subprocess.Popen(["zstd", "-d", "-q", "-c", str(archive)], stdout=subprocess.PIPE)
    try:
        with tarfile.open(fileobj=process.stdout, mode="r|") as contents:
            contents.extractall("/", filter=filter_member)
    finally:
        process.stdout.close()
        if process.wait() != 0:
            raise RuntimeError("Cache decompression failed")


def restore(job, state):
    paths, key, prefixes = configuration(job)
    state.write_text(json.dumps({"key": key, "exact": False}))
    url = add_query(api_base() + "cache", keys=",".join([key, *prefixes]), version=VERSION)
    with request(url) as response:
        entry = json.loads(response.read()) if response.status != 204 else None
    if not entry:
        print(f"Cache miss: {job}")
        return
    location = require_same_origin(entry["archiveLocation"], api_base())
    with tempfile.TemporaryDirectory() as directory:
        archive = Path(directory) / "cache.tar.zst"
        with request(location) as response, archive.open("wb") as output:
            shutil.copyfileobj(response, output)
        extract(archive, paths)
    state.write_text(json.dumps({"key": key, "exact": entry["cacheKey"] == key}))
    print(f"Cache restored: {job} ({'exact' if entry['cacheKey'] == key else 'previous build'})")


def save(job, state):
    paths, key, _ = configuration(job)
    if state.exists() and json.loads(state.read_text()).get("exact"):
        print(f"Cache unchanged: {job}")
        return
    members = [str(path).lstrip("/") for path in paths if path.exists()]
    if not members:
        return
    with tempfile.TemporaryDirectory() as directory:
        archive = Path(directory) / "cache.tar.zst"
        subprocess.run(["tar", "-C", "/", "--use-compress-program=zstd -T2 -1", "-cf", str(archive), *members],
                       check=True, env={**os.environ, "COPYFILE_DISABLE": "1"})
        size = archive.stat().st_size
        reserved = json_request(api_base() + "caches", "POST", {"key": key, "version": VERSION, "cacheSize": size})
        upload = api_base() + f"caches/{reserved['cacheId']}"
        with archive.open("rb") as content:
            start = 0
            while chunk := content.read(CHUNK_SIZE):
                with request(upload, "PATCH", chunk, {"Content-Type": "application/octet-stream",
                             "Content-Range": f"bytes {start}-{start + len(chunk) - 1}/*"}):
                    pass
                start += len(chunk)
        json_request(upload, "POST", {})
        print(f"Cache saved: {job} ({size // (1024 * 1024)} MiB)")


if __name__ == "__main__":
    mode, job = sys.argv[1:]
    if job not in ("web", "rust", "browser") or mode not in ("restore", "save"):
        raise SystemExit("Usage: cache.py restore|save web|rust|browser")
    state = Path(os.environ["RUNNER_TEMP"]) / f"particle-cache-{job}.json"
    try:
        (restore if mode == "restore" else save)(job, state)
    except Exception as error:
        # Caches are optional. Check failures are handled by the workflow gate.
        safe = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        print(f"::warning::Build cache unavailable: {safe}")
