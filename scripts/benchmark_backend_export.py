#!/usr/bin/env python3
"""Full-cost, isolated runtime/max versus dependency/min BuildKit experiment."""
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import time

from benchmark_backend_cache import Benchmark, context, context_digest, network_bytes, run
from benchmark_cargo_snapshot import Trial, image_digest, resource_checkpoint, save
import benchmark_cargo_snapshot as snapshot_resources
from render_backend_build_summary import buildkit_phases

VARIANTS = ('runtime-max', 'dependencies-min')


def cache_identity(reference):
    description = run(['docker', 'buildx', 'imagetools', 'inspect', reference])
    matches = re.findall(r'^Digest:\s*(sha256:[0-9a-f]{64})\s*$', description, re.M)
    if len(matches) != 1:
        raise ValueError('Immutable cache digest unavailable')
    digest = matches[0]
    pinned = reference.rsplit(':', 1)[0] + '@' + digest
    manifest = json.loads(run(['docker', 'buildx', 'imagetools', 'inspect', pinned, '--raw']))
    layers = manifest.get('layers')
    if not isinstance(layers, list) or not layers or any(type(layer.get('size')) is not int or layer['size'] < 0 for layer in layers):
        raise ValueError('Cache layer size evidence unavailable')
    return {'reference': pinned, 'layerBytes': sum(layer['size'] for layer in layers), 'layers': len(layers)}


def build_commands(base, source, package, label, variant, imported=None):
    if variant not in VARIANTS or imported is not None and not re.fullmatch(re.escape(package) + r'@sha256:[0-9a-f]{64}', imported):
        raise ValueError('Invalid isolated exporter variant or import')
    args = base + (['--cache-from', 'type=registry,ref=' + imported] if imported else [])
    cache = package + ':' + label + '-cache'
    export = 'type=registry,ref=' + cache
    common = ',image-manifest=true,oci-mediatypes=true'
    runtime = ['--target', 'runtime', '--push', '--tag', package + ':' + label,
               '--metadata-file', str(source.parent.parent / label / 'runtime-metadata.json')]
    if variant == 'runtime-max':
        commands = [('runtime', args + runtime + ['--cache-to', export + ',mode=max' + common, str(source)])]
    else:
        # min retains compiler-tools + recipe/chef parent chain, excludes planner
        # COPY and changing application layers. Both solves/exports are charged.
        commands = [('dependencies', args + ['--target', 'dependencies', '--output', 'type=cacheonly',
                     '--cache-to', export + ',mode=min' + common, str(source)]),
                    ('runtime', args + runtime + [str(source)])]
    return commands, cache


class ExportTrial(Trial):
    def runtime(self, reference):
        run(['docker', 'pull', reference])
        self.local_images.append(reference)
        try:
            config = json.loads(run(['docker', 'image', 'inspect', reference]))[0]['Config']
            uid = run(['docker', 'run', '--rm', '--entrypoint', 'id', reference, '-u'])
            binary = run(['docker', 'run', '--rm', '--entrypoint', 'sha256sum', reference, '/app/backend-school']).split()[0]
            migrations = run(['docker', 'run', '--rm', '--entrypoint', 'bash', reference, '-euc',
                              'find /app/migrations -type f -print0 | sort -z | xargs -0 sha256sum'])
            if config['User'] != 'appuser' or config['Cmd'] != ['/app/backend-school'] or uid != '1000' or not re.fullmatch('[0-9a-f]{64}', binary) or not migrations:
                raise ValueError('Runtime binary/configuration/migrations evidence invalid')
            return {'uid': 1000, 'binarySha256': binary,
                    'migrationsSha256': hashlib.sha256(migrations.encode()).hexdigest(),
                    'config': {key: config.get(key) for key in ('User', 'Cmd', 'Entrypoint', 'WorkingDir', 'Env', 'ExposedPorts')}}
        finally:
            run(['docker', 'image', 'rm', reference])
            self.local_images.remove(reference)

    def sample(self, source, source_hash, variant, scenario, sample, imported=None):
        label = f'{scenario}-{sample}-{variant}'
        folder = self.output / label
        builder, bootstrap, fixture = self.builder(label)
        before = network_bytes(builder)
        started = time.monotonic()
        commands, cache = build_commands(self.base(builder), source, self.package, label, variant, imported)
        for phase, command in commands:
            run(command, folder / (phase + '-build.log'))
        exported = cache_identity(cache)
        elapsed = time.monotonic() - started + bootstrap
        after = network_bytes(builder)
        runtime = self.runtime(self.package + '@' + image_digest(folder / 'runtime-metadata.json'))
        compiler = self.evidence(builder, source, folder, [])
        if scenario == 'source' and not any('backend-school' in name for name in compiler['cargo']['dirtyLocalPackages']):
            raise ValueError('Source case failed to compile the application')
        phases = {phase: buildkit_phases((folder / (phase + '-build.log')).read_text()) for phase, _ in commands}
        export_phase = 'runtime' if variant == 'runtime-max' else 'dependencies'
        cache_export = phases[export_phase].get('cache export', {}).get('duration')
        if type(cache_export) not in (int, float):
            cache_export = None
        # Probe actual Cargo after each build, rather than interpreting old timing
        # files in a reused layer as fresh execution or successful unchanged work.
        unchanged_started = time.monotonic()
        run(self.base(builder) + ['--target', 'cargo-freshness-evidence', '--build-arg',
            'SCHOOL_FRESHNESS_PROBE=export-' + label, '--output', f'type=local,dest={folder / "unchanged"}', str(source)], folder / 'unchanged.log')
        artifacts = [json.loads(line) for line in (folder / 'unchanged/cargo-artifacts.jsonl').read_text().splitlines()]
        artifacts = [item for item in artifacts if item.get('reason') == 'compiler-artifact']
        if not artifacts or not all(item.get('fresh') is True for item in artifacts):
            raise ValueError('Unchanged Cargo probe compiled or lacked evidence')
        unchanged_seconds = time.monotonic() - unchanged_started
        api = self.api(builder, source, folder, [])  # Retires builder before offline API container.
        if context_digest(self.compiler_snapshot) != self.compiler_hash:
            raise ValueError('Frozen compiler snapshot changed')
        record = {'variant': variant, 'scenario': scenario, 'sample': sample, 'seconds': elapsed,
                  'sourceContextSha256': source_hash, 'compilerSnapshotSha256': self.compiler_hash,
                  'import': imported, 'export': exported, 'runtime': runtime, 'api': api,
                  'compiler': compiler, 'phases': phases, 'freshBuilder': True,
                  'cacheExportSeconds': cache_export,
                  'bootstrapSeconds': bootstrap, 'fixturePrimingSeconds': fixture,
                  'unchangedDiagnosticSeconds': unchanged_seconds, 'unchangedArtifacts': len(artifacts),
                  'networkReceivedBytes': after[0] - before[0], 'networkSentBytes': after[1] - before[1]}
        save(folder / 'result.json', record)
        return record


def compare_pair(pair):
    if len(pair) != 2 or {row['variant'] for row in pair} != set(VARIANTS):
        raise ValueError('Incomplete exporter pair')
    for key in ('sourceContextSha256', 'compilerSnapshotSha256', 'runtime'):
        if pair[0][key] != pair[1][key]:
            raise ValueError('Paired source/compiler/runtime equivalence failed')
    if pair[0]['api']['canonicalSha256'] != pair[1]['api']['canonicalSha256']:
        raise ValueError('Paired offline API equivalence failed')
    for key in ('registryIdentitySha256', 'registryArtifactCount'):
        if pair[0]['compiler']['cargo'][key] != pair[1]['compiler']['cargo'][key]:
            raise ValueError('Paired compiler profile/features changed')


def report(records):
    lines = ['### BuildKit exporter topology experiment', '',
             'Fresh pinned CPU4 builders; identical frozen local compiler cache; unchanged production release flags.',
             'Charged: builder bootstrap, all dependency/runtime solves, registry image push, cache export and digest/size lookup.',
             'Separate diagnostic costs: compiler seeding/priming, evidence, unchanged Cargo and offline API verification. Checkout/login/common runner setup are excluded.',
             'One seed producer per variant, ordered runtime-max then dependencies-min; all warm transitions reuse those producer measurements, not four independent producer pairs.',
             'Network counters include all builder external traffic; layer sizes are registry manifest bytes, not incremental upload bytes.', '',
             '| Case | Variant | Samples | Mean build/export | Mean cache export | Mean producer + consumer |',
             '| --- | --- | ---: | ---: | ---: | ---: |']
    for scenario in ('seed', 'source', 'workspace'):
        for variant in VARIANTS:
            rows = [row for row in records if row['scenario'] == scenario and row['variant'] == variant]
            if rows:
                transition = f"{statistics.mean(row['transitionSeconds'] for row in rows):.2f}s" if scenario != 'seed' else 'separate seed'
                exports = [row.get('cacheExportSeconds') for row in rows]
                exported = f"{statistics.mean(exports):.2f}s" if all(type(value) in (int, float) for value in exports) else 'unavailable'
                lines.append(f"| {scenario} | {variant} | {len(rows)} | {statistics.mean(row['seconds'] for row in rows):.2f}s | {exported} | {transition} |")
    lines += ['', 'Seed is cold BuildKit with an already seeded compiler fixture, not an empty compiler target/cache baseline. Workspace cases are first recipe changes from the pinned seed.',
              'Production cache remains unchanged; export-only savings do not establish net gain.', '']
    return '\n'.join(lines)


def main():
    samples = os.environ.get('BENCH_SAMPLES', '2')
    run_id, attempt = os.environ.get('GITHUB_RUN_ID', ''), os.environ.get('GITHUB_RUN_ATTEMPT', '')
    pinned = os.environ.get('BENCH_BUILDKIT_IMAGE', '')
    owner = os.environ.get('GITHUB_REPOSITORY_OWNER', '').lower()
    if samples not in ('2', '3') or not re.fullmatch('[1-9][0-9]*', run_id) or not re.fullmatch('[1-9][0-9]*', attempt) or not re.fullmatch('[a-z0-9][a-z0-9-]{0,38}', owner):
        raise ValueError('Invalid bounded exporter experiment inputs')
    if os.cpu_count() != 4 or not re.fullmatch(r'[^\s]+@sha256:[a-f0-9]{64}', pinned):
        raise ValueError('Hosted CPU4 and pinned BuildKit are required')
    root = Path(__file__).resolve().parents[1]
    sha = run(['git', '-C', str(root), 'rev-parse', 'HEAD'])
    if os.environ.get('GITHUB_SHA') != sha or run(['git', '-C', str(root), 'status', '--porcelain', '--untracked-files=no']):
        raise ValueError('Exporter experiment requires the exact clean committed checkout')
    output = Path(os.environ['BENCH_OUTPUT']).resolve()
    output.mkdir(parents=True, exist_ok=True)
    snapshot_resources.DOCKER_ROOT = Path(run(['docker', 'info', '--format', '{{.DockerRootDir}}']))
    resource_checkpoint(output, 'initial')
    namespace = f'cache-benchmark-{run_id}-{attempt}'
    package = f'ghcr.io/{owner}/schoolorbit-{namespace}'
    seed = Benchmark(root, output, namespace, pinned)
    source = output / 'contexts/compiler-seed'
    digest = context(root, source, 'seed', 0, readonly=False)
    save(output / 'progress.json', {'complete': False, 'phase': 'compiler-seed'})
    started = time.monotonic()
    seed.build(source, digest, 'registry', 'seed', 0, compiler_seed=True)
    environment = {'sourceSha': sha, 'cpus': os.cpu_count(), 'buildkitImage': pinned, 'registryPackage': package,
                   'compilerSeedSeconds': time.monotonic() - started, 'compilerSnapshotSha256': seed.snapshot_digest,
                   'compilerSnapshotBytes': sum(file.stat().st_size for file in seed.snapshot.rglob('*') if file.is_file()),
                   'productionCachePolicy': 'unchanged', 'cargoTargetSnapshots': 'OFF'}
    save(output / 'environment.json', environment)
    trial = ExportTrial(root, output, pinned, package, seed.snapshot)
    records, producers = [], {}
    try:
        source = output / 'contexts/seed-0'
        digest = context(root, source, 'seed', 0)
        pair = []
        for variant in VARIANTS:
            save(output / 'progress.json', {'complete': False, 'phase': 'seed', 'variant': variant})
            print(f'exporter_experiment phase=seed variant={variant}', flush=True)
            record = trial.sample(source, digest, variant, 'seed', 0)
            records.append(record); pair.append(record); producers[variant] = record
            save(output / 'results.json', records)
        compare_pair(pair)
        index = 0
        for scenario in ('source', 'workspace'):
            for sample in range(1, int(samples) + 1):
                index += 1
                source = output / f'contexts/{scenario}-{sample}'
                digest = context(root, source, scenario, sample)
                order = VARIANTS if index % 2 == 0 else tuple(reversed(VARIANTS))
                pair = []
                for variant in order:
                    save(output / 'progress.json', {'complete': False, 'phase': scenario, 'sample': sample, 'variant': variant})
                    print(f'exporter_experiment phase={scenario} sample={sample} variant={variant}', flush=True)
                    producer = producers[variant]
                    record = trial.sample(source, digest, variant, scenario, sample, producer['export']['reference'])
                    record['producerSeconds'] = producer['seconds']
                    record['transitionSeconds'] = producer['seconds'] + record['seconds']
                    records.append(record); pair.append(record)
                    save(output / 'results.json', records)
                compare_pair(pair)
        gains = [next(row['transitionSeconds'] for row in records if row['scenario'] == scenario and row['sample'] == sample and row['variant'] == 'runtime-max') -
                 next(row['transitionSeconds'] for row in records if row['scenario'] == scenario and row['sample'] == sample and row['variant'] == 'dependencies-min')
                 for scenario in ('source', 'workspace') for sample in range(1, int(samples) + 1)]
        save(output / 'complete.json', {'complete': True, 'pairedNetSeconds': gains, 'meanNetSeconds': statistics.mean(gains),
                                      'allPairsNetPositive': all(gain > 0 for gain in gains), 'producerSamplesPerVariant': 1,
                                      'producerOrder': list(VARIANTS), 'consumerPairs': len(gains),
                                      'producerCostsReusedAcrossConsumerPairs': True,
                                      'productionActivation': 'unchanged; requires robust gain across producer/consumer variance or a bounded counterbalanced producer repeat'})
        save(output / 'progress.json', {'complete': True, 'phase': 'complete'})
        (output / 'summary.txt').write_text(report(records))
        if os.environ.get('GITHUB_STEP_SUMMARY'):
            with open(os.environ['GITHUB_STEP_SUMMARY'], 'a') as summary:
                summary.write(report(records))
    finally:
        trial.cleanup()


if __name__ == '__main__':
    main()
