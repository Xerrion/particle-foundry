"""Exercise deployment pinning, queue monitoring and safe error output."""

from contextlib import redirect_stderr, redirect_stdout
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlsplit

import deploy

SHA = "a" * 40
OLD_SHA = "b" * 40
APP_UUID = "test-app"


def record(uuid="new-deployment", commit=SHA, status="finished", pull_request_id=0):
    return {"deployment_uuid": uuid, "commit": commit, "status": status, "pull_request_id": pull_request_id}


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def handle_request(self):
        server = self.server
        route = urlsplit(self.path)
        body = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        server.requests.append((self.command, self.path, self.headers.get("Authorization"), body))
        status = 200
        if route.path == "/api/v1/repos/test/project/branches/main":
            result = {"commit": {"id": server.main_sha}}
        elif route.path == f"/api/v1/deployments/applications/{APP_UUID}":
            if parse_qs(route.query) != {"skip": ["0"], "take": ["10"]}:
                status = 400
            result = server.lists.pop(0) if len(server.lists) > 1 else server.lists[0]
        elif route.path == "/api/v1/deployments/new-deployment":
            result = server.details.pop(0) if len(server.details) > 1 else server.details[0]
        elif route.path == f"/api/v1/applications/{APP_UUID}" and self.command == "PATCH":
            status, result = server.patch_status, server.patch_result
        else:
            status, result = 500, {"message": "unexpected route"}
        self.send_response(status)
        if status == 302:
            self.send_header("Location", server.base + "/redirected")
        self.end_headers()
        self.wfile.write(result if isinstance(result, bytes) else json.dumps(result).encode())

    do_GET = do_PATCH = handle_request


class DeployTest(unittest.TestCase):
    def setUp(self):
        self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.server.base = f"http://127.0.0.1:{self.server.server_port}"
        self.server.main_sha = SHA
        self.server.requests = []
        self.server.lists = [{"count": 0, "deployments": []}, {"count": 1, "deployments": [record()]}]
        self.server.details = [record()]
        self.server.patch_status = 200
        self.server.patch_result = {"uuid": APP_UUID}
        self.thread = threading.Thread(target=lambda: self.server.serve_forever(poll_interval=0.01), daemon=True)
        self.thread.start()
        # Loopback HTTP is injected here. Production environment parsing rejects it.
        self.config = deploy.Configuration(self.server.base, APP_UUID, "test-coolify-token",
                                           self.server.base, "test/project", "test-forgejo-token", SHA)
        self.sleep = patch.object(deploy.time, "sleep", return_value=None)
        self.sleep.start()

    def tearDown(self):
        self.sleep.stop()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def run_main(self):
        output = io.StringIO()
        with patch.object(deploy, "configuration", return_value=self.config), redirect_stdout(output), redirect_stderr(output):
            result = deploy.main()
        return result, output.getvalue()

    def test_pins_exact_verified_commit_and_waits_for_same_deployment(self):
        self.server.lists = [
            {"deployments": [record("previous", status="finished")]},
            {"deployments": [record(status="queued")]},
        ]
        self.server.details = [record(status="in_progress"), record()]
        result, output = self.run_main()
        self.assertEqual(result, 0)
        self.assertIn("deployment finished", output)
        requests = [request for request in self.server.requests if request[0] == "PATCH"]
        self.assertEqual(len(requests), 1)
        self.assertEqual(json.loads(requests[0][3]), {"git_commit_sha": SHA, "instant_deploy": True})
        self.assertEqual(requests[0][2], "Bearer test-coolify-token")
        branch = next(request for request in self.server.requests if "branches/main" in request[1])
        self.assertEqual(branch[2], "Bearer test-forgejo-token")
        self.assertEqual([request[0] for request in self.server.requests], ["GET", "GET", "PATCH", "GET", "GET", "GET"])

    def test_stale_workflow_skips_without_mutation(self):
        self.server.main_sha = OLD_SHA
        result, output = self.run_main()
        self.assertEqual(result, 0)
        self.assertIn("Deployment skipped", output)
        self.assertFalse(any(request[0] == "PATCH" for request in self.server.requests))

    def test_tracks_uuid_after_newer_deployments_remove_it_from_recent_history(self):
        recent = [record(f"preview-{number}", commit=OLD_SHA, pull_request_id=number + 1) for number in range(10)]
        self.server.lists = [{"deployments": []}, {"deployments": [record(status="queued")]},
                             {"deployments": recent}]
        self.server.details = [record(status="in_progress"), record()]
        with patch.object(deploy.time, "monotonic", side_effect=[0, 0, 1, 2]), \
             patch.object(deploy, "DEPLOY_TIMEOUT_SECONDS", 2):
            result, output = self.run_main()
        self.assertEqual(result, 0)
        self.assertIn("deployment finished", output)
        lists = [request for request in self.server.requests if "/deployments/applications/" in request[1]]
        details = [request for request in self.server.requests if request[1] == "/api/v1/deployments/new-deployment"]
        self.assertEqual(len(lists), 2)
        self.assertEqual(len(details), 2)
        self.assertTrue(all(request[2] == "Bearer test-coolify-token" for request in details))

    def test_http_200_without_new_queue_record_fails(self):
        self.server.lists = [{"deployments": [record("previous")]}]
        with patch.object(deploy, "QUEUE_TIMEOUT_SECONDS", 0):
            result, output = self.run_main()
        self.assertEqual(result, 1)
        self.assertIn("did not queue a new deployment", output)
        self.assertNotIn("deployment finished", output)

    def test_another_commit_or_preview_cannot_satisfy_the_queue_check(self):
        for candidate in [record(commit=OLD_SHA), record(pull_request_id=42)]:
            with self.subTest(candidate=candidate):
                self.server.lists = [{"deployments": []}, {"deployments": [candidate]}]
                with patch.object(deploy, "QUEUE_TIMEOUT_SECONDS", 0):
                    result, output = self.run_main()
                self.assertEqual(result, 1)
                self.assertIn("did not queue a new deployment", output)

    def test_failed_and_cancelled_deployments_fail_the_job(self):
        for status in ["failed", "cancelled-by-user"]:
            with self.subTest(status=status):
                self.server.lists = [{"deployments": []}, {"deployments": [record(status="queued")]}]
                self.server.details = [record(status=status)]
                result, output = self.run_main()
                self.assertEqual(result, 1)
                self.assertIn(f"status {status}", output)

    def test_tracked_deployment_cannot_change_commit_or_use_a_different_uuid(self):
        for detail, message in [(record(commit=OLD_SHA), "no longer matches the verified commit"),
                                (record("different"), "unexpected deployment UUID")]:
            with self.subTest(detail=detail):
                self.server.lists = [{"deployments": []}, {"deployments": [record(status="queued")]}]
                self.server.details = [detail]
                result, output = self.run_main()
                self.assertEqual(result, 1)
                self.assertIn(message, output)

    def test_malformed_deployment_detail_cannot_report_success(self):
        self.server.lists = [{"deployments": []}, {"deployments": [record(status="queued")]}]
        self.server.details = [{"message": "internal-build-secret"}]
        result, output = self.run_main()
        self.assertEqual(result, 1)
        self.assertIn("unexpected deployment UUID", output)
        self.assertNotIn("internal-build-secret", output)

    def test_unfinished_deployment_times_out(self):
        self.server.lists = [{"deployments": []}, {"deployments": [record(status="in_progress")]}]
        with patch.object(deploy, "DEPLOY_TIMEOUT_SECONDS", 0):
            result, output = self.run_main()
        self.assertEqual(result, 1)
        self.assertIn("did not finish within 20 minutes", output)

    def test_malformed_deployment_list_fails_before_mutation(self):
        self.server.lists = [{"deployments": "invalid"}]
        result, output = self.run_main()
        self.assertEqual(result, 1)
        self.assertIn("invalid deployment list", output)
        self.assertFalse(any(request[0] == "PATCH" for request in self.server.requests))

    def test_http_failures_and_redirects_do_not_log_secrets(self):
        secret_body = b'test-coolify-token test-forgejo-token internal-build-secret'
        for status in [500, 302]:
            with self.subTest(status=status):
                self.server.lists = [{"deployments": []}]
                self.server.patch_status = status
                self.server.patch_result = secret_body
                result, output = self.run_main()
                self.assertEqual(result, 1)
                self.assertIn("Coolify API request failed", output)
                for secret in ["test-coolify-token", "test-forgejo-token", "internal-build-secret", self.server.base]:
                    self.assertNotIn(secret, output)
                self.assertFalse(any("redirected" in request[1] for request in self.server.requests))

    def test_unknown_status_does_not_log_response_data(self):
        self.server.lists = [{"deployments": []}, {"deployments": [record(status="queued")]}]
        self.server.details = [record(status="internal-build-secret")]
        result, output = self.run_main()
        self.assertEqual(result, 1)
        self.assertIn("unknown deployment status", output)
        self.assertNotIn("internal-build-secret", output)

    def test_environment_allows_tailnet_http_but_rejects_public_http_and_credentials(self):
        values = {"FORGEJO_EVENT_NAME": "push", "FORGEJO_REF": "refs/heads/main", "CI_SHA": SHA,
                  "COOLIFY_URL": "http://xcore.grayling-bigeye.ts.net:8000", "COOLIFY_APP_UUID": APP_UUID,
                  "COOLIFY_TOKEN": "test-coolify-token", "FORGEJO_SERVER_URL": "https://git.example.invalid",
                  "FORGEJO_REPOSITORY": "test/project", "CI_TOKEN": "test-forgejo-token"}
        with patch.dict(os.environ, values, clear=True):
            self.assertEqual(deploy.configuration().coolify_url, values["COOLIFY_URL"])
            for url in ["http://coolify.example.invalid", "https://token@example.invalid", self.server.base]:
                with patch.dict(os.environ, {"COOLIFY_URL": url}):
                    with self.assertRaises(deploy.DeploymentError):
                        deploy.configuration()
            with patch.dict(os.environ, {"FORGEJO_REF": "refs/heads/feature"}):
                with self.assertRaises(deploy.DeploymentError):
                    deploy.configuration()
            with patch.dict(os.environ, {"CI_SHA": SHA[:7]}):
                with self.assertRaises(deploy.DeploymentError):
                    deploy.configuration()


class WorkflowScopeTest(unittest.TestCase):
    def test_latest_docs_only_main_receives_full_checks_when_deployment_is_enabled(self):
        workflow = (Path(__file__).resolve().parents[1] / "workflows" / "ci.yml").read_text().splitlines()
        step = workflow.index("      - name: Detect code changes")
        start = next(index for index in range(step, len(workflow)) if workflow[index] == "        run: |") + 1
        command = []
        for line in workflow[start:]:
            if line and not line.startswith("          "):
                break
            command.append(line[10:])
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def git(*args):
                return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout.strip()
            git("init", "-q", "-b", "main")
            (root / "app.js").write_text("verified application revision\n")
            git("add", "app.js")
            git("-c", "user.name=CI-test", "-c", "user.email=ci-test@example.invalid", "commit", "-qm", "Application A")
            before = git("rev-parse", "HEAD")
            (root / "docs").mkdir()
            (root / "docs" / "readme.md").write_text("Documentation revision B\n")
            git("add", "docs/readme.md")
            git("-c", "user.name=CI-test", "-c", "user.email=ci-test@example.invalid", "commit", "-qm", "Documentation B")
            current = git("rev-parse", "HEAD")
            output = root / "scope-output"
            for event, ref, enabled, expected in [
                ("push", "refs/heads/main", "true", "true"),
                ("push", "refs/heads/main", "false", "false"),
                ("push", "refs/heads/main", "", "false"),
                ("pull_request", "refs/pull/7/head", "true", "false"),
                ("push", "refs/heads/feature", "true", "false"),
            ]:
                with self.subTest(event=event, ref=ref, enabled=enabled):
                    output.write_text("")
                    env = {**os.environ, "CI_BASE_SHA": before, "CI_SHA": current,
                           "FORGEJO_EVENT_NAME": event, "FORGEJO_REF": ref,
                           "COOLIFY_DEPLOY_ENABLED": enabled, "FORGEJO_OUTPUT": str(output), "RUNNER_TEMP": str(root)}
                    subprocess.run(["sh", "-e", "-c", "\n".join(command)], cwd=root, env=env,
                                   capture_output=True, text=True, check=True)
                    self.assertEqual(output.read_text().strip(), f"code={expected}")


if __name__ == "__main__":
    unittest.main()
