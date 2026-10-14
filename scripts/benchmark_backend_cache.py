#!/usr/bin/env python3
"""Compare BuildKit remote cache backends without changing production artifacts."""
import hashlib
import io
import json
import os
import pathlib
import re
import statistics
import subprocess
import tarfile
import time

from render_backend_build_summary import buildkit_phases, counter, load_json, number


def run(args, log=None):
    if log is None:
        return subprocess.check_output(args, text=True).strip()
    started = time.monotonic()
    with log.open("w") as output:
        process = subprocess.Popen(args, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        for line in process.stdout:
            output.write(line)
            print(line, end="", flush=True)
        code = process.wait()
    if code:
        raise subprocess.CalledProcessError(code, args)
    return time.monotonic() - started


def context_digest(folder):
    digest = hashlib.sha256()
    for file in sorted(folder.rglob("*")):
        if file.is_file():
            digest.update(file.relative_to(folder).as_posix().encode() + b"\0")
            digest.update(file.read_bytes())
    return digest.hexdigest()


def context(root, destination, scenario, sample, readonly=True):
    """Apply identical, disposable source/recipe changes to both cache backends."""
    destination.mkdir(parents=True)
    archive = subprocess.check_output(["git", "-C", str(root), "archive", "HEAD:backend-school"])
    with tarfile.open(fileobj=io.BytesIO(archive)) as source:
        source.extractall(destination, filter="data")
    dockerfile = destination / "Dockerfile"
    text = dockerfile.read_text()
    if readonly:
        marker = "FROM base AS builder\n"
        if text.count(marker) != 1:
            raise ValueError("School builder owner changed")
        mount = "--mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache"
        if text.count(mount) != 2:
            raise ValueError("School compiler cache mounts changed")
        text = text.replace(marker, marker + "ENV SCCACHE_LOCAL_RW_MODE=READ_ONLY\n")
        # sccache's disk LRU updates entry timestamps even in READ_ONLY mode.
        # Keep the copied mount writable for metadata; artifact writes stay disabled.
    else:
        # Export only the owned compiler cache mount after the untimed seed build.
        text += ("\nFROM builder AS compiler-cache-capture\n"
                 "RUN --mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache "
                 "mkdir /compiler-cache-seed && cp -a /var/cache/sccache/. /compiler-cache-seed/\n"
                 "FROM scratch AS compiler-cache-export\n"
                 "COPY --from=compiler-cache-capture /compiler-cache-seed/ /\n")
    dockerfile.write_text(text)
    if scenario == "source":
        with (destination / "src/main.rs").open("a") as file:
            file.write(f"\n// BuildKit cache benchmark source sample {sample}\n")
    elif scenario == "workspace":
        name = f"cache-benchmark-fixture-{sample}"
        crate = destination / "crates" / name
        (crate / "src").mkdir(parents=True)
        (crate / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion.workspace = true\nedition.workspace = true\n'
            '[lints]\nworkspace = true\n'
        )
        (crate / "src/lib.rs").write_text("pub fn fixture() -> u8 { 1 }\n")
        manifest = destination / "Cargo.toml"
        text = manifest.read_text()
        marker = "members = [\n"
        if text.count(marker) != 1:
            raise ValueError("Workspace membership owner changed")
        manifest.write_text(text.replace(marker, marker + f'    "crates/{name}",\n'))
        # A dependency-free local member needs no registry resolution or version updates.
        version = re.search(r'\[workspace.package\]\s*version\s*=\s*"([^"]+)"', text)
        if not version:
            raise ValueError("Workspace package version is unavailable")
        with (destination / "Cargo.lock").open("a") as file:
            file.write(f'\n[[package]]\nname = "{name}"\nversion = "{version.group(1)}"\n')
    elif scenario != "seed":
        raise ValueError("Unsupported benchmark scenario")
    return context_digest(destination)


def network_bytes(builder):
    data = run(["docker", "exec", f"buildx_buildkit_{builder}0", "cat", "/proc/net/dev"])
    received = sent = 0
    for line in data.splitlines():
        if ":" not in line:
            continue
        interface, values = line.split(":", 1)
        if interface.strip() == "lo":
            continue
        values = values.split()
        received += int(values[0])
        sent += int(values[8])
    return received, sent


def compiler_evidence(folder, readonly=True):
    result = {}
    for phase, prefix in [("dependencies", "dependency-"), ("application", "")]:
        metrics = load_json(folder / f"{prefix}build-metrics.json")
        raw = load_json(folder / f"{prefix}sccache-stats.json")
        stats = raw.get("stats", raw)
        if metrics.get("cacheEnabled") is not True or not isinstance(stats, dict):
            raise ValueError("Required compiler cache evidence is unavailable")
        values = {key: counter(stats, key) for key in (
            "cache_hits", "cache_misses", "cache_errors", "cache_read_errors", "cache_write_errors", "cache_timeouts"
        )}
        if any(value is None for value in values.values()):
            raise ValueError("Required compiler cache counters are unavailable")
        if any(values[key] for key in ("cache_errors", "cache_read_errors", "cache_write_errors", "cache_timeouts")):
            raise ValueError("Compiler cache errors invalidate this comparison")
        writes = counter(stats, "cache_writes")
        if writes is None or (readonly and writes != 0):
            raise ValueError("Read-only compiler cache write evidence is invalid")
        elapsed = number(metrics.get("compileNanoseconds"))
        if elapsed is None:
            raise ValueError("Cargo duration is unavailable")
        result[phase] = dict(values, cache_writes=writes, cargo_seconds=elapsed / 1e9)
    return result


def report(records):
    lines = ["### BuildKit remote cache comparison", "",
             "Same runner, fresh pinned BuildKit builders, production release flags; identical local compiler snapshots with artifact writes disabled and writable LRU metadata.",
             "The compiler snapshot isolates BuildKit storage performance from GHA compiler-cache service failures; production compiler caching is unchanged.",
             "Image export/load is included. Image push, evidence export, API checks, builder bootstrap and compiler snapshot priming are excluded from build wall time.",
             "Network bytes cover the builder's total external traffic, not cache traffic alone. Phase durations can overlap.", "",
             "| Case | Backend | Samples | Mean build wall | Mean cache export | Mean received / sent |",
             "| --- | --- | ---: | ---: | ---: | ---: |"]
    for scenario in ("seed", "source", "workspace"):
        for backend in ("gha", "registry"):
            selected = [r for r in records if r["scenario"] == scenario and r["backend"] == backend]
            if not selected:
                continue
            wall = statistics.mean(r["wall_seconds"] for r in selected)
            exports = [r["phases"].get("cache export", {}).get("duration") for r in selected]
            export = f"{statistics.mean(exports):.2f}s" if all(number(v) is not None for v in exports) else "unavailable"
            rx = statistics.mean(r["network_received_bytes"] for r in selected) / 1024 ** 2
            tx = statistics.mean(r["network_sent_bytes"] for r in selected) / 1024 ** 2
            lines.append(f"| {scenario} | {backend} | {len(selected)} | {wall:.2f}s | {export} | {rx:.1f} / {tx:.1f} MiB |")
    lines += ["", "Seed costs are separate from warm means. Compare full build wall time and dependent-layer reuse, not export alone.", ""]
    return "\n".join(lines)


class Benchmark:
    def __init__(self, root, output, namespace, buildkit_image):
        self.root, self.output, self.namespace, self.buildkit_image = root, output, namespace, buildkit_image
        self.sequence = 0
        self.snapshot = output / "compiler-cache-snapshot"
        self.snapshot_digest = None
        self.prime_source = output / "contexts/compiler-cache-prime"
        self.prime_source.mkdir(parents=True)
        runtime = re.search(r"^FROM (\S+@sha256:[a-f0-9]{64}) AS runtime$",
                            (root / "backend-school/Dockerfile").read_text(), re.M)
        if not runtime:
            raise ValueError("A pinned School runtime base is required")
        # Prime cache mounts in a separate untimed graph. Compiler snapshot bytes
        # must never become extra layers in either measured BuildKit exporter.
        (self.prime_source / "Dockerfile").write_text(
            f"FROM {runtime.group(1)} AS compiler-cache-prime\n"
            "RUN --mount=type=bind,from=compiler-cache,target=/compiler-cache,ro "
            "--mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache "
            "cp -a /compiler-cache/. /var/cache/sccache/\n")
        self.reference = json.loads((root / "contracts/openapi/school-api.json").read_text())

    def build(self, source, digest, backend, scenario, sample, cache_import=True, compiler_seed=False):
        self.sequence += 1
        builder = f"{self.namespace}-{self.sequence}"
        image = f"schoolorbit-cache-benchmark:{self.sequence}"
        folder = self.output / ("compiler-seed" if compiler_seed else f"{scenario}-{sample}-{backend}")
        folder.mkdir(parents=True)
        ref = f"ghcr.io/{os.environ['GITHUB_REPOSITORY_OWNER'].lower()}/schoolorbit-{self.namespace}:buildcache"
        if backend == "gha":
            cache = f"type=gha,scope={self.namespace},version=2"
        else:
            cache = f"type=registry,ref={ref}"
        exporter = cache + ",mode=max" + (",image-manifest=true,oci-mediatypes=true" if backend == "registry" else "")
        base = ["docker", "buildx", "build", "--builder", builder, "--platform", "linux/amd64",
                "--progress", "plain", "--build-arg", "SCHOOL_COMPILER_CACHE=local"]
        if not compiler_seed:
            if not self.snapshot_digest or context_digest(self.snapshot) != self.snapshot_digest:
                raise ValueError("The frozen compiler snapshot is unavailable or changed")
        try:
            run(["docker", "buildx", "create", "--name", builder, "--driver", "docker-container",
                 "--driver-opt", f"image={self.buildkit_image}", "--bootstrap"], folder / "bootstrap.log")
            priming = 0
            if not compiler_seed:
                priming = run(base + ["--target", "compiler-cache-prime", "--build-context", f"compiler-cache={self.snapshot}",
                                     "--output", "type=cacheonly", str(self.prime_source)], folder / "compiler-prime.log")
            before = network_bytes(builder)
            args = base + ["--target", "runtime", "--load", "--tag", image,
                           "--metadata-file", str(folder / "metadata.json")]
            if not compiler_seed:
                if cache_import:
                    args += ["--cache-from", cache]
                args += ["--cache-to", exporter]
            wall = run(args + [str(source)], folder / "build.log")
            after = network_bytes(builder)
            run(base + ["--target", "build-timings", "--output", f"type=local,dest={folder / 'timings'}", str(source)],
                folder / "evidence.log")
            api = run(["docker", "run", "--rm", "--entrypoint", "env", image, "-i",
                       "PATH=/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin", "/app/backend-school", "export-openapi"])
            if json.loads(api) != self.reference:
                raise ValueError("Runtime OpenAPI differs from the canonical contract")
            uid = run(["docker", "run", "--rm", "--entrypoint", "id", image, "-u"])
            config = json.loads(run(["docker", "image", "inspect", image]))[0]
            if uid != "1000" or config["Config"]["Cmd"] != ["/app/backend-school"]:
                raise ValueError("Runtime image configuration changed")
            compiler = compiler_evidence(folder / "timings", readonly=not compiler_seed)
            if compiler_seed:
                run(base + ["--target", "compiler-cache-export", "--output", f"type=local,dest={self.snapshot}", str(source)],
                    folder / "snapshot.log")
                if not any(file.is_file() for file in self.snapshot.rglob("*")):
                    raise ValueError("The compiler snapshot is empty")
                self.snapshot_digest = context_digest(self.snapshot)
            elif context_digest(self.snapshot) != self.snapshot_digest:
                raise ValueError("The frozen compiler snapshot changed during measurement")
            stored_bytes = None
            if backend == "registry" and not compiler_seed:
                manifest = json.loads(run(["docker", "buildx", "imagetools", "inspect", ref, "--raw"]))
                stored_bytes = sum(layer["size"] for layer in manifest["layers"])
            record = dict(backend=backend, scenario=scenario, sample=sample, context_sha256=digest,
                          wall_seconds=wall, network_received_bytes=after[0] - before[0],
                          network_sent_bytes=after[1] - before[1], image_id=config["Id"],
                          runtime_uid=1000, openapi_equal=True, registry_cache_layer_bytes=stored_bytes,
                          compiler_snapshot_sha256=self.snapshot_digest,
                          compiler_priming_seconds=priming,
                          phases=buildkit_phases((folder / "build.log").read_text()),
                          compiler=compiler)
            (folder / "result.json").write_text(json.dumps(record, indent=2) + "\n")
            return record
        finally:
            # Remove only this sample's owned builder and local image, including failure paths.
            subprocess.run(["docker", "buildx", "rm", "--force", builder], check=False)
            subprocess.run(["docker", "image", "rm", image], check=False)


def main():
    samples = os.environ.get("BENCH_SAMPLES", "2")
    run_id = os.environ.get("GITHUB_RUN_ID", "")
    attempt = os.environ.get("GITHUB_RUN_ATTEMPT", "")
    pinned = os.environ.get("BENCH_BUILDKIT_IMAGE", "")
    if samples not in ("2", "3") or not re.fullmatch(r"[0-9]+", run_id) or not re.fullmatch(r"[0-9]+", attempt):
        raise ValueError("Invalid bounded benchmark inputs")
    if not re.fullmatch(r"[^\s]+@sha256:[a-f0-9]{64}", pinned):
        raise ValueError("A pinned BuildKit image is required")
    for key in ("ACTIONS_RUNTIME_TOKEN", "ACTIONS_RESULTS_URL", "GITHUB_REPOSITORY_OWNER"):
        if not os.environ.get(key):
            raise ValueError("Required benchmark runtime is unavailable")
    root = pathlib.Path(__file__).resolve().parents[1]
    output = pathlib.Path(os.environ["BENCH_OUTPUT"]).resolve()
    output.mkdir(parents=True, exist_ok=True)
    namespace = f"cache-benchmark-{run_id}-{attempt}"
    bench = Benchmark(root, output, namespace, pinned)
    environment = dict(
        source_sha=run(["git", "-C", str(root), "rev-parse", "HEAD"]),
        buildkit_image=pinned, namespace=namespace, samples_per_warm_case=int(samples),
        compiler_cache="identical local snapshot seeded once; artifact writes disabled, LRU timestamps writable; production compiler cache unchanged",
        registry_cache_package=f"schoolorbit-{namespace}",
        runner=run(["uname", "-m"]), cpus=os.cpu_count(),
        docker=run(["docker", "version", "--format", "{{.Server.Version}}"]),
    )
    (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    source = output / "contexts/compiler-seed"
    digest = context(root, source, "seed", 0, readonly=False)
    bench.build(source, digest, "gha", "seed", 0, compiler_seed=True)
    environment["compiler_snapshot_sha256"] = bench.snapshot_digest
    environment["compiler_snapshot_bytes"] = sum(file.stat().st_size for file in bench.snapshot.rglob("*") if file.is_file())
    (output / "environment.json").write_text(json.dumps(environment, indent=2) + "\n")
    records = []
    cases = [("seed", 0)] + [(scenario, sample) for scenario in ("source", "workspace") for sample in range(1, int(samples) + 1)]
    for index, (scenario, sample) in enumerate(cases):
        source = output / f"contexts/{scenario}-{sample}"
        digest = context(root, source, scenario, sample)
        # Reverse each pair to reduce systematic ordering advantage on the same runner.
        order = ("gha", "registry") if index % 2 == 0 else ("registry", "gha")
        for backend in order:
            records.append(bench.build(source, digest, backend, scenario, sample, cache_import=scenario != "seed"))
            (output / "results.json").write_text(json.dumps(records, indent=2) + "\n")
            rendered = report(records)
            (output / "summary.txt").write_text(rendered)
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    (output / "complete.json").write_text(json.dumps({"complete": True, "samples": len(records)}) + "\n")
    if summary_path:
        with pathlib.Path(summary_path).open("a") as file:
            file.write(report(records))


if __name__ == "__main__":
    main()
