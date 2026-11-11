#!/usr/bin/env bash
# Sent from the trusted checkout through SSH; never execute an archive hook.
set -euo pipefail
if [[ ${BUNDLE_OPERATION:-install} != describe ]]; then
    [[ ${RELEASE_SHA-} =~ ^[0-9a-f]{40}$ && ${RELEASE_BUNDLE_SHA256-} =~ ^[0-9a-f]{64}$ ]] || exit 64
fi
command -v python3 >/dev/null || {
    echo 'Release bundle requires existing Python 3' >&2
    exit 78
}
case ${BUNDLE_INSTALL:-true} in
    true) ;;
    false) ;;
    *) exit 64 ;;
esac
if [[ ${BUNDLE_INSTALL:-true} == true ]]; then
    python3 - <<'PY'
import hashlib, io, json, os, pathlib, re, shutil, stat, tarfile, tempfile

# Single fixed protocol allowlist, also read by the public archive producer.
FILES = sorted([
    'podman-compose.yml',
    'nginx-configs/admin-api.conf.template',
    'nginx-configs/admin-api.maintenance.conf.template',
    'nginx-configs/school-api.conf.template',
    'nginx-configs/school-api.maintenance.conf.template',
    'scripts/render_nginx_config.sh', 'scripts/smoke_test.sh',
    'scripts/prune_runtime_images.sh', 'scripts/clamd_runtime_matches.sh',
    'scripts/reconcile_r2_cors.sh',
    'scripts/lib/schoolorbit-installer/remote/ensure_container_network.sh',
    'scripts/lib/schoolorbit-installer/remote/deployment_timing.sh',
    *['scripts/lib/pipeline-remote/' + name for name in [
        'release-paths.sh', 'runtime-env.sh', 'storage-namespace-input.sh',
        'maintenance.sh', 'admin-accounts.sh', 'admin-deploy.sh',
        'school-deploy-backend.sh', 'school-storage-rollout.sh',
        'school-verify-academic.sh', 'school-verify-release.sh',
    ]],
])
if os.environ.get('BUNDLE_OPERATION') == 'describe':
    print(json.dumps(FILES))
    raise SystemExit(0)
if os.environ.get('BUNDLE_OPERATION', 'install') != 'install':
    raise SystemExit('Unknown bundle operation')

def fail():
    raise SystemExit('Release bundle identity or contents invalid')

def object_pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            fail()
        result[key] = value
    return result

sha = os.environ.get('RELEASE_SHA', '')
digest = os.environ.get('RELEASE_BUNDLE_SHA256', '')
run = os.environ.get('BUNDLE_RUN_ID', '')
attempt = os.environ.get('BUNDLE_RUN_ATTEMPT', '')
if not re.fullmatch('[0-9a-f]{40}', sha) or not re.fullmatch('[0-9a-f]{64}', digest):
    fail()
if not re.fullmatch('[1-9][0-9]*', run) or not re.fullmatch('[1-9][0-9]*', attempt):
    fail()
root = pathlib.Path(os.environ.get('SCHOOLORBIT_STACK_ROOT', '/opt/stack')) / 'deployment'

def directory(path, create=False):
    if create:
        path.mkdir(mode=0o700, exist_ok=True)
    info = path.lstat()
    if not stat.S_ISDIR(info.st_mode) or info.st_uid != os.getuid() or info.st_mode & 0o022:
        fail()

# Reject ancestor substitution before opening any archive or publishing code.
directory(root)
incoming = root / 'release-bundle-incoming' / sha / digest / (run + '-' + attempt)
for path in [incoming.parents[3], incoming.parents[2], incoming.parents[1], incoming.parent, incoming]:
    directory(path)
archive = incoming / 'release-bundle.tar'
info = archive.lstat()
if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_size > 16 * 1024 * 1024:
    fail()
raw = archive.read_bytes()
if hashlib.sha256(raw).hexdigest() != digest:
    fail()
payload = {}
with tarfile.open(fileobj=io.BytesIO(raw), mode='r:') as source:
    members = source.getmembers()
    if len(members) != len(FILES) + 1:
        fail()
    for member in members:
        if member.name not in FILES + ['release-bundle.json'] or member.name in payload:
            fail()
        if not member.isreg() or member.pax_headers or member.linkname or member.size > 2 * 1024 * 1024:
            fail()
        expected_mode = 0o555 if member.name.endswith('.sh') else 0o444
        if member.mode != expected_mode or member.uid != 0 or member.gid != 0:
            fail()
        payload[member.name] = source.extractfile(member).read()
manifest = json.loads(payload['release-bundle.json'], object_pairs_hook=object_pairs)
if set(manifest) != {'schemaVersion', 'releaseSha', 'files'} or type(manifest['schemaVersion']) is not int or manifest['schemaVersion'] != 1 or manifest['releaseSha'] != sha:
    fail()
expected = [{'path': name, 'mode': 0o555 if name.endswith('.sh') else 0o444,
             'size': len(payload[name]), 'sha256': hashlib.sha256(payload[name]).hexdigest()} for name in FILES]
if manifest['files'] != expected:
    fail()
bundles = root / 'release-bundles'
directory(bundles, create=True)
directory(bundles / sha, create=True)
destination = bundles / sha / digest
if destination.exists() or destination.is_symlink():
    directory(destination)
    found = set()
    for path in destination.rglob('*'):
        if path.is_symlink():
            fail()
        relative = path.relative_to(destination).as_posix()
        info = path.lstat()
        if stat.S_ISDIR(info.st_mode):
            if info.st_mode & 0o222 or not any(name.startswith(relative + '/') for name in payload):
                fail()
            continue
        if not stat.S_ISREG(info.st_mode) or relative not in payload or info.st_uid != os.getuid():
            fail()
        if stat.S_IMODE(info.st_mode) != (0o555 if relative.endswith('.sh') else 0o444) or path.read_bytes() != payload[relative]:
            fail()
        found.add(relative)
    if found != set(payload):
        fail()
else:
    staging = pathlib.Path(tempfile.mkdtemp(prefix='.install-', dir=destination.parent))
    try:
        for name, content in payload.items():
            target = staging / name
            target.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
            with target.open('xb') as output:
                output.write(content)
            target.chmod(0o555 if name.endswith('.sh') else 0o444)
        for path in sorted(staging.rglob('*'), reverse=True):
            if path.is_dir():
                path.chmod(0o555)
        staging.chmod(0o555)
        # Same production lock serializes installation; never replace an existing bundle.
        if destination.exists() or destination.is_symlink():
            fail()
        os.rename(staging, destination)
    finally:
        if staging.exists():
            for path in staging.rglob('*'):
                if path.is_dir():
                    path.chmod(0o700)
            staging.chmod(0o700)
            shutil.rmtree(staging)
archive.unlink()
incoming.rmdir()
print('Release bundle verified and installed')
PY
fi
if [[ ${BUNDLE_OPERATION:-install} == describe ]]; then exit 0; fi
case ${BUNDLE_CHECK_ADMIN:-false} in
    true) bash "${SCHOOLORBIT_STACK_ROOT:-/opt/stack}/deployment/release-bundles/$RELEASE_SHA/$RELEASE_BUNDLE_SHA256/scripts/lib/pipeline-remote/admin-accounts.sh" --check-deploy-inputs ;;
    false) ;;
    *) exit 64 ;;
esac
case ${BUNDLE_ENTER_MAINTENANCE:-false} in
    true) bash "${SCHOOLORBIT_STACK_ROOT:-/opt/stack}/deployment/release-bundles/$RELEASE_SHA/$RELEASE_BUNDLE_SHA256/scripts/lib/pipeline-remote/maintenance.sh" enter ;;
    false) ;;
    *) exit 64 ;;
esac
