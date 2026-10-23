#!/usr/bin/env python3
"""Bounded runtime sampling. Raw subprocess output never leaves this collector."""
import json
import base64
import contextlib
import datetime
import http.cookiejar
import math
import os
import re
import selectors
import signal
import subprocess
import statistics
import sys
import time
import urllib.error
import urllib.request
import urllib.parse

WINDOW_SECONDS = 900
MAX_LINES = 5000
MAX_BYTES = 4 * 1024 * 1024
MAX_LINE_BYTES = 16 * 1024
MAX_GROUPS = 256
COMMAND_SECONDS = 5
CONTAINERS = ("schoolorbit-backend-school", "schoolorbit-backend-admin", "schoolorbit-nginx", "schoolorbit-clamd")
METHODS = {"GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "CONNECT", "TRACE", "OTHER"}
ROUTES = {"/api/staff/dashboard": "dashboard", "/api/students": "students", "/api/me/work-items/counts": "work_counts", "/api/auth/login": "auth_login", "/api/auth/me": "auth_me", "/api/auth/logout": "auth_logout", "/api/auth/sessions": "auth_sessions"}
PROXY_FAMILIES = {"other", "health", "ready", "deployment_status", "identity", "menu", "attendance", "files", "notifications_stream", "websocket"}
KINDS = {"hash", "non_truncating_hash", "verify"}
OUTCOMES = {"completed", "admission_timeout", "admission_unavailable", "cancelled_before_admission", "bcrypt_failed", "worker_failed"}
POOL_MESSAGES = {"acquired connection", "acquired connection, but time to acquire exceeded slow threshold"}
ANSI = re.compile(r"\x1b\[[0-9;]*m")
FMT = re.compile(r"^\S+\s+(?:TRACE|DEBUG|INFO|WARN|ERROR)\s+([A-Za-z0-9_:]+):\s+(.*)$")
FIELD = re.compile(r'(?<!\S)([a-z_]+)=("(?:[^"\\]|\\.)*"|[^\s]+)')


def number(value, maximum=86400000):
    return value if type(value) in (int, float) and 0 <= value <= maximum and math.isfinite(value) else None


def label(value, allowed, fallback):
    return value if isinstance(value, str) and value in allowed else fallback


def timestamp(value):
    if not isinstance(value, str):
        return None
    try:
        parsed = datetime.datetime.fromisoformat(value.replace("Z", "+00:00"))
        return parsed.timestamp() if parsed.tzinfo is not None else None
    except (ValueError, OverflowError, OSError):
        return None


def canonical_token(value):
    if not isinstance(value, str) or not re.fullmatch(r"[A-Za-z0-9_-]{43}", value):
        return False
    decoded = base64.urlsafe_b64decode(value + "=")
    return len(decoded) == 32 and base64.urlsafe_b64encode(decoded).decode().rstrip("=") == value


def metric(values, quantiles=True):
    if not values:
        return {"status": "unavailable", "samples": 0}
    ordered = sorted(values)
    result = {"status": "available", "samples": len(values), "max_ms": ordered[-1], "mean_ms": sum(ordered) / len(ordered)}
    if quantiles:
        result["p50_ms"] = ordered[math.ceil(.5 * len(ordered)) - 1]
        result["tail_availability"] = {}
        for name, fraction, minimum in (("p95", .95, 20), ("p99", .99, 100)):
            enough = len(ordered) >= minimum
            result[name + "_ms"] = ordered[math.ceil(fraction * len(ordered)) - 1] if enough else None
            result["tail_availability"][name] = "empirical_only" if enough else "unavailable_sparse"
    return result


def capture(command, timeout=COMMAND_SECONDS, byte_limit=MAX_BYTES):
    """Drain a fixed command with wall-time/byte caps; discard all error text."""
    process = None
    output = bytearray()
    reason = "complete"
    try:
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, start_new_session=True)
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            deadline = time.monotonic() + timeout
            while selector.get_map():
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    reason = "command_timeout"
                    break
                ready = selector.select(min(remaining, .1))
                for key, _ in ready:
                    chunk = os.read(key.fd, min(65536, byte_limit + 1 - len(output)))
                    if not chunk:
                        selector.unregister(key.fileobj)
                        continue
                    output.extend(chunk)
                    if len(output) > byte_limit:
                        reason = "byte_limit"
                        break
                if reason != "complete":
                    break
            if reason != "complete":
                os.killpg(process.pid, signal.SIGKILL)
            try:
                code = process.wait(timeout=max(.1, deadline - time.monotonic()))
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=1)
                reason, code = "command_timeout", -1
            if code != 0 and reason == "complete":
                reason = "command_failed"
    except (OSError, ValueError):
        reason = "command_unavailable"
    finally:
        if process is not None:
            if process.poll() is None:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=1)
                except (OSError, subprocess.TimeoutExpired):
                    pass
            if process.stdout is not None:
                process.stdout.close()
    # A byte-capped final fragment is never parsed as a complete event.
    raw = bytes(output[:byte_limit])
    if reason == "byte_limit":
        raw = raw.rsplit(b"\n", 1)[0] if b"\n" in raw else b""
    return raw, reason


def event(line, source):
    try:
        parsed = json.loads(line)
        if not isinstance(parsed, dict):
            return None
        if source == "proxy":
            return parsed, "proxy", timestamp(parsed.get("timestamp"))
        fields = parsed.get("fields")
        if not isinstance(fields, dict):
            return None
        return fields, parsed.get("target"), timestamp(parsed.get("timestamp"))
    except (ValueError, RecursionError):
        if source != "admin":
            return None
    match = FMT.fullmatch(ANSI.sub("", line))
    if not match or match[1] not in {"backend_admin::app", "backend_admin::auth::password_work", "sqlx::pool::acquire"}:
        return None
    fields = {}
    for key, raw in FIELD.findall(match[2]):
        if key in fields:
            return None
        try:
            fields[key] = json.loads(raw)
        except (ValueError, RecursionError):
            fields[key] = raw
    if match[1] == "sqlx::pool::acquire":
        # SQLx's fmt message precedes structured numeric fields.
        fields["message"] = match[2].split(" acquired_after_secs=", 1)[0]
    return fields, match[1], timestamp(ANSI.sub("", line).split()[0])


def status_family(value):
    if type(value) is not int or not 100 <= value <= 599:
        return None
    return str(value) if value in {401, 403, 429, 499, 500, 502, 503, 504} else f"{value // 100}xx"


def aggregate(raw, source, collection="complete"):
    collection = label(collection, {"complete", "command_timeout", "byte_limit", "command_failed", "command_unavailable"}, "command_unavailable")
    groups = {}
    observed_times = []
    counts = {"lines": 0, "accepted_events": 0, "aggregated_events": 0, "dropped_events": 0, "ignored_lines": 0, "oversized_lines": 0, "invalid_events": 0}
    reasons = set()
    bounded = collection != "complete"

    def add(labels, timings):
        nonlocal bounded
        key = tuple(labels.items())
        if key not in groups:
            if len(groups) >= MAX_GROUPS:
                bounded = True
                counts["dropped_events"] += 1
                reasons.add("group_limit")
                return
            groups[key] = {"labels": labels, "events": 0, "metrics": {}}
        group = groups[key]
        counts["aggregated_events"] += 1
        group["events"] += 1
        for name, value in timings.items():
            group["metrics"].setdefault(name, [])
            if value is not None:
                group["metrics"][name].append(value)

    lines = raw.splitlines()
    if len(lines) >= MAX_LINES:
        bounded = True
        reasons.add("line_limit")
    for line in lines[:MAX_LINES]:
        counts["lines"] += 1
        if len(line) > MAX_LINE_BYTES:
            counts["oversized_lines"] += 1
            bounded = True
            reasons.add("oversized_line")
            continue
        parsed = event(line.decode("utf-8", errors="replace"), source)
        if not parsed:
            counts["ignored_lines"] += 1
            continue
        fields, target, observed_at = parsed
        operation = label(fields.get("operation"), {"http_response_headers", "http_request_cancelled", "password_work", "school_proxy_request"}, None)
        timings = {}
        labels = None
        if source in {"school", "admin"} and operation in {"http_response_headers", "http_request_cancelled"}:
            duration = number(fields.get("duration_ms"))
            status = "cancelled" if operation == "http_request_cancelled" else status_family(fields.get("status"))
            if duration is not None and status is not None:
                route = fields.get("route")
                labels = {"event": operation, "route": ROUTES.get(route, "other") if isinstance(route, str) else "other", "method": label(fields.get("method"), METHODS, "OTHER"), "status": status}
                timings = {"duration_ms": duration}
        elif source in {"school", "admin"} and operation == "password_work":
            if label(fields.get("kind"), KINDS, None) is not None and label(fields.get("outcome"), OUTCOMES, None) is not None and type(fields.get("capacity")) is int and fields["capacity"] == 4 and number(fields.get("admission_ms")) is not None:
                labels = {"event": "password_work", "kind": fields["kind"], "outcome": fields["outcome"]}
                timings = {name: number(fields.get(name)) for name in ("admission_ms", "blocking_queue_ms", "execution_ms")}
        elif source in {"school", "admin"} and target == "sqlx::pool::acquire" and label(fields.get("message"), POOL_MESSAGES, None) is not None:
            seconds = number(fields.get("acquired_after_secs"), 86400)
            if seconds is not None:
                labels = {"event": "pool_acquisition", "coverage": "logged_successes_only", "sample_kind": "slow" if fields["message"] != "acquired connection" else "ordinary"}
                timings = {"acquisition_ms": seconds * 1000}
        elif source == "proxy" and operation == "school_proxy_request":
            status = status_family(fields.get("status"))
            if status is not None and number(fields.get("request_time"), 86400) is not None:
                labels = {"event": "school_proxy_request", "route": label(fields.get("route_family"), PROXY_FAMILIES, "other"), "method": label(fields.get("method"), METHODS, "OTHER"), "status": status}
                for name in ("request_time", "upstream_connect_time", "upstream_header_time", "upstream_response_time"):
                    seconds = number(fields.get(name), 86400)
                    timings[name + "_ms"] = seconds * 1000 if seconds is not None else None
        if labels is None:
            counts["invalid_events" if operation in {"http_response_headers", "http_request_cancelled", "password_work", "school_proxy_request"} else "ignored_lines"] += 1
            continue
        counts["accepted_events"] += 1
        if observed_at is not None:
            observed_times.append(observed_at)
        add(labels, timings)
    result = []
    for group in groups.values():
        group["metrics"] = {name: metric(values, group["labels"]["event"] != "pool_acquisition") for name, values in group["metrics"].items()}
        result.append(group)
    present = {group["labels"]["event"] for group in result}
    expected = {"school_proxy_request"} if source == "proxy" else {"http_response_headers", "http_request_cancelled", "password_work", "pool_acquisition"}
    if collection != "complete":
        reasons.add(collection)
    if counts["invalid_events"]:
        reasons.add("invalid_events")
    route_samples = {route: 0 for route in ROUTES.values()} if source != "proxy" else None
    if route_samples is not None:
        for group in result:
            route = group["labels"].get("route")
            if route in route_samples:
                route_samples[route] += group["events"]
    span = max(observed_times) - min(observed_times) if observed_times else None
    span = number(span, WINDOW_SECONDS) if span is not None else None
    return {"source": source, "status": "partial" if bounded or counts["invalid_events"] else "available" if result else "unavailable", "collection": collection, "partial_reasons": sorted(reasons), "coverage": "bounded_log_sample", "timestamped_events": len(observed_times), "observed_event_span_seconds": span, **counts, "unavailable_events": sorted(expected - present), "selected_routes": {route: {"events": count, "status": "available" if count else "unavailable"} for route, count in (route_samples or {}).items()}, "groups": result}


def bytes_value(value):
    if type(value) in (int, float):
        return number(value, 2 ** 60)
    if not isinstance(value, str):
        return None
    match = re.fullmatch(r"([0-9]+(?:\.[0-9]+)?)\s*(B|kB|KB|MB|GB|TB|KiB|MiB|GiB|TiB)?", value.strip())
    if not match:
        return None
    unit = match[2] or "B"
    powers = {"B": 1, "kB": 1000, "KB": 1000, "MB": 1000 ** 2, "GB": 1000 ** 3, "TB": 1000 ** 4, "KiB": 1024, "MiB": 1024 ** 2, "GiB": 1024 ** 3, "TiB": 1024 ** 4}
    return number(float(match[1]) * powers[unit], 2 ** 60)


def container_snapshot(raw, collection):
    result = {"status": "unavailable", "collection": collection}
    if collection != "complete":
        return result
    try:
        data = json.loads(raw)
        row = data[0] if isinstance(data, list) and len(data) == 1 else None
        if not isinstance(row, dict):
            return result
        # Podman v5.4.2 cmd/podman/containers/stats.go outputJSON uses
        # cpu_percent, mem_usage and pids (all formatted strings).
        cpu = row.get("cpu_percent", row.get("cpu", row.get("CPUPerc")))
        cpu_match = re.fullmatch(r"([0-9]+(?:\.[0-9]+)?)%", cpu) if isinstance(cpu, str) else None
        cpu = number(float(cpu_match[1]), 1000000) if cpu_match else None
        memory = row.get("mem_usage", row.get("MemUsage"))
        used = limit = None
        if isinstance(memory, str) and memory.count("/") == 1:
            used, limit = (bytes_value(part) for part in memory.split("/"))
            if limit == 0:
                limit = None
        pids = row.get("pids", row.get("PIDs"))
        pids = int(pids) if isinstance(pids, str) and re.fullmatch(r"[0-9]{1,9}", pids) else number(pids, 1000000000)
        values = {"cpu_percent": cpu, "memory_used_bytes": used, "memory_limit_bytes": limit, "pids": pids}
        return {"status": "available" if all(value is not None for value in values.values()) else "partial" if any(value is not None for value in values.values()) else "unavailable", "collection": collection, **values}
    except (ValueError, RecursionError):
        return result


def host_snapshot():
    try:
        def cpu():
            with open("/proc/stat", encoding="ascii") as stream:
                fields = stream.readline().split()
            values = [int(value) for value in fields[1:9]]
            return sum(values), values[3] + values[4]
        before = cpu()
        time.sleep(1)
        after = cpu()
        elapsed, idle = after[0] - before[0], after[1] - before[1]
        memory = {}
        with open("/proc/meminfo", encoding="ascii") as stream:
            for line in stream:
                key, value = line.split(":", 1)
                if key in {"MemTotal", "MemAvailable"}:
                    memory[key] = int(value.split()[0]) * 1024
        busy = number(100 * (elapsed - idle) / elapsed, 100) if elapsed > 0 else None
        return {"scope": "backend_host", "status": "available" if busy is not None and len(memory) == 2 else "partial", "cpu_sample_seconds": 1, "cpu_busy_percent": busy, "memory_total_bytes": memory.get("MemTotal"), "memory_available_bytes": memory.get("MemAvailable")}
    except (OSError, ValueError, IndexError):
        return {"scope": "backend_host", "status": "unavailable"}


def collect():
    sources = []
    for source, container in zip(("school", "admin", "proxy"), CONTAINERS):
        raw, reason = capture(["podman", "logs", "--since", "15m", "--tail", str(MAX_LINES), container])
        sources.append(aggregate(raw, source, reason))
    containers = []
    for container in CONTAINERS:
        raw, reason = capture(["podman", "stats", "--no-stream", "--format", "json", container], byte_limit=65536)
        containers.append({"container": container, **container_snapshot(raw, reason)})
    return {"operation": "runtime_observations", "schema_version": 1, "window_seconds": WINDOW_SECONDS, "tail_lines_per_source": MAX_LINES, "byte_limit_per_source": MAX_BYTES, "quantiles": "nearest_rank_ceil_p_times_n", "population_latency": "unavailable", "database_host_resources": "unavailable", "database_query_plans": "unavailable", "host": host_snapshot(), "containers": containers, "sources": sources}


PROBE_BODY_BYTES = 1024 * 1024
PROBE_REQUEST_SECONDS = 15
PROBE_TOTAL_SECONDS = 360


class ProbeFailure(Exception):
    def __init__(self, reason):
        self.reason = reason


class RefuseRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        fp.close()
        raise ProbeFailure("redirect_refused")


@contextlib.contextmanager
def request_deadline(seconds):
    def expired(_signal, _frame):
        raise ProbeFailure("request_timeout")
    previous = signal.signal(signal.SIGALRM, expired)
    signal.setitimer(signal.ITIMER_REAL, seconds)
    try:
        yield
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
        signal.signal(signal.SIGALRM, previous)


class SandboxClient:
    def __init__(self, api, origin):
        self.api, self.origin = api, origin
        self.cookies = http.cookiejar.CookieJar()
        # HTTPSHandler uses the default verified TLS context; redirects never
        # forward this session or its credentials to another location.
        self.opener = urllib.request.build_opener(RefuseRedirect(), urllib.request.HTTPCookieProcessor(self.cookies))
        self.csrf = None
        self.logout_credential_sent = False
        self.deadline = time.monotonic() + PROBE_TOTAL_SECONDS
        self.requests = 0

    def session_cookie(self):
        candidates = [cookie for cookie in self.cookies if cookie.name == "__Host-schoolorbit_session"]
        if len(candidates) != 1:
            return None
        cookie = candidates[0]
        attributes = {key.lower(): value for key, value in cookie._rest.items()}
        if (cookie.domain != urllib.parse.urlsplit(self.api).hostname or cookie.domain_specified or
                cookie.path != "/" or not cookie.path_specified or not cookie.secure or
                cookie.is_expired() or not canonical_token(cookie.value) or
                "httponly" not in attributes or str(attributes.get("samesite", "")).lower() != "lax"):
            return None
        return cookie

    def request(self, method, path, payload=None, cleanup=False):
        remaining = self.deadline + (PROBE_REQUEST_SECONDS if cleanup else 0) - time.monotonic()
        if remaining <= 0 or self.requests >= 24:
            raise ProbeFailure("probe_limit")
        headers = {"Origin": self.origin, "X-School-Subdomain": "sandbox", "Cache-Control": "no-store", "Accept": "application/json"}
        body = None
        if payload is not None:
            body = json.dumps(payload).encode()
            headers["Content-Type"] = "application/json"
        if method == "POST" and path == "/api/auth/logout" and self.csrf:
            headers["X-CSRF-Token"] = self.csrf
        request = urllib.request.Request(self.api + path, data=body, headers=headers, method=method)
        self.cookies.add_cookie_header(request)
        if path == "/api/auth/logout":
            cookie = self.session_cookie()
            if cookie is None or cookie.name + "=" + cookie.value not in (request.get_header("Cookie") or "").split("; "):
                raise ProbeFailure("cleanup_unknown")
            self.logout_credential_sent = True
        self.requests += 1
        started = time.monotonic()
        try:
            timeout = min(PROBE_REQUEST_SECONDS, remaining)
            with request_deadline(timeout):
                with self.opener.open(request, timeout=timeout) as response:
                    # Capture before body parsing/size validation, including login.
                    tokens = response.headers.get_all("X-CSRF-Token", [])
                    if len(tokens) == 1 and canonical_token(tokens[0]):
                        self.csrf = tokens[0]
                    if response.status != 200:
                        raise ProbeFailure("unexpected_status")
                    raw = response.read(PROBE_BODY_BYTES + 1)
                    if len(raw) > PROBE_BODY_BYTES:
                        raise ProbeFailure("body_limit")
                    data = json.loads(raw)
                    if not isinstance(data, dict) or data.get("success") is not True:
                        raise ProbeFailure("invalid_envelope")
                    return data.get("data"), len(raw), (time.monotonic() - started) * 1000
        except urllib.error.HTTPError as error:
            error.close()
            raise ProbeFailure("http_" + (status_family(error.code) or "other")) from None
        except (OSError, ValueError, RecursionError):
            raise ProbeFailure("request_failed") from None


def probe_summary(route, durations, sizes, reason=None):
    summary = {"route": route, "status": "available" if len(durations) == 5 else "partial" if durations else "unavailable", "samples": len(durations), "expected_samples": 5}
    if reason:
        summary["reason"] = reason
    if durations:
        summary.update({"client_wall_ms": {"median": statistics.median(durations), "min": min(durations), "max": max(durations)}, "response_body_bytes": {"median": statistics.median(sizes), "min": min(sizes), "max": max(sizes)}})
    return summary


def valid_probe_data(route, data):
    if not isinstance(data, dict):
        return False
    if route == "students":
        return isinstance(data.get("items"), list) and len(data["items"]) <= 20 and type(data.get("page")) is int and data["page"] == 1 and type(data.get("page_size")) is int and data["page_size"] == 20
    names = ("totalStaff", "totalStudents", "activeHomerooms") if route == "dashboard" else ("open", "dueSoon", "overdue", "submitted", "closed", "total")
    return all(type(data.get(name)) is int and 0 <= data[name] <= 1000000000 for name in names)


def sandbox_probe():
    report = {"operation": "sandbox_route_observations", "scope": "dedicated_sandbox_staff_session", "profile": "sequential_https_api_client", "population_latency": "unavailable", "status": "unavailable", "cleanup": "not_required", "routes": []}
    username, password = os.environ.get("SMOKE_USERNAME"), os.environ.get("SMOKE_PASSWORD")
    if not username or not password:
        return {**report, "reason": "credentials_missing"}
    domain, api = os.environ.get("BASE_DOMAIN", ""), os.environ.get("BACKEND_SCHOOL_URL", "")
    if not re.fullmatch(r"(?=.{1,253}$)[a-z0-9]+(?:[.-][a-z0-9]+)*\.[a-z]{2,63}", domain) or api.rstrip("/") != f"https://school-api.{domain}":
        return {**report, "reason": "trusted_configuration_invalid"}
    client = SandboxClient(api.rstrip("/"), f"https://sandbox.{domain}")
    login_attempted = False
    phase = "login"
    try:
        login_attempted = True
        client.request("POST", "/api/auth/login", {"username": username, "password": password, "rememberMe": False})
        del password
        if client.session_cookie() is None:
            raise ProbeFailure("session_cookie_unavailable")
        if client.csrf is None:
            raise ProbeFailure("csrf_missing")
        phase = "identity"
        user, _, _ = client.request("GET", "/api/auth/me")
        if not isinstance(user, dict) or user.get("username") != username or user.get("userType") != "staff" or user.get("status") != "active":
            raise ProbeFailure("active_staff_required")
        permissions = user.get("permissions")
        student_eligible = isinstance(permissions, list) and any(label(permission, {"student.read.school", "student.read.assigned", "student.read.own"}, None) is not None for permission in permissions)
        phase = "context"
        year = None
        context_reason = "context_missing"
        try:
            context, _, _ = client.request("GET", "/api/academic/context/options")
            if isinstance(context, dict) and isinstance(context.get("years"), list):
                active = context.get("activeAcademicYearId")
                if isinstance(active, str) and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", active) and any(isinstance(row, dict) and row.get("id") == active for row in context["years"]):
                    year = active
        except ProbeFailure as error:
            context_reason = error.reason
        for route, path in (("dashboard", "/api/staff/dashboard"), ("students", "/api/students?page=1&pageSize=20"), ("work_counts", "/api/me/work-items/counts")):
            phase = route
            if route == "students" and not student_eligible:
                report["routes"].append(probe_summary(route, [], [], "student_read_ineligible"))
                continue
            if route == "dashboard":
                if year is None:
                    report["routes"].append(probe_summary(route, [], [], context_reason))
                    continue
                path += "?academicYearId=" + year
            durations, sizes = [], []
            reason = None
            try:
                for run in range(6):
                    data, size, duration = client.request("GET", path)
                    if not valid_probe_data(route, data):
                        raise ProbeFailure("invalid_route_data")
                    if run:
                        durations.append(duration)
                        sizes.append(size)
            except ProbeFailure as error:
                reason = error.reason
            report["routes"].append(probe_summary(route, durations, sizes, reason))
        report["status"] = "available" if all(route["status"] == "available" for route in report["routes"]) else "partial"
    except ProbeFailure as error:
        report.update({"reason": error.reason, "phase": phase})
    except Exception:
        report.update({"reason": "probe_failed", "phase": phase})
    finally:
        if login_attempted:
            report["cleanup"] = "unavailable"
            try:
                if client.csrf is not None and client.session_cookie() is not None:
                    client.request("POST", "/api/auth/logout", {}, cleanup=True)
                    report["cleanup"] = "completed" if client.logout_credential_sent else "unavailable"
                elif client.session_cookie() is None:
                    report["cleanup_reason"] = "cleanup_unknown"
                else:
                    report["cleanup_reason"] = "csrf_missing"
            except ProbeFailure as error:
                report["cleanup_reason"] = error.reason
            except Exception:
                report["cleanup_reason"] = "cleanup_failed"
            if report["cleanup"] != "completed":
                report["status"] = "partial"
        client.cookies.clear()
    return report


if __name__ == "__main__":
    try:
        report = sandbox_probe() if sys.argv[1:] == ["--sandbox-probe"] else collect()
    except Exception:
        # A future parser/provider defect still cannot spill a raw input/traceback.
        report = {"operation": "runtime_observations", "schema_version": 1, "status": "unavailable", "reason": "collector_failed"}
    print(json.dumps(report, allow_nan=False, separators=(",", ":"), sort_keys=True))
