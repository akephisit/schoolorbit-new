#!/usr/bin/env python3
"""Render bounded Actions output; full compiler-unit evidence stays in artifacts."""
import json
import pathlib
import statistics
import sys


def render(root):
    root = pathlib.Path(root)
    rows = [
        "Each sample links once; profile/source priming is excluded from warm means.",
        "Wrapper disabled for Cargo invalidation. Full units, profiles and builder cache evidence are in the artifact.",
        "",
        "| Case | Cargo samples (s) | Mean (s) | Mean link (s) | Binary (MiB) | Prime (s) | Unchanged (s) |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: |",
    ]
    files = sorted(root.glob("*/*/result.json"))
    if not files:
        raise ValueError("Missing completed benchmark results")
    if len(files) > 8:
        raise ValueError("Too many cases for the bounded benchmark")
    rebuilt = []
    for file in files:
        result = json.loads(file.read_text())
        samples = result["samples"]
        case = "/".join(file.relative_to(root).parts[:2])
        durations = ", ".join(f'{sample["cargo_seconds"]:.2f}' for sample in samples)
        mean_link = statistics.mean(sample["link_seconds"] for sample in samples)
        size = samples[-1]["binary_bytes"] / 1024 / 1024
        rows.append(f'| {case} | {durations} | {result["mean_seconds"]:.2f} | {mean_link:.3f} | {size:.2f} | {result["prime_seconds"]:.2f} | {result["unchanged_seconds"]:.3f} |')
        units = sorted({unit for sample in samples for unit in sample["compiled_units"]})
        visible = ", ".join(unit[:80] for unit in units[:8])
        if len(units) > 8:
            visible += f", … ({len(units)} total; see artifact)"
        rebuilt.append(f'Rebuilt `{case}`: {visible}')
    rows.extend(["", *rebuilt])
    output = "\n".join(rows) + "\n"
    if len(output.encode()) > 32768:
        raise ValueError("Benchmark summary exceeds its bounded report budget")
    return output


if __name__ == "__main__":
    print(render(sys.argv[1]), end="")
