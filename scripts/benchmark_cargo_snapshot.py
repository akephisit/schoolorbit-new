#!/usr/bin/env python3
"""Two-run Cargo snapshot experiment. Scratch packages never become release artifacts."""
import hashlib
import json
import math
import os
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import time
from datetime import datetime, timedelta, timezone

from benchmark_backend_cache import Benchmark, context, context_digest, network_bytes, run, compiler_evidence

DOCKER_ROOT = None


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def resource_checkpoint(output, label, minimum_gib=8):
    paths = [output] + ([DOCKER_ROOT] if DOCKER_ROOT is not None else [])
    filesystems = [{'path': str(path), 'freeBytes': os.statvfs(path).f_bavail * os.statvfs(path).f_frsize} for path in paths]
    free = min(item['freeBytes'] for item in filesystems)
    path = output / 'resource-checkpoints.json'
    records = json.loads(path.read_text()) if path.exists() else []
    records.append({'label': label, 'freeBytes': free, 'minimumBytes': minimum_gib * 1024**3,
                    'timeNs': time.time_ns(), 'filesystems': filesystems, 'scope': 'runner temporary/workspace and Docker filesystem'})
    save(path, records)
    if free < minimum_gib * 1024**3:
        raise ValueError('Diagnostic disk reserve reached; no global/shared pruning permitted')


def required_seconds(values, key):
    value = values.get(key)
    if type(value) not in {float, int} or not math.isfinite(value) or value < 0:
        raise ValueError('Required timing evidence unavailable: ' + key)
    return value


def validate_costs(value):
    for key in ('commonProducerSeconds', 'snapshotExportSeconds', 'compilerFixtureExportSeconds',
                'producerBuilderRemovalSeconds', 'frozenCompilerSeedSeconds', 'fixturePrimingSeconds'):
        required_seconds(value.get('cost', {}), key)
    for key in ('producerSnapshotOnlySeconds', 'consumerResolveSeconds'):
        required_seconds(value.get('orchestration', {}), key)


def image_digest(metadata):
    value = json.loads(metadata.read_text()).get('containerimage.digest')
    if not isinstance(value, str) or not re.fullmatch(r'sha256:[0-9a-f]{64}', value):
        raise ValueError('Immutable image digest is unavailable')
    return value


class Trial:
    def __init__(self, root, output, buildkit_image, package, compiler_snapshot):
        self.root, self.output, self.buildkit_image = root, output, buildkit_image
        self.package, self.compiler_snapshot = package, compiler_snapshot
        self.sequence = 0
        self.builders = []
        self.local_images = []
        self.compiler_hash = context_digest(compiler_snapshot) if compiler_snapshot.exists() else None
        self.reference = json.loads((root / 'contracts/openapi/school-api.json').read_text())
        self.prime = output / 'contexts/compiler-prime'
        self.prime.mkdir(parents=True)
        runtime = re.search(r'^FROM (\S+@sha256:[a-f0-9]{64}) AS runtime$', (root / 'backend-school/Dockerfile').read_text(), re.M)
        if not runtime:
            raise ValueError('Pinned runtime base required')
        (self.prime / 'Dockerfile').write_text(
            f'FROM {runtime.group(1)} AS compiler-cache-prime\n'
            'RUN --mount=type=bind,from=compiler-cache,target=/compiler-cache,ro '
            '--mount=type=cache,id=backend-school-sccache,target=/var/cache/sccache '
            'cp -a /compiler-cache/. /var/cache/sccache/\n')

    def builder(self, label):
        resource_checkpoint(self.output, label + '-before-builder')
        self.sequence += 1
        name = f'cargo-snapshot-{os.environ["GITHUB_RUN_ID"]}-{os.environ["GITHUB_RUN_ATTEMPT"]}-{self.sequence}'
        folder = self.output / label
        folder.mkdir(parents=True, exist_ok=True)
        started = time.monotonic()
        run(['docker', 'buildx', 'create', '--name', name, '--driver', 'docker-container',
             '--driver-opt', f'image={self.buildkit_image}', '--bootstrap'], folder / 'bootstrap.log')
        self.builders.append(name)
        run(['docker', 'update', '--cpus', '4', f'buildx_buildkit_{name}0'])
        quota = run(['docker', 'exec', f'buildx_buildkit_{name}0', 'cat', '/sys/fs/cgroup/cpu.max'])
        if quota != '400000 100000':
            raise ValueError('Exactly4CPU builder quota required')
        bootstrap = time.monotonic() - started
        if context_digest(self.compiler_snapshot) != self.compiler_hash:
            raise ValueError('Frozen compiler snapshot changed')
        fixture_started = time.monotonic()
        run(self.base(name) + ['--target', 'compiler-cache-prime', '--build-context',
             f'compiler-cache={self.compiler_snapshot}', '--output', 'type=cacheonly', str(self.prime)], folder / 'compiler-prime.log')
        resource_checkpoint(self.output, label + '-after-compiler-prime')
        return name, bootstrap, time.monotonic() - fixture_started

    def base(self, builder):
        return ['docker', 'buildx', 'build', '--builder', builder, '--platform', 'linux/amd64',
                '--progress', 'plain', '--build-arg', 'SCHOOL_COMPILER_CACHE=local']

    def identity(self, source):
        return json.loads(run(['python3', str(source / 'scripts/cargo_snapshot.py'), 'identity', str(source), '--cache-mode', 'local']))

    def evidence(self, builder, source, folder, args):
        run(self.base(builder) + args + ['--target', 'build-timings', '--output', f'type=local,dest={folder / "timings"}', str(source)], folder / 'evidence.log')
        compiler = compiler_evidence(folder / 'timings', readonly=True)
        artifacts = [json.loads(line) for line in (folder / 'timings/cargo-artifacts.jsonl').read_text().splitlines()]
        artifacts = [record for record in artifacts if record.get('reason') == 'compiler-artifact']
        if not artifacts or not all(type(record.get('fresh')) is bool for record in artifacts):
            raise ValueError('Actual Cargo freshness evidence unavailable')
        external = sorted((record['package_id'], tuple(sorted(record.get('features', []))),
                           json.dumps(record['profile'], sort_keys=True))
                          for record in artifacts if record['package_id'].startswith('registry+'))
        if not external:
            raise ValueError('Registry Cargo artifact identity unavailable')
        dirty_local = sorted({record['package_id'] for record in artifacts
                              if not record['package_id'].startswith('registry+') and not record['fresh']})
        compiler['cargo'] = {'registryIdentitySha256': hashlib.sha256(json.dumps(external, separators=(',', ':')).encode()).hexdigest(),
                             'registryArtifactCount': len(external), 'dirtyLocalPackages': dirty_local,
                             'freshCount': sum(record['fresh'] for record in artifacts),
                             'artifactCount': len(artifacts)}
        return compiler

    def runtime(self, reference):
        # Verification/cleanup is outside production-style push timing.
        run(['docker', 'pull', reference])
        self.local_images.append(reference)
        try:
            config = json.loads(run(['docker', 'image', 'inspect', reference]))[0]['Config']
            if config['User'] != 'appuser' or config['Cmd'] != ['/app/backend-school']:
                raise ValueError('Runtime USER/CMD changed')
            uid = run(['docker', 'run', '--rm', '--entrypoint', 'id', reference, '-u'])
            binary = run(['docker', 'run', '--rm', '--entrypoint', 'sha256sum', reference, '/app/backend-school']).split()[0]
            if uid != '1000' or not re.fullmatch(r'[0-9a-f]{64}', binary):
                raise ValueError('Runtime identity unavailable')
            return {'user': config['User'], 'uid': 1000, 'cmd': config['Cmd'], 'binarySha256': binary}
        finally:
            run(['docker', 'image', 'rm', reference])
            self.local_images.remove(reference)

    def api(self, builder, source, folder, args):
        started = time.monotonic()
        image = f'schoolorbit-cargo-snapshot-docs:{os.environ["GITHUB_RUN_ID"]}-{self.sequence}'
        self.local_images.append(image)
        try:
            run(self.base(builder) + args + ['--target', 'builder', '--load', '--tag', image, str(source)], folder / 'documentation-builder.log')
            # All measured/evidence/unchanged work is complete. Avoid retaining a
            # BuildKit filesystem alongside the large offline API container.
            removal_seconds = self.remove_builder(builder)
            resource_checkpoint(self.output, folder.name + '-before-api-container')
            with (folder / 'offline-api.stderr').open('wb') as errors:
                raw = subprocess.check_output(['docker', 'run', '--rm', '--cpus=4', '--entrypoint', 'bash',
                    '-v', f'{self.root / "scripts/export_benchmark_openapi.sh"}:/export_benchmark_openapi.sh:ro',
                    image, '/export_benchmark_openapi.sh'], stderr=errors)
            (folder / 'offline-api.json').write_bytes(raw)
            decoded = json.loads(raw)
            if decoded != self.reference:
                raise ValueError('Fresh offline OpenAPI changed')
            return {'canonicalSha256': hashlib.sha256(json.dumps(decoded, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
                    'rawSha256': hashlib.sha256(raw).hexdigest(), 'rawBytes': len(raw),
                    'builderRemovalSeconds': removal_seconds,
                    'verificationSeconds': time.monotonic() - started}
        finally:
            run(['docker', 'image', 'rm', image])
            self.local_images.remove(image)

    def remove_builder(self, builder):
        started = time.monotonic()
        run(['docker', 'buildx', 'rm', '--force', builder])
        self.builders.remove(builder)
        return time.monotonic() - started

    def cleanup(self):
        for builder in self.builders:
            subprocess.run(['docker', 'buildx', 'rm', '--force', builder], check=False)
        for image in self.local_images:
            subprocess.run(['docker', 'image', 'rm', image], check=False)


def producer(root, output, buildkit_image):
    namespace = f'cache-benchmark-{os.environ["GITHUB_RUN_ID"]}-{os.environ["GITHUB_RUN_ATTEMPT"]}'
    package = f'ghcr.io/{os.environ["GITHUB_REPOSITORY_OWNER"].lower()}/schoolorbit-{namespace}'
    # Existing machinery seeds once; copied mounts may update LRU timestamps, never frozen artifact bytes.
    seed = Benchmark(root, output, namespace, buildkit_image)
    source = output / 'contexts/compiler-seed'
    source_hash = context(root, source, 'seed', 0, readonly=False)
    seed_started = time.monotonic()
    seed.build(source, source_hash, 'registry', 'seed', 0, compiler_seed=True)
    seed_seconds = time.monotonic() - seed_started
    trial = Trial(root, output, buildkit_image, package, seed.snapshot)
    try:
        source = output / 'contexts/producer'
        source_hash = context(root, source, 'seed', 0, readonly=True)
        identity = trial.identity(source)
        builder, bootstrap, fixture = trial.builder('producer')
        folder = output / 'producer'
        before = network_bytes(builder)
        started = time.monotonic()
        runtime_ref = package + ':producer-runtime'
        run(trial.base(builder) + ['--target', 'runtime', '--push', '--tag', runtime_ref,
            '--cache-to', f'type=registry,ref={package}:producer-buildcache,mode=max,image-manifest=true,oci-mediatypes=true',
            '--metadata-file', str(folder / 'runtime-metadata.json'), str(source)], folder / 'runtime-build.log')
        common = time.monotonic() - started
        snapshot_args = ['--build-arg', f'SCHOOL_CARGO_CONFIG_HASH={identity["buildConfigurationHash"]}']
        started = time.monotonic()
        carrier = folder / 'snapshot-data'
        run(trial.base(builder) + snapshot_args + ['--target', 'cargo-snapshot', '--output',
            f'type=local,dest={carrier}', str(source)], folder / 'snapshot-local-export.log')
        (carrier / 'Dockerfile').write_text('FROM scratch\nCOPY snapshot.tar /snapshot.tar\nCOPY snapshot.json /snapshot.json\n')
        run(trial.base(builder) + ['--push', '--tag', package + ':cargo-snapshot',
            '--metadata-file', str(folder / 'snapshot-image.json'), str(carrier)], folder / 'snapshot-export.log')
        image_digest(folder / 'snapshot-image.json')  # Verify immutable push metadata before retiring local transport state.
        metadata = json.loads((folder / 'snapshot-data/snapshot.json').read_text())
        (folder / 'snapshot-data/snapshot.tar').unlink()
        snapshot_seconds = time.monotonic() - started
        resource_checkpoint(output, 'after-snapshot-export-and-push')
        after = network_bytes(builder)
        compiler_export_started = time.monotonic()
        carrier = output / 'contexts/compiler-carrier'
        carrier.mkdir(parents=True)
        (carrier / 'Dockerfile').write_text('FROM scratch\nCOPY --from=compiler-cache / /compiler-cache/\n')
        run(trial.base(builder) + ['--build-context', f'compiler-cache={seed.snapshot}', '--push', '--tag', package + ':compiler-cache',
            '--metadata-file', str(folder / 'compiler-image.json'), str(carrier)], folder / 'compiler-export.log')
        compiler_export_seconds = time.monotonic() - compiler_export_started
        cache_description = run(['docker', 'buildx', 'imagetools', 'inspect', package + ':producer-buildcache'])
        cache_matches = re.findall(r'^Digest:\s*(sha256:[0-9a-f]{64})\s*$', cache_description, re.M)
        if len(cache_matches) != 1:
            raise ValueError('Producer BuildKit cache digest unavailable')
        cache_digest = cache_matches[0]
        runtime = trial.runtime(package + '@' + image_digest(folder / 'runtime-metadata.json'))
        compiler = trial.evidence(builder, source, folder, [])
        api_evidence = trial.api(builder, source, folder, [])
        api_hash = api_evidence['canonicalSha256']
        value = {'schemaVersion': 1, 'producer': {'repository': os.environ['GITHUB_REPOSITORY'],
            'workflowPath': '.github/workflows/operations.yml', 'runId': os.environ['GITHUB_RUN_ID'],
            'runAttempt': int(os.environ['GITHUB_RUN_ATTEMPT']), 'sha': os.environ['GITHUB_SHA'],
            'ref': os.environ['GITHUB_REF'], 'event': 'workflow_dispatch'}, 'registryPackage': package,
            'images': {'cargoSnapshot': image_digest(folder / 'snapshot-image.json'),
                       'compilerCache': image_digest(folder / 'compiler-image.json'), 'buildkitCache': cache_digest},
            'identity': identity, 'sourceManifestHash': metadata['sourceManifestSha256'],
            'compilerSnapshotHash': seed.snapshot_digest, 'sourceContextHash': source_hash, 'buildkitImage': buildkit_image,
            'storage': {'snapshotArchiveBytes': metadata['archiveBytes'],
                        'compilerFixtureBytes': sum(p.stat().st_size for p in seed.snapshot.rglob('*') if p.is_file()),
                        'snapshotTarCopiesDuringExport': 3, 'apiDebugOutputForecastBytes': int(1.5 * 1024**3)},
            'cpus': 4, 'runtime': runtime, 'apiHash': api_hash, 'apiEvidence': api_evidence,
            'cost': {'commonProducerSeconds': common + bootstrap, 'snapshotExportSeconds': snapshot_seconds,
                     'frozenCompilerSeedSeconds': seed_seconds, 'fixturePrimingSeconds': fixture,
                     'compilerFixtureExportSeconds': compiler_export_seconds,
                     'networkReceivedBytes': after[0] - before[0], 'networkSentBytes': after[1] - before[1]},
            'compiler': compiler, 'expiresAt': (datetime.now(timezone.utc) + timedelta(hours=24)).isoformat()}
        value['cost']['producerBuilderRemovalSeconds'] = api_evidence['builderRemovalSeconds']
        save(output / 'cargo-snapshot-trial-producer.json', value)
    finally:
        trial.cleanup()


def consumer(root, output, value, samples):
    validate_costs(value)
    package = value['registryPackage']
    fixture_started = time.monotonic()
    compiler_root = output / 'compiler-cache-snapshot'
    export_context = output / 'contexts/compiler-import'
    export_context.mkdir(parents=True)
    (export_context / 'Dockerfile').write_text('FROM scratch\nCOPY --from=compiler-carrier /compiler-cache/ /\n')
    # A fresh process and builder on the second runner retrieves the exact producer's frozen compiler state.
    import_builder = f'cargo-snapshot-import-{os.environ["GITHUB_RUN_ID"]}-{os.environ["GITHUB_RUN_ATTEMPT"]}'
    run(['docker', 'buildx', 'create', '--name', import_builder, '--driver', 'docker-container', '--driver-opt',
         f'image={value["buildkitImage"]}', '--bootstrap'])
    fixture_bootstrap_seconds = time.monotonic() - fixture_started
    try:
        run(['docker', 'update', '--cpus', '4', f'buildx_buildkit_{import_builder}0'])
        import_started = time.monotonic()
        run(['docker', 'buildx', 'build', '--builder', import_builder, '--build-context',
             f'compiler-carrier=docker-image://{package}@{value["images"]["compilerCache"]}', '--output', f'type=local,dest={compiler_root}', str(export_context)], output / 'compiler-import.log')
        fixture_import_seconds = time.monotonic() - import_started
    finally:
        removal_started = time.monotonic()
        run(['docker', 'buildx', 'rm', '--force', import_builder])
        fixture_removal_seconds = time.monotonic() - removal_started
    validation_started = time.monotonic()
    if context_digest(compiler_root) != value['compilerSnapshotHash']:
        raise ValueError('Cross-run frozen compiler state mismatch')
    fixture_transport = {'producerExportSeconds': required_seconds(value['cost'], 'compilerFixtureExportSeconds'),
                         'consumerBootstrapSeconds': fixture_bootstrap_seconds, 'consumerImportSeconds': fixture_import_seconds,
                         'consumerRemovalSeconds': fixture_removal_seconds,
                         'consumerValidationSeconds': time.monotonic() - validation_started,
                         'consumerTotalSeconds': time.monotonic() - fixture_started,
                         'includedInNetGain': False}
    trial = Trial(root, output, value['buildkitImage'], package, compiler_root)
    records = []
    try:
        for scenario in ('source', 'workspace'):
            for sample in range(1, samples + 1):
                source = output / f'contexts/{scenario}-{sample}'
                source_hash = context(root, source, scenario, sample, readonly=True)
                if trial.identity(source) != value['identity']:
                    raise ValueError('Consumer compiler/configuration identity changed')
                pair = []
                for variant in (('control', 'snapshot') if sample % 2 else ('snapshot', 'control')):
                    label = f'{scenario}-{sample}-{variant}'
                    folder = output / label
                    builder, bootstrap, fixture = trial.builder(label)
                    args = ['--cache-from', f'type=registry,ref={package}@{value["images"]["buildkitCache"]}']
                    if variant == 'snapshot':
                        args += ['--build-arg', 'SCHOOL_CARGO_BUILD_STAGE=snapshot-builder', '--build-context',
                                 f'cargo-snapshot-data=docker-image://{package}@{value["images"]["cargoSnapshot"]}', '--build-arg',
                                 f'SCHOOL_CARGO_CONFIG_HASH={value["identity"]["buildConfigurationHash"]}', '--build-arg',
                                 f'SCHOOL_CARGO_SOURCE_MANIFEST_HASH={value["sourceManifestHash"]}']
                    before = network_bytes(builder)
                    started = time.monotonic()
                    preflight_seconds = 0
                    if variant == 'snapshot':
                        preflight_started = time.monotonic()
                        run(trial.base(builder) + args + ['--target', 'snapshot-restored', '--output', 'type=cacheonly', str(source)], folder / 'snapshot-preflight.log')
                        run(trial.base(builder) + args + ['--target', 'cargo-native-gate', '--output',
                            f'type=local,dest={folder / "native-gate"}', str(source)], folder / 'native-gate.log')
                        native = json.loads((folder / 'native-gate/native-gate.json').read_text())
                        if native.get('compatible') is not True:
                            raise ValueError('Diagnostic snapshot refused by current native compiler identity')
                        run(trial.base(builder) + args + ['--target', 'cargo-materialization-gate', '--output',
                            f'type=local,dest={folder / "materialization-gate"}', str(source)], folder / 'materialization-gate.log')
                        materialized = json.loads((folder / 'materialization-gate/materialization-gate.json').read_text())
                        if materialized.get('compatible') is not True:
                            raise ValueError('Diagnostic snapshot refused during final COPY validation')
                        preflight_seconds = time.monotonic() - preflight_started
                    tag = package + ':' + f'consumer-{os.environ["GITHUB_RUN_ID"]}-{label}'
                    run(trial.base(builder) + args + ['--target', 'runtime', '--push', '--tag', tag,
                        '--cache-to', f'type=registry,ref={tag}-buildcache,mode=max,image-manifest=true,oci-mediatypes=true',
                        '--metadata-file', str(folder / 'runtime-metadata.json'), str(source)], folder / 'runtime-build.log')
                    elapsed = time.monotonic() - started + bootstrap
                    resource_checkpoint(output, label + '-after-runtime-push')
                    after = network_bytes(builder)
                    runtime = trial.runtime(package + '@' + image_digest(folder / 'runtime-metadata.json'))
                    compiler = trial.evidence(builder, source, folder, args)
                    if (compiler['cargo']['registryIdentitySha256'] != value['compiler']['cargo']['registryIdentitySha256']
                            or compiler['cargo']['registryArtifactCount'] != value['compiler']['cargo']['registryArtifactCount']):
                        raise ValueError('Actual registry features/profile changed across runs')
                    if scenario == 'source' and not any('backend-school' in name for name in compiler['cargo']['dirtyLocalPackages']):
                        raise ValueError('Source-changing case did not compile the application')
                    restoration = None
                    if variant == 'snapshot':
                        run(trial.base(builder) + args + ['--target', 'cargo-restore-evidence', '--output',
                            f'type=local,dest={folder / "restore"}', str(source)], folder / 'restore-evidence.log')
                        restoration = json.loads((folder / 'restore/snapshot-restore.json').read_text())
                        if restoration.get('restored') is not True or restoration.get('changedFiles', 0) < 1:
                            raise ValueError('Actual source reconciliation evidence unavailable')
                        expected_change = 'src/main.rs' if scenario == 'source' else 'Cargo.toml'
                        if expected_change not in restoration.get('changedPaths', []):
                            raise ValueError('Expected source/recipe input was not reconciled')
                    unchanged_started = time.monotonic()
                    run(trial.base(builder) + args + ['--target', 'cargo-freshness-evidence', '--build-arg',
                        f'SCHOOL_FRESHNESS_PROBE={label}', '--output', f'type=local,dest={folder / "unchanged"}', str(source)], folder / 'unchanged.log')
                    unchanged = [json.loads(line) for line in (folder / 'unchanged/cargo-artifacts.jsonl').read_text().splitlines()]
                    unchanged = [record for record in unchanged if record.get('reason') == 'compiler-artifact']
                    if not unchanged or not all(record.get('fresh') is True for record in unchanged):
                        raise ValueError('Explicit unchanged Cargo probe compiled or lacked evidence')
                    unchanged_seconds = time.monotonic() - unchanged_started
                    api_evidence = trial.api(builder, source, folder, args)
                    api_hash = api_evidence['canonicalSha256']
                    record = {'variant': variant, 'scenario': scenario, 'sample': sample, 'consumerSeconds': elapsed + (value['orchestration']['consumerResolveSeconds'] if variant == 'snapshot' else 0),
                        'producerSeconds': value['cost']['commonProducerSeconds'] + (value['cost']['snapshotExportSeconds'] + value['orchestration']['producerSnapshotOnlySeconds'] if variant == 'snapshot' else 0),
                        'preflightNativeAndMaterializationSeconds': preflight_seconds, 'restoration': restoration,
                        'unchangedDiagnosticSeconds': unchanged_seconds, 'unchangedArtifacts': len(unchanged),
                        'authorityCost': value['orchestration'] if variant == 'snapshot' else {},
                        'fixtureTransport': fixture_transport,
                        'fixturePrimingSeconds': fixture, 'sourceContextHash': source_hash,
                        'compilerSnapshotHash': value['compilerSnapshotHash'], 'runtime': runtime, 'apiHash': api_hash, 'apiEvidence': api_evidence,
                        'compiler': compiler, 'dependencyMetricsReusedFromProducer': variant == 'snapshot',
                        'networkReceivedBytes': after[0] - before[0], 'networkSentBytes': after[1] - before[1],
                        'producerRunId': value['producer']['runId'], 'consumerRunId': os.environ['GITHUB_RUN_ID'],
                        'freshBuilder': True, 'sameBuilderReuseClaim': False}
                    record['transitionSeconds'] = record['producerSeconds'] + record['consumerSeconds']
                    pair.append(record); records.append(record)
                    save(output / 'results.json', records)
                    record['fixtureBuilderRemovalSeconds'] = api_evidence['builderRemovalSeconds']
                    save(output / 'results.json', records)
                if pair[0]['runtime'] != pair[1]['runtime'] or pair[0]['apiHash'] != pair[1]['apiHash']:
                    raise ValueError('Paired runtime binary/configuration or fresh offline API differs')
        pairs = [(next(r for r in records if r['scenario'] == scenario and r['sample'] == sample and r['variant'] == 'control'),
                  next(r for r in records if r['scenario'] == scenario and r['sample'] == sample and r['variant'] == 'snapshot'))
                 for scenario in ('source', 'workspace') for sample in range(1, samples + 1)]
        gains = [control['transitionSeconds'] - snapshot['transitionSeconds'] for control, snapshot in pairs]
        save(output / 'complete.json', {'complete': True, 'producerRunId': value['producer']['runId'],
             'consumerRunId': os.environ['GITHUB_RUN_ID'], 'distinctRuns': True,
             'fixtureTransport': fixture_transport, 'allPairsNetPositive': all(gain > 0 for gain in gains), 'pairedNetSeconds': gains,
             'meanNetSeconds': statistics.mean(gains), 'productionActivation': 'OFF; requires reviewed repeated net gain and diagnostic freshness proof',
             'limits': ['Transition costs are not total workflow elapsed: checkout, exact Node setup, login and common runner setup are excluded/common to both variants and cancel in the paired delta.',
                        'Diagnostic Operations origin, not a production MAIN Pipeline origin attestation.',
                        'Controlled same4CPU runner class; hosted machines differ between runs.',
                        'Fixture compiler seeding/transfer/priming and offline API checks are reported separately from production-style transition costs.',
                        'OCI timestamps/history may differ; paired runtime binary, USER/CMD and offline API must match.']})
    finally:
        trial.cleanup()


def main():
    global DOCKER_ROOT
    phase = os.environ.get('SNAPSHOT_PHASE')
    if phase not in {'producer', 'consumer'} or os.cpu_count() != 4:
        raise ValueError('Producer/consumer phase on existing4CPU runner required')
    root = Path(__file__).resolve().parents[1]
    output = Path(os.environ['BENCH_OUTPUT']).resolve()
    output.mkdir(parents=True, exist_ok=True)
    DOCKER_ROOT = Path(run(['docker', 'info', '--format', '{{.DockerRootDir}}']))
    (output / 'initial-df.txt').write_text(run(['df', '-B1', str(output), str(DOCKER_ROOT)]) + '\n')
    save(output / 'resource-plan.json', {'initialFreeGiBRequired': 32, 'stageFreeGiBRequired': 8,
         'estimatedPeakAdditionalGiB': '20–30, not guaranteed fit',
         'model': 'One owned builder; target ~3.3GiB, source/target carrier and strict export can temporarily hold three TAR copies; fixture ~1GiB; API debug ~1.5GiB. Verification images removed immediately; builder removed before API container.',
         'cleanup': 'Only owned local builders/images/TARs; no global or shared pruning.'})
    resource_checkpoint(output, 'initial', 32)
    if phase == 'producer':
        producer(root, output, os.environ['BENCH_BUILDKIT_IMAGE'])
    else:
        value = json.loads((Path(os.environ['RUNNER_TEMP']) / 'cargo-snapshot-trial-producer.json').read_text())
        if value['producer']['runId'] == os.environ['GITHUB_RUN_ID']:
            raise ValueError('Cross-run consumer cannot reuse its current run')
        consumer(root, output, value, int(os.environ['BENCH_SAMPLES']))

if __name__ == '__main__':
    main()
