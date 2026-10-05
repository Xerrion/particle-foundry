"""Exercise preview isolation, current CI gates, lifecycle races and safe errors."""

from contextlib import redirect_stderr, redirect_stdout
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import copy
import io
import json
import os
import threading
import unittest
from unittest.mock import patch
from urllib.parse import parse_qs, urlsplit

import preview

SHA = "a" * 40
OLD_SHA = "b" * 40
APP = "preview-app"
PRODUCTION = "production-app"
REPOSITORY = "git@charles.example.ts.net:2222/test/project.git"


def pr(state="open", sha=SHA, branch="codex/test"):
    repository = {"id": 7, "full_name": "test/project"}
    return {"number": 16, "state": state, "head": {"sha": sha, "ref": branch, "repo": repository.copy()},
            "base": {"ref": "main", "repo": repository.copy()}}


def run(status="success", id=100):
    return {"id": id, "index_in_repo": 42, "workflow_id": "ci.yml", "commit_sha": SHA,
            "event": "pull_request", "trigger_event": "pull_request", "status": status,
            "is_fork_pull_request": False, "need_approval": False}


def tasks():
    return [{"id": index + 1, "name": name, "head_sha": SHA, "run_number": 42,
             "workflow_id": "ci.yml", "event": "pull_request", "status": "success"}
            for index, name in reversed(list(enumerate(preview.REQUIRED_JOBS)))]


def deployment(uuid="new-deployment", status="finished", sha=SHA, pull_request_id=0):
    return {"deployment_uuid": uuid, "status": status, "commit": sha, "pull_request_id": pull_request_id}


def list_value(values):
    return {"total_count": len(values), "workflow_runs": values}


def next_value(values):
    return values.pop(0) if len(values) > 1 else values[0]


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def handle_request(self):
        server = self.server
        route = urlsplit(self.path)
        data = self.rfile.read(int(self.headers.get("Content-Length", 0)))
        server.requests.append((self.command, route.path, parse_qs(route.query),
                                self.headers.get("Authorization"), json.loads(data) if data else None))
        status, result = 200, None
        if server.error_path == route.path:
            status, result = server.error_status, b"test-coolify-token test-forgejo-token private-response-secret"
        elif route.path == "/api/v1/repos/test/project/pulls/16":
            result = next_value(server.prs)
        elif route.path == "/api/v1/repos/test/project/actions/runs":
            result = next_value(server.runs)
        elif route.path == "/api/v1/repos/test/project/actions/tasks":
            result = next_value(server.tasks)
        elif route.path == f"/api/v1/applications/{PRODUCTION}":
            result = {"uuid": PRODUCTION, "git_repository": REPOSITORY, "environment_id": 1}
        elif route.path == "/api/v1/projects/project-uuid/environments":
            result = [{"id": 1, "uuid": "production-env"}, {"id": 2, "uuid": "preview-env"}]
        elif route.path == "/api/v1/applications":
            result = server.apps
        elif route.path == "/api/v1/security/keys":
            result = server.keys
        elif route.path == "/api/v1/applications/private-deploy-key" and self.command == "POST":
            body = json.loads(data)
            app = {key: value for key, value in body.items() if key not in preview.SETTING_FLAGS}
            app.update(uuid=APP, environment_id=2, fqdn=body["domains"],
                       settings={key: body[key] for key in preview.SETTING_FLAGS})
            server.apps.append(app)
            if server.change_after_create:
                server.prs = [pr(sha=OLD_SHA)]
            status, result = 201, {"uuid": APP, "domains": body["domains"]}
        elif route.path == f"/api/v1/applications/{APP}":
            if self.command == "DELETE":
                server.apps = []
                result = {"message": "Application deletion request queued."}
            elif self.command == "PATCH":
                result = {"uuid": APP}
            else:
                result = next(app for app in server.apps if app["uuid"] == APP)
        elif route.path == "/api/v1/servers/server-uuid/resources":
            result = server.resources
        elif route.path == f"/api/v1/applications/{APP}/envs":
            result = server.envs
        elif route.path == f"/api/v1/applications/{APP}/storages":
            result = server.storages
        elif route.path == f"/api/v1/deployments/applications/{APP}":
            result = next_value(server.deployments)
        elif route.path == "/api/v1/deployments/new-deployment":
            result = next_value(server.details)
        else:
            status, result = 500, {"message": "unexpected request"}
        self.send_response(status)
        if status == 302:
            self.send_header("Location", server.base + "/redirected")
        self.end_headers()
        self.wfile.write(result if isinstance(result, bytes) else json.dumps(result).encode())

    do_GET = do_POST = do_PATCH = do_DELETE = handle_request


class PreviewTest(unittest.TestCase):
    def setUp(self):
        server = self.server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        server.base = f"http://127.0.0.1:{server.server_port}"
        server.requests, server.apps = [], []
        server.prs, server.runs, server.tasks = [pr()], [list_value([run()])], [list_value(tasks())]
        server.keys = [{"uuid": "read-key", "name": "particle-foundry-readonly"}]
        server.resources = [{"uuid": APP, "type": "application"}]
        server.envs, server.storages = [], {"persistent_storages": [], "file_storages": []}
        server.deployments = [{"count": 0, "deployments": []}, {"count": 1, "deployments": [deployment()]}]
        server.details = [deployment()]
        server.error_path, server.error_status, server.change_after_create = None, 500, False
        self.thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=0.01), daemon=True)
        self.thread.start()
        self.config = preview.Configuration(server.base, PRODUCTION, "test-coolify-token", server.base,
                                            "test/project", "test-forgejo-token", 16, "project-uuid", "server-uuid",
                                            "preview-env", "https://sand-pr-16.example.invalid",
                                            "particle-foundry-readonly", SHA)
        self.sleep = patch.object(preview.time, "sleep", return_value=None)
        self.sleep.start()

    def tearDown(self):
        self.sleep.stop()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def execute(self):
        output = io.StringIO()
        with patch.object(preview, "configuration", return_value=self.config), redirect_stdout(output), redirect_stderr(output):
            result = preview.main()
        self.assertFalse(any(method != "GET" and path == f"/api/v1/applications/{PRODUCTION}"
                             for method, path, *_ in self.server.requests))
        for secret in ("test-coolify-token", "test-forgejo-token", "private-response-secret"):
            self.assertNotIn(secret, output.getvalue())
        return result, output.getvalue()

    def mutations(self):
        return [request for request in self.server.requests if request[0] != "GET"]

    def existing_app(self):
        self.assertEqual(self.execute()[0], 0)
        self.server.requests = []
        self.server.deployments = [{"count": 0, "deployments": []}, {"count": 1, "deployments": [deployment()]}]
        return self.server.apps[0]

    def test_fresh_app_pins_sha_isolates_settings_and_monitors_new_deployment(self):
        self.server.deployments = [{"count": 1, "deployments": [deployment("previous")]},
                                   {"count": 1, "deployments": [deployment(status="queued")]}]
        self.server.details = [deployment(status="in_progress"), deployment()]
        result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertIn(f"deployed commit {SHA}", output)
        create, deploy = self.mutations()
        self.assertEqual(create[0:2], ("POST", "/api/v1/applications/private-deploy-key"))
        self.assertEqual(create[4]["git_commit_sha"], SHA)
        self.assertFalse(create[4]["instant_deploy"])
        self.assertEqual(create[4]["environment_uuid"], "preview-env")
        self.assertEqual(create[4]["private_key_uuid"], "read-key")
        self.assertEqual(create[4]["domains"], self.config.domain)
        self.assertTrue(all(create[4][name] is False for name in preview.SETTING_FLAGS))
        self.assertEqual(deploy[4], {"git_branch": "codex/test", "git_commit_sha": SHA, "instant_deploy": True})
        self.assertNotIn("ports_mappings", create[4])
        self.assertNotIn("custom_docker_run_options", create[4])
        self.assertTrue(all(request[3] == "Bearer test-coolify-token" for request in self.server.requests
                            if "/repos/" not in request[1]))
        queries = [request[2] for request in self.server.requests if request[1].endswith("/actions/runs")]
        self.assertTrue(all(query["head_sha"] == [SHA] and "event" not in query for query in queries))

    def test_rerun_reuses_exact_managed_app(self):
        self.existing_app()
        self.assertEqual(self.execute()[0], 0)
        self.assertEqual([request[0] for request in self.mutations()], ["PATCH"])

    def test_fork_other_base_and_stale_event_skip_all_mutations(self):
        for change in ("fork", "base", "stale"):
            with self.subTest(change=change):
                value = pr()
                if change == "fork":
                    value["head"]["repo"] = {"id": 8, "full_name": "attacker/project"}
                elif change == "base":
                    value["base"]["ref"] = "develop"
                else:
                    value["head"]["sha"] = OLD_SHA
                self.server.prs, self.server.requests = [value], []
                result, output = self.execute()
                self.assertEqual(result, 0)
                self.assertTrue("skipped" in output or "no managed application" in output)
                self.assertFalse(self.mutations())
                if change != "base":
                    self.assertFalse(any("/applications" in request[1] for request in self.server.requests))

    def test_every_required_gate_must_succeed_and_latest_attempt_wins(self):
        for name in preview.REQUIRED_JOBS:
            with self.subTest(name=name):
                rows = tasks()
                rows.append({**next(row for row in rows if row["name"] == name), "id": 1000, "status": "skipped"})
                self.server.tasks, self.server.requests = [list_value(sorted(rows, key=lambda task: task["id"], reverse=True))], []
                result, output = self.execute()
                self.assertEqual(result, 1)
                self.assertIn(f"gate {name} is skipped", output)
                self.assertFalse(self.mutations())

    def test_missing_gate_and_latest_failed_run_cannot_use_old_success(self):
        self.server.tasks = [list_value([task for task in tasks() if task["name"] != "verify"])]
        self.assertIn("gate verify is missing", self.execute()[1])
        self.server.runs = [list_value([run(), run(status="failure", id=101)])]
        self.assertIn("Latest preview CI run did not succeed", self.execute()[1])
        self.assertFalse(self.mutations())

    def test_ci_runs_from_other_events_cannot_satisfy_preview_gate(self):
        self.server.runs = [list_value([{**run(), "event": "push", "trigger_event": "push"}])]
        with patch.object(preview, "CI_TIMEOUT_SECONDS", 0):
            result, output = self.execute()
        self.assertEqual(result, 1)
        self.assertIn("CI did not finish", output)
        self.assertFalse(self.mutations())

    def test_pending_ci_waits_and_checks_live_head(self):
        self.server.runs = [list_value([run(status="running")]), list_value([run()])]
        self.assertEqual(self.execute()[0], 0)
        self.server.requests, self.server.apps = [], []
        self.server.runs = [list_value([run(status="running")])]
        self.server.prs = [pr(), pr(), pr(sha=OLD_SHA)]
        result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertIn("changed before verification", output)
        self.assertFalse(self.mutations())

    def test_fork_or_unapproved_ci_run_fails(self):
        for flag in ("is_fork_pull_request", "need_approval"):
            self.server.runs = [list_value([{**run(), flag: True}])]
            self.assertEqual(self.execute()[0], 1)
        self.assertFalse(self.mutations())

    def test_head_change_after_create_never_queues_branch_head(self):
        self.server.change_after_create = True
        result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertIn("changed before deployment", output)
        self.assertEqual([request[0] for request in self.mutations()], ["POST"])

    def test_owned_name_collision_ambiguity_and_production_uuid_are_rejected(self):
        app = self.existing_app()
        self.server.apps.append(copy.deepcopy(app))
        self.assertIn("Multiple applications", self.execute()[1])
        self.server.apps = [app]
        app["description"] = "a different owner"
        self.assertIn("ownership does not match", self.execute()[1])
        app["description"], app["uuid"] = self.config.marker, PRODUCTION
        self.assertIn("ownership does not match", self.execute()[1])
        self.assertFalse(self.mutations())

    def test_runtime_env_storage_settings_host_options_and_server_are_rejected(self):
        app = self.existing_app()
        original = copy.deepcopy(app)
        changes = ("env", "storage", "ports", "options", "settings", "server")
        for change in changes:
            with self.subTest(change=change):
                self.server.apps = [copy.deepcopy(original)]
                self.server.envs, self.server.storages = [], {"persistent_storages": [], "file_storages": []}
                self.server.resources = [{"uuid": APP, "type": "application"}]
                if change == "env":
                    self.server.envs = [{"key": "SENSITIVE_NAME", "value": "private-response-secret"}]
                elif change == "storage":
                    self.server.storages["persistent_storages"] = [{"id": 1}]
                elif change == "ports":
                    self.server.apps[0]["ports_mappings"] = "8080:80"
                elif change == "options":
                    self.server.apps[0]["custom_docker_run_options"] = "privileged"
                elif change == "settings":
                    self.server.apps[0]["settings"]["use_build_secrets"] = True
                else:
                    self.server.resources = []
                self.assertEqual(self.execute()[0], 1)
                self.assertFalse(self.mutations())

    def test_key_name_requires_one_match(self):
        for values in ([], [self.server.keys[0], self.server.keys[0]]):
            self.server.keys = values
            self.assertIn("one matching repository read key", self.execute()[1])
            self.assertFalse(self.mutations())

    def test_shell_metacharacters_and_option_names_in_branch_fail_before_coolify(self):
        for branch in ("-main", "codex/$(secret)", "codex/test;cmd", "codex/a\nsecret", "codex/../main", "codex/a.lock"):
            with self.subTest(branch=branch):
                self.server.prs, self.server.requests = [pr(branch=branch)], []
                result, output = self.execute()
                self.assertEqual(result, 1)
                self.assertIn("unsafe preview branch", output)
                self.assertNotIn(branch, output)
                self.assertFalse(self.mutations())

    def test_old_deployment_wrong_commit_and_builtin_preview_never_prove_success(self):
        for value in (deployment("previous"), deployment(sha=OLD_SHA), deployment(pull_request_id=16)):
            with self.subTest(value=value):
                self.server.apps, self.server.requests = [], []
                self.server.deployments = [{"count": 1, "deployments": [deployment("previous")]},
                                           {"count": 1, "deployments": [value]}]
                with patch.object(preview, "QUEUE_TIMEOUT_SECONDS", 0):
                    result, output = self.execute()
                self.assertEqual(result, 1)
                self.assertIn("did not queue a new preview", output)

    def test_failed_cancelled_unknown_and_changed_tracked_deployment_fail(self):
        for value in (deployment(status="failed"), deployment(status="cancelled-by-user"),
                      deployment(status="private-response-secret"), deployment(sha=OLD_SHA), deployment("wrong-uuid")):
            with self.subTest(value=value):
                self.server.apps = []
                self.server.deployments = [{"count": 0, "deployments": []},
                                           {"count": 1, "deployments": [deployment(status="queued")]}]
                self.server.details = [value]
                self.assertEqual(self.execute()[0], 1)

    def test_http_errors_and_redirects_have_safe_numeric_diagnostics(self):
        for status in (403, 500, 302):
            self.server.error_path, self.server.error_status = "/api/v1/security/keys", status
            result, output = self.execute()
            self.assertEqual(result, 1)
            self.assertIn(f"Coolify API request failed (HTTP {status})", output)
            self.assertFalse(any(request[1] == "/redirected" for request in self.server.requests))
            self.assertFalse(self.mutations())

    def test_closed_pr_deletes_only_managed_configuration_with_explicit_safe_flags(self):
        self.existing_app()
        self.server.prs = [pr(state="closed")]
        self.server.deployments = [{"count": 1, "deployments": [deployment()]}]
        result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertIn("deletion queued", output)
        deletion, = self.mutations()
        self.assertEqual(deletion[0:2], ("DELETE", f"/api/v1/applications/{APP}"))
        self.assertEqual(deletion[2], {"delete_configurations": ["true"], "delete_volumes": ["false"],
                                      "delete_connected_networks": ["false"], "docker_cleanup": ["false"]})
        self.assertFalse(any("/actions/" in request[1] for request in self.server.requests))

    def test_cleanup_waits_for_all_active_deployments(self):
        self.existing_app()
        self.server.prs = [pr(state="closed")]
        self.server.deployments = [{"count": 1, "deployments": [deployment(status="queued")]},
                                   {"count": 1, "deployments": [deployment()]}]
        self.assertEqual(self.execute()[0], 0)
        self.assertEqual(len(self.mutations()), 1)

    def test_cleanup_reopen_race_and_timeout_never_delete(self):
        self.existing_app()
        self.server.prs = [pr(state="closed"), pr(state="closed"), pr()]
        self.server.deployments = [{"count": 1, "deployments": [deployment()]}]
        result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertIn("reopened or returned to main before deletion", output)
        self.assertFalse(self.mutations())
        self.server.prs = [pr(state="closed")]
        self.server.deployments = [{"count": 1, "deployments": [deployment(status="in_progress")]}]
        with patch.object(preview, "DEPLOY_TIMEOUT_SECONDS", 0):
            self.assertEqual(self.execute()[0], 1)
        self.assertFalse(self.mutations())

    def test_retargeted_same_repository_preview_is_removed_and_return_to_main_aborts_cleanup(self):
        self.existing_app()
        retargeted = pr()
        retargeted["base"]["ref"] = "develop"
        self.server.prs = [retargeted, retargeted, pr()]
        self.server.deployments = [{"count": 1, "deployments": [deployment()]}]
        result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertIn("returned to main before deletion", output)
        self.assertFalse(self.mutations())
        self.server.prs = [retargeted]
        result, output = self.execute()
        self.assertEqual(result, 0)
        self.assertIn("deletion queued", output)
        self.assertEqual([request[0] for request in self.mutations()], ["DELETE"])

    def test_paginated_tasks_select_latest_attempt_across_pages(self):
        rows = tasks()
        self.server.tasks = [{"total_count": 6, "workflow_runs": rows[:3]},
                             {"total_count": 6, "workflow_runs": rows[3:]}]
        # Each gate repeats the two-page response.
        with patch.object(preview, "ci_gate", wraps=preview.ci_gate) as gate:
            run_id, _ = gate(self.config, SHA)
        self.assertEqual(run_id, 100)
        pages = [request[2]["page"] for request in self.server.requests if request[1].endswith("/actions/tasks")]
        self.assertEqual(pages, [["1"], ["2"]])

    def test_recent_tasks_do_not_scan_large_historical_repository(self):
        self.server.tasks = [{"total_count": 100000, "workflow_runs": tasks()}]
        self.assertEqual(preview.ci_gate(self.config, SHA)[0], 100)
        requests = [request for request in self.server.requests if request[1].endswith("/actions/tasks")]
        self.assertEqual(len(requests), 1)

    def test_unexpected_task_order_fails_before_any_mutation(self):
        self.server.tasks = [list_value(list(reversed(tasks())))]
        result, output = self.execute()
        self.assertEqual(result, 1)
        self.assertIn("unexpected CI task ordering", output)
        self.assertFalse(self.mutations())

    def test_configuration_requires_trusted_main_control_and_safe_domain(self):
        values = {"FORGEJO_EVENT_NAME": "pull_request_target", "FORGEJO_REF": "refs/heads/main",
                  "COOLIFY_URL": "http://xcore.example.ts.net:8000", "COOLIFY_APP_UUID": PRODUCTION,
                  "COOLIFY_TOKEN": "test-coolify-token", "FORGEJO_SERVER_URL": "https://git.example.invalid",
                  "FORGEJO_REPOSITORY": "test/project", "CI_TOKEN": "test-forgejo-token", "PR_NUMBER": "16",
                  "PREVIEW_CONTROL_SHA": OLD_SHA, "CI_SHA": SHA, "COOLIFY_PREVIEW_PROJECT_UUID": "project-uuid",
                  "COOLIFY_PREVIEW_SERVER_UUID": "server-uuid", "COOLIFY_PREVIEW_ENV_UUID": "preview-env",
                  "COOLIFY_PREVIEW_DOMAIN_TEMPLATE": "https://sand-pr-{pr}.example.invalid",
                  "COOLIFY_PREVIEW_KEY_NAME": "particle-foundry-readonly"}
        with patch.dict(os.environ, values, clear=True):
            self.assertEqual(preview.configuration().domain, self.config.domain)
            self.assertNotIn("test-coolify-token", repr(preview.configuration()))
            for overrides in ({"FORGEJO_EVENT_NAME": "pull_request"}, {"PR_NUMBER": "16;cmd"},
                              {"CI_SHA": SHA[:7]}, {"CI_SHA": ""},
                              {"COOLIFY_PREVIEW_DOMAIN_TEMPLATE": "http://sand-pr-{pr}.example.invalid"},
                              {"COOLIFY_PREVIEW_DOMAIN_TEMPLATE": "https://token@sand-pr-{pr}.example.invalid"},
                              {"COOLIFY_PREVIEW_DOMAIN_TEMPLATE": "https://sand-pr-{pr}.{evil}.invalid"},
                              {"FORGEJO_EVENT_NAME": "workflow_dispatch", "FORGEJO_REF": "refs/heads/feature"}):
                with self.subTest(overrides=overrides), patch.dict(os.environ, overrides):
                    with self.assertRaises(preview.DeploymentError):
                        preview.configuration()
            with patch.dict(os.environ, {"FORGEJO_EVENT_NAME": "workflow_dispatch", "CI_SHA": ""}):
                self.assertEqual(preview.configuration().expected_sha, "")


if __name__ == "__main__":
    unittest.main()
