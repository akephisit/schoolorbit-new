#!/usr/bin/env python3
"""Probe the pinned Rust/GHA cache and publish only bounded, allowlisted evidence."""
import json
import os
import pathlib
import re
import subprocess
import tempfile
import time

from render_backend_build_summary import counter


class ProbeFailure(Exception):
    def __init__(self, stage, code):
        self.stage, self.code = stage, code


def command(args, stage):
    result = subprocess.run(args, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
    if result.returncode:
        # Command output can contain signed URLs or daemon diagnostics. Never echo it.
        raise ProbeFailure(stage, result.returncode)
    return result.stdout.strip()


def classify_private_log(raw):
    """Never return log fragments, paths, keys, URLs, headers or environment values."""
    errors = []
    for match in re.finditer(r"(?:Error executing cache write:|storage write check failed:|cache storage failed to read:)(.*?)(?=\n\[[0-9]{4}-|\Z)", raw, re.S):
        block = match.group(0)
        kind = re.search(r"\b(RateLimited|PermissionDenied|AlreadyExists|Unexpected|Unsupported|NotFound|ConfigInvalid|ConditionNotMatch)\b", block)
        statuses = sorted({int(x) for x in re.findall(r"\bstatus(?:_code)?[\s\"':=]+([1-5][0-9]{2})\b", block, re.I)})
        signals = []
        for label, pattern in (
            ("quota", r"quota|cache storage limit|maximum allowed cache"),
            ("throttled", r"rate.?limit|too many requests"),
            ("conflict", r"already exists|conflict"),
            ("read_only", r"read.only"),
        ):
            if re.search(pattern, block, re.I):
                signals.append(label)
        stage = "cache_write" if block.startswith("Error executing") else "write_check" if block.startswith("storage write") else "read_check"
        errors.append(dict(stage=stage, kind=kind.group(1) if kind else "unclassified", http_statuses=statuses, signals=signals))
    return errors[:20]


def probe_recipe(dockerfile):
    dockerfile = dockerfile.replace("\\\n", " ")
    base = re.search(r"^FROM (\S+@sha256:[a-f0-9]{64}) AS base$", dockerfile, re.M)
    binary = re.search(r"ADD --checksum=sha256:([a-f0-9]{64})\s+(https://github.com/mozilla/sccache/releases/download/[^\s]+)\s+/tmp/sccache.tar.gz", dockerfile)
    if not base or not binary:
        raise ValueError("Pinned compiler owners changed")
    folder = binary.group(2).rsplit("/", 1)[-1].removesuffix(".tar.gz")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+", folder):
        raise ValueError("Invalid pinned binary owner")
    return (f"FROM {base.group(1)}\n"
            f"ADD --checksum=sha256:{binary.group(1)} {binary.group(2)} /tmp/sccache.tar.gz\n"
            f"RUN tar -xzf /tmp/sccache.tar.gz -C /tmp && install -m 0755 /tmp/{folder}/sccache /usr/local/bin/sccache "
            f"&& rm -rf /tmp/sccache.tar.gz /tmp/{folder}\n"
            "COPY --chmod=0755 rustc-cache /usr/local/bin/rustc-cache\n"
            "WORKDIR /probe\n")


def stats(container):
    raw = json.loads(command(["docker", "exec", container, "sccache", "--show-stats", "--stats-format=json"], "statistics"))
    values = {key: counter(raw.get("stats", {}), key) for key in (
        "cache_hits", "cache_misses", "cache_writes", "cache_errors", "cache_read_errors", "cache_write_errors", "cache_timeouts"
    )}
    if any(value is None for value in values.values()):
        raise ValueError("Compiler counters are unavailable")
    return values


def main():
    run_id, attempt = os.environ.get("GITHUB_RUN_ID", ""), os.environ.get("GITHUB_RUN_ATTEMPT", "")
    if not re.fullmatch(r"[0-9]+", run_id) or not re.fullmatch(r"[0-9]+", attempt):
        raise ValueError("A bounded operation identity is required")
    for key in ("ACTIONS_RESULTS_URL", "ACTIONS_RUNTIME_TOKEN"):
        if not os.environ.get(key):
            raise ValueError("Cache runtime is unavailable")
    root = pathlib.Path(__file__).resolve().parents[1]
    output = pathlib.Path(os.environ["COMPILER_CACHE_OUTPUT"])
    output.mkdir(parents=True, exist_ok=True)
    namespace = f"compiler-cache-probe-{run_id}-{attempt}"
    image = f"schoolorbit-compiler-cache-probe:{run_id}-{attempt}"
    result = dict(schemaVersion=1, source_sha=command(["git", "-C", str(root), "rev-parse", "HEAD"], "source"),
                  complete=False, healthy=False, samples=[])
    evidence = output / "evidence.json"
    try:
        with tempfile.TemporaryDirectory(prefix=namespace + "-") as temporary:
            context = pathlib.Path(temporary)
            (context / "Dockerfile").write_text(probe_recipe((root / "backend-school/Dockerfile").read_text()))
            (context / "rustc-cache").write_text('#!/bin/sh\nexec /usr/local/bin/sccache "$@"\n')
            fixture = context / "fixture"
            (fixture / "src").mkdir(parents=True)
            (fixture / "Cargo.toml").write_text('[package]\nname = "compiler-cache-probe"\nversion = "0.0.0"\nedition = "2024"\n')
            (fixture / "Cargo.lock").write_text('version = 4\n\n[[package]]\nname = "compiler-cache-probe"\nversion = "0.0.0"\n')
            (fixture / "src/lib.rs").write_text(f'pub const PROBE_ID: &str = "{namespace}";\npub fn sum(a: u64, b: u64) -> u64 {{ a + b }}\n')
            command(["docker", "build", "--platform", "linux/amd64", "--tag", image, str(context)], "pinned_image")
            for index in (1, 2):
                container = f"{namespace}-{index}"
                sample = dict(sample=index, counters=None, diagnostics=[])
                try:
                    command(["docker", "run", "--detach", "--name", container,
                             "--tmpfs", "/private:rw,nosuid,nodev,noexec,mode=0700,size=16m", "--tmpfs", "/probe:rw,nosuid,nodev",
                             "-e", "ACTIONS_RESULTS_URL", "-e", "ACTIONS_RUNTIME_TOKEN",
                             "-e", "ACTIONS_CACHE_SERVICE_V2=true", "-e", "SCCACHE_GHA_ENABLED=on",
                             "-e", "SCCACHE_GHA_VERSION=schoolorbit-backend-school",
                             "-e", "SCCACHE_LOG=sccache::server=debug", "-e", "SCCACHE_ERROR_LOG=/private/server.log",
                             "-e", "RUSTC_WRAPPER=/usr/local/bin/rustc-cache", "-e", "CARGO_INCREMENTAL=0",
                             "--entrypoint", "sleep", image, "infinity"], "fresh_container")
                    command(["docker", "cp", f"{fixture}/.", f"{container}:/probe"], "probe_context")
                    started = time.monotonic()
                    command(["docker", "exec", container, "cargo", "build", "--release", "--lib", "--locked", "--offline"], "compile")
                    # Cache writes finish asynchronously. Wait for an observed write, error or hit.
                    values = stats(container)
                    deadline = time.monotonic() + 60
                    while not (values["cache_hits"] or values["cache_writes"] or values["cache_write_errors"]) and time.monotonic() < deadline:
                        time.sleep(0.2)
                        values = stats(container)
                    sample.update(duration_seconds=time.monotonic() - started, counters=values)
                except ProbeFailure as error:
                    sample["failure"] = dict(stage=error.stage, exit_code=error.code)
                    raise
                finally:
                    private = subprocess.run(["docker", "exec", container, "cat", "/private/server.log"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                    sample["diagnostics"] = classify_private_log(private.stdout[-8 * 1024 * 1024:])
                    result["samples"].append(sample)
                    evidence.write_text(json.dumps(result, indent=2) + "\n")
                    subprocess.run(["docker", "rm", "--force", container], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            first, second = [x["counters"] for x in result["samples"]]
            errors = ("cache_errors", "cache_read_errors", "cache_write_errors", "cache_timeouts")
            result["healthy"] = (first["cache_misses"] == first["cache_writes"] == 1 and first["cache_hits"] == 0
                                  and second["cache_hits"] == 1 and second["cache_misses"] == second["cache_writes"] == 0
                                  and all(v[key] == 0 for v in (first, second) for key in errors))
            result["complete"] = True
    except ProbeFailure as error:
        result["failure"] = dict(stage=error.stage, exit_code=error.code)
    except (ValueError, OSError) as error:
        result["failure"] = dict(stage="invalid_evidence", category=type(error).__name__)
    finally:
        subprocess.run(["docker", "image", "rm", image], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        evidence.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(result, indent=2))
    if not result["healthy"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
