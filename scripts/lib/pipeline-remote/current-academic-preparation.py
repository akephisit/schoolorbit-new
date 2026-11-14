#!/usr/bin/env python3
"""Fixed 156/157 release preparation; production SQL is read-only.

Invoked only by the immutable serialized release owner after old writers stop.
The only DDL target is this controller's isolated, no-egress PostgreSQL copy.
No SQL, tenant, database, file path or shell command is accepted as CLI input.
"""
import contextlib
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import secrets
import select
import shutil
import stat
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
import uuid

PG_IMAGE = 'public.ecr.aws/docker/library/postgres:18.6-alpine@sha256:d8703cd7fba306b9fec9268ecedfa8a966846c053036a60e3635791957eb2f66'
CHECKS = sorted(['CURRENT_DELIVERY_GRAPH_RECONCILED', 'CURRENT_TEACHERS_RECONCILED', 'CURRENT_TIMETABLE_IDS_RECONCILED', 'CURRENT_FACTS_PRESERVED', 'CURRENT_SUPERVISION_PRESERVED', 'CURRENT_CATALOG_PINS_PRESERVED', 'CURRENT_LEGACY_SCHEMA_REMOVED', 'CURRENT_SCOPE_GUARDS_ENABLED'])
OPERATION = 'academic-current-156157'
MAX_TENANTS = 10000
MAX_JSON = 1024 * 1024
MAX_SECONDS = 1800

# Exact sanitized refusal codes exposed by the canonical runner, by migration.
COPY_REFUSALS = {
    147: frozenset(['ACADEMIC_CONTEXT_147_RUNNING_CONTEXT_MISMATCH', 'ACADEMIC_CONTEXT_DEFAULT_MUST_BE_VISIBLE']),
    148: frozenset(['ACADEMIC_CONTEXT_148_CALENDAR_MATERIALIZATION_TOO_LARGE', 'ACADEMIC_CONTEXT_148_CALENDAR_ROOM_YEAR_MISMATCH', 'ACADEMIC_CONTEXT_148_CALENDAR_YEAR_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_CURRICULUM_CREATED_INTENT_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_POST_CLEANUP_RECONCILIATION_FAILED', 'ACADEMIC_CONTEXT_148_PREPARATION_CREATED_INTENT_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_PREPARATION_REQUEST_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_PROMOTION_ITEM_INTENT_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_PROMOTION_ITEM_SNAPSHOT_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_PROMOTION_RUN_SNAPSHOT_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_RECONCILIATION_FAILED', 'ACADEMIC_CONTEXT_148_TERM_RECEIPT_SOURCE_UNMAPPABLE', 'ACADEMIC_CONTEXT_148_UNKNOWN_DATE_DEPENDENCY', 'ACADEMIC_CONTEXT_148_YEAR_ACTIVATION_RECEIPT_UNMAPPABLE', 'PROMOTION_RUN_HISTORY_RETAINED', 'PROMOTION_RUN_IDENTITY_IMMUTABLE']),
    149: frozenset(['ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID', 'ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED', 'ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY', 'ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE']),
    151: frozenset(['PUBLICATION_151_ASSIGNMENT_CONTEXT_INVALID', 'PUBLICATION_151_INITIAL_SOURCE_UNMAPPABLE', 'PUBLICATION_151_INITIAL_TEACHERS_UNMAPPABLE', 'PUBLICATION_151_PRESERVATION_FAILED', 'PUBLICATION_151_RECONCILIATION_FAILED', 'PUBLICATION_151_SOURCE_LINEAGE_INVALID']),
    152: frozenset(['MANUAL_ROLLOVER152_PRESERVATION_FAILED', 'MANUAL_ROLLOVER_LEGACY_EXECUTION_UNRESOLVED']),
    155: frozenset(['ACADEMIC_SCHOOL_DAYS155_INVALID_YEAR_PRESET', 'ACADEMIC_SCHOOL_DAYS155_MAPPING_FAILED', 'ACADEMIC_SCHOOL_DAYS155_PRESERVATION_FAILED', 'ACADEMIC_SCHOOL_DAYS155_TRIGGER_STATE_INVALID']),
    157: frozenset(['ACADEMIC_CURRENT_ATTENDANCE_EVIDENCE_UNMAPPABLE', 'ACADEMIC_CURRENT_ATTENDANCE_NOTICE_UNMAPPABLE', 'ACADEMIC_CURRENT_ATTENDANCE_SOURCE_AMBIGUOUS', 'ACADEMIC_CURRENT_BEHAVIOR_ARCHIVE_UNMAPPABLE', 'ACADEMIC_CURRENT_BEHAVIOR_EFFECT_PROTECTED', 'ACADEMIC_CURRENT_CATALOG_PIN_MISMATCH', 'ACADEMIC_CURRENT_DELIVERY_SOURCE_AMBIGUOUS', 'ACADEMIC_CURRENT_DELIVERY_SOURCE_MISSING', 'ACADEMIC_CURRENT_EXAM_ITEM_DEPENDENT_UNMAPPABLE', 'ACADEMIC_CURRENT_GRAPH_UNMAPPABLE', 'ACADEMIC_CURRENT_GROUP_ARCHIVE_UNMAPPABLE', 'ACADEMIC_CURRENT_GROUP_RESULTS_UNMAPPABLE', 'ACADEMIC_CURRENT_INACTIVE_SYNC_UNMAPPABLE', 'ACADEMIC_CURRENT_OFFERING_NOT_FOUND', 'ACADEMIC_CURRENT_PREPARATION_MISMATCH', 'ACADEMIC_CURRENT_PREPARATION_REQUIRED', 'ACADEMIC_CURRENT_RECONCILIATION_FAILED', 'ACADEMIC_CURRENT_RESULTS_JSON_UNMAPPABLE', 'ACADEMIC_CURRENT_TIMETABLE_DELIVERY_MISMATCH', 'ACADEMIC_CURRENT_TIMETABLE_SOURCE_AMBIGUOUS', 'ACADEMIC_CURRENT_TIMETABLE_SOURCE_MISSING']),
}


class Refusal(Exception):
    pass


def refuse(code):
    raise Refusal(code)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def valid_hash(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None


def private(path, directory=False):
    try:
        info = path.lstat()
    except OSError:
        refuse('private_path_unavailable')
    expected = 0o700 if directory else 0o600
    if stat.S_ISLNK(info.st_mode) or info.st_uid != os.getuid() or stat.S_IMODE(info.st_mode) != expected or (not stat.S_ISDIR(info.st_mode) if directory else not stat.S_ISREG(info.st_mode)):
        refuse('private_path_invalid')
    return path


def durable(path, value):
    """Publish a complete private file only after its bytes and directory fsync."""
    raw = json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()
    fd, temporary = tempfile.mkstemp(prefix='.publish-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as output:
            os.fchmod(output.fileno(), 0o600)
            output.write(raw)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
        directory = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory)
        finally:
            os.close(directory)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def read_json(path):
    private(path)
    if path.stat().st_size > MAX_JSON:
        refuse('private_evidence_too_large')
    try:
        return json.loads(path.read_bytes())
    except (ValueError, OSError):
        refuse('private_evidence_invalid')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        refuse('admin_redirect_refused')


def inventory(raw):
    if not isinstance(raw, dict) or not isinstance(raw.get('schools'), list) or type(raw.get('total')) is not int or raw['total'] != len(raw['schools']) or raw['total'] > MAX_TENANTS:
        refuse('inventory_invalid')
    ids, names, result = set(), set(), []
    for row in raw['schools']:
        try:
            identifier = str(uuid.UUID(row['id']))
            name, status, url = row['subdomain'], row['status'], row['db_connection_string']
            parsed = urllib.parse.urlsplit(url)
        except (KeyError, ValueError, TypeError):
            refuse('inventory_invalid')
        if identifier == str(uuid.UUID(int=0)) or identifier in ids or name in names or not isinstance(name, str) or not re.fullmatch('[a-z0-9-]{1,63}', name) or not isinstance(status, str) or not status.strip() or parsed.scheme not in ['postgres', 'postgresql'] or not parsed.hostname or not re.fullmatch('[a-zA-Z0-9_]{1,63}', parsed.path[1:]):
            refuse('inventory_invalid')
        ids.add(identifier)
        names.add(name)
        result.append({'id': identifier, 'subdomain': name, 'status': status, 'url': url})
    return sorted(result, key=lambda row: uuid.UUID(row['id']).int)


def inventory_hash(rows):
    value = hashlib.sha256()
    for row in rows:
        route = urllib.parse.urlsplit(row['url'])
        for part in [row['id'], row['subdomain'], row['status'], route.hostname.lower(), str(route.port or 5432), route.path]:
            encoded = part.encode()
            value.update(len(encoded).to_bytes(8, 'big'))
            value.update(encoded)
    return value.hexdigest()


def owner_connection(url, row):
    try:
        parsed = urllib.parse.urlsplit(url)
        query = urllib.parse.parse_qs(parsed.query, strict_parsing=True)
        password = urllib.parse.unquote(parsed.password or '')
        owner = 'so_' + uuid.UUID(row['id']).hex + '_owner'
        runtime = urllib.parse.urlsplit(row['url'])
        same_host = parsed.hostname and runtime.hostname and parsed.hostname.lower() == runtime.hostname.lower()
        same_port = (parsed.port or 5432) == (runtime.port or 5432)
    except (TypeError, ValueError):
        refuse('source_connection_invalid')
    expected_db = runtime.path[1:]
    if parsed.scheme not in ['postgres', 'postgresql'] or parsed.username != owner or parsed.path[1:] != expected_db or not same_host or not same_port or parsed.fragment or not password or any(c in password for c in '\r\n\0') or query != {'sslmode': ['verify-full'], 'sslrootcert': ['/run/secrets/postgresql-ca.crt']}:
        refuse('source_connection_invalid')
    return {'host': parsed.hostname, 'port': str(parsed.port or 5432), 'database': expected_db, 'user': owner, 'password': password}


def ident(value):
    if not isinstance(value, str) or not re.fullmatch('[A-Za-z_][A-Za-z0-9_]{0,62}', value):
        refuse('database_identifier_invalid')
    return '"' + value + '"'


def literal(value):
    return "'" + value.replace("'", "''") + "'"


def verify_audit(raw):
    if not isinstance(raw, dict) or type(raw.get('version')) is not int or raw['version'] < 157 or raw.get('passed') is not True or not valid_hash(raw.get('selectionFingerprint')) or not isinstance(raw.get('cutoverDate'), str):
        refuse('copy_cutover_not_reconciled')
    try:
        dt.date.fromisoformat(raw['cutoverDate'])
    except ValueError:
        refuse('copy_cutover_not_reconciled')
    checks = raw.get('checks')
    if not isinstance(checks, list) or len(checks) != 8 or sorted(c.get('code', '') for c in checks) != CHECKS or any(c.get('passed') is not True or type(c.get('mismatchCount')) is not int or c['mismatchCount'] != 0 for c in checks) or raw.get('runtimeGrants') is not True:
        refuse('copy_cutover_not_reconciled')
    return raw


def copy_refusal(raw):
    if len(raw)>4096: return None
    try: value=json.loads(raw)
    except (ValueError,UnicodeDecodeError): return None
    if not isinstance(value,dict) or set(value)!={'schemaVersion','operation','status','version','sqlstate','code'} or value.get('schemaVersion')!=1 or value.get('operation')!='migrate_tenant_schema' or value.get('status')!='refused': return None
    version=value.get('version');code=value.get('code');sqlstate=value.get('sqlstate')
    if version is not None and (type(version) is not int or not 1<=version<=10000): return None
    if sqlstate is not None and (not isinstance(sqlstate,str) or not re.fullmatch('[0-9A-Z]{5}',sqlstate)): return None
    if not isinstance(code,str): return None
    if code!='unclassified' and code not in COPY_REFUSALS.get(version,frozenset()): return None
    return value


class Controller:
    def __init__(self):
        self.release = os.environ.get('RELEASE_SHA', '')
        self.image_digest = os.environ.get('BACKEND_IMAGE_DIGEST', '')
        self.image = 'ghcr.io/akephisit/schoolorbit-backend-school@' + self.image_digest
        if not re.fullmatch('[0-9a-f]{40}', self.release) or not re.fullmatch('sha256:[0-9a-f]{64}', self.image_digest) or os.getuid() == 0:
            refuse('release_context_invalid')
        self.code = Path(__file__).resolve().parent
        self.root = Path.home() / '.local/share/schoolorbit-academic-recovery'
        self.root.mkdir(mode=0o700, parents=True, exist_ok=True)
        private(self.root, True)
        self.point = self.root / (self.release + '-' + self.image_digest[7:])
        self.point.mkdir(mode=0o700, exist_ok=True)
        private(self.point, True)
        self.key = self.point / 'identity.agekey'
        self.proof = self.point / 'proof.json'
        self.state = self.point / 'state.json'
        self.client = 'schoolorbit-academic-source-' + self.release[:12]
        self.copy = 'schoolorbit-academic-copy-' + self.release[:12]
        self.crypto = 'schoolorbit-academic-crypto-' + self.release[:12]
        self.worker = 'schoolorbit-academic-migration-copy-' + self.release[:12]
        self.verify = 'schoolorbit-academic-archive-check-' + self.release[:12]
        self.volume = 'schoolorbit-academic-copy-data-' + self.release[:12]
        self.secret = self.runtime_value('INTERNAL_API_SECRET')
        self.ca = Path(self.runtime_value('POSTGRES_CA_FILE', default='/opt/stack/postgresql/ca.crt'))
        if not self.ca.is_absolute():
            self.ca = Path('/opt/stack') / self.ca
        self.rows = inventory(self.admin_get('/internal/schools'))
        self.inventory_hash = inventory_hash(self.rows)
        self.exits = 0
        self.reconcile_owned()
        self.compiled=self.compiled_history()

    def reconcile_owned(self):
        path=self.point/'controller.json'
        old=read_json(path) if path.exists() else None
        if old:
            if old.get('releaseSha')!=self.release or old.get('imageDigest')!=self.image_digest or not re.fullmatch('[0-9a-f]{64}',old.get('nonce','')):
                refuse('controller_evidence_invalid')
            process=Path('/proc')/str(old.get('pid'))/'stat'
            if process.exists() and process.read_text().split(') ',1)[1].split()[19]==old.get('startIdentity'):
                refuse('controller_still_running')
            # These resources can only read production or mutate the no-egress
            # local copy. Never reconcile the separate live migration container.
            for name in [self.client,self.crypto,self.worker,self.verify,self.copy]:
                if self.exists('container',name):
                    raw=self.command(['podman','inspect','--format','{{json .Config.Labels}}',name])
                    labels=json.loads(raw)
                    if labels.get('org.schoolorbit.preparation.nonce')!=old['nonce'] or labels.get('org.schoolorbit.preparation.release')!=self.release:
                        refuse('foreign_preparation_resource')
                    self.command(['podman','rm','-f',name])
            if self.exists('volume',self.volume):
                raw=self.command(['podman','volume','inspect','--format','{{json .Labels}}',self.volume])
                labels=json.loads(raw)
                if labels.get('org.schoolorbit.preparation.nonce')!=old['nonce']:
                    refuse('foreign_preparation_volume')
                self.command(['podman','volume','rm',self.volume])
        self.nonce=secrets.token_hex(32)
        durable(path,{'releaseSha':self.release,'imageDigest':self.image_digest,'pid':os.getpid(),'startIdentity':Path('/proc/self/stat').read_text().split(') ',1)[1].split()[19],'nonce':self.nonce})

    def labels(self):
        return ['--label','org.schoolorbit.preparation.release='+self.release,'--label','org.schoolorbit.preparation.nonce='+self.nonce]

    def compiled_history(self):
        # Image SQL files and compiled CLI belong to the same immutable digest.
        # SQLx hashes exact UTF-8 file bytes with SHA-384; no normalization.
        raw=self.command(['podman','run','--log-driver=none','--rm','--name',self.worker,*self.labels(),'--network','none','--read-only',self.image,'/bin/sh','-c','sha384sum /app/migrations/*.sql'])
        history=[]
        for line in raw.decode().splitlines():
            match=re.fullmatch(r'([0-9a-f]{96})  /app/migrations/([0-9]+)_[a-z0-9_]+\.sql',line)
            if not match: refuse('compiled_history_unavailable')
            history.append([int(match[2]),True,match[1]])
        history.sort()
        if len(history)<157 or len(history)>10000 or [r[0] for r in history]!=list(range(1,len(history)+1)): refuse('compiled_history_unavailable')
        return history

    def source_history(self,base,environment):
        exists=self.command(base+['psql','-XAtq','-v','ON_ERROR_STOP=1','-c',"SELECT to_regclass('public._sqlx_migrations') IS NOT NULL"],environment=environment).strip()==b't'
        rows=json.loads(self.command(base+['psql','-XAtq','-v','ON_ERROR_STOP=1','-c',"SELECT COALESCE(jsonb_agg(jsonb_build_array(version,success,encode(checksum,'hex')) ORDER BY version),'[]') FROM _sqlx_migrations"],environment=environment)) if exists else []
        if rows!=self.compiled[:len(rows)] or len(rows)>len(self.compiled): refuse('source_history_invalid')
        return rows[-1][0] if rows else 0

    def runtime_value(self, name, default=None):
        # Match configured(): an absent/empty optional CA uses the canonical
        # default; a present malformed or duplicate value must still refuse.
        script = '. "$1"; if [[ "$4" == optional && -f /opt/stack/.env && ! -L /opt/stack/.env && $(stat -c %a /opt/stack/.env) == 600 ]] && ! grep -q "^${2}=" /opt/stack/.env; then printf "%s" "$3"; else schoolorbit_runtime_env_value "$2" /opt/stack/.env; fi'
        result = subprocess.run(['bash', '-c', script, 'runtime', str(self.code / 'runtime-env.sh'), name, default or '', 'optional' if default is not None else 'required'], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15, check=False)
        if result.returncode or (not result.stdout and default is None) or len(result.stdout) > 65536:
            refuse('runtime_configuration_unavailable')
        return result.stdout.decode() or default

    def admin_get(self, route):
        request = urllib.request.Request('http://127.0.0.1:8080' + route, headers={'X-Internal-Caller': 'backend-school', 'X-Internal-Secret': self.secret})
        try:
            with urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect()).open(request, timeout=15) as response:
                raw = response.read(MAX_JSON + 1)
        except (OSError, ValueError):
            refuse('admin_unavailable')
        if len(raw) > MAX_JSON:
            refuse('inventory_too_large')
        try:
            return json.loads(raw)
        except ValueError:
            refuse('admin_response_invalid')

    def command(self, args, *, data=None, environment=None, limit=MAX_JSON, timeout=MAX_SECONDS):
        try:
            result = subprocess.run(args, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment, timeout=timeout, check=False)
        except (OSError, subprocess.TimeoutExpired):
            refuse('child_outcome_unknown')
        self.exits += 1
        durable(self.point / 'last-child-exit.json', {'schemaVersion': 1, 'sequence': self.exits, 'exitCode': result.returncode, 'stdoutSha256': digest(result.stdout)})
        if result.returncode or len(result.stdout) > limit:
            if args[-1:]==['/app/migrate_tenant_schema']:
                diagnostic=copy_refusal(result.stderr)
                if diagnostic:
                    durable(self.point/'last-copy-refusal.json',diagnostic)
                    failure=Refusal('copy_migration_refused')
                    failure.diagnostic=diagnostic
                    raise failure
            refuse('child_failed')
        return result.stdout

    def exists(self, kind, name):
        result = subprocess.run(['podman', kind, 'exists', name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=15, check=False)
        if result.returncode not in [0, 1]:
            refuse('resource_state_unknown')
        return result.returncode == 0

    def assert_quiesced(self):
        if self.exists('container','schoolorbit-backend-school'):
            refuse('school_writers_not_drained')

    def unchanged_inventory(self):
        self.assert_quiesced()
        # Full equality includes private credentials/routes without writing them.
        if inventory(self.admin_get('/internal/schools')) != self.rows:
            refuse('inventory_changed')

    @contextlib.contextmanager
    def source(self, row):
        metadata = self.admin_get('/internal/schools/' + row['subdomain'])
        if metadata.get('id') != row['id'] or metadata.get('subdomain') != row['subdomain'] or metadata.get('status') != row['status'] or metadata.get('mutation_pending') is True:
            refuse('source_namespace_changed')
        body = self.admin_get('/internal/schools/' + row['subdomain'] + '/migration-connection?include_inactive=true')
        connection = owner_connection(body.get('db_migration_connection_string'), row)
        runtime = Path('/run/user') / str(os.getuid())
        if not runtime.is_dir() or runtime.stat().st_uid != os.getuid():
            refuse('private_runtime_unavailable')
        with tempfile.TemporaryDirectory(prefix='academic-source-', dir=runtime) as temporary:
            work = Path(temporary)
            escape = lambda value: str(value).replace('\\', '\\\\').replace(':', '\\:')
            (work / 'pgpass').write_text(':'.join(escape(connection[k]) for k in ['host', 'port', 'database', 'user', 'password']) + '\n')
            (work / 'pgpass').chmod(0o600)
            environment = {**os.environ, 'PGHOST': connection['host'], 'PGPORT': connection['port'], 'PGDATABASE': connection['database'], 'PGUSER': connection['user'], 'PGPASSFILE': '/run/source/pgpass', 'PGSSLMODE': 'verify-full', 'PGSSLROOTCERT': '/run/secrets/postgresql-ca.crt', 'PGCONNECT_TIMEOUT': '10', 'PGAPPNAME': OPERATION, 'PGOPTIONS': '-c default_transaction_read_only=on -c statement_timeout=900000'}
            args = ['podman', 'run', '--log-driver=none', '--rm', '--name', self.client, *self.labels(), '--network', 'schoolorbit-web', '--read-only', '--cap-drop=all', '--security-opt=no-new-privileges', '--userns=keep-id', '--user', str(os.getuid()) + ':' + str(os.getgid()), '-v', str(work) + ':/run/source:ro', '-v', str(self.ca) + ':/run/secrets/postgresql-ca.crt:ro']
            for key in ['PGHOST', 'PGPORT', 'PGDATABASE', 'PGUSER', 'PGPASSFILE', 'PGSSLMODE', 'PGSSLROOTCERT', 'PGCONNECT_TIMEOUT', 'PGAPPNAME', 'PGOPTIONS']:
                args += ['--env', key]
            yield connection, args + [PG_IMAGE], environment

    def identity(self, connection, base, environment):
        owner = connection['user']
        runtime = owner[:-6] + '_runtime'
        # Only known per-school and existing provisioner role dependencies are
        # supported. Unknown ownership/ACL never disappears via --no-owner.
        sql = "SELECT jsonb_build_object('user',current_user,'database',current_database(),'owner',r.rolname,'marker',shobj_description(r.oid,'pg_authid'),'super',r.rolsuper,'createDb',r.rolcreatedb,'createRole',r.rolcreaterole,'bypass',r.rolbypassrls,'ssl',(SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()),'server',current_setting('server_version_num'),'date',(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date,'roles',(SELECT jsonb_agg(jsonb_build_object('name',rolname,'login',rolcanlogin,'super',rolsuper,'createDb',rolcreatedb,'createRole',rolcreaterole,'inherit',rolinherit,'replication',rolreplication,'bypass',rolbypassrls,'limit',rolconnlimit,'comment',shobj_description(oid,'pg_authid')) ORDER BY rolname) FROM pg_roles WHERE rolname IN(" + ','.join(literal(x) for x in [owner, runtime, 'schoolorbit_provisioner']) + ")),'otherConnections',(SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND backend_type='client backend'),'unknownRoles',(SELECT count(*) FROM pg_shdepend dependency JOIN pg_roles role ON role.oid=dependency.refobjid WHERE dependency.dbid=(SELECT oid FROM pg_database WHERE datname=current_database()) AND dependency.refclassid='pg_authid'::regclass AND role.rolname NOT IN(" + ','.join(literal(x) for x in [owner, runtime, 'schoolorbit_provisioner', 'pg_database_owner']) + "))) FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database();"
        raw = self.command(base + ['psql', '-XAtq', '-v', 'ON_ERROR_STOP=1', '-c', sql], environment=environment)
        try:
            value = json.loads(raw)
        except ValueError:
            refuse('source_identity_unavailable')
        expected = {'user': owner, 'database': connection['database'], 'owner': owner, 'marker': 'schoolorbit:' + str(uuid.UUID(owner[3:-6])) + ':migration', 'super': False, 'createDb': False, 'createRole': False, 'bypass': False, 'ssl': True, 'server': '180006', 'unknownRoles': 0, 'otherConnections': 0}
        if any(value.get(k) != v for k, v in expected.items()) or not isinstance(value.get('roles'), list) or {r['name'] for r in value['roles']} != {owner, runtime, 'schoolorbit_provisioner'}:
            refuse('source_identity_invalid')
        value['version']=self.source_history(base,environment)
        return value

    def age(self, *arguments):
        return ['podman', 'run', '--log-driver=none', '--rm', '-i', '--name', self.crypto, *self.labels(), '--network', 'none', '--read-only', '--cap-drop=all', '--security-opt=no-new-privileges', '--userns=keep-id', '--user', str(os.getuid()) + ':' + str(os.getgid()), '-v', str(self.point) + ':/run/recovery:ro', self.image, *arguments]

    def ensure_key(self):
        if not self.key.exists():
            raw = self.command(self.age('/usr/local/bin/age-keygen'))
            fd = os.open(self.key, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
            with os.fdopen(fd, 'wb') as output:
                output.write(raw)
                output.flush()
                os.fsync(output.fileno())
        private(self.key)
        recipient = self.command(self.age('/usr/local/bin/age-keygen', '-y', '/run/recovery/identity.agekey')).decode().strip()
        if not re.fullmatch('age1[0-9a-z]{58}', recipient):
            refuse('recovery_key_invalid')
        return recipient

    def stream(self, first, second, *, environment=None, source=None, destination=None):
        """Pipe without holding plaintext in Python memory; record actual exits."""
        a = subprocess.Popen(first, stdin=source, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=environment)
        try:
            b = subprocess.Popen(second, stdin=a.stdout, stdout=destination if destination is not None else subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            a.stdout.close()
            try:
                b_exit = b.wait(timeout=MAX_SECONDS)
                a_exit = a.wait(timeout=30)
            except subprocess.TimeoutExpired:
                a.kill()
                b.kill()
                a.wait()
                b.wait()
                refuse('stream_outcome_unknown')
            self.exits += 1
            durable(self.point / 'last-stream-exit.json', {'schemaVersion': 1, 'sequence': self.exits, 'producerExit': a_exit, 'consumerExit': b_exit})
            if a_exit or b_exit:
                refuse('stream_failed')
        finally:
            if a.poll() is None:
                a.kill()
                a.wait()

    def fingerprint(self, base, environment, nonce):
        # pg_dump itself owns normalization. A fixed restrict key is the ONLY
        # formatting input; ownership, ACL, comments and every data byte remain.
        with contextlib.nullcontext():
            child = subprocess.Popen(base + ['pg_dump', '--format=plain', '--no-password', '--restrict-key=' + nonce], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=environment)
            value = hashlib.sha256()
            try:
                deadline=time.monotonic()+MAX_SECONDS
                while True:
                    if time.monotonic()>=deadline:
                        child.kill()
                        child.wait()
                        durable(self.point/'last-fingerprint-exit.json',{'schemaVersion':1,'sequence':self.exits+1,'exitCode':child.returncode,'outcome':'unknown'})
                        refuse('fingerprint_outcome_unknown')
                    ready,_,_=select.select([child.stdout],[],[],min(1,deadline-time.monotonic()))
                    if not ready: continue
                    chunk=os.read(child.stdout.fileno(),1024*1024)
                    if not chunk: break
                    value.update(chunk)
                code = child.wait(timeout=30)
            finally:
                child.stdout.close()
                if child.poll() is None:
                    child.kill()
                    child.wait()
            self.exits += 1
            durable(self.point / 'last-fingerprint-exit.json', {'schemaVersion': 1, 'sequence': self.exits, 'exitCode': code})
            if code:
                refuse('fingerprint_failed')
            return value.hexdigest()

    def copy_sql(self, sql, database='postgres', user='postgres'):
        # The fixed owned container has no egress or production credentials.
        return self.command(['podman', 'exec', '-i', self.copy, 'psql', '-XAtq', '-v', 'ON_ERROR_STOP=1', '-U', user, '-d', database], data=sql.encode())

    @contextlib.contextmanager
    def isolated(self, connection, identity):
        if self.exists('container', self.copy) or self.exists('volume', self.volume):
            refuse('copy_resource_unresolved')
        self.command(['podman', 'volume', 'create', *self.labels(), self.volume])
        password = secrets.token_hex(32)
        environment = {**os.environ, 'POSTGRES_PASSWORD': password}
        created = False
        try:
            self.command(['podman', 'run', '--log-driver=none', '-d', '--name', self.copy, *self.labels(), '--network', 'none', '--env', 'POSTGRES_PASSWORD', '--mount', 'type=volume,source=' + self.volume + ',target=/var/lib/postgresql', PG_IMAGE], environment=environment)
            created = True
            for _ in range(60):
                ready = subprocess.run(['podman', 'exec', self.copy, 'pg_isready', '-U', 'postgres'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5, check=False)
                if ready.returncode == 0:
                    break
                time.sleep(1)
            else:
                refuse('copy_postgres_unavailable')
            roles = []
            for role in identity['roles']:
                flags = ['LOGIN' if role['login'] else 'NOLOGIN', 'SUPERUSER' if role['super'] else 'NOSUPERUSER', 'CREATEDB' if role['createDb'] else 'NOCREATEDB', 'CREATEROLE' if role['createRole'] else 'NOCREATEROLE', 'INHERIT' if role['inherit'] else 'NOINHERIT', 'REPLICATION' if role['replication'] else 'NOREPLICATION', 'BYPASSRLS' if role['bypass'] else 'NOBYPASSRLS']
                roles.append('CREATE ROLE ' + ident(role['name']) + ' ' + ' '.join(flags) + ' CONNECTION LIMIT ' + str(int(role['limit'])) + ' PASSWORD ' + literal(secrets.token_hex(32)) + '; COMMENT ON ROLE ' + ident(role['name']) + ' IS ' + (literal(role['comment']) if role['comment'] is not None else 'NULL') + ';')
            roles.append('GRANT ' + ident(connection['user']) + ' TO schoolorbit_provisioner WITH SET TRUE, INHERIT FALSE;')
            roles.append('CREATE DATABASE ' + ident(connection['database']) + ' OWNER ' + ident(connection['user']) + " TEMPLATE template0 ENCODING 'UTF8' LOCALE_PROVIDER builtin BUILTIN_LOCALE 'C.UTF-8';")
            roles.append('REVOKE ALL ON DATABASE '+ident(connection['database'])+' FROM PUBLIC; GRANT CONNECT ON DATABASE '+ident(connection['database'])+' TO '+ident(connection['user'])+','+ident(connection['user'][:-6]+'_runtime')+';')
            self.copy_sql('\n'.join(roles))
            yield
        finally:
            # Only the exact resources created by this invocation may be removed.
            if created:
                self.command(['podman', 'rm', '-f', self.copy])
            self.command(['podman', 'volume', 'rm', self.volume])

    def restored_fingerprint(self, connection, nonce):
        base = ['podman', 'exec', '--env', 'PGOPTIONS=-c default_transaction_read_only=on', '--env', 'PGUSER='+connection['user'], '--env', 'PGDATABASE='+connection['database'], self.copy]
        return self.fingerprint(base, os.environ.copy(), nonce)

    def restore_and_migrate(self, row, connection, identity, archive, nonce, source_hash):
        with self.isolated(connection, identity):
            with archive.open('rb') as encrypted:
                self.stream(self.age('/usr/local/bin/age', '-d', '-i', '/run/recovery/identity.agekey'), ['podman', 'exec', '-i', self.copy, 'pg_restore', '--exit-on-error', '--single-transaction', '--clean', '--if-exists', '-U', 'postgres', '-d', connection['database']], source=encrypted)
            if self.restored_fingerprint(connection, nonce) != source_hash:
                refuse('restored_source_mismatch')
            # Fresh local role password, never a source password. Only the copy's
            # loopback namespace is shared; it has no egress or host port.
            local_password = secrets.token_hex(32)
            self.copy_sql('ALTER ROLE ' + ident(connection['user']) + ' PASSWORD ' + literal(local_password) + ';', connection['database'])
            local_url = 'postgresql://' + connection['user'] + ':' + local_password + '@127.0.0.1:5432/' + connection['database'] + '?sslmode=disable'
            environment = {**os.environ, 'MIGRATION_SCHEMA_DATABASE_URL': local_url, 'MIGRATION_SCHEMA_NAME': 'public', 'MIGRATION_SCHEMA_ALLOW_PUBLIC': '1'}
            migrated=self.command(['podman', 'run', '--log-driver=none', '--rm', '--name', self.worker, *self.labels(), '--network', 'container:' + self.copy, '--read-only', '--cap-drop=all', '--security-opt=no-new-privileges', '--env', 'MIGRATION_SCHEMA_DATABASE_URL', '--env', 'MIGRATION_SCHEMA_NAME', '--env', 'MIGRATION_SCHEMA_ALLOW_PUBLIC', self.image, '/app/migrate_tenant_schema'], environment=environment)
            try: migrated=json.loads(migrated)
            except ValueError: refuse('copy_runner_evidence_invalid')
            if migrated!={'schemaVersion':1,'operation':'migrate_tenant_schema','status':'migrated','version':self.compiled[-1][0],'checksumsVerified':True}: refuse('copy_runner_evidence_invalid')
            runtime = connection['user'][:-6] + '_runtime'
            sql = "SELECT jsonb_build_object('version',(SELECT max(version) FROM _sqlx_migrations WHERE success),'passed',passed,'selectionFingerprint',source_fingerprint,'cutoverDate',cutover_date,'checks',checks,'runtimeGrants',has_table_privilege(" + literal(runtime) + ", '_sqlx_migrations','SELECT') AND NOT has_table_privilege(" + literal(runtime) + ", '_sqlx_migrations','INSERT,UPDATE,DELETE') AND NOT has_table_privilege(" + literal(runtime) + ", 'academic_result_removal_scope','SELECT,INSERT,UPDATE,DELETE') AND (SELECT bool_and(has_table_privilege(" + literal(runtime) + ", 'learning_offerings',privilege)) FROM unnest(ARRAY['SELECT','INSERT','UPDATE','DELETE']) privilege) AND NOT has_database_privilege(" + literal(runtime) + ",current_database(),'TEMP')) FROM academic_current_operations_migration_audit WHERE migration_version=157;"
            try:
                return verify_audit(json.loads(self.copy_sql(sql, connection['database'], runtime)))
            except ValueError:
                refuse('copy_audit_unavailable')

    def prepare(self):
        if self.exists('container', self.client) or self.exists('container', self.copy) or self.exists('volume', self.volume):
            refuse('previous_controller_resources_unresolved')
        if self.proof.exists():
            self.check()
            return
        pending_rows=[]
        for row in self.rows:
            with self.source(row) as (connection,base,environment):
                if self.identity(connection,base,environment)['version']<157: pending_rows.append(row)
        self.unchanged_inventory()
        if not pending_rows:
            durable(self.state,{'schemaVersion':1,'state':'already-current','completedAt':None,'retainUntil':None})
            print(json.dumps({'schemaVersion':1,'tenants':len(self.rows),'alreadyCurrent':True}))
            return
        recipient = self.ensure_key()
        tenants = []
        for row in pending_rows:
            name = uuid.UUID(row['id']).hex
            archive = self.point / (name + '.dump.age')
            item_path = self.point / (name + '.json')
            with self.source(row) as (connection, base, environment):
                identity = self.identity(connection, base, environment)
                if item_path.exists():
                    item = read_json(item_path)
                    nonce = item['restrictKey']
                    if item.get('roleMetadata')!=identity['roles']: refuse('source_roles_changed')
                    source_hash = self.fingerprint(base, environment, nonce)
                    if source_hash != item['proof']['sourceFingerprint']:
                        refuse('prepared_source_changed')
                    private(archive)
                    if digest_file(archive) != item['proof']['archiveSha256']:
                        refuse('archive_changed')
                else:
                    nonce = secrets.token_hex(32)
                    source_hash = self.fingerprint(base, environment, nonce)
                    if archive.exists():
                        refuse('unresolved_archive')
                    # Bound space before both encrypted backup and copy restoration.
                    size_sql = 'SELECT pg_database_size(current_database());'
                    size = int(self.command(base + ['psql', '-XAtq', '-v', 'ON_ERROR_STOP=1', '-c', size_sql], environment=environment))
                    if shutil.disk_usage(self.root).free < size * 4 + 1024**3:
                        refuse('insufficient_recovery_space')
                    partial=archive.with_suffix('.partial')
                    if partial.exists():
                        private(partial)
                        partial.unlink() # prior read-only producers are proven dead/reconciled
                    fd = os.open(partial, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
                    with os.fdopen(fd, 'wb') as output:
                        self.stream(base + ['pg_dump', '--format=custom', '--no-password'], self.age('/usr/local/bin/age', '-r', recipient), environment=environment, destination=output)
                        output.flush()
                        os.fsync(output.fileno())
                    os.replace(partial,archive)
                    durable(item_path, {'restrictKey': nonce, 'roleMetadata':identity['roles'], 'proof': {'sourceFingerprint': source_hash, 'archiveSha256': digest_file(archive)}})
                # Complete decryption and the actual restore validate the entire
                # encrypted custom archive, not merely its listing/header.
                audit = self.restore_and_migrate(row, connection, identity, archive, nonce, source_hash)
                if identity['version']<157 and audit['cutoverDate'] != identity['date']:
                    refuse('cutover_date_changed')
                if self.fingerprint(base, environment, nonce) != source_hash:
                    refuse('source_changed_during_copy')
                proof = {'schoolId': row['id'], 'subdomain': row['subdomain'], 'databaseName': connection['database'], 'cutoverDate': audit['cutoverDate'], 'selectionFingerprint': audit['selectionFingerprint'], 'sourceFingerprint': source_hash, 'restoredSourceFingerprint': source_hash, 'archiveSha256': digest_file(archive), 'decryptionVerified': True, 'historyVersion': audit['version'], 'checks': audit['checks']}
                durable(item_path, {'restrictKey': nonce, 'roleMetadata':identity['roles'], 'proof': proof})
                tenants.append(proof)
            self.unchanged_inventory()
        durable(self.proof, {'schemaVersion': 1, 'operation': OPERATION, 'releaseSha': self.release, 'imageDigest': self.image_digest, 'inventoryFingerprint': self.inventory_hash, 'tenants': tenants})
        durable(self.state, {'schemaVersion': 1, 'state': 'prepared', 'completedAt': None, 'retainUntil': None})
        self.check()

    def retained_recovery(self):
        # Completion must revalidate the recovery bytes and key independently
        # of the now-intentionally-contracted live business source.
        private(self.key)
        proof = read_json(self.proof)
        if proof.get('schemaVersion') != 1 or proof.get('operation') != OPERATION or proof.get('releaseSha') != self.release or proof.get('imageDigest') != self.image_digest or proof.get('inventoryFingerprint') != self.inventory_hash or not isinstance(proof.get('tenants'),list) or not 1<=len(proof['tenants'])<=len(self.rows):
            refuse('preparation_binding_changed')
        seen=set()
        for item in proof['tenants']:
            if not isinstance(item,dict): refuse('preparation_binding_changed')
            row=next((row for row in self.rows if row['id']==item.get('schoolId') and row['subdomain']==item.get('subdomain')),None)
            if row is None or row['id'] in seen: refuse('preparation_coverage_changed')
            seen.add(row['id'])
            archive = private(self.point / (uuid.UUID(row['id']).hex + '.dump.age'))
            if not valid_hash(item.get('archiveSha256')) or digest_file(archive) != item['archiveSha256']:
                refuse('archive_changed')
            # Verify the retained key still decrypts the complete archive now.
            with archive.open('rb') as encrypted:
                self.stream(self.age('/usr/local/bin/age', '-d', '-i', '/run/recovery/identity.agekey'), ['podman', 'run', '--log-driver=none', '--rm', '-i', '--name', self.verify, *self.labels(), '--network', 'none', '--read-only', PG_IMAGE, 'pg_restore', '--file=/dev/null'], source=encrypted)
        return proof

    def check(self):
        if self.state.exists() and read_json(self.state).get('state')=='already-current':
            for row in self.rows:
                with self.source(row) as (connection,base,environment):
                    if self.identity(connection,base,environment)['version']<157: refuse('current_prefix_changed')
            self.unchanged_inventory()
            return
        proof=self.retained_recovery()
        for row in self.rows:
            item=next((item for item in proof['tenants'] if item['schoolId']==row['id']),None)
            if item is None:
                with self.source(row) as (connection,base,environment):
                    if self.identity(connection,base,environment)['version']<157: refuse('preparation_coverage_changed')
                continue
            state = read_json(self.point / (uuid.UUID(row['id']).hex + '.json'))
            with self.source(row) as (connection, base, environment):
                identity = self.identity(connection, base, environment)
                if identity['version']>=157:
                    audit_sql="SELECT jsonb_build_object('date',cutover_date,'hash',source_fingerprint,'passed',passed,'checks',checks) FROM academic_current_operations_migration_audit WHERE migration_version=157"
                    audit=json.loads(self.command(base+['psql','-XAtq','-v','ON_ERROR_STOP=1','-c',audit_sql],environment=environment))
                    if audit.get('date')!=item['cutoverDate'] or audit.get('hash')!=item['selectionFingerprint'] or audit.get('passed') is not True or audit.get('checks')!=item['checks']:
                        refuse('converted_source_receipt_changed')
                else:
                    if identity['date'] != item['cutoverDate']:
                        refuse('cutover_date_changed')
                    if self.fingerprint(base, environment, state['restrictKey']) != item['sourceFingerprint']:
                        refuse('source_changed_before_migration')
        self.unchanged_inventory()
        print(json.dumps({'schemaVersion': 1, 'tenants': len(self.rows), 'prepared': True}))

    def complete(self):
        if self.state.exists() and read_json(self.state).get('state')=='already-current':
            self.check()
            return
        # No source hash comparison after migration: contraction is intentional.
        proof = self.retained_recovery()
        self.unchanged_inventory()
        for item in proof['tenants']:
            row=next((row for row in self.rows if row['id']==item['schoolId'] and row['subdomain']==item['subdomain']),None)
            if row is None: refuse('completion_inventory_changed')
            with self.source(row) as (connection,base,environment):
                self.identity(connection,base,environment)
                sql="SELECT jsonb_build_object('version',(SELECT max(version) FROM _sqlx_migrations WHERE success),'passed',passed,'selectionFingerprint',source_fingerprint,'cutoverDate',cutover_date,'checks',checks,'runtimeGrants',(SELECT bool_and(has_table_privilege("+literal(connection['user'][:-6]+'_runtime')+",'learning_offerings',privilege)) FROM unnest(ARRAY['SELECT','INSERT','UPDATE','DELETE']) privilege) AND NOT has_database_privilege("+literal(connection['user'][:-6]+'_runtime')+",current_database(),'TEMP') AND NOT has_table_privilege("+literal(connection['user'][:-6]+'_runtime')+",'academic_result_removal_scope','SELECT,INSERT,UPDATE,DELETE') AND NOT has_table_privilege("+literal(connection['user'][:-6]+'_runtime')+",'_sqlx_migrations','INSERT,UPDATE,DELETE')) FROM academic_current_operations_migration_audit WHERE migration_version=157"
                audit=verify_audit(json.loads(self.command(base+['psql','-XAtq','-v','ON_ERROR_STOP=1','-c',sql],environment=environment)))
                if audit['selectionFingerprint']!=item['selectionFingerprint'] or audit['cutoverDate']!=item['cutoverDate'] or audit['checks']!=item['checks']:
                    refuse('completion_cutover_unverified')
        now = dt.datetime.now(dt.timezone.utc)
        previous = read_json(self.state)
        until = now + dt.timedelta(days=7)
        if previous.get('retainUntil'):
            until = max(until, dt.datetime.fromisoformat(previous['retainUntil']))
        durable(self.state, {'schemaVersion': 1, 'state': 'completed', 'completedAt': now.isoformat(), 'retainUntil': until.isoformat()})
        print(json.dumps({'schemaVersion': 1, 'tenants': len(self.rows), 'completed': True}))


def digest_file(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        for part in iter(lambda: source.read(1024 * 1024), b''):
            value.update(part)
    return value.hexdigest()


def main():
    if len(sys.argv) != 2 or sys.argv[1] not in ['prepare', 'check', 'complete']:
        refuse('fixed_operation_required')
    controller = Controller()
    getattr(controller, sys.argv[1])()


if __name__ == '__main__':
    try:
        main()
    except (Refusal, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        code = str(error) if isinstance(error, Refusal) else 'preparation_unavailable'
        body={'schemaVersion':1,'error':code}
        if isinstance(error,Refusal) and getattr(error,'diagnostic',None): body['migrationRefusal']=error.diagnostic
        print(json.dumps(body),file=sys.stderr)
        sys.exit(1)
