#!/usr/bin/env python3
"""Validate aggregate reports and atomically retain their original JSON bytes."""
import json
import math
import os
from pathlib import Path
import sys
import tempfile

MAX_BYTES = 2 * 1024 * 1024
ROUTES = {"dashboard", "students", "work_counts", "auth_login", "auth_me", "auth_logout", "auth_sessions"}
CONTAINERS = {"schoolorbit-backend-school", "schoolorbit-backend-admin", "schoolorbit-nginx", "schoolorbit-clamd"}
COLLECTIONS = {"complete", "command_timeout", "byte_limit", "command_failed", "command_unavailable"}
EVENTS = {"http_response_headers", "http_request_cancelled", "password_work", "pool_acquisition", "school_proxy_request"}
HTTP_STATUS = {"1xx", "2xx", "3xx", "4xx", "5xx", "401", "403", "429", "499", "500", "502", "503", "504"}
PROBE_REASONS = {"credentials_missing", "trusted_configuration_invalid", "request_timeout", "probe_limit", "redirect_refused", "unexpected_status", "body_limit", "invalid_envelope", "request_failed", "csrf_missing", "context_missing", "invalid_route_data", "active_staff_required", "student_read_ineligible", "probe_failed", "session_cookie_unavailable", "cleanup_unknown", "cleanup_failed"} | {"http_" + value for value in HTTP_STATUS | {"other"}}


def require(condition):
    if not condition:
        raise ValueError("invalid aggregate")


def choice(value, allowed):
    require(type(value) is str and value in allowed)


def numeric(value, maximum=2 ** 60, minimum=0, integer=False, nullable=False):
    if value is None and nullable:
        return
    require(type(value) in ((int,) if integer else (int, float)))
    require(minimum <= value <= maximum and math.isfinite(value))


def shape(value, required, optional=()):
    require(type(value) is dict and set(required) <= value.keys() and value.keys() <= set(required) | set(optional))


def strings(values, allowed, maximum):
    require(type(values) is list and len(values) <= maximum)
    for value in values:
        choice(value, allowed)
    require(len(values) == len(set(values)))


def metric(value):
    shape(value, {"status", "samples"}, {"max_ms", "mean_ms", "p50_ms", "p95_ms", "p99_ms", "tail_availability"})
    numeric(value["samples"], 5000, integer=True)
    if value["samples"] == 0:
        require(value == {"status": "unavailable", "samples": 0})
        return
    require(value["status"] == "available")
    require({"max_ms", "mean_ms"} <= value.keys())
    for key in value.keys() - {"status", "samples", "tail_availability"}:
        numeric(value[key], 86400000, nullable=key in {"p95_ms", "p99_ms"})
    if "p50_ms" in value:
        require({"p95_ms", "p99_ms", "tail_availability"} <= value.keys())
        shape(value["tail_availability"], {"p95", "p99"})
        for name, count in (("p95", 20), ("p99", 100)):
            require(value["tail_availability"][name] == ("empirical_only" if value["samples"] >= count else "unavailable_sparse"))
            require((value[name + "_ms"] is None) == (value["samples"] < count))
    else:
        require(not ({"p95_ms", "p99_ms", "tail_availability"} & value.keys()))


def group(value):
    shape(value, {"labels", "events", "metrics"})
    numeric(value["events"], 5000, 1, integer=True)
    labels = value["labels"]
    require(type(labels) is dict)
    choice(labels.get("event"), EVENTS)
    event = labels["event"]
    if event == "password_work":
        shape(labels, {"event", "kind", "outcome"})
        choice(labels["kind"], {"hash", "non_truncating_hash", "verify"})
        choice(labels["outcome"], {"completed", "admission_timeout", "admission_unavailable", "cancelled_before_admission", "bcrypt_failed", "worker_failed"})
        metrics = {"admission_ms", "blocking_queue_ms", "execution_ms"}
    elif event == "pool_acquisition":
        shape(labels, {"event", "coverage", "sample_kind"})
        require(labels["coverage"] == "logged_successes_only")
        choice(labels["sample_kind"], {"slow", "ordinary"})
        metrics = {"acquisition_ms"}
    else:
        shape(labels, {"event", "route", "method", "status"})
        routes = {"other", "health", "ready", "deployment_status", "identity", "menu", "attendance", "files", "notifications_stream", "websocket"} if event == "school_proxy_request" else ROUTES | {"other"}
        choice(labels["route"], routes)
        choice(labels["method"], {"GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "CONNECT", "TRACE", "OTHER"})
        choice(labels["status"], {"cancelled"} if event == "http_request_cancelled" else HTTP_STATUS)
        metrics = {"request_time_ms", "upstream_connect_time_ms", "upstream_header_time_ms", "upstream_response_time_ms"} if event == "school_proxy_request" else {"duration_ms"}
    shape(value["metrics"], metrics)
    for observation in value["metrics"].values():
        metric(observation)


def source(value):
    counts = {"timestamped_events", "lines", "accepted_events", "aggregated_events", "dropped_events", "ignored_lines", "oversized_lines", "invalid_events"}
    shape(value, counts | {"source", "status", "collection", "partial_reasons", "coverage", "observed_event_span_seconds", "unavailable_events", "selected_routes", "groups"})
    choice(value["source"], {"school", "admin", "proxy"})
    choice(value["status"], {"available", "partial", "unavailable"})
    choice(value["collection"], COLLECTIONS)
    require(value["coverage"] == "bounded_log_sample")
    strings(value["partial_reasons"], COLLECTIONS - {"complete"} | {"group_limit", "line_limit", "oversized_line", "invalid_events"}, 8)
    strings(value["unavailable_events"], EVENTS, 4)
    for name in counts:
        numeric(value[name], 5000, integer=True)
    numeric(value["observed_event_span_seconds"], 900, nullable=True)
    require(value["accepted_events"] == value["aggregated_events"] + value["dropped_events"])
    require(type(value["groups"]) is list and len(value["groups"]) <= 256)
    for item in value["groups"]:
        group(item)
    require(sum(item["events"] for item in value["groups"]) == value["aggregated_events"])
    shape(value["selected_routes"], ROUTES if value["source"] == "school" else set())
    for item in value["selected_routes"].values():
        shape(item, {"events", "status"})
        numeric(item["events"], 5000, integer=True)
        require(item["status"] == ("available" if item["events"] else "unavailable"))


def runtime(value):
    if set(value) == {"operation", "schema_version", "status", "reason"}:
        require(value == {"operation": "runtime_observations", "schema_version": 1, "status": "unavailable", "reason": "collector_failed"})
        return
    shape(value, {"operation", "schema_version", "window_seconds", "tail_lines_per_source", "byte_limit_per_source", "quantiles", "population_latency", "database_host_resources", "database_query_plans", "host", "containers", "sources"})
    for key, expected in {"operation": "runtime_observations", "schema_version": 1, "window_seconds": 900, "tail_lines_per_source": 5000, "byte_limit_per_source": 4194304, "quantiles": "nearest_rank_ceil_p_times_n", "population_latency": "unavailable", "database_host_resources": "unavailable", "database_query_plans": "unavailable"}.items():
        require(type(value[key]) is type(expected) and value[key] == expected)
    host = value["host"]
    shape(host, {"scope", "status"}, {"cpu_sample_seconds", "cpu_busy_percent", "memory_total_bytes", "memory_available_bytes"})
    require(host["scope"] == "backend_host")
    choice(host["status"], {"available", "partial", "unavailable"})
    for key in host.keys() - {"scope", "status"}:
        numeric(host[key], 100 if key == "cpu_busy_percent" else 2 ** 60, nullable=True)
    require(type(value["containers"]) is list and len(value["containers"]) == 4)
    names = []
    for item in value["containers"]:
        shape(item, {"container", "status", "collection"}, {"cpu_percent", "memory_used_bytes", "memory_limit_bytes", "pids"})
        choice(item["container"], CONTAINERS)
        names.append(item["container"])
        choice(item["status"], {"available", "partial", "unavailable"})
        choice(item["collection"], COLLECTIONS)
        for key in item.keys() - {"container", "status", "collection"}:
            numeric(item[key], 1000000 if key == "cpu_percent" else 2 ** 60, nullable=True)
    require(set(names) == CONTAINERS)
    require(type(value["sources"]) is list and len(value["sources"]) == 3)
    for item in value["sources"]:
        source(item)
    require({item["source"] for item in value["sources"]} == {"school", "admin", "proxy"})


def probe(value):
    shape(value, {"operation", "scope", "profile", "population_latency", "status", "cleanup", "routes"}, {"reason", "phase", "cleanup_reason"})
    require(value["operation"] == "sandbox_route_observations" and value["scope"] == "dedicated_sandbox_staff_session" and value["profile"] == "sequential_https_api_client" and value["population_latency"] == "unavailable")
    choice(value["status"], {"available", "partial", "unavailable"})
    choice(value["cleanup"], {"not_required", "unavailable", "completed"})
    for name in ("reason", "cleanup_reason"):
        if name in value:
            choice(value[name], PROBE_REASONS)
    if "phase" in value:
        choice(value["phase"], {"login", "identity", "context", "dashboard", "students", "work_counts"})
    require(type(value["routes"]) is list and len(value["routes"]) <= 3)
    names = []
    for item in value["routes"]:
        shape(item, {"route", "status", "samples", "expected_samples"}, {"reason", "client_wall_ms", "response_body_bytes"})
        choice(item["route"], {"dashboard", "students", "work_counts"})
        names.append(item["route"])
        numeric(item["samples"], 5, integer=True)
        require(type(item["expected_samples"]) is int and item["expected_samples"] == 5)
        require(item["status"] == ("available" if item["samples"] == 5 else "partial" if item["samples"] else "unavailable"))
        if "reason" in item:
            choice(item["reason"], PROBE_REASONS)
        for name in ("client_wall_ms", "response_body_bytes"):
            require((name in item) == (item["samples"] > 0))
            if name in item:
                shape(item[name], {"median", "min", "max"})
                for number in item[name].values():
                    numeric(number, 1048576 if name == "response_body_bytes" else 86400000)
                require(item[name]["min"] <= item[name]["median"] <= item[name]["max"])
    require(len(names) == len(set(names)))


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def promote(kind, candidate, destination):
    require(kind in {"runtime", "probe"})
    destination = Path(destination)
    require(destination.name == ("runtime.json" if kind == "runtime" else "sandbox-probe.json"))
    with open(candidate, "rb") as stream:
        raw = stream.read(MAX_BYTES + 1)
    require(0 < len(raw) <= MAX_BYTES)
    report = json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object, parse_constant=lambda _value: require(False))
    require(type(report) is dict)
    (runtime if kind == "runtime" else probe)(report)
    destination.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    require(not destination.parent.is_symlink())
    os.chmod(destination.parent, 0o700)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=".validated-", delete=False) as stream:
            temporary = stream.name
            os.chmod(temporary, 0o600)
            stream.write(raw)
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(temporary, destination)
        temporary = None
    finally:
        if temporary is not None:
            os.unlink(temporary)


if __name__ == "__main__":
    try:
        require(len(sys.argv) == 4)
        promote(*sys.argv[1:])
    except Exception:
        print("runtime_observation_artifact=invalid", file=sys.stderr)
        raise SystemExit(1)
    print("runtime_observation_artifact=validated")
