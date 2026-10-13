#!/usr/bin/env python3
"""Render bounded build evidence without exporting logs, environment or cache locations."""
import json
import math
import pathlib
import re
import sys


def load_json(file):
    try:
        value = json.loads(file.read_text())
        return value if isinstance(value, dict) else {}
    except (OSError, ValueError):
        return {}


def number(value):
    return value if isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value) and value >= 0 else None


def counter(stats, key):
    value = stats.get(key)
    if isinstance(value, dict):
        return number(value.get("counts", {}).get("Rust"))
    return number(value)


def seconds(value):
    value = number(value)
    return f"{value:.2f}s" if value is not None else "unavailable"


def buildkit_phases(text):
    stages = {}
    for line in text.splitlines():
        match = re.match(r"#(\d+)\s+(.*)", line)
        if not match:
            continue
        key, content = match.groups()
        stage = stages.setdefault(key, {})
        if "importing cache manifest" in content:
            stage["kind"] = "cache manifest import"
        elif "exporting cache" in content:
            stage["kind"] = "cache export"
        elif "exporting to image" in content:
            stage["kind"] = "image export/push"
        elif "RUN" in content and ("bash scripts/build_release.sh" in content or re.search(r"cargo (?:rustc|build)\b", content)):
            stage["kind"] = "compiler layer"
        if content == "CACHED":
            stage.update(cached=True, duration=0)
        duration = re.fullmatch(r"DONE ([0-9.]+)s", content)
        if duration:
            stage.update(cached=False, duration=float(duration.group(1)))
    return {value["kind"]: value for value in stages.values() if "kind" in value}


def cargo_units(folder):
    try:
        html = (pathlib.Path(folder) / "cargo-timing.html").read_text()
        match = re.search(r"const UNIT_DATA = (\[.*?\]);", html, re.S)
        units = json.loads(match.group(1)) if match else []
    except (OSError, ValueError):
        return []
    return [unit for unit in units if isinstance(unit, dict) and number(unit.get("duration")) is not None]


def executable_seconds(component, units):
    # Cargo's HTML target label is a string, e.g. `bin "backend-school"`.
    matches = [unit["duration"] for unit in units if unit.get("name") == component and "bin" in str(unit.get("target", ""))]
    return max(matches) if matches else None


def link_seconds(folder):
    try:
        samples = (pathlib.Path(folder) / "link-nanoseconds.txt").read_text().splitlines()
        if not samples or any(not re.fullmatch(r"[0-9]+", sample) for sample in samples):
            return None
        return sum(int(sample) for sample in samples) / 1e9
    except OSError:
        return None


def summary(component, folder, log_file):
    if component not in {"backend-school", "backend-admin"}:
        raise ValueError("Unsupported build component")
    folder = pathlib.Path(folder)
    metrics = load_json(folder / "build-metrics.json")
    cache = load_json(folder / "sccache-stats.json")
    stats = cache.get("stats", cache)
    if not isinstance(stats, dict):
        stats = {}
    try:
        phases = buildkit_phases(pathlib.Path(log_file).read_text())
    except OSError:
        phases = {}
    units = cargo_units(folder)
    layer = phases.get("compiler layer", {})
    state = "cached" if layer.get("cached") else "executed" if "duration" in layer else "unconfirmed"
    elapsed = number(metrics.get("compileNanoseconds"))
    lines = [f"### {component} build evidence", "", f"- Current compiler layer: **{state}**.",
             f"- Recorded Cargo invocation: {seconds(elapsed / 1e9 if elapsed is not None else None)}.",
             f"- Recorded executable unit (includes link): {seconds(executable_seconds(component, units))}.",
             f"- Recorded final link (within executable unit): {seconds(link_seconds(folder))}.",
             "- Builder-layer evidence can come from an earlier invocation when BuildKit reuses that layer.",
             "", "| BuildKit phase | Duration |", "| --- | ---: |"]
    for name in ["cache manifest import", "compiler layer", "image export/push", "cache export"]:
        lines.append(f"| {name} | {seconds(phases.get(name, {}).get('duration'))} |")
    lines += ["", "Phase durations can overlap; do not sum them. Cached-layer transfer/extraction may occur outside these rows.", "", "| Rust library cache counter | Count |", "| --- | ---: |"]
    for label, key in [("hits", "cache_hits"), ("misses", "cache_misses"), ("errors", "cache_errors"), ("write errors", "cache_write_errors"), ("non-cacheable compiler calls", "non_cacheable_calls")]:
        value = counter(stats, key)
        lines.append(f"| {label} | {value if value is not None else 'unavailable'} |")
    lines += ["", "The executable compile/link is not covered by the library hit count.", "", "| Longest Cargo units | Duration |", "| --- | ---: |"]
    for unit in sorted(units, key=lambda item: item["duration"], reverse=True)[:5]:
        name = re.sub(r"[^a-zA-Z0-9_. -]", "_", str(unit.get("name", "unknown")))[:64]
        target = re.sub(r"[^a-zA-Z0-9_. -]", "_", str(unit.get("target", "unknown")))[:32]
        lines.append(f"| {name} ({target}) | {seconds(unit['duration'])} |")
    lines += ["", "Complete Cargo timings and cache statistics remain in the compiler evidence artifact.", ""]
    return "\n".join(lines)


if __name__ == "__main__":
    print(summary(*sys.argv[1:4]))
    folder = pathlib.Path(sys.argv[2])
    metrics = load_json(folder / "build-metrics.json")
    cache = load_json(folder / "sccache-stats.json")
    stats = cache.get("stats", cache)
    if not isinstance(stats, dict):
        stats = {}
    elapsed = number(metrics.get("compileNanoseconds"))
    # Actions annotations remain API-readable when signed log/artifact downloads
    # are unavailable. Emit only bounded numeric evidence, never cache locations.
    hits = counter(stats, "cache_hits")
    misses = counter(stats, "cache_misses")
    try:
        phases = buildkit_phases(pathlib.Path(sys.argv[3]).read_text())
    except OSError:
        phases = {}
    layer = phases.get("compiler layer", {})
    state = "cached" if layer.get("cached") else "executed" if "duration" in layer else "unconfirmed"
    print(f"::notice title=Recorded compiler evidence::{sys.argv[1]} current_layer={state} builder_cargo={seconds(elapsed / 1e9 if elapsed is not None else None)} executable={seconds(executable_seconds(sys.argv[1], cargo_units(folder)))} link={seconds(link_seconds(folder))} library_hits={hits if hits is not None else 'unavailable'} library_misses={misses if misses is not None else 'unavailable'}; cached layers can reuse older evidence", file=sys.stderr)
    print(f"::notice title=BuildKit phases::{sys.argv[1]} " + " ".join(f"{name.replace(' ', '_')}={seconds(phases.get(name, {}).get('duration'))}" for name in ["cache manifest import", "compiler layer", "image export/push", "cache export"]) + "; phases can overlap and exclude some lazy transfers", file=sys.stderr)
