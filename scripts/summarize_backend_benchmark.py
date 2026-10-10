#!/usr/bin/env python3
"""Summarize completed warm Docker samples, refusing incomplete or changed APIs."""
import json
import pathlib
import re
import statistics
import sys


def summarize(folder, backend, variant, owner, count):
    folder = pathlib.Path(folder)
    baseline = None
    if backend == "backend-school":
        baseline = json.loads((folder / "openapi-baseline.json").read_text())
        if baseline.get("openapi") != "3.1.0" or not baseline.get("paths"):
            raise ValueError("Invalid baseline OpenAPI")
        reference = json.loads((folder / "../../openapi-baseline.json").read_text())
        if baseline != reference:
            raise ValueError("API differs from the default compiler profile")
    samples = []
    for sample in range(1, count + 1):
        timings = (folder / f"cargo-timing-{sample}.html").read_text()
        match = re.search(r"const UNIT_DATA = (\[.*?\]);", timings, re.S)
        if not match:
            raise ValueError("Missing Cargo timing units")
        units = json.loads(match.group(1))
        compiled = re.findall(r"^\s*Compiling (\S+)", (folder / f"compile-{sample}.log").read_text(), re.M)
        if backend not in compiled:
            raise ValueError("Source-changing sample did not rebuild the application")
        link_times = (folder / f"link-nanoseconds-{sample}.txt").read_text().splitlines()
        if len(link_times) != 1:
            raise ValueError("Expected exactly one final binary link per sample")
        if baseline is not None:
            if json.loads((folder / f"openapi-{sample}.json").read_text()) != baseline:
                raise ValueError("API contract changed")
        samples.append({
            "cargo_seconds": int((folder / f"cargo-nanoseconds-{sample}.txt").read_text()) / 1e9,
            "link_seconds": int(link_times[0]) / 1e9,
            "binary_bytes": int((folder / f"binary-bytes-{sample}.txt").read_text()),
            "compiled_units": compiled,
            "units_by_duration": sorted(({
                "name": unit["name"], "target": unit["target"],
                "duration_seconds": unit["duration"], "start_seconds": unit["start"],
                "sections": unit.get("sections"),
            } for unit in units), key=lambda unit: unit["duration_seconds"], reverse=True),
            "link_driver_flags": (folder / f"link-driver-{sample}.txt").read_text().splitlines(),
        })
    return {
        "backend": backend, "variant": variant, "owner": owner,
        "compiler_cache": "disabled for Cargo invalidation isolation; see builder-cache evidence separately",
        "prime_seconds": int((folder / "prime-nanoseconds.txt").read_text()) / 1e9,
        "unchanged_seconds": int((folder / "unchanged-nanoseconds.txt").read_text()) / 1e9,
        "mean_seconds": statistics.mean(sample["cargo_seconds"] for sample in samples),
        "samples": samples,
    }


if __name__ == "__main__":
    result = summarize(*sys.argv[1:5], int(sys.argv[5]))
    (pathlib.Path(sys.argv[1]) / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
