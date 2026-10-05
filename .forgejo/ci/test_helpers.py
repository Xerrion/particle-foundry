"""Exercise the Forgejo HTTP contracts without a running instance."""

import base64
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import os
from pathlib import Path
import subprocess
import tarfile
import tempfile
import threading
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlsplit

import artifact
import cache
import runtime


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def handle_request(self):
        length = int(self.headers.get("Content-Length", 0))
        body = self.rfile.read(length)
        route = urlsplit(self.path)
        query = parse_qs(route.query)
        server = self.server
        status, result = 200, {}
        if route.path.endswith("/cache"):
            keys = query["keys"][0].split(",")
            hits = [key for key in server.entries if any(key.startswith(prefix) for prefix in keys)]
            if not hits:
                status, result = 204, None
            else:
                result = {"cacheKey": hits[-1], "archiveLocation": server.base + "/archive/" + hits[-1]}
        elif route.path.startswith("/archive/"):
            result = server.entries[route.path.split("/")[-1]]
        elif route.path.endswith("/caches"):
            server.reservations.append(json.loads(body))
            result = {"cacheId": len(server.reservations)}
        elif "/caches/" in route.path:
            cache_id = int(route.path.split("/")[-1])
            if self.command == "PATCH":
                start = len(server.pending.get(cache_id, b""))
                expected = f"bytes {start}-{start + len(body) - 1}/*"
                if self.headers["Content-Range"] != expected:
                    status = 400
                server.pending[cache_id] = server.pending.get(cache_id, b"") + body
            else:
                reservation = server.reservations[cache_id - 1]
                content = server.pending[cache_id]
                if len(content) != reservation["cacheSize"]:
                    status = 400
                server.entries[reservation["key"]] = content
        elif route.path.endswith("/artifacts"):
            if self.headers.get("Authorization") != "Bearer test-runtime-token":
                status = 401
            elif "/workflows/42/" not in route.path:
                status = 400
            elif self.command == "POST":
                server.artifact_metadata = json.loads(body)
                result = {"fileContainerResourceUrl": server.base + "/container?opaque=test"}
            else:
                server.finalized = query["artifactName"][0]
        elif route.path == "/container":
            if self.headers.get("Authorization") != "Bearer test-runtime-token":
                status = 401
            md5 = base64.b64encode(hashlib.md5(body).digest()).decode()
            if self.headers["x-actions-results-md5"] != md5 or query.get("opaque") != ["test"]:
                status = 400
            name = query["itemPath"][0]
            start = len(server.files.get(name, b""))
            size = self.headers["x-tfs-filelength"]
            if self.headers["Content-Range"] != f"bytes {start}-{start + len(body) - 1}/{size}":
                status = 400
            server.files[name] = server.files.get(name, b"") + body
        else:
            status = 302 if route.path == "/redirect" else 500
        self.send_response(status)
        if status == 302:
            self.send_header("Location", server.base + "/redirected")
        self.end_headers()
        if result is not None:
            self.wfile.write(result if isinstance(result, bytes) else json.dumps(result).encode())

    do_GET = do_POST = do_PATCH = do_PUT = handle_request


class HelpersTest(unittest.TestCase):
    def setUp(self):
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.base = f"http://127.0.0.1:{self.server.server_port}"
        self.server.entries, self.server.pending, self.server.files = {}, {}, {}
        self.server.reservations = []
        self.server.finalized = None
        self.thread = threading.Thread(target=lambda: self.server.serve_forever(poll_interval=0.01), daemon=True)
        self.thread.start()
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.environ = patch.dict(os.environ, {"ACTIONS_CACHE_URL": self.server.base + "/run/opaque/",
            "ACTIONS_RUNTIME_URL": self.server.base + "/", "ACTIONS_RUNTIME_TOKEN": "test-runtime-token",
            "FORGEJO_RUN_ID": "42"})
        self.environ.start()

    def tearDown(self):
        self.environ.stop()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.temp.cleanup()

    def test_cache_miss_exact_and_changed_source(self):
        output = self.root / "target"
        output.mkdir()
        binary = output / "compiled"
        binary.write_bytes(b"compiled output" * 1000)
        state = self.root / "state.json"
        with patch.object(cache, "configuration", return_value=([output], "prefix-one", ["prefix-"])), \
             patch.object(cache, "CHUNK_SIZE", 64):
            cache.restore("rust", state)
            self.assertFalse(json.loads(state.read_text())["exact"])
            cache.save("rust", state)
            binary.write_bytes(b"changed")
            cache.restore("rust", state)
            self.assertEqual(binary.read_bytes(), b"compiled output" * 1000)
            cache.save("rust", state)
        self.assertEqual(len(self.server.reservations), 1)
        with patch.object(cache, "configuration", return_value=([output], "prefix-two", ["prefix-"])):
            cache.restore("rust", state)
            self.assertFalse(json.loads(state.read_text())["exact"])
            cache.save("rust", state)
        self.assertEqual(len(self.server.reservations), 2)

    def test_cache_rejects_paths_outside_selected_directories(self):
        archive = self.root / "bad.tar.zst"
        payload = io.BytesIO()
        with tarfile.open(fileobj=payload, mode="w") as contents:
            member = tarfile.TarInfo("etc/forgejo-cache-test")
            member.size = 1
            contents.addfile(member, io.BytesIO(b"x"))
        compressed = subprocess.run(["zstd", "-q", "-c"], input=payload.getvalue(), capture_output=True, check=True)
        archive.write_bytes(compressed.stdout)
        with self.assertRaisesRegex(RuntimeError, "unexpected path"):
            cache.extract(archive, [self.root / "target"])

    def test_failed_exact_restore_can_save_rebuilt_outputs(self):
        output = self.root / "target"
        output.mkdir()
        (output / "compiled").write_bytes(b"rebuilt output")
        state = self.root / "state.json"
        self.server.entries["prefix-one"] = b"invalid archive"
        with patch.object(cache, "configuration", return_value=([output], "prefix-one", ["prefix-"])):
            with self.assertRaises(Exception):
                cache.restore("rust", state)
            self.assertFalse(json.loads(state.read_text())["exact"])
            cache.save("rust", state)
            cache.restore("rust", state)
        self.assertTrue(json.loads(state.read_text())["exact"])

    def test_artifact_chunks_auth_and_finalize(self):
        report = self.root / "evidence"
        report.mkdir()
        (report / "report.json").write_bytes(b"browser evidence" * 100)
        (report / "empty.log").touch()
        (report / "external").symlink_to("/etc/passwd")
        with patch.object(artifact, "CHUNK_SIZE", 64):
            artifact.upload(report, "engine-browser-smoke")
        self.assertEqual(self.server.files, {"engine-browser-smoke/report.json": b"browser evidence" * 100})
        self.assertEqual(self.server.finalized, "engine-browser-smoke")
        self.assertEqual(self.server.artifact_metadata["RetentionDays"], 1)

    def test_runtime_rejects_redirects_and_other_origins_without_url_in_error(self):
        secret_url = self.server.base + "/opaque-secret/error"
        with self.assertRaisesRegex(RuntimeError, "HTTP 500") as failure:
            runtime.json_request(secret_url)
        self.assertNotIn("opaque-secret", str(failure.exception))
        with self.assertRaisesRegex(RuntimeError, "HTTP 302"):
            runtime.json_request(self.server.base + "/redirect")
        with self.assertRaisesRegex(RuntimeError, "unexpected origin"):
            runtime.require_same_origin("https://example.invalid/upload", self.server.base)


if __name__ == "__main__":
    unittest.main()
