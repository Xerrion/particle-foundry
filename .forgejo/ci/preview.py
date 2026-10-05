"""Reconcile isolated Coolify applications from trusted Forgejo control code."""

from dataclasses import dataclass, field
import os
import re
import sys
import time
from urllib.parse import quote, urlsplit

from deploy import DeploymentError, ID_PATTERN, SHA_PATTERN, base_url
from runtime import add_query, json_request

POLL_SECONDS = 5
CI_TIMEOUT_SECONDS = 30 * 60
DEPLOY_TIMEOUT_SECONDS = 20 * 60
QUEUE_TIMEOUT_SECONDS = 60
PAGE_SIZE = 100
MAX_PAGES = 50
REQUIRED_JOBS = ("changes", "docs", "web", "rust", "browser", "verify")
PENDING = {"waiting", "running", "blocked", "unknown"}
TERMINAL_DEPLOYMENTS = {"finished", "failed", "cancelled-by-user"}
SETTING_FLAGS = ("is_auto_deploy_enabled", "is_preview_deployments_enabled", "connect_to_docker_network",
                 "use_build_secrets", "inject_build_args_to_dockerfile", "is_git_submodules_enabled",
                 "is_git_lfs_enabled")


@dataclass(frozen=True)
class Configuration:
    coolify_url: str
    production_uuid: str
    coolify_token: str = field(repr=False)
    forgejo_url: str
    repository: str
    ci_token: str = field(repr=False)
    pr_number: int
    project_uuid: str
    server_uuid: str
    environment_uuid: str
    domain: str
    key_name: str
    expected_sha: str = ""

    @property
    def name(self):
        return f"{self.repository.split('/')[1]}-pr-{self.pr_number}"

    @property
    def marker(self):
        return f"forgejo-pr-preview:v1:{self.repository}:{self.pr_number}"


def configuration():
    event = os.environ.get("FORGEJO_EVENT_NAME")
    if event not in {"pull_request_target", "workflow_dispatch"}:
        raise DeploymentError("Preview requires trusted target or manual workflow control")
    if event == "workflow_dispatch" and os.environ.get("FORGEJO_REF") != "refs/heads/main":
        raise DeploymentError("Manual preview control requires Forgejo main")
    names = ("COOLIFY_URL", "COOLIFY_APP_UUID", "COOLIFY_TOKEN", "FORGEJO_SERVER_URL", "FORGEJO_REPOSITORY",
             "CI_TOKEN", "PR_NUMBER", "PREVIEW_CONTROL_SHA", "COOLIFY_PREVIEW_PROJECT_UUID",
             "COOLIFY_PREVIEW_SERVER_UUID", "COOLIFY_PREVIEW_ENV_UUID", "COOLIFY_PREVIEW_DOMAIN_TEMPLATE",
             "COOLIFY_PREVIEW_KEY_NAME")
    if any(not os.environ.get(name) for name in names):
        raise DeploymentError("Preview configuration is incomplete")
    repository = os.environ["FORGEJO_REPOSITORY"]
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*/[A-Za-z0-9][A-Za-z0-9_.-]*", repository):
        raise DeploymentError("Preview repository must have a valid owner and repository name")
    pr = os.environ["PR_NUMBER"]
    if not re.fullmatch(r"[1-9][0-9]{0,8}", pr):
        raise DeploymentError("Preview requires a valid pull request number")
    sha = os.environ.get("CI_SHA", "")
    if not SHA_PATTERN.fullmatch(os.environ["PREVIEW_CONTROL_SHA"]) or (sha and not SHA_PATTERN.fullmatch(sha)):
        raise DeploymentError("Preview requires full commit SHAs")
    if event == "pull_request_target" and not sha:
        raise DeploymentError("Target preview control requires the event head SHA")
    ids = [os.environ[name] for name in ("COOLIFY_APP_UUID", "COOLIFY_PREVIEW_PROJECT_UUID",
                                       "COOLIFY_PREVIEW_SERVER_UUID", "COOLIFY_PREVIEW_ENV_UUID")]
    if any(not ID_PATTERN.fullmatch(value) for value in ids):
        raise DeploymentError("Preview requires valid Coolify UUIDs")
    template = os.environ["COOLIFY_PREVIEW_DOMAIN_TEMPLATE"]
    if template.count("{pr}") != 1 or any(char in template.replace("{pr}", "") for char in "{}"):
        raise DeploymentError("Preview domain template requires one PR number placeholder")
    domain = template.replace("{pr}", pr)
    parsed = urlsplit(domain)
    if not (parsed.scheme == "https" and parsed.hostname and parsed.netloc == parsed.hostname
            and re.fullmatch(r"[A-Za-z0-9.-]+", parsed.hostname) and parsed.path in {"", "/"}
            and not parsed.query and not parsed.fragment):
        raise DeploymentError("Preview domain must be one HTTPS hostname")
    return Configuration(base_url(os.environ["COOLIFY_URL"], "Coolify"), ids[0], os.environ["COOLIFY_TOKEN"],
                         base_url(os.environ["FORGEJO_SERVER_URL"], "Forgejo"), repository,
                         os.environ["CI_TOKEN"], int(pr), ids[1], ids[2], ids[3], domain.rstrip("/"),
                         os.environ["COOLIFY_PREVIEW_KEY_NAME"], sha)


def api(config, service, path, method="GET", data=None, **query):
    base, token = ((config.coolify_url, config.coolify_token) if service == "Coolify"
                   else (config.forgejo_url, config.ci_token))
    try:
        return json_request(add_query(f"{base}/api/v1/{path}", **query), method, data,
                            {"Authorization": f"Bearer {token}"})
    except Exception as error:
        status = re.fullmatch(r"Runner API returned HTTP ([1-5][0-9]{2})", str(error))
        detail = f" (HTTP {status[1]})" if status else ""
        raise DeploymentError(f"{service} API request failed{detail}") from None


def repo_path(config):
    return "repos/" + "/".join(quote(part, safe="") for part in config.repository.split("/"))


def branch_name(value):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._/-]{0,199}", value):
        raise DeploymentError("Forgejo returned an unsafe preview branch name")
    if (".." in value or "//" in value or value.endswith(("/", "."))
            or any(part.startswith(".") or part.endswith(".lock") for part in value.split("/"))):
        raise DeploymentError("Forgejo returned an unsafe preview branch name")
    return value


def pull_request(config):
    value = api(config, "Forgejo", f"{repo_path(config)}/pulls/{config.pr_number}")
    if not isinstance(value, dict) or value.get("number") != config.pr_number or value.get("state") not in {"open", "closed"}:
        raise DeploymentError("Forgejo returned an invalid pull request")
    head, base = value.get("head"), value.get("base")
    if not isinstance(head, dict) or not isinstance(base, dict):
        raise DeploymentError("Forgejo returned invalid pull request branches")
    head_repo, base_repo = head.get("repo"), base.get("repo")
    if not isinstance(head_repo, dict) or not isinstance(base_repo, dict):
        return None
    if (head_repo.get("full_name") != config.repository or base_repo.get("full_name") != config.repository
            or not isinstance(base_repo.get("id"), int) or head_repo.get("id") != base_repo["id"]):
        return None
    branch_name(base.get("ref"))
    if not isinstance(head.get("sha"), str) or not SHA_PATTERN.fullmatch(head["sha"]):
        raise DeploymentError("Forgejo returned an invalid preview commit")
    branch_name(head.get("ref"))
    return value


def current(config, sha=None, state="open"):
    value = pull_request(config)
    return value if (value and value["state"] == state and value["base"]["ref"] == "main"
                     and (sha is None or value["head"]["sha"] == sha)) else None


def cleanup_is_current(config):
    value = pull_request(config)
    return value is not None and (value["state"] == "closed" or value["base"]["ref"] != "main")


def pages(config, service, path, collection, **query):
    rows = []
    for page in range(1, MAX_PAGES + 1):
        result = api(config, service, path, page=page, limit=PAGE_SIZE, **query)
        values = result.get(collection) if isinstance(result, dict) else None
        count = result.get("total_count") if isinstance(result, dict) else None
        if not isinstance(values, list) or not isinstance(count, int) or count < 0:
            raise DeploymentError(f"{service} returned an invalid paginated list")
        rows.extend(values)
        if len(rows) >= count:
            return rows
        if not values:
            raise DeploymentError(f"{service} returned an incomplete paginated list")
    raise DeploymentError(f"{service} list exceeds the preview pagination limit")


def ci_gate(config, sha):
    values = pages(config, "Forgejo", f"{repo_path(config)}/actions/runs", "workflow_runs",
                   head_sha=sha, workflow_id="ci.yml")
    runs = []
    for run in values:
        if (not isinstance(run, dict) or not isinstance(run.get("id"), int)
                or run.get("commit_sha") != sha or run.get("workflow_id") != "ci.yml"
                or not isinstance(run.get("index_in_repo"), int)):
            raise DeploymentError("Forgejo returned an unexpected CI run")
        # Synchronize runs use a different stored event. Filter normalized API fields.
        if run.get("event") == "pull_request" and run.get("trigger_event") == "pull_request":
            runs.append(run)
    if not runs:
        return None, "waiting for CI run"
    run = max(runs, key=lambda candidate: candidate["id"])
    if run.get("is_fork_pull_request") is not False or run.get("need_approval") is not False:
        raise DeploymentError("Preview CI run is not an approved same-repository run")
    if run.get("status") in PENDING:
        return None, "waiting for CI completion"
    if run.get("status") != "success":
        raise DeploymentError("Latest preview CI run did not succeed")
    latest = latest_jobs(config, sha, run["index_in_repo"])
    for name in REQUIRED_JOBS:
        status = latest.get(name, {}).get("status")
        if status != "success":
            category = status if status in {"failure", "cancelled", "skipped", *PENDING} else "missing"
            raise DeploymentError(f"Preview CI gate {name} is {category}")
    return run["id"], "CI passed"


def latest_jobs(config, sha, run_number):
    # Forgejo orders tasks by descending ID. Older repository history is unnecessary.
    latest = {}
    previous, seen = None, 0
    for page in range(1, MAX_PAGES + 1):
        result = api(config, "Forgejo", f"{repo_path(config)}/actions/tasks", page=page, limit=PAGE_SIZE)
        tasks = result.get("workflow_runs") if isinstance(result, dict) else None
        count = result.get("total_count") if isinstance(result, dict) else None
        if not isinstance(tasks, list) or not isinstance(count, int) or count < 0:
            raise DeploymentError("Forgejo returned an invalid CI task list")
        for task in tasks:
            if (not isinstance(task, dict) or not isinstance(task.get("id"), int)
                    or (previous is not None and task["id"] >= previous)):
                raise DeploymentError("Forgejo returned unexpected CI task ordering")
            previous = task["id"]
            if (task.get("run_number") == run_number and task.get("head_sha") == sha
                    and task.get("workflow_id") == "ci.yml" and task.get("event") == "pull_request"
                    and task.get("name") in REQUIRED_JOBS):
                latest.setdefault(task["name"], task)
                if len(latest) == len(REQUIRED_JOBS):
                    return latest
        seen += len(tasks)
        if seen >= count:
            return latest
        if not tasks:
            raise DeploymentError("Forgejo returned an incomplete CI task list")
    raise DeploymentError("Current CI tasks exceed the preview pagination limit")


def wait_for_ci(config, sha):
    started, message = time.monotonic(), None
    while True:
        if not current(config, sha):
            return None
        run_id, reason = ci_gate(config, sha)
        if run_id is not None:
            return run_id
        if reason != message:
            print(f"Preview PR {config.pr_number}: {reason}")
            message = reason
        if time.monotonic() - started >= CI_TIMEOUT_SECONDS:
            raise DeploymentError("Preview CI did not finish within 30 minutes")
        time.sleep(POLL_SECONDS)


def app_list(config):
    values = api(config, "Coolify", "applications")
    if not isinstance(values, list) or any(not isinstance(value, dict) for value in values):
        raise DeploymentError("Coolify returned an invalid application list")
    return values


def metadata(config):
    production = api(config, "Coolify", f"applications/{config.production_uuid}")
    repository = production.get("git_repository") if isinstance(production, dict) else None
    if (not isinstance(repository, str) or not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9@._:/-]*", repository)
            or not repository.endswith(f"/{config.repository}.git")):
        raise DeploymentError("Production repository metadata does not match the preview repository")
    environments = api(config, "Coolify", f"projects/{config.project_uuid}/environments")
    if not isinstance(environments, list):
        raise DeploymentError("Coolify returned an invalid environment list")
    matches = [value for value in environments if isinstance(value, dict) and value.get("uuid") == config.environment_uuid]
    if (len(matches) != 1 or not isinstance(matches[0].get("id"), int)
            or matches[0]["id"] == production.get("environment_id")):
        raise DeploymentError("Preview requires one dedicated environment outside production")
    return repository, matches[0]["id"]


def owned_app(config, repository, environment_id):
    candidates = [app for app in app_list(config) if app.get("environment_id") == environment_id
                  and (app.get("name") == config.name or app.get("description") == config.marker)]
    if len(candidates) > 1:
        raise DeploymentError("Multiple applications match the preview ownership marker")
    if not candidates:
        return None
    app = candidates[0]
    if (app.get("name") != config.name or app.get("description") != config.marker
            or app.get("git_repository") != repository or app.get("uuid") == config.production_uuid
            or not isinstance(app.get("uuid"), str) or not ID_PATTERN.fullmatch(app["uuid"])):
        raise DeploymentError("Preview application ownership does not match")
    return validate_app(config, app["uuid"], repository, environment_id)


def validate_app(config, uuid, repository, environment_id):
    if uuid == config.production_uuid or not ID_PATTERN.fullmatch(uuid):
        raise DeploymentError("Preview cannot mutate the production application")
    app = api(config, "Coolify", f"applications/{uuid}")
    expected = {"uuid": uuid, "name": config.name, "description": config.marker, "environment_id": environment_id,
                "git_repository": repository, "build_pack": "dockerfile", "fqdn": config.domain,
                "ports_exposes": "80", "base_directory": "/", "dockerfile_location": "/Dockerfile",
                "health_check_enabled": True, "health_check_type": "http", "health_check_path": "/",
                "health_check_port": 80, "health_check_method": "GET", "health_check_return_code": 200,
                "health_check_scheme": "http"}
    if not isinstance(app, dict) or any(app.get(name) != value for name, value in expected.items()):
        raise DeploymentError("Preview application configuration does not match")
    if app.get("ports_mappings") not in {None, ""} or app.get("custom_docker_run_options") not in {None, ""}:
        raise DeploymentError("Preview application has unsafe host options")
    settings = app.get("settings")
    if not isinstance(settings, dict) or any(settings.get(name) is not False for name in SETTING_FLAGS):
        raise DeploymentError("Preview application settings do not match")
    resources = api(config, "Coolify", f"servers/{config.server_uuid}/resources")
    matches = [value for value in resources if isinstance(value, dict) and value.get("uuid") == uuid] if isinstance(resources, list) else []
    if len(matches) != 1 or matches[0].get("type") != "application":
        raise DeploymentError("Preview application is not on the configured server")
    envs = api(config, "Coolify", f"applications/{uuid}/envs")
    storages = api(config, "Coolify", f"applications/{uuid}/storages")
    if envs != [] or not isinstance(storages, dict) or storages.get("persistent_storages") != [] or storages.get("file_storages") != []:
        raise DeploymentError("Preview application contains environment variables or storage")
    return uuid


def create_app(config, repository, environment_id, head):
    keys = api(config, "Coolify", "security/keys")
    matches = [value for value in keys if isinstance(value, dict) and value.get("name") == config.key_name] if isinstance(keys, list) else []
    if len(matches) != 1 or not isinstance(matches[0].get("uuid"), str) or not ID_PATTERN.fullmatch(matches[0]["uuid"]):
        raise DeploymentError("Preview requires one matching repository read key")
    data = {"project_uuid": config.project_uuid, "server_uuid": config.server_uuid,
            "environment_uuid": config.environment_uuid, "private_key_uuid": matches[0]["uuid"],
            "git_repository": repository, "git_branch": head["ref"], "git_commit_sha": head["sha"],
            "build_pack": "dockerfile", "name": config.name, "description": config.marker,
            "domains": config.domain, "ports_exposes": "80", "base_directory": "/",
            "dockerfile_location": "/Dockerfile", "instant_deploy": False, "autogenerate_domain": False,
            "health_check_enabled": True, "health_check_type": "http", "health_check_path": "/",
            "health_check_port": 80, "health_check_method": "GET", "health_check_return_code": 200,
            "health_check_scheme": "http", **{name: False for name in SETTING_FLAGS}}
    result = api(config, "Coolify", "applications/private-deploy-key", "POST", data)
    uuid = result.get("uuid") if isinstance(result, dict) else None
    if not isinstance(uuid, str) or not ID_PATTERN.fullmatch(uuid):
        raise DeploymentError("Coolify returned an invalid preview application UUID")
    return validate_app(config, uuid, repository, environment_id)


def deployments(config, uuid):
    rows = []
    for page in range(MAX_PAGES):
        result = api(config, "Coolify", f"deployments/applications/{uuid}", skip=page * PAGE_SIZE, take=PAGE_SIZE)
        values = result.get("deployments") if isinstance(result, dict) else None
        count = result.get("count") if isinstance(result, dict) else None
        if not isinstance(values, list) or not isinstance(count, int) or count < 0:
            raise DeploymentError("Coolify returned an invalid preview deployment list")
        for value in values:
            if (not isinstance(value, dict) or not isinstance(value.get("deployment_uuid"), str)
                    or not ID_PATTERN.fullmatch(value["deployment_uuid"])):
                raise DeploymentError("Coolify returned an invalid preview deployment UUID")
        rows.extend(values)
        if len(rows) >= count:
            return rows
        if not values:
            raise DeploymentError("Coolify returned an incomplete preview deployment list")
    raise DeploymentError("Preview deployment history exceeds the pagination limit")


def monitor(config, uuid, sha, previous):
    started, tracked = time.monotonic(), None
    while True:
        record = None
        if tracked is None:
            record = next((value for value in deployments(config, uuid)
                           if value["deployment_uuid"] not in previous and value.get("commit") == sha
                           and value.get("pull_request_id") == 0), None)
            if record:
                tracked = record["deployment_uuid"]
        else:
            record = api(config, "Coolify", f"deployments/{tracked}")
        if record is not None:
            if (not isinstance(record, dict) or record.get("deployment_uuid") != tracked
                    or record.get("commit") != sha or record.get("pull_request_id") != 0):
                raise DeploymentError("Tracked preview deployment no longer matches")
            status = record.get("status")
            if status == "finished":
                print(f"Preview PR {config.pr_number} deployed commit {sha} at {config.domain}")
                return
            if status in {"failed", "cancelled-by-user"}:
                raise DeploymentError(f"Preview deployment ended with status {status}")
            if status not in {"queued", "in_progress"}:
                raise DeploymentError("Coolify returned an unknown preview deployment status")
        elapsed = time.monotonic() - started
        if tracked is None and elapsed >= QUEUE_TIMEOUT_SECONDS:
            raise DeploymentError("Coolify did not queue a new preview deployment")
        if elapsed >= DEPLOY_TIMEOUT_SECONDS:
            raise DeploymentError("Preview deployment did not finish within 20 minutes")
        time.sleep(POLL_SECONDS)


def cleanup(config, repository, environment_id, uuid):
    started = time.monotonic()
    while True:
        if not cleanup_is_current(config):
            print("Preview cleanup skipped: pull request is open and targets main")
            return
        rows = deployments(config, uuid)
        if any(value.get("status") not in TERMINAL_DEPLOYMENTS | {"queued", "in_progress"} for value in rows):
            raise DeploymentError("Coolify returned an unknown cleanup deployment status")
        if all(value.get("status") in TERMINAL_DEPLOYMENTS for value in rows):
            break
        if time.monotonic() - started >= DEPLOY_TIMEOUT_SECONDS:
            raise DeploymentError("Preview cleanup requires completed deployments")
        time.sleep(POLL_SECONDS)
    validate_app(config, uuid, repository, environment_id)
    if not cleanup_is_current(config):
        print("Preview cleanup skipped: pull request reopened or returned to main before deletion")
        return
    api(config, "Coolify", f"applications/{uuid}", "DELETE", delete_configurations="true", delete_volumes="false",
        delete_connected_networks="false", docker_cleanup="false")
    print(f"Preview PR {config.pr_number} deletion queued without volume, network or global Docker cleanup")


def reconcile(config):
    pr = pull_request(config)
    if pr is None:
        print("Preview skipped: only same-repository pull requests are eligible")
        return
    if pr["state"] == "closed" or pr["base"]["ref"] != "main":
        repository, environment_id = metadata(config)
        uuid = owned_app(config, repository, environment_id)
        if uuid:
            cleanup(config, repository, environment_id, uuid)
        else:
            print(f"Preview PR {config.pr_number} has no managed application to delete")
        return
    sha = pr["head"]["sha"]
    if config.expected_sha and sha != config.expected_sha:
        print("Preview skipped: event head no longer matches the current pull request")
        return
    run_id = wait_for_ci(config, sha)
    if run_id is None:
        print("Preview skipped: pull request changed before verification")
        return
    repository, environment_id = metadata(config)
    uuid = owned_app(config, repository, environment_id)
    if not current(config, sha) or ci_gate(config, sha)[0] != run_id:
        print("Preview skipped: pull request or latest CI run changed")
        return
    if uuid is None:
        uuid = create_app(config, repository, environment_id, pr["head"])
    previous = {value["deployment_uuid"] for value in deployments(config, uuid)}
    # A stale target event must never queue a newer branch head or roll back a preview.
    if not current(config, sha) or ci_gate(config, sha)[0] != run_id:
        print("Preview skipped: pull request or latest CI run changed before deployment")
        return
    api(config, "Coolify", f"applications/{uuid}", "PATCH",
        {"git_branch": pr["head"]["ref"], "git_commit_sha": sha, "instant_deploy": True})
    print(f"Preview PR {config.pr_number} requested verified commit {sha}")
    monitor(config, uuid, sha, previous)


def main():
    try:
        reconcile(configuration())
        return 0
    except DeploymentError as error:
        print(f"Preview failed: {error}", file=sys.stderr)
    except Exception:
        print("Preview failed: unexpected error", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
