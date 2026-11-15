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
import importlib.util

sys.dont_write_bytecode = True
_fidelity_spec = importlib.util.spec_from_file_location('academic_restore_fidelity', Path(__file__).with_name('academic_restore_fidelity.py'))
fidelity = importlib.util.module_from_spec(_fidelity_spec)
_fidelity_spec.loader.exec_module(fidelity)

PG_IMAGE = 'docker.io/library/postgres:18.6-trixie@sha256:e3d24f6906d0760bf4da0dae872415391ededefad23fd471c47dc53b0ddf3c42'
CHECKS = sorted(['CURRENT_DELIVERY_GRAPH_RECONCILED', 'CURRENT_TEACHERS_RECONCILED', 'CURRENT_TIMETABLE_IDS_RECONCILED', 'CURRENT_FACTS_PRESERVED', 'CURRENT_SUPERVISION_PRESERVED', 'CURRENT_CATALOG_PINS_PRESERVED', 'CURRENT_LEGACY_SCHEMA_REMOVED', 'CURRENT_SCOPE_GUARDS_ENABLED'])
OPERATION = 'academic-current-156157'
MAX_TENANTS = 10000
MAX_JSON = 1024 * 1024
MAX_SECONDS = 1800

# Fixed PG18.6 catalog queries; no business rows or credentials are selected.
CATALOG = r"""
SET search_path=pg_catalog;
SELECT jsonb_build_object(
 'relations',(SELECT jsonb_agg(jsonb_build_object(
   'schema',n.nspname,'name',r.relname,'kind',r.relkind,'partition',r.relispartition,
   'inheritance',(SELECT count(*) FROM pg_inherits i WHERE i.inhrelid=r.oid OR i.inhparent=r.oid),
   'columns',(SELECT jsonb_agg(jsonb_build_object(
     'num',a.attnum,'name',a.attname,'dropped',a.attisdropped,'notnull',a.attnotnull,
     'typeSchema',tn.nspname,'typeName',t.typname,'typeKind',t.typtype,'typeCategory',t.typcategory,
     'typeMod',a.atttypmod,'dimensions',a.attndims,'identity',a.attidentity,'generated',a.attgenerated,
     'collationSchema',cn.nspname,'collationName',co.collname,
     'collationEncoding',co.collencoding,'collationProvider',co.collprovider,
     'collationDeterministic',co.collisdeterministic,'collationLocale',co.colllocale,
     'collationCollate',co.collcollate,'collationCType',co.collctype,'collationRules',co.collicurules,
     'collationVersion',co.collversion,
     'elementSchema',en.nspname,'elementName',et.typname,'elementKind',et.typtype)
     ORDER BY a.attnum) FROM pg_attribute a LEFT JOIN pg_type t ON t.oid=a.atttypid
     LEFT JOIN pg_namespace tn ON tn.oid=t.typnamespace
     LEFT JOIN pg_collation co ON co.oid=a.attcollation LEFT JOIN pg_namespace cn ON cn.oid=co.collnamespace
     LEFT JOIN pg_type et ON et.oid=t.typelem LEFT JOIN pg_namespace en ON en.oid=et.typnamespace
     WHERE a.attrelid=r.oid AND a.attnum>0)) ORDER BY n.nspname,r.relname)
   FROM pg_class r JOIN pg_namespace n ON n.oid=r.relnamespace WHERE n.nspname='public' AND r.relkind IN('r','p','f')),
 'constraints',(SELECT jsonb_agg(jsonb_build_object(
   'schema',n.nspname,'table',r.relname,'name',c.conname,'kind',c.contype,
   'domain',c.contypid<>0,'parent',c.conparentid<>0,'index',c.conindid<>0,
   'foreignRelation',c.confrelid<>0,
   'flags',to_jsonb(c)-ARRAY['oid','conname','connamespace','conrelid','contypid','conindid','conparentid','confrelid','conkey','confkey','conbin'],
   'comment',obj_description(c.oid,'pg_constraint'),
   'keys',(SELECT jsonb_agg(a.attname ORDER BY k.ord) FROM unnest(c.conkey) WITH ORDINALITY k(num,ord) JOIN pg_attribute a ON a.attrelid=c.conrelid AND a.attnum=k.num),
   'rawKeys',c.conkey,'expr',pg_get_expr(c.conbin,c.conrelid,false),'definition',pg_get_constraintdef(c.oid,false),'tree',c.conbin::text,
   'dependencies',(SELECT jsonb_agg(jsonb_build_object(
      'kind',d.deptype,'class',cl.relname,
      'object',CASE WHEN d.refclassid='pg_class'::regclass AND d.refobjid=c.conrelid THEN '<own-relation>' ELSE (pg_identify_object(d.refclassid,d.refobjid,d.refobjsubid)).identity END,
      'column',CASE WHEN d.refclassid='pg_class'::regclass AND d.refobjid=c.conrelid AND d.refobjsubid<>0 THEN (SELECT a.attname FROM pg_attribute a WHERE a.attrelid=c.conrelid AND a.attnum=d.refobjsubid) ELSE NULL END)
      ORDER BY d.deptype,cl.relname,CASE WHEN d.refclassid='pg_class'::regclass AND d.refobjid=c.conrelid THEN '<own-relation>' ELSE (pg_identify_object(d.refclassid,d.refobjid,d.refobjsubid)).identity END,
      CASE WHEN d.refclassid='pg_class'::regclass AND d.refobjid=c.conrelid AND d.refobjsubid<>0 THEN (SELECT a.attname FROM pg_attribute a WHERE a.attrelid=c.conrelid AND a.attnum=d.refobjsubid) ELSE NULL END)
     FROM pg_depend d JOIN pg_class cl ON cl.oid=d.refclassid WHERE d.classid='pg_constraint'::regclass AND d.objid=c.oid))
    ORDER BY n.nspname,r.relname,c.conname)
    FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace LEFT JOIN pg_class r ON r.oid=c.conrelid
    WHERE n.nspname='public' AND c.contype IN('c','n')));
"""

DATABASE_SQL = r"""
SET search_path=pg_catalog;
SELECT jsonb_build_object('metadata',to_jsonb(d)-ARRAY['oid','datdba','datfrozenxid','datminmxid','dattablespace','datacl'],
 'owner',pg_get_userbyid(d.datdba),'tablespace',(SELECT spcname FROM pg_tablespace WHERE oid=d.dattablespace),
 'acl',(SELECT jsonb_agg(jsonb_build_array(CASE WHEN x.grantee=0 THEN '<PUBLIC>' ELSE pg_get_userbyid(x.grantee) END,pg_get_userbyid(x.grantor),x.privilege_type,x.is_grantable) ORDER BY CASE WHEN x.grantee=0 THEN '<PUBLIC>' ELSE pg_get_userbyid(x.grantee) END,pg_get_userbyid(x.grantor),x.privilege_type,x.is_grantable) FROM aclexplode(COALESCE(d.datacl,acldefault('d',d.datdba))) x),
 'settings',(SELECT jsonb_agg(jsonb_build_object('global',s.setdatabase=0,'role',CASE WHEN s.setrole=0 THEN '<ALL>' ELSE pg_get_userbyid(s.setrole) END,'config',s.setconfig) ORDER BY s.setdatabase=0,CASE WHEN s.setrole=0 THEN '<ALL>' ELSE pg_get_userbyid(s.setrole) END) FROM pg_db_role_setting s WHERE s.setdatabase=d.oid OR (s.setdatabase=0 AND (s.setrole=0 OR s.setrole IN (SELECT oid FROM pg_roles WHERE rolname IN(pg_get_userbyid(d.datdba),regexp_replace(pg_get_userbyid(d.datdba),'_owner$','_runtime'),'schoolorbit_provisioner'))))))
FROM pg_database d WHERE d.datname=current_database();
"""

INDEX_SQL = r"""
SET search_path=pg_catalog;
SELECT COALESCE(jsonb_agg(jsonb_build_object(
 'schema',n.nspname,'table',t.relname,'name',r.relname,'definition',pg_get_indexdef(r.oid),
 'predicate',pg_get_expr(i.indpred,i.indrelid,false),'tree',i.indpred::text,'expression',i.indexprs IS NOT NULL,
 'flags',to_jsonb(i)-ARRAY['indexrelid','indrelid','indcollation','indclass','indkey','indexprs','indpred'],
 'owner',pg_get_userbyid(r.relowner),'method',am.amname,'kind',r.relkind,'partition',r.relispartition,
 'persistence',r.relpersistence,'options',r.reloptions,'tablespace',(SELECT spcname FROM pg_tablespace WHERE oid=r.reltablespace),
 'comment',obj_description(r.oid,'pg_class'),
 'keys',(SELECT jsonb_agg(jsonb_build_object('column',a.attname,'number',k.ord,
   'opclassSchema',onsp.nspname,'opclassName',op.opcname,'opclassDefault',op.opcdefault,
   'opclassInput',(pg_identify_object('pg_type'::regclass,op.opcintype,0)).identity,
   'opclassFamily',(pg_identify_object('pg_opfamily'::regclass,op.opcfamily,0)).identity,
   'collationSchema',cn.nspname,'collationName',co.collname,
   'option',i.indoption[k.ord-1],
   'attributeOptions',(SELECT aa.attoptions FROM pg_attribute aa WHERE aa.attrelid=r.oid AND aa.attnum=k.ord)) ORDER BY k.ord)
   FROM unnest(i.indkey) WITH ORDINALITY k(num,ord)
   LEFT JOIN pg_attribute a ON a.attrelid=t.oid AND a.attnum=k.num
   LEFT JOIN pg_opclass op ON op.oid=i.indclass[k.ord-1] LEFT JOIN pg_namespace onsp ON onsp.oid=op.opcnamespace
   LEFT JOIN pg_collation co ON co.oid=i.indcollation[k.ord-1] LEFT JOIN pg_namespace cn ON cn.oid=co.collnamespace),
 'dependencies',(SELECT jsonb_agg(jsonb_build_object('kind',x.deptype,'class',cl.relname,
   'object',CASE WHEN x.refclassid='pg_class'::regclass AND x.refobjid=t.oid THEN '<own-relation>' ELSE (pg_identify_object(x.refclassid,x.refobjid,x.refobjsubid)).identity END,
   'column',CASE WHEN x.refclassid='pg_class'::regclass AND x.refobjid=t.oid AND x.refobjsubid<>0 THEN (SELECT a.attname FROM pg_attribute a WHERE a.attrelid=t.oid AND a.attnum=x.refobjsubid) ELSE NULL END)
   ORDER BY x.deptype,cl.relname,CASE WHEN x.refclassid='pg_class'::regclass AND x.refobjid=t.oid THEN '<own-relation>' ELSE (pg_identify_object(x.refclassid,x.refobjid,x.refobjsubid)).identity END,
   CASE WHEN x.refclassid='pg_class'::regclass AND x.refobjid=t.oid AND x.refobjsubid<>0 THEN (SELECT a.attname FROM pg_attribute a WHERE a.attrelid=t.oid AND a.attnum=x.refobjsubid) ELSE NULL END)
   FROM pg_depend x JOIN pg_class cl ON cl.oid=x.refclassid WHERE x.classid='pg_class'::regclass AND x.objid=r.oid))
 ORDER BY n.nspname,t.relname,r.relname),'[]')
FROM pg_index i JOIN pg_class r ON r.oid=i.indexrelid JOIN pg_class t ON t.oid=i.indrelid JOIN pg_namespace n ON n.oid=t.relnamespace JOIN pg_am am ON am.oid=r.relam
WHERE n.nspname='public';
"""

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


def read_json(path, limit=MAX_JSON):
    private(path)
    if path.stat().st_size > limit:
        refuse('private_evidence_too_large')
    try:
        return decode_json(path.read_bytes())
    except (ValueError, OSError):
        refuse('private_evidence_invalid')


def decode_json(raw):
    def object_pairs(pairs):
        result={}
        for key,value in pairs:
            if key in result:refuse('private_evidence_invalid')
            result[key]=value
        return result
    def nonfinite(_):
        refuse('private_evidence_invalid')
    return json.loads(raw,object_pairs_hook=object_pairs,parse_constant=nonfinite)


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
        self.diagnostic_stage='source_profile'
        owner = connection['user']
        runtime = owner[:-6] + '_runtime'
        # Only known per-school and existing provisioner role dependencies are
        # supported. Unknown ownership/ACL never disappears via --no-owner.
        sql = "SELECT jsonb_build_object('user',current_user,'database',current_database(),'owner',r.rolname,'marker',shobj_description(r.oid,'pg_authid'),'super',r.rolsuper,'createDb',r.rolcreatedb,'createRole',r.rolcreaterole,'bypass',r.rolbypassrls,'ssl',(SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid()),'server',current_setting('server_version_num'),'date',(clock_timestamp() AT TIME ZONE 'Asia/Bangkok')::date,'roles',(SELECT jsonb_agg(jsonb_build_object('name',rolname,'login',rolcanlogin,'super',rolsuper,'createDb',rolcreatedb,'createRole',rolcreaterole,'inherit',rolinherit,'replication',rolreplication,'bypass',rolbypassrls,'limit',rolconnlimit,'comment',shobj_description(oid,'pg_authid')) ORDER BY rolname) FROM pg_roles WHERE rolname IN(" + ','.join(literal(x) for x in [owner, runtime, 'schoolorbit_provisioner']) + ")),'otherConnections',(SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND pid<>pg_backend_pid() AND backend_type='client backend'),'unknownRoles',(SELECT count(*) FROM pg_shdepend dependency JOIN pg_roles role ON role.oid=dependency.refobjid WHERE dependency.dbid=(SELECT oid FROM pg_database WHERE datname=current_database()) AND dependency.refclassid='pg_authid'::regclass AND role.rolname NOT IN(" + ','.join(literal(x) for x in [owner, runtime, 'schoolorbit_provisioner', 'pg_database_owner']) + "))) FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database();"
        raw = self.bounded_capture(base + ['psql', '-XAtq', '-v', 'ON_ERROR_STOP=1', '-c', sql], environment=environment,limit=MAX_JSON)
        try:
            value = json.loads(raw)
        except ValueError:
            refuse('source_identity_unavailable')
        expected = {'user': owner, 'database': connection['database'], 'owner': owner, 'marker': 'schoolorbit:' + str(uuid.UUID(owner[3:-6])) + ':migration', 'super': False, 'createDb': False, 'createRole': False, 'bypass': False, 'ssl': True, 'server': '180006', 'unknownRoles': 0, 'otherConnections': 0}
        if any(value.get(k) != v for k, v in expected.items()) or not isinstance(value.get('roles'), list) or {r['name'] for r in value['roles']} != {owner, runtime, 'schoolorbit_provisioner'}:
            refuse('source_identity_invalid')
        value['version']=self.source_history(base,environment)
        membership_sql="SET search_path=pg_catalog; SELECT COALESCE(jsonb_agg(jsonb_build_object('role',pg_get_userbyid(m.roleid),'member',pg_get_userbyid(m.member),'grantor',pg_get_userbyid(m.grantor),'admin',m.admin_option,'inherit',m.inherit_option,'set',m.set_option) ORDER BY pg_get_userbyid(m.roleid),pg_get_userbyid(m.member),pg_get_userbyid(m.grantor)),'[]') FROM pg_auth_members m WHERE m.roleid IN (SELECT oid FROM pg_roles WHERE rolname IN("+literal(owner)+","+literal(runtime)+")) OR m.member IN (SELECT oid FROM pg_roles WHERE rolname IN("+literal(owner)+","+literal(runtime)+"))"
        value['memberships']=decode_json(self.bounded_capture(base+['psql','-XAtq','-v','ON_ERROR_STOP=1','-c',membership_sql],environment=environment,limit=MAX_JSON))
        known={owner,runtime,'schoolorbit_provisioner'}
        fidelity.require(isinstance(value['memberships'],list) and len(value['memberships'])<=fidelity.MAX_OBJECTS)
        for membership in value['memberships']:
            fidelity.require(set(membership)=={'role','member','grantor','admin','inherit','set'} and membership['role'] in known and membership['member'] in known and isinstance(membership['grantor'],str) and all(type(membership[x]) is bool for x in ['admin','inherit','set']))
        fidelity.validate_profiles(value['roles'],value['memberships'])
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

    def fingerprint(self, base, environment, nonce, evidence=None, side='source'):
        self.diagnostic_stage='copy_stream' if side=='restored' else 'source_stream'
        # Raw SHA covers untouched pg_dump bytes (fixed restrict key only).
        # Evidence mode separately frames proven expression spans for canonical
        # SHA; it never changes the bytes contributing to the raw digest.
        child=subprocess.Popen(base+['pg_dump','--format=plain','--no-password','--restrict-key='+nonce],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,env=environment)
        value=hashlib.sha256() if evidence is None else fidelity.DumpFingerprint(evidence,side)
        completed=False
        try:
            deadline=time.monotonic()+MAX_SECONDS
            while True:
                if time.monotonic()>=deadline:refuse('fingerprint_outcome_unknown')
                ready,_,_=select.select([child.stdout],[],[],min(1,deadline-time.monotonic()))
                if not ready:continue
                chunk=os.read(child.stdout.fileno(),1024*1024)
                if not chunk:break
                if evidence is None:value.update(chunk)
                else:value.feed(chunk)
            code=child.wait(timeout=30);completed=True
            if code:refuse('fingerprint_failed')
            return value.hexdigest() if evidence is None else value.finish()
        finally:
            child.stdout.close()
            if child.poll() is None:child.kill();child.wait()
            self.exits+=1
            receipt={'schemaVersion':1,'sequence':self.exits,'exitCode':child.returncode}
            if not completed:receipt['outcome']='unknown'
            durable(self.point/'last-fingerprint-exit.json',receipt)

    def copy_sql(self, sql, database='postgres', user='postgres'):
        # The fixed owned container has no egress or production credentials.
        return self.command(['podman', 'exec', '-i', self.copy, 'psql', '-XAtq', '-v', 'ON_ERROR_STOP=1', '-U', user, '-d', database], data=sql.encode())

    def copy_role_profile(self, owner):
        self.diagnostic_stage='copy_profile'
        names=[owner,owner[:-6]+'_runtime','schoolorbit_provisioner']
        selected=','.join(literal(name) for name in names)
        target=','.join(literal(name) for name in names[:2])
        sql="SET search_path=pg_catalog; SELECT jsonb_build_object('roles',(SELECT jsonb_agg(jsonb_build_object('name',rolname,'login',rolcanlogin,'super',rolsuper,'createDb',rolcreatedb,'createRole',rolcreaterole,'inherit',rolinherit,'replication',rolreplication,'bypass',rolbypassrls,'limit',rolconnlimit,'comment',shobj_description(oid,'pg_authid')) ORDER BY rolname) FROM pg_roles WHERE rolname IN("+selected+")),'memberships',(SELECT COALESCE(jsonb_agg(jsonb_build_object('role',pg_get_userbyid(m.roleid),'member',pg_get_userbyid(m.member),'grantor',pg_get_userbyid(m.grantor),'admin',m.admin_option,'inherit',m.inherit_option,'set',m.set_option) ORDER BY pg_get_userbyid(m.roleid),pg_get_userbyid(m.member),pg_get_userbyid(m.grantor)),'[]') FROM pg_auth_members m WHERE m.roleid IN(SELECT oid FROM pg_roles WHERE rolname IN("+target+")) OR m.member IN(SELECT oid FROM pg_roles WHERE rolname IN("+target+"))));"
        raw=self.bounded_capture(['podman','exec','-i',self.copy,'psql','-XAtq','-v','ON_ERROR_STOP=1','-U','postgres','-d','postgres'],data=sql.encode(),limit=MAX_JSON)
        profile=decode_json(raw)
        fidelity.require(isinstance(profile,dict) and set(profile)=={'roles','memberships'})
        fidelity.supported_profile(owner,profile['roles'],profile['memberships'])
        return profile

    def verify_copy_profile(self, connection, identity):
        self.diagnostic_stage='copy_profile'
        profile=self.copy_role_profile(connection['user'])
        fidelity.require(fidelity.exact(profile,{'roles':identity['roles'],'memberships':identity['memberships']}),'retained',('profile',))
        return profile

    def migrate_copy(self, connection, identity, environment):
        self.diagnostic_stage='copy_profile'
        # This readback happens after restore/oracle, at the actual CLI boundary.
        profile=self.verify_copy_profile(connection,identity)
        raw=self.command(['podman', 'run', '--log-driver=none', '--rm', '--name', self.worker, *self.labels(), '--network', 'container:' + self.copy, '--read-only', '--cap-drop=all', '--security-opt=no-new-privileges', '--env', 'MIGRATION_SCHEMA_DATABASE_URL', '--env', 'MIGRATION_SCHEMA_NAME', '--env', 'MIGRATION_SCHEMA_ALLOW_PUBLIC', self.image, '/app/migrate_tenant_schema'], environment=environment)
        return raw,profile

    def bounded_capture(self, args, data=b'', environment=None, limit=fidelity.MAX_METADATA):
        """Bound stdout while reading, and feed oracle SQL without pipe deadlock."""
        child = subprocess.Popen(args, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, env=environment)
        output = bytearray()
        position = 0
        recorded = False
        deadline = time.monotonic()+MAX_SECONDS
        os.set_blocking(child.stdin.fileno(), False)
        os.set_blocking(child.stdout.fileno(), False)
        try:
            while True:
                if time.monotonic() >= deadline:
                    refuse('child_outcome_unknown')
                readable, writable, _ = select.select([child.stdout], [child.stdin] if not child.stdin.closed else [], [], 1)
                if writable:
                    if position == len(data):
                        child.stdin.close()
                    else:
                        try:
                            position += os.write(child.stdin.fileno(), data[position:position+65536])
                        except BlockingIOError:
                            pass
                        except BrokenPipeError:
                            child.stdin.close()
                if readable:
                    chunk = os.read(child.stdout.fileno(), 65536)
                    if not chunk:
                        break
                    if len(output)+len(chunk) > limit:
                        refuse('private_evidence_too_large')
                    output.extend(chunk)
            code = child.wait(timeout=30)
            self.exits += 1
            durable(self.point/'last-metadata-exit.json', {'schemaVersion':1,'sequence':self.exits,'exitCode':code,'stdoutSha256':digest(output)})
            recorded = True
            if code or position != len(data):
                refuse('child_failed')
            return bytes(output)
        finally:
            if not child.stdin.closed:
                child.stdin.close()
            child.stdout.close()
            if child.poll() is None:
                child.kill()
                child.wait()
            if not recorded:
                self.exits += 1
                durable(self.point/'last-metadata-exit.json',{'schemaVersion':1,'sequence':self.exits,'exitCode':child.returncode,'outcome':'unknown'})

    def catalog(self, base, environment=None):
        self.diagnostic_stage='copy_catalog' if base[1:2]==['exec'] else 'source_catalog'
        # Count and expression-size preflight precede the larger fixed metadata
        # queries. The bounded reader is an independent hard output ceiling.
        preflight = "SET search_path=pg_catalog; SELECT jsonb_build_object('objects',(SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public')+(SELECT count(*) FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace WHERE n.nspname='public')+(SELECT count(*) FROM pg_attribute a JOIN pg_class c ON c.oid=a.attrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public' AND a.attnum>0),'expressionBytes',greatest((SELECT COALESCE(max(octet_length(pg_get_expr(c.conbin,c.conrelid,false))),0) FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace WHERE n.nspname='public'),(SELECT COALESCE(max(octet_length(pg_get_expr(i.indpred,i.indrelid,false))),0) FROM pg_index i JOIN pg_class c ON c.oid=i.indrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public')))"
        def query(sql, limit=fidelity.MAX_METADATA):
            raw=self.bounded_capture(base+['psql','-XAtq','-v','ON_ERROR_STOP=1','-c',sql],environment=environment,limit=limit)
            return decode_json(raw)
        size=query(preflight,4096)
        fidelity.require(type(size['objects']) is int and size['objects']<=fidelity.MAX_OBJECTS and type(size['expressionBytes']) is int and size['expressionBytes']<=fidelity.MAX_EXPRESSION,
                         'catalog_budget',tuple(key for key,limit in [('objects',fidelity.MAX_OBJECTS),('expressionBytes',fidelity.MAX_EXPRESSION)] if type(size[key]) is not int or size[key]>limit))
        # Bound variable-width strings and dependency count before jsonb_agg
        # materializes the full catalog. Sixfold accounts for JSON escaping;
        # fixed property/flag overhead is bounded per catalog object/dependency.
        budget_sql=r"""SET search_path=pg_catalog;
WITH relations AS (SELECT c.oid FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='public'),
constraints AS (SELECT c.* FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace WHERE n.nspname='public'),
dependencies AS (SELECT d.* FROM pg_depend d WHERE (d.classid='pg_constraint'::regclass AND d.objid IN(SELECT oid FROM constraints)) OR (d.classid='pg_class'::regclass AND d.objid IN(SELECT oid FROM relations)))
SELECT jsonb_build_object('dependencies',(SELECT count(*) FROM dependencies),'bytes',
 (SELECT COALESCE(sum(octet_length(c::text)+COALESCE(octet_length(pg_get_expr(c.conbin,c.conrelid,false)),0)+octet_length(pg_get_constraintdef(c.oid,false))+COALESCE(octet_length(obj_description(c.oid,'pg_constraint')),0)),0) FROM constraints c)
 +(SELECT COALESCE(sum(octet_length(i::text)+octet_length(pg_get_indexdef(i.indexrelid))+COALESCE(octet_length(pg_get_expr(i.indpred,i.indrelid,false)),0)+COALESCE(octet_length(obj_description(i.indexrelid,'pg_class')),0)),0) FROM pg_index i WHERE i.indrelid IN(SELECT oid FROM relations))
 +(SELECT COALESCE(sum(octet_length(a.attname::text)+octet_length(t.typname::text)+octet_length(n.nspname::text)+COALESCE(octet_length(co::text),0)+COALESCE(octet_length(en.nspname::text),0)+COALESCE(octet_length(et.typname::text),0)),0) FROM pg_attribute a LEFT JOIN pg_type t ON t.oid=a.atttypid LEFT JOIN pg_namespace n ON n.oid=t.typnamespace LEFT JOIN pg_collation co ON co.oid=a.attcollation LEFT JOIN pg_type et ON et.oid=t.typelem LEFT JOIN pg_namespace en ON en.oid=et.typnamespace WHERE a.attrelid IN(SELECT oid FROM relations) AND a.attnum>0)
 +(SELECT COALESCE(sum(octet_length((pg_identify_object(d.refclassid,d.refobjid,d.refobjsubid)).identity)),0) FROM dependencies d)
 +(SELECT octet_length(d::text) FROM pg_database d WHERE datname=current_database()));"""
        budget=query(budget_sql,4096)
        fidelity.require(type(budget['dependencies']) is int and budget['dependencies']<=fidelity.MAX_OBJECTS and type(budget['bytes']) is int and budget['bytes']*6+size['objects']*1024+budget['dependencies']*256<=fidelity.MAX_METADATA,
                         'catalog_budget',tuple(key for key,bad in [('dependencies',type(budget['dependencies']) is not int or budget['dependencies']>fidelity.MAX_OBJECTS),('bytes',type(budget['bytes']) is not int or (type(budget['dependencies']) is int and budget['bytes']*6+size['objects']*1024+budget['dependencies']*256>fidelity.MAX_METADATA))] if bad))
        result={'catalog':query(CATALOG),'database':query(DATABASE_SQL),'indexes':query(INDEX_SQL)}
        result['catalog']['constraintNames']=query("SET search_path=pg_catalog; SELECT COALESCE(jsonb_agg(jsonb_build_object('schema',n.nspname,'table',r.relname,'name',c.conname,'kind',c.contype) ORDER BY n.nspname,r.relname,c.conname),'[]') FROM pg_constraint c JOIN pg_namespace n ON n.oid=c.connamespace LEFT JOIN pg_class r ON r.oid=c.conrelid WHERE n.nspname='public'")
        fidelity.require(len(fidelity.encoded(result))<=fidelity.MAX_METADATA,'catalog_budget',('encodedBytes',))
        return result

    def copy_read_base(self, connection):
        return ['podman','exec','--env','PGOPTIONS=-c default_transaction_read_only=on','--env','PGUSER='+connection['user'],'--env','PGDATABASE='+connection['database'],self.copy]

    def copy_write(self, sql, connection):
        fidelity.require(len(sql.encode())<=fidelity.MAX_METADATA)
        raw=self.bounded_capture(['podman','exec','-i',self.copy,'psql','-XAtq','-v','ON_ERROR_STOP=1','-U',connection['user'],'-d',connection['database']],data=sql.encode())
        return [decode_json(line) for line in raw.splitlines() if line]

    def oracle(self, pairs, connection):
        self.diagnostic_stage='oracle'
        tables={}
        statements=['BEGIN; SET LOCAL search_path=pg_catalog;']
        for pair in pairs:
            row=pair['source'];key=(row['schema'],row['table'])
            if key not in tables:
                name='academic_restore_reference_'+str(len(tables));tables[key]=name
                statements.append('CREATE TEMP TABLE '+ident(name)+' (LIKE '+ident(key[0])+'.'+ident(key[1])+') ON COMMIT DROP;')
        for number,pair in enumerate(pairs):
            row=pair['source'];ref=tables[(row['schema'],row['table'])];name='academic_restore_expression_'+str(number)
            if pair['kind']=='check':
                statements.append('ALTER TABLE pg_temp.'+ident(ref)+' ADD CONSTRAINT '+ident(name)+' CHECK ('+row['expr']+')'+(' NO INHERIT' if row['flags']['connoinherit'] else '')+' NOT VALID;')
                relation='c.conrelid';tree='c.conbin';object_id='c.oid';object_class='pg_constraint'
                selection="pg_constraint c WHERE c.conrelid="+literal('pg_temp.'+ref)+"::regclass AND c.conname="+literal(name)
            else:
                keys=[]
                for a in row['keys']:
                    term=ident(a['column'])
                    if a['collationName'] is not None:term+=' COLLATE '+ident(a['collationSchema'])+'.'+ident(a['collationName'])
                    term+=' '+ident(a['opclassSchema'])+'.'+ident(a['opclassName'])
                    term+=' DESC' if a['option']&1 else ' ASC'
                    term+=' NULLS FIRST' if a['option']&2 else ' NULLS LAST'
                    keys.append(term)
                statements.append('CREATE INDEX '+ident(name)+' ON pg_temp.'+ident(ref)+' USING btree ('+', '.join(keys)+') WHERE ('+row['predicate']+');')
                relation='i.indrelid';tree='i.indpred';object_id='i.indexrelid';object_class='pg_class'
                selection='pg_index i WHERE i.indexrelid='+literal('pg_temp.'+name)+'::regclass'
            actual=literal(ident(row['schema'])+'.'+ident(row['table']))+'::regclass'
            columns=lambda rel:"(SELECT jsonb_agg(jsonb_build_array(a.attnum,a.attname,a.atttypid,a.atttypmod,a.attcollation) ORDER BY a.attnum) FROM pg_attribute a WHERE a.attrelid="+rel+" AND a.attnum>0 AND NOT a.attisdropped)"
            dep_object="CASE WHEN x.refclassid='pg_class'::regclass AND x.refobjid="+relation+" THEN '<own-relation>' ELSE (pg_identify_object(x.refclassid,x.refobjid,x.refobjsubid)).identity END"
            dep_column="CASE WHEN x.refclassid='pg_class'::regclass AND x.refobjid="+relation+" AND x.refobjsubid<>0 THEN (SELECT a.attname FROM pg_attribute a WHERE a.attrelid="+relation+" AND a.attnum=x.refobjsubid) ELSE NULL END"
            dependencies="(SELECT jsonb_agg(jsonb_build_object('kind',x.deptype,'class',cl.relname,'object',"+dep_object+",'column',"+dep_column+") ORDER BY x.deptype,cl.relname,"+dep_object+","+dep_column+") FROM pg_depend x JOIN pg_class cl ON cl.oid=x.refclassid WHERE x.classid="+literal(object_class)+"::regclass AND x.objid="+object_id+")"
            statements.append("SELECT jsonb_build_object('number',"+str(number)+",'tree',"+tree+"::text,'columns',"+columns(relation)+",'actualColumns',"+columns(actual)+",'dependencies',"+dependencies+") FROM "+selection+';')
        statements.append('ROLLBACK;')
        statements.append("SELECT jsonb_build_object('rollbackConfirmed',txid_current_if_assigned() IS NULL,'remainingReferences',(SELECT count(*) FROM pg_class WHERE relnamespace=pg_my_temp_schema() AND relname LIKE 'academic_restore_%'),'role',current_user);")
        replies=self.copy_write('\n'.join(statements),connection)
        fidelity.require(len(replies)==len(pairs)+1 and fidelity.exact(replies[-1],{'rollbackConfirmed':True,'remainingReferences':0,'role':connection['user']}),
                         'oracle',('replyCount',) if len(replies)!=len(pairs)+1 else fidelity.difference_properties(replies[-1],{'rollbackConfirmed':True,'remainingReferences':0,'role':connection['user']},'oracle'))
        evidence=[]
        for number,(pair,reply) in enumerate(zip(pairs,replies[:-1])):
            fidelity.require(set(reply)=={'number','tree','columns','actualColumns','dependencies'} and type(reply['number']) is int and reply['number']==number and fidelity.exact(reply['columns'],reply['actualColumns']) and fidelity.exact(reply['dependencies'],pair['restored']['dependencies']),
                             'oracle',('shape','number','columns','dependencies'))
            fidelity.require(isinstance(reply['tree'],str) and reply['tree']==pair['restored']['tree'],'oracle',('tree',))
            evidence.append(dict(pair,sourceReplayTree=reply['tree'],sourceTreeSha256=digest(reply['tree'].encode()),restoredTreeSha256=digest(pair['restored']['tree'].encode())))
        return evidence

    @contextlib.contextmanager
    def isolated(self, connection, identity, source_database):
        self.diagnostic_stage='copy_setup'
        profile=fidelity.supported_profile(connection['user'],identity['roles'],identity['memberships'])
        fidelity.database_locale(connection['user'],connection['database'],source_database)
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
                ready = subprocess.run(['podman', 'exec', self.copy, 'pg_isready', '-h', '127.0.0.1', '-U', 'postgres'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5, check=False)
                if ready.returncode == 0:
                    break
                time.sleep(1)
            else:
                refuse('copy_postgres_unavailable')
            def create_role(role):
                flags = ['LOGIN' if role['login'] else 'NOLOGIN', 'SUPERUSER' if role['super'] else 'NOSUPERUSER', 'CREATEDB' if role['createDb'] else 'NOCREATEDB', 'CREATEROLE' if role['createRole'] else 'NOCREATEROLE', 'INHERIT' if role['inherit'] else 'NOINHERIT', 'REPLICATION' if role['replication'] else 'NOREPLICATION', 'BYPASSRLS' if role['bypass'] else 'NOBYPASSRLS']
                return 'CREATE ROLE ' + ident(role['name']) + ' ' + ' '.join(flags) + ' CONNECTION LIMIT ' + str(role['limit']) + ' PASSWORD ' + literal(secrets.token_hex(32)) + '; COMMENT ON ROLE ' + ident(role['name']) + ' IS ' + (literal(role['comment']) if role['comment'] is not None else 'NULL') + ';'
            roles={role['name']:role for role in identity['roles']}
            self.copy_sql(create_role(roles['schoolorbit_provisioner']))
            tenant_sql='\n'.join(create_role(roles[name]) for name in [connection['user'],connection['user'][:-6]+'_runtime'])
            tenant_sql+='\nGRANT '+ident(connection['user'])+' TO schoolorbit_provisioner WITH SET TRUE, INHERIT FALSE;'
            if profile=='provider3':
                # Effective current_user is the original non-super creator.
                # SET ROLE also preserves a legitimate source NOLOGIN flag.
                tenant_sql='SET ROLE schoolorbit_provisioner;\n'+tenant_sql+'\nRESET ROLE;'
            self.copy_sql(tenant_sql)
            self.verify_copy_profile(connection,identity)
            # The original custom archive owns the database definition,
            # properties and ACL. Setup must not approximate them.
            yield
        finally:
            # Only the exact resources created by this invocation may be removed.
            if created:
                self.command(['podman', 'rm', '-f', self.copy])
            self.command(['podman', 'volume', 'rm', self.volume])

    def restored_fingerprint(self, connection, nonce, evidence=None):
        return self.fingerprint(self.copy_read_base(connection), os.environ.copy(), nonce, evidence, 'restored')

    def restore_and_migrate(self, row, connection, identity, archive, nonce, source_hash):
        with self.source(row) as (_,source_base,source_environment):
            source_metadata=self.catalog(source_base,source_environment)
        with self.isolated(connection, identity, source_metadata['database']):
            # CREATE DATABASE cannot run inside --single-transaction. Any
            # failed/partial restore remains confined to this disposable copy.
            with archive.open('rb') as encrypted:
                self.stream(self.age('/usr/local/bin/age', '-d', '-i', '/run/recovery/identity.agekey'), ['podman', 'exec', '-i', self.copy, 'pg_restore', '--create', '--clean', '--if-exists', '--exit-on-error', '-U', 'postgres', '-d', 'postgres'], source=encrypted)
            before_hash=self.restored_fingerprint(connection,nonce)
            before_metadata=self.catalog(self.copy_read_base(connection))
            self.diagnostic_stage='nn_reconcile'
            renames=fidelity.plan_nn(source_metadata,before_metadata)
            if renames:
                sql='BEGIN; SET LOCAL search_path=pg_catalog;\n'+'\n'.join('ALTER TABLE '+ident(x['schema'])+'.'+ident(x['table'])+' RENAME CONSTRAINT '+ident(x['from'])+' TO '+ident(x['to'])+';' for x in renames)+'\nCOMMIT;'
                fidelity.require(self.copy_write(sql,connection)==[])
            restored_metadata=self.catalog(self.copy_read_base(connection))
            self.diagnostic_stage='expression_pairing'
            pairs,physical=fidelity.expression_pairs(source_metadata,restored_metadata)
            evidence=self.oracle(pairs,connection)
            restored=self.restored_fingerprint(connection,nonce,evidence)
            with self.source(row) as (_,source_base,source_environment):
                source=self.fingerprint(source_base,source_environment,nonce,evidence,'source')
            self.diagnostic_stage='canonical_compare'
            fidelity.require(source['raw']==source_hash and source['canonical']==restored['canonical'],'canonical',tuple(key for key,bad in [('source_raw',source['raw']!=source_hash),('canonical',source['canonical']!=restored['canonical'])] if bad))
            restored_profile=self.verify_copy_profile(connection,identity)
            self.diagnostic_stage='retained_validation'
            name=uuid.UUID(row['id']).hex
            metadata={'schemaVersion':2,'policy':fidelity.POLICY,'schoolId':row['id'],'archiveSha256':digest_file(archive),'roleMetadata':identity['roles'],'membershipMetadata':identity['memberships'],'restoredRoleMetadata':restored_profile['roles'],'restoredMembershipMetadata':restored_profile['memberships'],'source':source_metadata,'restoredBefore':before_metadata,'restored':restored_metadata,'oracle':evidence,'physicalIndexBookkeepingDifferenceCount':physical}
            mapping={'schemaVersion':2,'policy':fidelity.POLICY,'schoolId':row['id'],'archiveSha256':digest_file(archive),'renames':renames}
            fidelity.require(len(fidelity.encoded(metadata))<=fidelity.MAX_METADATA and len(fidelity.encoded(mapping))<=fidelity.MAX_METADATA)
            metadata_path=self.point/(name+'.restore-metadata.json')
            mapping_path=self.point/(name+'.nn-identities.json')
            durable(metadata_path,metadata);durable(mapping_path,mapping)
            restore={'restoredSourceFingerprint':before_hash,'restoredReconciledFingerprint':restored['raw'],'restoreFidelity':{'policy':fidelity.POLICY,'sourceCanonicalFingerprint':source['canonical'],'restoredCanonicalFingerprint':restored['canonical'],'metadataSha256':digest_file(metadata_path),'nnIdentityMapSha256':digest_file(mapping_path),'checkCount':sum(x['kind']=='check' for x in evidence),'partialIndexPredicateCount':sum(x['kind']=='index' for x in evidence),'nnRenameCount':len(renames),'unclassifiedDifferenceCount':0}}
            # Fresh local role password, never a source password. Only the copy's
            # loopback namespace is shared; it has no egress or host port.
            local_password = secrets.token_hex(32)
            self.copy_sql('ALTER ROLE ' + ident(connection['user']) + ' PASSWORD ' + literal(local_password) + ';', connection['database'])
            local_url = 'postgresql://' + connection['user'] + ':' + local_password + '@127.0.0.1:5432/' + connection['database'] + '?sslmode=disable'
            environment = {**os.environ, 'MIGRATION_SCHEMA_DATABASE_URL': local_url, 'MIGRATION_SCHEMA_NAME': 'public', 'MIGRATION_SCHEMA_ALLOW_PUBLIC': '1'}
            migrated,restored_profile=self.migrate_copy(connection,identity,environment)
            metadata['restoredRoleMetadata']=restored_profile['roles']
            metadata['restoredMembershipMetadata']=restored_profile['memberships']
            durable(metadata_path,metadata)
            restore['restoreFidelity']['metadataSha256']=digest_file(metadata_path)
            try: migrated=json.loads(migrated)
            except ValueError: refuse('copy_runner_evidence_invalid')
            if migrated!={'schemaVersion':1,'operation':'migrate_tenant_schema','status':'migrated','version':self.compiled[-1][0],'checksumsVerified':True}: refuse('copy_runner_evidence_invalid')
            runtime = connection['user'][:-6] + '_runtime'
            sql = "SELECT jsonb_build_object('version',(SELECT max(version) FROM _sqlx_migrations WHERE success),'passed',passed,'selectionFingerprint',source_fingerprint,'cutoverDate',cutover_date,'checks',checks,'runtimeGrants',has_table_privilege(" + literal(runtime) + ", '_sqlx_migrations','SELECT') AND NOT has_table_privilege(" + literal(runtime) + ", '_sqlx_migrations','INSERT,UPDATE,DELETE') AND NOT has_table_privilege(" + literal(runtime) + ", 'academic_result_removal_scope','SELECT,INSERT,UPDATE,DELETE') AND (SELECT bool_and(has_table_privilege(" + literal(runtime) + ", 'learning_offerings',privilege)) FROM unnest(ARRAY['SELECT','INSERT','UPDATE','DELETE']) privilege) AND NOT has_database_privilege(" + literal(runtime) + ",current_database(),'TEMP')) FROM academic_current_operations_migration_audit WHERE migration_version=157;"
            try:
                return dict(verify_audit(decode_json(self.copy_sql(sql, connection['database'], runtime))),**restore)
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
                identity=self.identity(connection,base,environment)
                if identity['version']<157:
                    fidelity.supported_profile(connection['user'],identity['roles'],identity['memberships'])
                    pending_rows.append(row)
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
                    if not fidelity.exact(item.get('roleMetadata'),identity['roles']) or not fidelity.exact(item.get('membershipMetadata'),identity['memberships']): refuse('source_roles_changed')
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
                    durable(item_path, {'restrictKey': nonce, 'roleMetadata':identity['roles'],'membershipMetadata':identity['memberships'], 'proof': {'sourceFingerprint': source_hash, 'archiveSha256': digest_file(archive)}})
                # Complete decryption and the actual restore validate the entire
                # encrypted custom archive, not merely its listing/header.
                audit = self.restore_and_migrate(row, connection, identity, archive, nonce, source_hash)
                if identity['version']<157 and audit['cutoverDate'] != identity['date']:
                    refuse('cutover_date_changed')
                if self.fingerprint(base, environment, nonce) != source_hash:
                    refuse('source_changed_during_copy')
                proof = {'schoolId': row['id'], 'subdomain': row['subdomain'], 'databaseName': connection['database'], 'cutoverDate': audit['cutoverDate'], 'selectionFingerprint': audit['selectionFingerprint'], 'sourceFingerprint': source_hash, 'restoredSourceFingerprint':audit['restoredSourceFingerprint'],'restoredReconciledFingerprint':audit['restoredReconciledFingerprint'],'restoreFidelity':audit['restoreFidelity'], 'archiveSha256': digest_file(archive), 'decryptionVerified': True, 'historyVersion': audit['version'], 'checks': audit['checks']}
                self.diagnostic_stage='retained_validation'
                proof['restoreFidelity']['evidenceSha256']=fidelity.evidence_digest({'releaseSha':self.release,'imageDigest':self.image_digest,'inventoryFingerprint':self.inventory_hash},proof)
                durable(item_path, {'restrictKey': nonce, 'roleMetadata':identity['roles'],'membershipMetadata':identity['memberships'], 'proof': proof})
                tenants.append(proof)
            self.unchanged_inventory()
        bundle={'schemaVersion':2,'operation':OPERATION,'releaseSha':self.release,'imageDigest':self.image_digest,'inventoryFingerprint':self.inventory_hash,'tenants':tenants}
        self.diagnostic_stage='retained_validation'
        fidelity.require(len(fidelity.encoded(bundle))<=MAX_JSON)
        durable(self.proof,bundle)
        durable(self.state, {'schemaVersion': 1, 'state': 'prepared', 'completedAt': None, 'retainUntil': None})
        self.check()

    def retained_recovery(self):
        self.diagnostic_stage='retained_validation'
        # Completion must revalidate the recovery bytes and key independently
        # of the now-intentionally-contracted live business source.
        private(self.key)
        self._recovery_profiles={}
        proof = read_json(self.proof)
        if type(proof.get('schemaVersion')) is not int or proof.get('schemaVersion') != 2 or set(proof)!={'schemaVersion','operation','releaseSha','imageDigest','inventoryFingerprint','tenants'} or proof.get('operation') != OPERATION or proof.get('releaseSha') != self.release or proof.get('imageDigest') != self.image_digest or proof.get('inventoryFingerprint') != self.inventory_hash or not isinstance(proof.get('tenants'),list) or not 1<=len(proof['tenants'])<=len(self.rows):
            refuse('preparation_binding_changed')
        seen=set()
        for item in proof['tenants']:
            if not isinstance(item,dict): refuse('preparation_binding_changed')
            row=next((row for row in self.rows if row['id']==item.get('schoolId') and row['subdomain']==item.get('subdomain')),None)
            if row is None or row['id'] in seen: refuse('preparation_coverage_changed')
            seen.add(row['id'])
            self.retained_fidelity(proof,item,row)
            archive = private(self.point / (uuid.UUID(row['id']).hex + '.dump.age'))
            if not valid_hash(item.get('archiveSha256')) or digest_file(archive) != item['archiveSha256']:
                refuse('archive_changed')
            # Verify the retained key still decrypts the complete archive now.
            with archive.open('rb') as encrypted:
                self.stream(self.age('/usr/local/bin/age', '-d', '-i', '/run/recovery/identity.agekey'), ['podman', 'run', '--log-driver=none', '--rm', '-i', '--name', self.verify, *self.labels(), '--network', 'none', '--read-only', PG_IMAGE, 'pg_restore', '--file=/dev/null'], source=encrypted)
        return proof

    @fidelity.facet_context('retained')
    def retained_fidelity(self, bundle, item, row):
        self.diagnostic_stage='retained_validation'
        fields={'schoolId','subdomain','databaseName','cutoverDate','selectionFingerprint','sourceFingerprint','restoredSourceFingerprint','restoredReconciledFingerprint','restoreFidelity','archiveSha256','decryptionVerified','historyVersion','checks'}
        fidelity.require(set(item)==fields and item['databaseName']==urllib.parse.urlsplit(row['url']).path[1:])
        fidelity.require(type(item['historyVersion']) is int and item['historyVersion']==self.compiled[-1][0])
        fidelity.require(isinstance(item['cutoverDate'],str) and dt.date.fromisoformat(item['cutoverDate']).isoformat()==item['cutoverDate'])
        for key in ['selectionFingerprint','sourceFingerprint','restoredSourceFingerprint','restoredReconciledFingerprint','archiveSha256']:
            fidelity.require(fidelity.valid_hash(item[key]))
        f=item['restoreFidelity']
        fidelity.require(isinstance(f,dict) and set(f)=={'policy','sourceCanonicalFingerprint','restoredCanonicalFingerprint','metadataSha256','nnIdentityMapSha256','evidenceSha256','checkCount','partialIndexPredicateCount','nnRenameCount','unclassifiedDifferenceCount'})
        for key in ['sourceCanonicalFingerprint','restoredCanonicalFingerprint','metadataSha256','nnIdentityMapSha256','evidenceSha256']:
            fidelity.require(fidelity.valid_hash(f[key]))
        fidelity.require(f['sourceCanonicalFingerprint']==f['restoredCanonicalFingerprint'] and f['evidenceSha256']==fidelity.evidence_digest(bundle,item))
        name=uuid.UUID(row['id']).hex
        metadata_path=self.point/(name+'.restore-metadata.json');mapping_path=self.point/(name+'.nn-identities.json')
        metadata=read_json(metadata_path,fidelity.MAX_METADATA);mapping=read_json(mapping_path,fidelity.MAX_METADATA)
        fidelity.require(digest_file(metadata_path)==f['metadataSha256'] and digest_file(mapping_path)==f['nnIdentityMapSha256'],'retained',('hash',))
        fidelity.require(set(metadata)=={'schemaVersion','policy','schoolId','archiveSha256','roleMetadata','membershipMetadata','restoredRoleMetadata','restoredMembershipMetadata','source','restoredBefore','restored','oracle','physicalIndexBookkeepingDifferenceCount'} and set(mapping)=={'schemaVersion','policy','schoolId','archiveSha256','renames'})
        for document in [metadata,mapping]:
            fidelity.require(type(document['schemaVersion']) is int and document['schemaVersion']==2 and document['policy']==fidelity.POLICY and document['schoolId']==row['id'] and document['archiveSha256']==item['archiveSha256'])
        owner='so_'+uuid.UUID(row['id']).hex+'_owner'
        fidelity.supported_profile(owner,metadata['roleMetadata'],metadata['membershipMetadata'])
        fidelity.supported_profile(owner,metadata['restoredRoleMetadata'],metadata['restoredMembershipMetadata'])
        fidelity.require(fidelity.exact(metadata['roleMetadata'],metadata['restoredRoleMetadata']) and fidelity.exact(metadata['membershipMetadata'],metadata['restoredMembershipMetadata']))
        state=read_json(self.point/(name+'.json'))
        fidelity.require(fidelity.exact(metadata['roleMetadata'],state['roleMetadata']) and fidelity.exact(metadata['membershipMetadata'],state['membershipMetadata']))
        fidelity.require(fidelity.exact(fidelity.plan_nn(metadata['source'],metadata['restoredBefore']),mapping['renames']))
        pairs,physical=fidelity.expression_pairs(metadata['source'],metadata['restored'])
        fidelity.require(type(metadata['physicalIndexBookkeepingDifferenceCount']) is int and metadata['physicalIndexBookkeepingDifferenceCount']==physical)
        oracle=metadata['oracle']
        fidelity.require(isinstance(oracle,list) and len(oracle)==len(pairs))
        for pair,entry in zip(pairs,oracle):
            fidelity.require(set(entry)=={'kind','key','source','restored','sourceReplayTree','sourceTreeSha256','restoredTreeSha256'})
            fidelity.require(fidelity.exact({k:entry[k] for k in pair},pair) and isinstance(entry['sourceReplayTree'],str) and entry['sourceReplayTree']==entry['restored']['tree'])
            fidelity.require(digest(entry['sourceReplayTree'].encode())==entry['sourceTreeSha256'] and digest(entry['restored']['tree'].encode())==entry['restoredTreeSha256'])
        fidelity.require(f['checkCount']==sum(x['kind']=='check' for x in pairs) and f['partialIndexPredicateCount']==sum(x['kind']=='index' for x in pairs) and f['nnRenameCount']==len(mapping['renames']))
        self._recovery_profiles[row['id']]={'roles':metadata['roleMetadata'],'memberships':metadata['membershipMetadata'],'database':metadata['source']['database']}

    def verify_source_profile(self,row,identity,base,environment):
        self.diagnostic_stage='source_profile'
        expected=self._recovery_profiles[row['id']]
        fidelity.require(fidelity.exact(identity['roles'],expected['roles']) and fidelity.exact(identity['memberships'],expected['memberships']))
        current=decode_json(self.bounded_capture(base+['psql','-XAtq','-v','ON_ERROR_STOP=1','-c',DATABASE_SQL],environment=environment))
        fidelity.require(fidelity.exact(current,expected['database']))

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
                self.verify_source_profile(row,identity,base,environment)
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
                identity=self.identity(connection,base,environment)
                self.verify_source_profile(row,identity,base,environment)
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
    controller.diagnostic_stage='unknown'
    try:
        getattr(controller, sys.argv[1])()
    except fidelity.FidelityError as error:
        detail=dict(error.diagnostic)
        detail['stage']=getattr(controller,'diagnostic_stage','unknown')
        if not fidelity.validate_diagnostic(detail):detail=fidelity.diagnostic()
        error.diagnostic=detail
        # Diagnostic receipts are never proof/retry inputs, and failed receipt IO
        # must not replace the original refusal. Do not follow an existing link.
        try:
            private(controller.point,directory=True)
            receipt=controller.point/'last-fidelity-refusal.json'
            if receipt.exists() or receipt.is_symlink():private(receipt)
            durable(receipt,{'schemaVersion':1,'error':'restored_source_mismatch','diagnostic':detail})
        except (OSError,Refusal):
            pass
        raise


if __name__ == '__main__':
    try:
        main()
    except (Refusal, OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        code = str(error) if isinstance(error, (Refusal,fidelity.FidelityError)) else 'preparation_unavailable'
        body={'schemaVersion':1,'error':code}
        if isinstance(error,fidelity.FidelityError) and fidelity.validate_diagnostic(error.diagnostic):body['diagnostic']=error.diagnostic
        if isinstance(error,Refusal) and getattr(error,'diagnostic',None): body['migrationRefusal']=error.diagnostic
        print(json.dumps(body),file=sys.stderr)
        sys.exit(1)
