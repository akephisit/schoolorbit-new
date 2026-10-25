"""One reviewed owned-fixture operation. No caller SQL, credentials, or paths."""
import base64
import fcntl
from contextlib import contextmanager
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import tempfile
import time
import urllib.parse
import urllib.request

from owned_cleanup_proof import CleanupRefusal, validate_snapshot, validate_preservation, validate_postcleanup

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, message, headers, target):
        refuse('admin_redirect_refused')


CLIENT = 'schoolorbit-owned-load-cleanup-3962938a'
PG_IMAGE = 'public.ecr.aws/docker/library/postgres:18.6-alpine@sha256:d8703cd7fba306b9fec9268ecedfa8a966846c053036a60e3635791957eb2f66'


class ControllerRefusal(ValueError):
    def __init__(self, code):
        self.code = code if re.fullmatch('[a-z_]{1,80}', code) else 'controller_refused'
        super().__init__(self.code)


def refuse(code):
    raise ControllerRefusal(code)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False) + '\n').encode()


def private_directory(path, create=False):
    created = create and not os.path.lexists(path)
    if create:
        path.mkdir(mode=0o700, parents=False, exist_ok=True)
    st = path.lstat()
    if not stat.S_ISDIR(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o700:
        refuse('private_directory_invalid')
    if created:
        sync_directory(path)
        sync_directory(path.parent)


def private_bytes(path, limit=2 * 1024 * 1024):
    st = path.lstat()
    if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o600 or st.st_size > limit:
        refuse('private_input_invalid')
    return path.read_bytes()


def durable(path, value):
    raw = value if isinstance(value, bytes) else canonical(value)
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600)
    with os.fdopen(fd, 'wb') as output:
        output.write(raw)
        output.flush()
        os.fsync(output.fileno())
    sync_directory(path.parent)
    return sha(raw)


def sync_directory(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def json_record(path):
    value = json.loads(private_bytes(path))
    if not isinstance(value, dict):
        refuse('private_record_invalid')
    return value


def process_identity(pid):
    if type(pid) is not int or pid < 1:
        refuse('process_identity_invalid')
    try:
        raw = Path(f'/proc/{pid}/stat').read_text()
    except FileNotFoundError:
        return None
    except OSError:
        refuse('process_identity_unavailable')
    try:
        fields = raw.rsplit(')', 1)[1].split()
        ticks = int(fields[19])
        boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
        if ticks < 1 or not re.fullmatch('[0-9a-f-]{36}', boot):
            refuse('process_identity_invalid')
        return {'pid': pid, 'startTicks': ticks, 'bootId': boot}
    except (OSError, ValueError, IndexError):
        refuse('process_identity_unavailable')


def controller_dead(record):
    if type(record) is not dict or set(record) != {'pid', 'startTicks', 'bootId'} or type(record['pid']) is not int or record['pid'] < 1 or type(record['startTicks']) is not int or record['startTicks'] < 1 or not isinstance(record['bootId'], str) or not re.fullmatch('[0-9a-f-]{36}', record['bootId']):
        refuse('controller_identity_invalid')
    current = process_identity(record['pid'])
    return current is None or current != record


def timestamp_ns(value):
    match = re.fullmatch(r'(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d)(?:\.(\d{1,9}))?(Z|[+-]\d\d:\d\d)', value)
    if not match:
        refuse('process_timestamp_invalid')
    seconds = int(datetime.fromisoformat(match[1] + match[3].replace('Z', '+00:00')).timestamp())
    return seconds * 10**9 + int((match[2] or '').ljust(9, '0'))


def utc_timestamp(value):
    seconds, fraction = divmod(timestamp_ns(value), 10**9)
    return datetime.fromtimestamp(seconds, timezone.utc).strftime('%Y-%m-%dT%H:%M:%S') + f'.{fraction:09d}Z'


def extract_object(raw, key):
    objects = []
    for line in raw.decode('utf8').splitlines():
        if line.startswith('{'):
            try:
                item = json.loads(line)
            except ValueError:
                refuse('postgres_output_invalid')
            if isinstance(item, dict) and key in item:
                objects.append(item)
    if len(objects) != 1:
        refuse('postgres_output_invalid')
    return objects[0]


class OwnerJob:
    def __init__(self, catalogue, catalogue_raw, env):
        self.cat = catalogue
        self.catalogue_raw = catalogue_raw
        self.catalogue_hash = sha(catalogue_raw)
        self.env = env
        self.root = Path('/opt/stack/deployment/owned-load-cleanup')
        self.inputs = self.root / 'inputs' / self.cat['operationId']
        self.journal = self.root / 'operations' / self.cat['operationId']
        self.active = self.root / 'active'
        self.run = env['CLEANUP_RUN_ID'] + '-' + env['CLEANUP_RUN_ATTEMPT']
        if not re.fullmatch(r'[0-9]+-[0-9]+', self.run):
            refuse('operation_run_invalid')
        self.stage = self.root / 'staging' / self.run
        self.identity = process_identity(os.getpid())
        self.common = {'schemaVersion': 1, 'operationId': self.cat['operationId'], 'inputSha256': self.cat['inputSha256']}
        self.connection = None

    def command(self, arguments, timeout=30):
        result = subprocess.run(arguments, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout, check=False)
        if result.returncode or len(result.stdout) > 4 * 1024 * 1024:
            refuse('host_command_refused')
        return result.stdout

    def client_absent(self):
        result = subprocess.run(['podman', 'container', 'exists', CLIENT], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
        if result.returncode != 1:
            refuse('cleanup_client_present_or_unknown')

    def initialize(self):
        if os.geteuid() == 0 or self.command(['podman', 'info', '--format', '{{.Host.Security.Rootless}}']).strip() != b'true':
            refuse('rootless_operator_required')
        for path in [Path('/opt'), Path('/opt/stack'), Path('/opt/stack/deployment')]:
            if not path.is_dir() or path.is_symlink():
                refuse('stack_path_invalid')
        private_directory(self.root, True)
        for name in ['inputs', 'staging', 'operations']:
            private_directory(self.root / name, True)
        self.client_absent()
        if self.env['CLEANUP_CATALOGUE_SHA256'] != self.catalogue_hash:
            refuse('catalogue_hash_mismatch')
        if self.cat['operationId'] != 'cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b' or self.cat['tenantId'] != '0e297ca4-0809-4aab-a03f-1915045257b8':
            refuse('catalogue_identity_invalid')
        private_directory(self.inputs)
        if set(p.name for p in self.inputs.iterdir()) != set(self.cat['files']):
            refuse('bundle_members_invalid')
        for name, expected in self.cat['files'].items():
            if '/' in name or name in ['.', '..'] or sha(private_bytes(self.inputs / name)) != expected:
                refuse('bundle_hash_mismatch')
        self.floor = json_record(self.inputs / 'original-watch-counters.json')

    def legacy_clear(self):
        deployment = self.root.parent
        if os.path.lexists(deployment / 'pending-release'):
            refuse('pending_release_exists')
        for name in ['schoolorbit-school-rollout', 'schoolorbit-admin-migration']:
            result = subprocess.run(['podman', 'container', 'exists', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
            if result.returncode != 1:
                refuse('owner_rollout_present_or_unknown')
        tls = deployment / 'api-tls-operation.json'
        if os.path.lexists(tls) and json_record(tls).get('phase') != 'accepted':
            refuse('tls_recovery_pending')
        dns = deployment / 'api-dns'
        if os.path.lexists(dns):
            private_directory(dns)
            for path in dns.iterdir():
                if path.suffix == '.json' and json_record(path).get('status') not in ['accepted', 'rollback_applied']:
                    refuse('dns_recovery_pending')

    def actual_binding(self):
        self.legacy_clear()
        accepted = (self.root.parent / 'accepted-release').read_text().strip()
        if accepted != self.env['CLEANUP_ACCEPTED_SHA'] or not re.fullmatch('[0-9a-f]{40}', accepted):
            refuse('accepted_release_changed')
        raw = self.command(['podman', 'inspect', 'schoolorbit-backend-school'])
        items = json.loads(raw)
        if not isinstance(items, list) or len(items) != 1:
            refuse('backend_inventory_invalid')
        current = items[0]
        state = current['State']
        image = current['ImageDigest']
        config_id = current['Image'].removeprefix('sha256:')
        if not re.fullmatch('[0-9a-f]{64}', config_id) or current.get('ImageName') != 'ghcr.io/akephisit/schoolorbit-backend-school@' + self.cat['image']:
            refuse('reviewed_backend_changed')
        if image != self.cat['image'] or state['Running'] is not True or current['RestartCount'] != 0 or timestamp_ns(state['StartedAt']) < timestamp_ns(self.cat['authCutoff']):
            refuse('reviewed_backend_changed')
        inventory = json.loads(self.command(['podman', 'ps', '--format', 'json']))
        peers = [item for item in inventory if 'schoolorbit-backend-school' in str(item.get('Names', '')) or 'schoolorbit-backend-school' in item.get('Image', '') or str(item.get('ImageID', '')).removeprefix('sha256:') == config_id]
        if len(peers) != 1:
            refuse('sole_backend_required')
        kernel = process_identity(state['Pid'])
        if kernel is None:
            refuse('backend_process_missing')
        self.public_ready(accepted)
        self.backend_env = current['Config']['Env']
        return {**self.common, 'tenantId': self.cat['tenantId'], 'subdomain': self.cat['subdomain'],
            'databaseSchemaVersion': self.cat['databaseSchemaVersion'], 'acceptedRelease': accepted,
            'backendInputHash': self.env['CLEANUP_BACKEND_INPUT_HASH'], 'catalogueSha256': self.catalogue_hash,
            'controllerSha256': self.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': self.env['CLEANUP_PROOF_SHA256'],
            'backend': {'image': image, 'configId': config_id, 'containerId': current['Id'], 'pid': state['Pid'], 'startedAt': utc_timestamp(state['StartedAt']), 'restartCount': current['RestartCount'], **kernel}}

    def same_binding(self, expected):
        if self.actual_binding() != expected:
            refuse('serving_binding_changed')

    def public_ready(self, accepted):
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
        for path in ['/deployment-status', '/ready']:
            with opener.open(self.cat['apiOrigin'] + path, timeout=10) as response:
                raw = response.read(65537)
            if len(raw) > 65536:
                refuse('public_readiness_invalid')
            value = json.loads(raw)
            if value.get('status') != 'ready' or (path == '/deployment-status' and value.get('releaseId') != accepted):
                refuse('public_serving_release_changed')

    def admin_get(self, route):
        secrets = [item.split('=', 1)[1] for item in self.backend_env if item.startswith('INTERNAL_API_SECRET=')]
        if len(secrets) != 1 or not secrets[0] or '\n' in secrets[0]:
            refuse('internal_caller_unavailable')
        request = urllib.request.Request('http://127.0.0.1:8080' + route, headers={'X-Internal-Caller': 'backend-school', 'X-Internal-Secret': secrets[0]})
        with urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect()).open(request, timeout=10) as response:
            raw = response.read(65537)
        if len(raw) > 65536:
            refuse('admin_response_invalid')
        return json.loads(raw)

    def owner_connection(self):
        metadata = self.admin_get('/internal/schools/' + self.cat['subdomain'])
        if metadata.get('id') != self.cat['tenantId'] or metadata.get('subdomain') != self.cat['subdomain'] or metadata.get('status') != 'active' or metadata.get('migration_version') != 138 or metadata.get('migration_status') != 'migrated' or metadata.get('mutation_pending', False):
            refuse('active_tenant_metadata_invalid')
        # Admin is the credential decryptor; no include_inactive or human DB-host key.
        value = self.admin_get('/internal/schools/' + self.cat['subdomain'] + '/migration-connection')['db_migration_connection_string']
        parsed = urllib.parse.urlsplit(value)
        query = urllib.parse.parse_qs(parsed.query, strict_parsing=True)
        owner = 'so_' + self.cat['tenantId'].replace('-', '') + '_owner'
        if parsed.scheme not in ['postgres', 'postgresql'] or parsed.username != owner or urllib.parse.unquote(parsed.path[1:]) != self.cat['databaseName'] or not parsed.password or not parsed.hostname or parsed.fragment or query.get('sslmode') != ['verify-full'] or query.get('sslrootcert') != ['/run/secrets/postgresql-ca.crt'] or set(query) != {'sslmode', 'sslrootcert'}:
            refuse('owner_connection_invalid')
        password = urllib.parse.unquote(parsed.password)
        if '\n' in password or '\r' in password:
            refuse('owner_connection_invalid')
        self.connection = {'host': parsed.hostname, 'port': parsed.port or 5432, 'user': owner, 'database': self.cat['databaseName'], 'password': password}

    @contextmanager
    def work(self, approved=None):
        runtime = Path('/run/user') / str(os.getuid())
        if not runtime.is_dir() or runtime.is_symlink() or runtime.stat().st_uid != os.getuid():
            refuse('private_runtime_unavailable')
        with tempfile.TemporaryDirectory(prefix='owned-cleanup-', dir=runtime) as directory:
            work = Path(directory)
            for name in ['preflight.sql', 'cleanup-commit.sql', 'reconcile.sql']:
                (work / name).write_bytes(private_bytes(self.inputs / name)); (work / name).chmod(0o600)
            if approved is not None:
                payload = json.dumps(approved, separators=(',', ':'), allow_nan=False).replace("'", "''")
                (work / 'approved-capture.sql').write_text("CREATE TEMP TABLE cleanup_approved(body jsonb) ON COMMIT DROP;\nINSERT INTO cleanup_approved VALUES('" + payload + "'::jsonb);\n")
                (work / 'approved-capture.sql').chmod(0o600)
            escape = lambda value: str(value).replace('\\', '\\\\').replace(':', '\\:')
            connection = self.connection
            (work / 'pgpass').write_text(':'.join(escape(connection[key]) for key in ['host', 'port', 'database', 'user', 'password']) + '\n')
            (work / 'pgpass').chmod(0o600)
            yield work

    def psql(self, work, filename, *, dispatch=False):
        c = self.connection
        environment = {**os.environ, 'PGHOST': c['host'], 'PGPORT': str(c['port']), 'PGDATABASE': c['database'], 'PGUSER': c['user'], 'PGSSLMODE': 'verify-full', 'PGSSLROOTCERT': '/run/secrets/postgresql-ca.crt', 'PGPASSFILE': '/run/owned-cleanup/pgpass', 'PGCONNECT_TIMEOUT': '5', 'PGAPPNAME': 'owned-load-cleanup-3962938a'}
        arguments = ['podman', 'run', '--rm', '--name', CLIENT, '--network', 'schoolorbit-web', '--read-only', '--cap-drop', 'all', '--security-opt', 'no-new-privileges', '-v', str(work) + ':/run/owned-cleanup:ro', '-v', '/opt/stack/postgresql/ca.crt:/run/secrets/postgresql-ca.crt:ro']
        for key in ['PGHOST', 'PGPORT', 'PGDATABASE', 'PGUSER', 'PGSSLMODE', 'PGSSLROOTCERT', 'PGPASSFILE', 'PGCONNECT_TIMEOUT', 'PGAPPNAME']:
            arguments.extend(['--env', key])
        arguments += [PG_IMAGE, 'psql', '-X', '-A', '-t', '-q', '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=sqlstate', '-f', '/run/owned-cleanup/' + filename]
        try:
            result = subprocess.run(arguments, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=False)
        except subprocess.TimeoutExpired:
            if dispatch:
                durable(self.journal / 'postgres-exit.json', {**self.receipt_common, 'status': 'unknown', 'exitCode': None})
            refuse('postgres_outcome_unknown')
        if dispatch:
            # Actual client exit is durable BEFORE interpreting SQL output emitted before COMMIT.
            durable(self.journal / 'postgres-exit.json', {**self.receipt_common, 'status': 'exited', 'exitCode': result.returncode, 'stdoutSha256': sha(result.stdout)})
        if result.returncode or len(result.stdout) > 2 * 1024 * 1024:
            refuse('postgres_operation_refused')
        return result.stdout

    def database_identity(self, work):
        sql = """SELECT jsonb_build_object('identity',true,'database',current_database(),'user',current_user,'owner',r.rolname,'marker',shobj_description(r.oid,'pg_authid'),'temp',has_database_privilege(current_user,current_database(),'TEMP'),'ssl',(SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()),'superuser',r.rolsuper,'createRole',r.rolcreaterole,'createDb',r.rolcreatedb,'bypassRls',r.rolbypassrls) FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database();"""
        (work / 'identity.sql').write_text(sql); (work / 'identity.sql').chmod(0o600)
        row = extract_object(self.psql(work, 'identity.sql'), 'identity')
        owner = 'so_' + self.cat['tenantId'].replace('-', '') + '_owner'
        if row != {'identity': True, 'database': self.cat['databaseName'], 'user': owner, 'owner': owner, 'marker': 'schoolorbit:' + self.cat['tenantId'] + ':migration', 'temp': True, 'ssl': True, 'superuser': False, 'createRole': False, 'createDb': False, 'bypassRls': False}:
            refuse('database_owner_identity_invalid')

    def prepare(self):
        if os.path.lexists(self.active) or os.path.lexists(self.journal):
            refuse('cleanup_already_reserved')
        self.legacy_clear()
        binding = self.actual_binding()
        self.owner_connection()
        with self.work() as work:
            self.database_identity(work)
            snapshot = extract_object(self.psql(work, 'preflight.sql'), 'outside')
        validate_snapshot(snapshot, self.floor, self.cat)
        self.same_binding(binding)
        private_directory(self.stage, True)
        durable(self.stage / 'actual-binding.json', binding)
        durable(self.stage / 'approved-snapshot.json', snapshot)
        durable(self.stage / 'prepared.json', {**self.common, 'status': 'prepared', 'actualBindingSha256': sha(canonical(binding)), 'snapshotSha256': sha(canonical(snapshot)), 'catalogueSha256': self.catalogue_hash, 'controllerSha256': self.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': self.env['CLEANUP_PROOF_SHA256']})
        reservation = {**self.common, 'status': 'prepared', 'run': self.run, 'controller': self.identity, 'catalogueSha256': self.catalogue_hash, 'controllerSha256': self.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': self.env['CLEANUP_PROOF_SHA256']}
        durable(self.stage / 'reservation.json', reservation)
        os.link(self.stage / 'reservation.json', self.active, follow_symlinks=False)
        sync_directory(self.root)
        print('Readonly cleanup preparation passed; reservation retained, no cleanup DML dispatched.')

    def commit(self):
        if os.path.lexists(self.journal):
            refuse('cleanup_already_dispatched')
        self.active_fd = os.open(self.active, os.O_RDONLY | os.O_NOFOLLOW)
        fcntl.flock(self.active_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        reservation = json_record(self.active)
        if reservation.get('status') != 'prepared' or reservation.get('run') != self.run or reservation.get('operationId') != self.cat['operationId'] or reservation.get('catalogueSha256') != self.catalogue_hash:
            refuse('prepared_reservation_invalid')
        private_directory(self.stage)
        binding = json_record(self.stage / 'actual-binding.json')
        approved = json_record(self.stage / 'approved-snapshot.json')
        prepared = json_record(self.stage / 'prepared.json')
        if prepared.get('actualBindingSha256') != sha(canonical(binding)) or prepared.get('snapshotSha256') != sha(canonical(approved)) or prepared.get('catalogueSha256') != self.catalogue_hash:
            refuse('prepared_input_changed')
        self.same_binding(binding)
        self.owner_connection()
        # A durable prepared reservation already fenced the gate-to-SSH cancellation gap.
        durable(self.stage / 'commit-reservation.json', {**self.common, 'status': 'reserved', 'run': self.run, 'controller': self.identity, 'catalogueSha256': self.catalogue_hash, 'controllerSha256': self.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': self.env['CLEANUP_PROOF_SHA256']})
        next_active = self.stage / 'active-next'
        os.link(self.stage / 'commit-reservation.json', next_active, follow_symlinks=False)
        self.commit_fd = os.open(next_active, os.O_RDONLY | os.O_NOFOLLOW)
        fcntl.flock(self.commit_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        os.replace(next_active, self.active)
        sync_directory(self.root)
        with self.work(approved) as work:
            self.database_identity(work)
            current = extract_object(self.psql(work, 'preflight.sql'), 'outside')
            validate_snapshot(current, self.floor, self.cat)
            validate_preservation(approved, current)
            self.same_binding(binding)
            private_directory(self.journal, True)
            durable(self.journal / 'actual-binding.json', binding)
            durable(self.journal / 'approved-snapshot.json', approved)
            self.binding_hash = sha(canonical(binding))
            self.receipt_common = {**self.common, 'actualBindingSha256': self.binding_hash, 'approvedSnapshotSha256': sha(canonical(approved)), 'sqlSha256': self.cat['files']['cleanup-commit.sql']}
            durable(self.journal / 'dispatch-intent.json', {**self.receipt_common, 'status': 'dispatching', 'controller': self.identity, 'run': self.run, 'catalogueSha256': self.catalogue_hash})
            raw = self.psql(work, 'cleanup-commit.sql', dispatch=True)
            candidate = extract_object(raw, 'committedCandidate')
            if candidate.get('operationId') != self.cat['operationId'] or candidate.get('committedCandidate') is not True:
                refuse('commit_output_invalid')
            validate_postcleanup(approved, candidate['snapshot'])
            observed = extract_object(self.psql(work, 'reconcile.sql'), 'outside')
            validate_postcleanup(approved, observed)
            self.same_binding(binding)
            exited_hash = sha(private_bytes(self.journal / 'postgres-exit.json'))
            durable(self.journal / 'commit-receipt.json', {**self.receipt_common, 'status': 'committed', 'postgresExitSha256': exited_hash, 'postCommitVerified': True, 'snapshotSha256': sha(canonical(observed))})
            self.watch(work, approved, binding)

    def observation_floor(self, approved):
        previous = approved
        observations = sorted(self.journal.glob('observation-*.json'), key=lambda path: int(path.stem.split('-')[1]))
        if len(observations) > 1000:
            refuse('observation_history_invalid')
        for path in observations:
            row = json_record(path)
            if any(row.get(key) != value for key, value in self.receipt_common.items()):
                refuse('observation_binding_changed')
            basis = {**approved, 'watchCounters': row['watchCounters'], 'behaviorRevision': row['behaviorRevision']}
            validate_preservation(previous, basis)
            previous = basis
        return previous

    def observe_floor(self, floor, approved, observed):
        # Compare only counter floors using the unchanged approved proof structure.
        # Neither persisted historical snapshots nor business fingerprints are rewritten.
        basis = {**approved, 'watchCounters': observed['watchCounters'], 'behaviorRevision': observed['behaviorRevision']}
        validate_preservation(floor, basis)
        return basis

    def watch(self, work, approved, binding):
        started = time.monotonic()
        samples = 0
        previous = self.observation_floor(approved)
        while True:
            self.same_binding(binding)
            observed = extract_object(self.psql(work, 'reconcile.sql'), 'outside')
            validate_postcleanup(approved, observed)
            # The original active approved snapshot remains the preservation baseline.
            previous = self.observe_floor(previous, approved, observed)
            self.same_binding(binding)
            samples += 1
            elapsed = time.monotonic() - started
            durable(self.journal / ('observation-' + str(time.monotonic_ns()) + '.json'), {**self.receipt_common, 'status': 'preserved', 'elapsedSeconds': elapsed, 'snapshotSha256': sha(canonical(observed)), 'watchCounters': observed['watchCounters'], 'behaviorRevision': observed['behaviorRevision']})
            if elapsed >= self.cat['watchSeconds']:
                break
            time.sleep(min(15, self.cat['watchSeconds'] - elapsed))
        durable(self.journal / 'watch-receipt.json', {**self.receipt_common, 'status': 'preserved', 'watchSeconds': int(elapsed), 'samples': samples, 'stableBackend': True, 'preserved': True, 'watchCounters': observed['watchCounters'], 'behaviorRevision': observed['behaviorRevision']})
        self.finalize(observed)

    def finalize(self, observed):
        binding = json_record(self.journal / 'actual-binding.json')
        approved = json_record(self.journal / 'approved-snapshot.json')
        self.same_binding(binding)
        validate_postcleanup(approved, observed)
        self.observe_floor(self.observation_floor(approved), approved, observed)
        intent_hash = sha(private_bytes(self.journal / 'dispatch-intent.json'))
        commit = json_record(self.journal / 'commit-receipt.json')
        watch = json_record(self.journal / 'watch-receipt.json')
        exited = json_record(self.journal / 'postgres-exit.json')
        for item in [json_record(self.journal / 'dispatch-intent.json'), exited, commit, watch]:
            if any(item.get(key) != value for key, value in self.receipt_common.items()):
                refuse('receipt_binding_changed')
        if commit.get('status') != 'committed' or commit.get('postCommitVerified') is not True or commit.get('postgresExitSha256') != sha(private_bytes(self.journal / 'postgres-exit.json')) or exited.get('status') != 'exited' or type(exited.get('exitCode')) is not int or exited['exitCode'] != 0:
            refuse('commit_receipt_required')
        if watch.get('status') != 'preserved' or type(watch.get('watchSeconds')) is not int or watch['watchSeconds'] < self.cat['watchSeconds'] or type(watch.get('samples')) is not int or watch['samples'] < 2 or watch.get('stableBackend') is not True or watch.get('preserved') is not True:
            refuse('watch_receipt_invalid')
        self.observe_floor({**approved, 'watchCounters': watch['watchCounters'], 'behaviorRevision': watch['behaviorRevision']}, approved, observed)
        completed = {**self.receipt_common, 'status': 'completed', 'watchSeconds': watch['watchSeconds'], 'dispatchIntentSha256': intent_hash, 'commitReceiptSha256': sha(private_bytes(self.journal / 'commit-receipt.json')), 'watchReceiptSha256': sha(private_bytes(self.journal / 'watch-receipt.json'))}
        if os.path.lexists(self.journal / 'completed.json'):
            if json_record(self.journal / 'completed.json') != completed:
                refuse('completed_receipt_invalid')
        else:
            durable(self.journal / 'completed.json', completed)
        self.client_absent()
        self.active.unlink()
        sync_directory(self.root)
        print('Owned cleanup completed and observed; private receipts retained.')

    def reconcile(self):
        self.active_fd = os.open(self.active, os.O_RDONLY | os.O_NOFOLLOW)
        fcntl.flock(self.active_fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        reservation = json_record(self.active)
        if reservation.get('status') not in ['prepared', 'reserved'] or type(reservation.get('run')) is not str or not re.fullmatch('[0-9]+-[0-9]+', reservation['run']) or reservation.get('operationId') != self.cat['operationId'] or reservation.get('inputSha256') != self.cat['inputSha256'] or reservation.get('catalogueSha256') != self.catalogue_hash or reservation.get('controllerSha256') != self.env['CLEANUP_CONTROLLER_SHA256'] or reservation.get('proofSha256') != self.env['CLEANUP_PROOF_SHA256'] or not controller_dead(reservation['controller']):
            refuse('reservation_reconciliation_refused')
        self.client_absent()
        intent = self.journal / 'dispatch-intent.json'
        if not os.path.lexists(intent):
            # Frozen prepare/preflight cannot persist business DML. No dispatch means abort,
            # never successful cleanup; preserve evidence and permit a later explicit execution.
            for name in ['postgres-exit.json', 'commit-receipt.json', 'watch-receipt.json', 'completed.json']:
                if os.path.lexists(self.journal / name):
                    refuse('unknown_postdispatch_outcome')
            origin = self.root / 'staging' / reservation['run']
            private_directory(origin)
            if os.path.lexists(self.journal):
                os.rename(self.journal, origin / 'aborted-journal')
            sync_directory(self.root / 'operations')
            reservation_raw = private_bytes(self.active)
            durable(origin / 'aborted-reservation.json', reservation_raw)
            private_directory(self.root / 'aborted', True)
            durable(self.root / 'aborted' / (reservation['run'] + '.json'), {**self.common, 'status': 'aborted-before-dispatch', 'run': reservation['run'], 'reservationSha256': sha(reservation_raw), 'catalogueSha256': self.catalogue_hash, 'controllerSha256': self.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': self.env['CLEANUP_PROOF_SHA256'], 'noDispatchIntent': True, 'controllerDead': True, 'clientAbsent': True})
            self.active.unlink()
            sync_directory(self.root)
            print('Pre-dispatch reservation aborted; no cleanup success claimed.')
            return
        private_directory(self.journal)
        binding = json_record(self.journal / 'actual-binding.json')
        approved = json_record(self.journal / 'approved-snapshot.json')
        exited = json_record(self.journal / 'postgres-exit.json')
        commit = json_record(self.journal / 'commit-receipt.json') if os.path.lexists(self.journal / 'commit-receipt.json') else None
        self.binding_hash = sha(canonical(binding))
        self.receipt_common = {**self.common, 'actualBindingSha256': self.binding_hash, 'approvedSnapshotSha256': sha(canonical(approved)), 'sqlSha256': self.cat['files']['cleanup-commit.sql']}
        if exited.get('status') != 'exited' or type(exited.get('exitCode')) is not int or exited['exitCode'] != 0:
            refuse('commit_receipt_required')
        if commit is not None and (commit.get('status') != 'committed' or commit.get('postgresExitSha256') != sha(private_bytes(self.journal / 'postgres-exit.json')) or commit.get('postCommitVerified') is not True):
            refuse('commit_receipt_required')
        for item in [json_record(intent), exited] + ([commit] if commit is not None else []):
            if any(item.get(key) != value for key, value in self.receipt_common.items()):
                refuse('receipt_binding_changed')
        self.same_binding(binding)
        self.owner_connection()
        with self.work(approved) as work:
            self.database_identity(work)
            observed = extract_object(self.psql(work, 'reconcile.sql'), 'outside')
            validate_postcleanup(approved, observed)
            self.same_binding(binding)
            if commit is None:
                durable(self.journal / 'commit-receipt.json', {**self.receipt_common, 'status': 'committed', 'postgresExitSha256': sha(private_bytes(self.journal / 'postgres-exit.json')), 'postCommitVerified': True, 'derivedFromExitAndReadback': True, 'snapshotSha256': sha(canonical(observed))})
            if os.path.lexists(self.journal / 'completed.json') or os.path.lexists(self.journal / 'watch-receipt.json'):
                self.finalize(observed)
            else:
                # Interrupted observations restart a complete window, without repeating DML.
                self.watch(work, approved, binding)


def main():
    environment = os.environ
    raw = base64.b64decode(environment['CLEANUP_CATALOGUE'], validate=True)
    catalogue = json.loads(raw)
    action, phase = environment['CLEANUP_ACTION'], environment['CLEANUP_PHASE']
    if (action, phase) not in [('execute', 'prepare'), ('execute', 'commit'), ('reconcile', 'reconcile')]:
        refuse('cleanup_action_invalid')
    job = OwnerJob(catalogue, raw, environment)
    job.initialize()
    {'prepare': job.prepare, 'commit': job.commit, 'reconcile': job.reconcile}[phase]()


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        # Provider output/URLs/credentials and raw row details never reach workflow logs.
        code = getattr(error, 'code', 'owned_cleanup_refused')
        if not re.fullmatch('[a-z_]{1,80}', str(code)):
            code = 'owned_cleanup_refused'
        print('Owned cleanup refused: ' + code, file=__import__('sys').stderr)
        raise SystemExit(78) from None
