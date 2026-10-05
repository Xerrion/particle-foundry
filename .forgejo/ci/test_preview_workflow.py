"""Check preview workflow trust boundaries and full validation scope."""

import os
import ast
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest


WORKFLOWS = Path(__file__).resolve().parents[1] / "workflows"


def step_command(workflow, name):
    lines = (WORKFLOWS / workflow).read_text().splitlines()
    step = lines.index(f"      - name: {name}")
    start = next(index for index in range(step, len(lines)) if lines[index].startswith("        run: "))
    if lines[start] != "        run: |":
        return lines[start].removeprefix("        run: ")
    command = []
    for line in lines[start + 1:]:
        if line and not line.startswith("          "):
            break
        command.append(line[10:])
    return "\n".join(command)


def git(root, *args):
    return subprocess.run(["git", *args], cwd=root, capture_output=True, text=True, check=True).stdout.strip()


def commit(root, message):
    git(root, "add", ".")
    git(root, "-c", "user.name=CI-test", "-c", "user.email=ci-test@example.invalid", "commit", "-qm", message)
    return git(root, "rev-parse", "HEAD")


class PreviewScopeTest(unittest.TestCase):
    def test_docs_only_pr_receives_full_checks_only_when_preview_is_eligible(self):
        command = step_command("ci.yml", "Detect code changes")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            git(root, "init", "-q", "-b", "main")
            (root / "app.js").write_text("application revision\n")
            before = commit(root, "Application")
            (root / "docs").mkdir()
            (root / "docs" / "readme.md").write_text("documentation revision\n")
            current = commit(root, "Documentation")
            output = root / "scope-output"
            cases = [
                ("pull_request", "main", "test/project", "true", "true"),
                ("pull_request", "main", "test/project", "false", "false"),
                ("pull_request", "main", "test/project", "", "false"),
                ("pull_request", "main", "fork/project", "true", "false"),
                ("pull_request", "release", "test/project", "true", "false"),
                ("pull_request_target", "main", "test/project", "true", "false"),
                ("push", "main", "test/project", "true", "false"),
            ]
            for event, base, head_repository, enabled, expected in cases:
                with self.subTest(event=event, base=base, head_repository=head_repository, enabled=enabled):
                    output.write_text("")
                    env = {**os.environ, "CI_BASE_SHA": before, "CI_SHA": current,
                           "FORGEJO_EVENT_NAME": event, "FORGEJO_REF": "refs/pull/7/head",
                           "FORGEJO_REPOSITORY": "test/project", "PR_BASE_REF": base,
                           "PR_HEAD_REPOSITORY": head_repository, "COOLIFY_PREVIEW_ENABLED": enabled,
                           "COOLIFY_DEPLOY_ENABLED": "false", "FORGEJO_OUTPUT": str(output),
                           "RUNNER_TEMP": str(root)}
                    subprocess.run(["sh", "-e", "-c", command], cwd=root, env=env,
                                   capture_output=True, text=True, check=True)
                    self.assertEqual(output.read_text().strip(), f"code={expected}")


class TrustedCheckoutTest(unittest.TestCase):
    def test_controller_uses_base_commit_even_when_pr_head_changes_its_code(self):
        command = step_command("preview.yml", "Checkout trusted preview controller")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            origin = root / "test" / "project.git"
            origin.mkdir(parents=True)
            git(origin, "init", "-q", "-b", "main")
            helper = origin / ".forgejo" / "ci" / "preview.py"
            helper.parent.mkdir(parents=True)
            helper.write_text("print('trusted controller')\n")
            trusted_sha = commit(origin, "Trusted main controller")
            git(origin, "checkout", "-qb", "pr-head")
            helper.write_text("raise RuntimeError('PR controller must not execute')\n")
            head_sha = commit(origin, "Untrusted PR controller")
            checkout = root / "checkout"
            checkout.mkdir()
            env = {**os.environ, "FORGEJO_SERVER_URL": root.as_uri(),
                   "FORGEJO_REPOSITORY": "test/project", "CI_TOKEN": "test-forgejo-token",
                   "PREVIEW_CONTROL_SHA": head_sha, "CI_SHA": head_sha,
                   "FORGEJO_ENV": str(root / "workflow-env"),
                   "GIT_TERMINAL_PROMPT": "0"}
            result = subprocess.run(["sh", "-e", "-c", command], cwd=checkout, env=env,
                                    capture_output=True, text=True, check=True)
            self.assertEqual(git(checkout, "rev-parse", "HEAD"), trusted_sha)
            self.assertNotEqual(trusted_sha, head_sha)
            controller = subprocess.run([sys.executable, ".forgejo/ci/preview.py"], cwd=checkout,
                                        capture_output=True, text=True, check=True)
            self.assertEqual(controller.stdout.strip(), "trusted controller")
            self.assertNotIn("test-forgejo-token", result.stdout + result.stderr)
            self.assertNotIn("Authorization", (checkout / ".git" / "config").read_text())
            self.assertNotIn("test-forgejo-token", (checkout / ".git" / "config").read_text())
            self.assertEqual((root / "workflow-env").read_text().strip(),
                             f"PREVIEW_CONTROL_SHA={trusted_sha}")

    def test_missing_main_ref_stops_without_using_the_event_commit(self):
        command = step_command("preview.yml", "Checkout trusted preview controller")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            origin = root / "test" / "project.git"
            origin.mkdir(parents=True)
            git(origin, "init", "-q", "-b", "release")
            (origin / "controller.py").write_text("raise RuntimeError('Do not execute release controller')\n")
            event_sha = commit(origin, "Release without main")
            checkout = root / "checkout"
            checkout.mkdir()
            env = {**os.environ, "FORGEJO_SERVER_URL": root.as_uri(),
                   "FORGEJO_REPOSITORY": "test/project", "CI_TOKEN": "test-forgejo-token",
                   "PREVIEW_CONTROL_SHA": event_sha, "CI_SHA": event_sha,
                   "FORGEJO_ENV": str(root / "workflow-env"), "GIT_TERMINAL_PROMPT": "0"}
            result = subprocess.run(["sh", "-e", "-c", command], cwd=checkout, env=env,
                                    capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((checkout / "controller.py").exists())
            self.assertFalse((root / "workflow-env").exists())

    def test_coolify_secret_is_bound_only_to_the_trusted_reconciliation_step(self):
        workflow = (WORKFLOWS / "preview.yml").read_text()
        checkout = step_command("preview.yml", "Checkout trusted preview controller")
        before_reconcile, reconcile = workflow.split("      - name: Reconcile the verified PR preview", 1)
        self.assertNotIn("COOLIFY_TOKEN", before_reconcile)
        self.assertIn("COOLIFY_TOKEN: ${{ secrets.COOLIFY_TOKEN }}", reconcile)
        self.assertNotIn("CI_SHA", checkout)
        self.assertNotIn("mise", checkout)
        self.assertEqual(step_command("preview.yml", "Reconcile the verified PR preview"),
                         "python .forgejo/ci/preview.py")


class PreviewEventTest(unittest.TestCase):
    def test_target_events_require_main_and_manual_reconciliation_requires_main_ref(self):
        lines = (WORKFLOWS / "preview.yml").read_text().splitlines()
        start = lines.index("  pull_request_target:") + 1
        end = lines.index("  workflow_dispatch:")
        trigger = lines[start:end]
        types = next(line.strip().removeprefix("types: [").removesuffix("]").split(", ")
                     for line in trigger if line.strip().startswith("types: ["))
        branches = next((line.strip().removeprefix("branches: [").removesuffix("]").split(", ")
                         for line in trigger if line.strip().startswith("branches: [")), None)
        condition = next(line.removeprefix("    if: ") for line in lines if line.startswith("    if: "))
        expression = ast.parse(condition.replace("&&", "and").replace("||", "or"), mode="eval")
        allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.Compare, ast.Eq,
                   ast.Name, ast.Attribute, ast.Load, ast.Constant)
        self.assertTrue(all(isinstance(node, allowed) for node in ast.walk(expression)))
        cases = [
            ("pull_request_target", "opened", "main", "test/project", "true", True),
            ("pull_request_target", "edited", "main", "test/project", "true", True),
            ("pull_request_target", "edited", "release", "test/project", "true", False),
            ("pull_request_target", "closed", "release", "test/project", "true", False),
            ("pull_request_target", "closed", "main", "test/project", "true", True),
            ("pull_request_target", "closed", "release", "fork/project", "true", False),
            ("pull_request_target", "edited", "main", "fork/project", "true", False),
            ("pull_request_target", "closed", "main", "test/project", "false", False),
            ("workflow_dispatch", "", "main", "test/project", "true", True),
            ("workflow_dispatch", "", "release", "test/project", "true", False),
        ]
        for event, action, base, head_repository, enabled, expected in cases:
            with self.subTest(event=event, action=action, base=base, repository=head_repository, enabled=enabled):
                triggered = event == "workflow_dispatch" or (action in types and (branches is None or base in branches))
                forgejo = SimpleNamespace(event_name=event, ref=f"refs/heads/{base}", repository="test/project",
                                          event=SimpleNamespace(pull_request=SimpleNamespace(
                                              base=SimpleNamespace(ref=base),
                                              head=SimpleNamespace(repo=SimpleNamespace(full_name=head_repository)))))
                permitted = eval(compile(expression, "preview job condition", "eval"), {"__builtins__": {}},
                                 {"vars": SimpleNamespace(COOLIFY_PREVIEW_ENABLED=enabled), "forgejo": forgejo})
                self.assertEqual(triggered and permitted, expected)


if __name__ == "__main__":
    unittest.main()
