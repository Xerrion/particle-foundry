"""Deploy the verified Forgejo main commit through the Coolify v4 API."""

from dataclasses import dataclass
import os
import re
import sys
import time
from urllib.parse import quote, urlsplit

from runtime import add_query, json_request

POLL_SECONDS = 5
QUEUE_TIMEOUT_SECONDS = 60
DEPLOY_TIMEOUT_SECONDS = 20 * 60
SHA_PATTERN = re.compile(r"[0-9a-f]{40}")
ID_PATTERN = re.compile(r"[A-Za-z0-9][A-Za-z0-9_-]{0,127}")


class DeploymentError(RuntimeError):
    """An error with a fixed message that is safe for CI logs."""


@dataclass(frozen=True)
class Configuration:
    coolify_url: str
    app_uuid: str
    coolify_token: str
    forgejo_url: str
    repository: str
    ci_token: str
    sha: str


def base_url(value, service):
    try:
        parsed = urlsplit(value)
        valid = (parsed.scheme in {"https", "http"} and parsed.hostname
                 and parsed.username is None and parsed.password is None
                 and parsed.path in {"", "/"} and not parsed.query and not parsed.fragment)
        # Plain HTTP is permitted only for an explicitly configured tailnet endpoint.
        if parsed.scheme == "http" and not (parsed.hostname or "").endswith(".ts.net"):
            valid = False
        parsed.port
    except ValueError:
        valid = False
    if not valid:
        raise DeploymentError(f"{service} URL must use HTTPS or a private .ts.net HTTP endpoint")
    return value.rstrip("/")


def configuration():
    if os.environ.get("FORGEJO_EVENT_NAME") != "push" or os.environ.get("FORGEJO_REF") != "refs/heads/main":
        raise DeploymentError("Deployment requires a push to Forgejo main")
    required = ("COOLIFY_URL", "COOLIFY_APP_UUID", "COOLIFY_TOKEN", "FORGEJO_SERVER_URL",
                "FORGEJO_REPOSITORY", "CI_TOKEN", "CI_SHA")
    if any(not os.environ.get(name) for name in required):
        raise DeploymentError("Deployment configuration is incomplete")
    sha, app_uuid = os.environ["CI_SHA"], os.environ["COOLIFY_APP_UUID"]
    if not SHA_PATTERN.fullmatch(sha) or not ID_PATTERN.fullmatch(app_uuid):
        raise DeploymentError("Deployment requires a full commit SHA and a valid application UUID")
    repository = os.environ["FORGEJO_REPOSITORY"]
    parts = repository.split("/")
    if len(parts) != 2 or any(not part or part in {".", ".."} for part in parts):
        raise DeploymentError("Forgejo repository must contain an owner and a repository name")
    return Configuration(base_url(os.environ["COOLIFY_URL"], "Coolify"), app_uuid,
                         os.environ["COOLIFY_TOKEN"], base_url(os.environ["FORGEJO_SERVER_URL"], "Forgejo"),
                         repository, os.environ["CI_TOKEN"], sha)


def api(url, token, service, method="GET", data=None):
    try:
        return json_request(url, method, data, {"Authorization": f"Bearer {token}"})
    except Exception:
        # Responses and exception messages can contain credentials or build configuration.
        raise DeploymentError(f"{service} API request failed") from None


def source_is_current(config):
    repository = "/".join(quote(part, safe="") for part in config.repository.split("/"))
    branch = api(f"{config.forgejo_url}/api/v1/repos/{repository}/branches/main", config.ci_token, "Forgejo")
    commit = branch.get("commit") if isinstance(branch, dict) else None
    current = commit.get("id") if isinstance(commit, dict) else None
    if not isinstance(current, str) or not SHA_PATTERN.fullmatch(current):
        raise DeploymentError("Forgejo returned an invalid main commit")
    return current == config.sha


def deployments(config):
    url = add_query(f"{config.coolify_url}/api/v1/deployments/applications/{config.app_uuid}", skip=0, take=10)
    result = api(url, config.coolify_token, "Coolify")
    records = result.get("deployments") if isinstance(result, dict) else None
    if not isinstance(records, list):
        raise DeploymentError("Coolify returned an invalid deployment list")
    for record in records:
        uuid = record.get("deployment_uuid") if isinstance(record, dict) else None
        if not isinstance(uuid, str) or not ID_PATTERN.fullmatch(uuid):
            raise DeploymentError("Coolify returned an invalid deployment UUID")
    return records


def deployment(config, deployment_uuid):
    result = api(f"{config.coolify_url}/api/v1/deployments/{deployment_uuid}", config.coolify_token, "Coolify")
    if not isinstance(result, dict) or result.get("deployment_uuid") != deployment_uuid:
        raise DeploymentError("Coolify returned an unexpected deployment UUID")
    return result


def deploy(config):
    previous = {record["deployment_uuid"] for record in deployments(config)}
    # Check main immediately before the mutation. Old workflow reruns must not roll back main.
    if not source_is_current(config):
        print("Deployment skipped: Forgejo main no longer matches the verified commit")
        return
    # Coolify saves and queues from the same application object. A separate deploy request would race.
    api(f"{config.coolify_url}/api/v1/applications/{config.app_uuid}", config.coolify_token, "Coolify",
        "PATCH", {"git_commit_sha": config.sha, "instant_deploy": True})
    print(f"Coolify deployment requested for verified commit {config.sha}")
    started = time.monotonic()
    deployment_uuid = None
    while True:
        if deployment_uuid is None:
            records = deployments(config)
            record = next((record for record in records
                           if record["deployment_uuid"] not in previous and record.get("commit") == config.sha
                           and record.get("pull_request_id") == 0), None)
            if record is not None:
                deployment_uuid = record["deployment_uuid"]
        else:
            record = deployment(config, deployment_uuid)
        if record is not None:
            if record.get("commit") != config.sha or record.get("pull_request_id") != 0:
                raise DeploymentError("Tracked Coolify deployment no longer matches the verified commit")
            status = record.get("status")
            if status == "finished":
                print(f"Coolify deployment finished for verified commit {config.sha}")
                return
            if status in {"failed", "cancelled-by-user"}:
                raise DeploymentError(f"Coolify deployment ended with status {status}")
            if status not in {"queued", "in_progress"}:
                raise DeploymentError("Coolify returned an unknown deployment status")
        elapsed = time.monotonic() - started
        if deployment_uuid is None and elapsed >= QUEUE_TIMEOUT_SECONDS:
            raise DeploymentError("Coolify did not queue a new deployment for the verified commit")
        if elapsed >= DEPLOY_TIMEOUT_SECONDS:
            raise DeploymentError("Coolify deployment did not finish within 20 minutes")
        time.sleep(POLL_SECONDS)


def main():
    try:
        deploy(configuration())
        return 0
    except DeploymentError as error:
        print(f"Deployment failed: {error}", file=sys.stderr)
    except Exception:
        print("Deployment failed: unexpected error", file=sys.stderr)
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
