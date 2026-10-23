#!/usr/bin/env python3
"""Bounded runtime sampling. Raw subprocess output never leaves this collector."""
import json
import math
import os
import re
import selectors
import signal
import subprocess
import time

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


def metric(values, quantiles=True):
    if not values:
        return {"status": "unavailable", "samples": 0}
    ordered = sorted(values)
    result = {"status": "available", "samples": len(values), "max_ms": ordered[-1], "mean_ms": sum(ordered) / len(ordered)}
    if quantiles:
        for label, fraction in (("p50_ms", .5), ("p95_ms", .95), ("p99_ms", .99)):
            result[label] = ordered[max(0, math.ceil(fraction * len(ordered)) - 1)]
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
            return parsed, "proxy"
        fields = parsed.get("fields")
        if not isinstance(fields, dict):
            return None
        return fields, parsed.get("target")
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
    return fields, match[1]


def status_family(value):
    if type(value) is not int or not 100 <= value <= 599:
        return None
    return str(value) if value in {401, 403, 429, 499, 500, 502, 503, 504} else f"{value // 100}xx"


def aggregate(raw, source, collection="complete"):
    groups = {}
    counts = {"lines": 0, "accepted_events": 0, "ignored_lines": 0, "oversized_lines": 0, "invalid_events": 0}
    bounded = collection != "complete"

    def add(labels, timings):
        nonlocal bounded
        key = tuple(labels.items())
        if key not in groups:
            if len(groups) >= MAX_GROUPS:
                bounded = True
                return
            groups[key] = {"labels": labels, "events": 0, "metrics": {}}
        group = groups[key]
        group["events"] += 1
        for name, value in timings.items():
            group["metrics"].setdefault(name, [])
            if value is not None:
                group["metrics"][name].append(value)

    lines = raw.splitlines()
    if len(lines) >= MAX_LINES:
        bounded = True
    for line in lines[:MAX_LINES]:
        counts["lines"] += 1
        if len(line) > MAX_LINE_BYTES:
            counts["oversized_lines"] += 1
            bounded = True
            continue
        parsed = event(line.decode("utf-8", errors="replace"), source)
        if not parsed:
            counts["ignored_lines"] += 1
            continue
        fields, target = parsed
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
        add(labels, timings)
    result = []
    for group in groups.values():
        group["metrics"] = {name: metric(values, group["labels"]["event"] != "pool_acquisition") for name, values in group["metrics"].items()}
        result.append(group)
    present = {group["labels"]["event"] for group in result}
    expected = {"school_proxy_request"} if source == "proxy" else {"http_response_headers", "http_request_cancelled", "password_work", "pool_acquisition"}
    return {"source": source, "status": "partial" if bounded or counts["invalid_events"] else "available" if result else "unavailable", "collection": collection, "coverage": "bounded_log_sample", **counts, "unavailable_events": sorted(expected - present), "groups": result}


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
        cpu = row.get("cpu", row.get("CPUPerc"))
        cpu_match = re.fullmatch(r"([0-9]+(?:\.[0-9]+)?)%", cpu) if isinstance(cpu, str) else None
        cpu = number(float(cpu_match[1]), 1000000) if cpu_match else None
        memory = row.get("mem_usage", row.get("MemUsage"))
        used = limit = None
        if isinstance(memory, str) and memory.count("/") == 1:
            used, limit = (bytes_value(part) for part in memory.split("/"))
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


if __name__ == "__main__":
    try:
        report = collect()
    except Exception:
        # A future parser/provider defect still cannot spill a raw input/traceback.
        report = {"operation": "runtime_observations", "schema_version": 1, "status": "unavailable", "reason": "collector_failed"}
    print(json.dumps(report, allow_nan=False, separators=(",", ":"), sort_keys=True))
