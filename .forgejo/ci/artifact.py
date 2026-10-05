"""Upload browser evidence through Forgejo's native artifact API."""

import base64
import hashlib
import os
from pathlib import Path
import sys

from runtime import add_query, json_request, request, require_same_origin

CHUNK_SIZE = 8 * 1024 * 1024


def upload(directory, name):
    files = sorted(path for path in directory.rglob("*")
                   if path.is_file() and not path.is_symlink() and path.stat().st_size)
    if not files:
        print("No browser evidence was generated")
        return
    base = os.environ["ACTIONS_RUNTIME_URL"].rstrip("/")
    run_id = os.environ["FORGEJO_RUN_ID"]
    api = add_query(f"{base}/_apis/pipelines/workflows/{run_id}/artifacts", **{"api-version": "6.0-preview"})
    headers = {"Authorization": f"Bearer {os.environ['ACTIONS_RUNTIME_TOKEN']}"}
    created = json_request(api, "POST", {"Type": "actions_storage", "Name": name, "RetentionDays": 1}, headers)
    location = require_same_origin(created["fileContainerResourceUrl"], base)
    for path in files:
        url = add_query(location, itemPath=f"{name}/{path.relative_to(directory).as_posix()}")
        size = path.stat().st_size
        with path.open("rb") as content:
            start = 0
            while True:
                chunk = content.read(CHUNK_SIZE)
                chunk_headers = {**headers, "Content-Type": "application/octet-stream",
                                 "Content-Range": f"bytes {start}-{max(start, start + len(chunk) - 1)}/{size}",
                                 "x-tfs-filelength": str(size),
                                 "x-actions-results-md5": base64.b64encode(hashlib.md5(chunk, usedforsecurity=False).digest()).decode()}
                with request(url, "PUT", chunk, chunk_headers):
                    pass
                start += len(chunk)
                if start >= size:
                    break
    json_request(add_query(api, artifactName=name), "PATCH", {}, headers)
    print(f"Artifact uploaded: {name} ({len(files)} files)")


if __name__ == "__main__":
    try:
        upload(Path("artifacts/validation/browser"), "engine-browser-smoke")
    except Exception as error:
        safe = str(error) if isinstance(error, RuntimeError) else type(error).__name__
        raise SystemExit(f"Artifact upload failed: {safe}") from None
