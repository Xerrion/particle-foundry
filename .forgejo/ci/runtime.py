"""Small HTTP client for the APIs supplied by Forgejo Runner."""

import json
import urllib.error
import urllib.parse
import urllib.request


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


OPENER = urllib.request.build_opener(NoRedirect)


def request(url, method="GET", data=None, headers=None):
    request_headers = dict(headers or {})
    if isinstance(data, dict):
        data = json.dumps(data).encode()
        request_headers["Content-Type"] = "application/json"
    req = urllib.request.Request(url, data=data, method=method, headers=request_headers)
    try:
        return OPENER.open(req, timeout=60)
    except urllib.error.HTTPError as error:
        # urllib errors include the opaque URL. Do not expose it in job logs.
        code = error.code
        error.close()
        raise RuntimeError(f"Runner API returned HTTP {code}") from None
    except urllib.error.URLError:
        raise RuntimeError("Runner API is unreachable") from None


def json_request(url, method="GET", data=None, headers=None):
    with request(url, method, data, headers) as response:
        content = response.read()
        return json.loads(content) if content else None


def add_query(url, **values):
    parts = urllib.parse.urlsplit(url)
    query = urllib.parse.parse_qsl(parts.query, keep_blank_values=True)
    query.extend(values.items())
    return urllib.parse.urlunsplit(parts._replace(query=urllib.parse.urlencode(query)))


def require_same_origin(url, base):
    candidate = urllib.parse.urlsplit(url)
    trusted = urllib.parse.urlsplit(base)
    if (candidate.scheme, candidate.netloc) != (trusted.scheme, trusted.netloc):
        raise RuntimeError("Runner API returned an unexpected origin")
    return url
