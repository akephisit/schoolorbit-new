#!/usr/bin/env python3
"""Small actual Cargo freshness proof; never uses the application's shared target."""
import argparse
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('cargo_snapshot', ROOT / 'backend-school/scripts/cargo_snapshot.py')
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--cargo', required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists() or not args.cargo.is_file():
        raise ValueError('New owned output and existing pinned Cargo executable required')
    args.output.mkdir(parents=True)
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith(('CARGO_PROFILE_', 'SCCACHE_', 'RUSTC_', 'CARGO_BUILD_'))
                   and key not in {'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_TARGET_DIR'}}
    environment.update(PATH=str(args.cargo.parent) + ':' + environment['PATH'], CARGO_INCREMENTAL='0',
                       SNAPSHOT_PROBE_VALUE='environment-A')
    with tempfile.TemporaryDirectory(prefix='cargo-snapshot-toolproof-') as scratch:
        root = Path(scratch)
        app = root / 'app'; app.mkdir()
        (app / 'src').mkdir(); (app / 'migrations').mkdir()
        (app / 'Cargo.toml').write_text('[package]\nname="snapshot-freshness-fixture"\nversion="0.1.0"\nedition="2021"\n')
        (app / 'src/main.rs').write_text('include!("value.rs"); fn main(){println!("{}:{}:{}", VALUE, env!("MIGRATIONS"), env!("WATCHED"));}\n')
        (app / 'src/value.rs').write_text('const VALUE:u8=1;\n')
        (app / 'migrations/one.sql').write_text('SELECT 1;')
        (app / 'build.rs').write_text('''fn main(){
 println!("cargo:rerun-if-changed=migrations");
 println!("cargo:rerun-if-env-changed=SNAPSHOT_PROBE_VALUE");
 let mut names=std::fs::read_dir("migrations").unwrap().map(|x|x.unwrap().path()).collect::<Vec<_>>();names.sort();
 let value=names.iter().map(|x|{
  let name=x.file_name().unwrap().to_str().unwrap();
  if x.is_dir(){format!("{}/",name)}else{format!("{}={}",name,std::fs::read_to_string(x).unwrap())}
 }).collect::<Vec<_>>().join(";");
 println!("cargo:rustc-env=MIGRATIONS={}",value);
 println!("cargo:rustc-env=WATCHED={}",std::env::var("SNAPSHOT_PROBE_VALUE").unwrap());
}\n''')
        def build(name, expected_missing_include=False):
            process = subprocess.run([str(args.cargo), 'build', '--release', '--offline', '--message-format=json-render-diagnostics'],
                                     cwd=app, env=environment, capture_output=True, text=True)
            (args.output / f'{name}.jsonl').write_text(process.stdout)
            (args.output / f'{name}.stderr').write_text(process.stderr)
            (args.output / f'{name}.status.json').write_text(json.dumps({
                'exitCode': process.returncode, 'expectedMissingInclude': expected_missing_include,
                'rawCargoMessages': f'{name}.jsonl', 'rawStderr': f'{name}.stderr'}, indent=2) + '\n')
            if expected_missing_include:
                messages = [json.loads(line) for line in process.stdout.splitlines()]
                finished = [record['success'] for record in messages if record.get('reason') == 'build-finished']
                errors = [record['message'] for record in messages
                          if record.get('reason') == 'compiler-message'
                          and record['target']['name'] == 'snapshot-freshness-fixture'
                          and record['target']['kind'] == ['bin']
                          and record['message']['level'] == 'error'
                          and "couldn't read" in record['message']['message']
                          and 'src/value.rs' in record['message']['message']]
                artifacts = [record for record in messages if record.get('reason') == 'compiler-artifact']
                if (process.returncode != 101 or finished != [False] or not errors
                        or any(record['target']['kind'] == ['bin'] for record in artifacts)):
                    raise AssertionError('Missing include! input did not cause the expected compiler refusal')
                return {'exitCode': process.returncode, 'buildFinished': False,
                        'expectedMissingIncludeRefusal': True, 'binaryExecuted': False,
                        'artifacts': len(artifacts), 'missingIncludeDiagnostics': errors}
            process.check_returncode()
            artifacts = [json.loads(line) for line in process.stdout.splitlines()]
            artifacts = [record for record in artifacts if record.get('reason') == 'compiler-artifact']
            if not artifacts:
                raise ValueError('Missing actual Cargo artifacts')
            binary = subprocess.check_output([str(app / 'target/release/snapshot-freshness-fixture')], text=True).strip()
            return {'artifacts': len(artifacts), 'dirty': sum(not record['fresh'] for record in artifacts), 'binaryOutput': binary}
        records = {'producer': build('producer')}
        incoming = root / 'incoming'
        shutil.copytree(app, incoming, ignore=shutil.ignore_patterns('target'))
        # Wait for the explicit trusted-clock fence rather than changing Cargo metadata.
        maximum = max(path.lstat().st_mtime_ns for path in [app, *app.rglob('*')])
        delay = max(0, (maximum + cache.SAFETY_NS - time.time_ns()) / 1e9 + 0.05)
        time.sleep(delay)
        # Native gate has its own real compiler-stage check; this fixture isolates Cargo/source freshness.
        with patch.object(cache, 'native_identity', return_value={'schemaVersion': 1, 'diagnosticFixture': True}):
            metadata = cache.create_archive(app, root / 'snapshot', 'fixture-configuration')
            def restore(name, change=None, expected_missing_include=False):
                shutil.rmtree(app)
                case = root / f'incoming-{name}'; shutil.copytree(incoming, case)
                if change: change(case)
                result = cache.preflight(root / 'snapshot/snapshot.tar', root / 'snapshot/snapshot.json', case,
                                         app, 'fixture-configuration', metadata['sourceManifestSha256'])
                # Model final Docker COPY losing directory times, then exercise the real materializer.
                for path in [app, *(p for p in app.rglob('*') if p.is_dir())]:
                    os.utime(path, None)
                cache.materialize(app, root / 'prevalidated.json', 'fixture-configuration')
                observed = build(name, expected_missing_include=expected_missing_include)
                records[name] = {'restore': result, **observed}
                return observed
            unchanged = restore('unchanged')
            assert unchanged['dirty'] == 0 and unchanged['binaryOutput'] == records['producer']['binaryOutput']
            def include_change(case):
                path = case / 'src/value.rs'; path.write_text('const VALUE:u8=2;\n'); os.utime(path, ns=(1, 1))
            observed = restore('include-forged-mtime', include_change)
            assert observed['dirty'] > 0 and observed['binaryOutput'].startswith('2:')
            observed = restore('build-script', lambda case: (case / 'build.rs').write_text((case / 'build.rs').read_text() + '// changed build script\n'))
            assert observed['dirty'] > 0
            observed = restore('rename', lambda case: (case / 'migrations/one.sql').rename(case / 'migrations/renamed.sql'))
            assert observed['dirty'] > 0 and 'renamed.sql=' in observed['binaryOutput']
            observed = restore('deletion', lambda case: (case / 'migrations/one.sql').unlink())
            assert observed['dirty'] > 0 and 'one.sql=' not in observed['binaryOutput']
            observed = restore('mode', lambda case: (case / 'src/value.rs').chmod(0o755))
            assert observed['dirty'] > 0
            def add_watched_sql(case):
                path = case / 'migrations/two.sql'; path.write_text('SELECT 2;'); os.utime(path, ns=(1, 1))
            observed = restore('add-watched-sql', add_watched_sql)
            assert observed['dirty'] > 0 and 'two.sql=SELECT 2;' in observed['binaryOutput']
            def add_empty_watched_directory(case):
                path = case / 'migrations/empty'; path.mkdir()
                os.utime(path, ns=(1, 1)); os.utime(path.parent, ns=(1, 1))
            observed = restore('add-empty-watched-directory', add_empty_watched_directory)
            assert observed['dirty'] > 0 and 'empty/' in observed['binaryOutput']
            environment['SNAPSHOT_PROBE_VALUE'] = 'environment-B'
            observed = restore('tracked-environment')
            assert observed['dirty'] > 0 and observed['binaryOutput'].endswith('environment-B')
            observed = restore('delete-include-input', lambda case: (case / 'src/value.rs').unlink(),
                               expected_missing_include=True)
            assert observed['exitCode'] == 101 and observed['expectedMissingIncludeRefusal'] and not observed['binaryExecuted']
        (args.output / 'complete.json').write_text(json.dumps({'passed': True, 'cases': records,
            'scope': 'Actual tiny Cargo fixture at one fixed path; native identity mocked here and verified separately by compiler-stage gate.',
            'applicationTargetUsed': False}, indent=2) + '\n')


if __name__ == '__main__':
    main()
