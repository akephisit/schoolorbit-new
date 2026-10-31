#!/usr/bin/env python3
"""Fixed, aggregate-only observation of the academic-148 curriculum guard."""
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request
import uuid

PG_IMAGE = 'public.ecr.aws/docker/library/postgres:18.6-alpine@sha256:d8703cd7fba306b9fec9268ecedfa8a966846c053036a60e3635791957eb2f66'
CLIENT = 'schoolorbit-academic148-observation'
MAX_BYTES = 1048576
COUNTS = ('receipts','violating_receipts','created_count_mismatches','matched_created_lower','matched_created_higher','zero_recorded_positive_created','all_created_count_mismatches','invalid_output_receipts','nonunique_output_receipts','missing_output_entries','wrong_context_output_entries','null_output_entries','duplicate_nonnull_output_entries','extra_timestamp_offerings','shared_timestamp_cohorts','shared_timestamp_receipts')
KINDS = ('before050','equal050','after050','unknown050')
DIVERGENCE = ('lower_receipts','recorded_created_total','timestamp_created_total','difference_total','distinct_output_total','recorded_above_output_receipts','output_entries_earlier','output_entries_equal','output_entries_later','later_entries_with_current_receipt','later_entries_with_delete_item_audit')

class Refusal(Exception):
    pass

def require(value):
    if not value:
        raise Refusal()

def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result

def decode(raw):
    require(0 < len(raw) <= MAX_BYTES)
    return json.loads(raw, object_pairs_hook=unique, parse_constant=lambda _: require(False))

def shape(value, keys):
    require(type(value) is dict and set(value) == set(keys))

def integer(value):
    require(type(value) is int and 0 <= value <= 9007199254740991)

def validate_divergence(value):
    shape(value, DIVERGENCE)
    for count in value.values(): integer(count)
    require(value['recorded_created_total'] - value['timestamp_created_total'] == value['difference_total'])
    require(value['output_entries_equal'] == value['timestamp_created_total'])
    require(value['difference_total'] >= value['lower_receipts'])
    require(value['recorded_above_output_receipts'] <= value['lower_receipts'])
    require(value['later_entries_with_current_receipt'] <= value['output_entries_later'])
    require(value['later_entries_with_delete_item_audit'] <= value['output_entries_later'])
    require(value['lower_receipts'] > 0 or all(count == 0 for count in value.values()))

def validate_metrics(value):
    shape(value['counts'], COUNTS)
    for count in value['counts'].values(): integer(count)
    require(type(value['masks']) is list and len(value['masks']) == 16)
    for mask, item in enumerate(value['masks']):
        shape(item, ('mask','count')); require(type(item['mask']) is int and item['mask'] == mask); integer(item['count'])
    require(sum(item['count'] for item in value['masks']) == value['counts']['receipts'])
    require(sum(item['count'] for item in value['masks'][1:]) == value['counts']['violating_receipts'])
    for bit, name in ((1,'created_count_mismatches'),(2,'all_created_count_mismatches'),(4,'invalid_output_receipts'),(8,'nonunique_output_receipts')):
        require(sum(item['count'] for item in value['masks'] if item['mask'] & bit) == value['counts'][name])
    require(value['counts']['matched_created_lower'] + value['counts']['matched_created_higher'] == value['counts']['created_count_mismatches'])
    require(value['counts']['zero_recorded_positive_created'] <= value['counts']['matched_created_higher'])
    require(type(value['timing']) is list and len(value['timing']) == 4)
    for kind, item in zip(KINDS, value['timing']):
        shape(item, ('kind','count','violating','zero_recorded_positive_created')); require(item['kind'] == kind)
        for key in ('count','violating','zero_recorded_positive_created'): integer(item[key])
        require(item['violating'] <= item['count'] and item['zero_recorded_positive_created'] <= item['violating'])
    for key, count in (('count','receipts'),('violating','violating_receipts'),('zero_recorded_positive_created','zero_recorded_positive_created')):
        require(sum(item[key] for item in value['timing']) == value['counts'][count])

def validate(value):
    shape(value, ('operation','schema_version','history_version','history_verified','counts','masks','timing','checkpoints','divergence'))
    require(value['operation'] == 'academic148_curriculum' and type(value['schema_version']) is int and value['schema_version'] == 2)
    require(type(value['history_version']) is int and value['history_version'] in (147,148) and value['history_verified'] is True)
    validate_metrics(value)
    validate_divergence(value['divergence'])
    require(value['divergence']['lower_receipts'] == value['counts']['matched_created_lower'])
    require(type(value['checkpoints']) is list and len(value['checkpoints']) == 2)
    for version, item in zip((138,141), value['checkpoints']):
        shape(item, ('version','present','unique','count_match','hash_match')); require(type(item['version']) is int and item['version'] == version)
        for key in ('present','unique','count_match','hash_match'): require(type(item[key]) is bool)
        require(not item['unique'] or item['present'])
        require(not (item['count_match'] or item['hash_match']) or item['unique'])
    return value

def capture(args, *, env=None, seconds=30):
    process = subprocess.Popen(args, env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    output = bytearray(); total = 0
    try:
        with selectors.DefaultSelector() as selected:
            selected.register(process.stdout, selectors.EVENT_READ, True)
            selected.register(process.stderr, selectors.EVENT_READ, False)
            deadline = time.monotonic() + seconds
            while selected.get_map():
                require(time.monotonic() < deadline)
                for key, _ in selected.select(min(0.1, max(0, deadline-time.monotonic()))):
                    chunk = os.read(key.fileobj.fileno(), 16384)
                    if not chunk:
                        selected.unregister(key.fileobj); continue
                    total += len(chunk); require(total <= MAX_BYTES)
                    if key.data: output.extend(chunk)
            require(process.wait(timeout=max(0.01,deadline-time.monotonic())) == 0)
        return bytes(output)
    finally:
        if process.poll() is None:
            os.killpg(process.pid, signal.SIGTERM)
            try: process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL); process.wait()
        process.stdout.close(); process.stderr.close()

def regular(path):
    info = path.lstat(); require(stat.S_ISREG(info.st_mode) and not path.is_symlink())
    return path.read_bytes()

def prepare():
    require(os.environ.get('GITHUB_REF') == 'refs/heads/main')
    root = Path(os.environ['GITHUB_WORKSPACE']).resolve()
    output = Path(os.environ['RUNNER_TEMP']) / 'academic148-inputs'
    output.mkdir(mode=0o700); manifest = []
    files = sorted((root/'backend-school/migrations').glob('*.sql'))
    require(len(files) == 149)
    for version, path in enumerate(files, 1):
        require(re.fullmatch(f'{version:03d}_[a-z0-9_]+\\.sql', path.name))
        checksum = hashlib.sha384(regular(path)).hexdigest()
        if version <= 148:
            manifest.append({'version':version,'checksum':checksum})
    payloads = {'manifest.json':json.dumps(manifest,separators=(',',':')).encode()}
    for name in ('pipeline-academic148-observations.py','pipeline-academic148-observations.sql'):
        payloads[name] = regular(root/'scripts/lib'/name)
    digests = {}
    for name, raw in payloads.items():
        path = output/name; path.write_bytes(raw); path.chmod(0o600)
        digests[name] = hashlib.sha256(raw).hexdigest()
    path = output/'digests.json'; path.write_text(json.dumps(digests)); path.chmod(0o600)

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args, **_kwargs): raise Refusal()

def admin_get(route, secret):
    request = urllib.request.Request('http://127.0.0.1:8080'+route, headers={'X-Internal-Caller':'backend-school','X-Internal-Secret':secret})
    with urllib.request.build_opener(urllib.request.ProxyHandler({}),NoRedirect()).open(request,timeout=10) as response:
        require(response.status == 200); return decode(response.read(MAX_BYTES+1))

def inventory(secret):
    value = admin_get('/internal/schools',secret)
    require(type(value) is dict and type(value.get('total')) is int and type(value.get('schools')) is list)
    require(value['total'] == len(value['schools']) and 0 <= value['total'] <= 64)
    tenants = []; ids = set(); names = set()
    for item in value['schools']:
        require(type(item) is dict)
        identity = str(uuid.UUID(item['id'])); require(identity == item['id'] and identity != str(uuid.UUID(int=0)))
        name = item['subdomain']; require(type(name) is str and re.fullmatch('[a-z0-9-]{1,63}',name))
        require(identity not in ids and name not in names and type(item['status']) is str and item['status'].strip())
        require(type(item['db_connection_string']) is str and item['db_connection_string'])
        require(item.get('mutation_pending',False) is False)
        ids.add(identity); names.add(name)
        tenants.append({key:item[key] for key in ('id','subdomain','status','db_connection_string')})
    return sorted(tenants,key=lambda item:item['id'])

def connection(value):
    require(type(value) is str)
    route = urllib.parse.urlsplit(value); query = urllib.parse.parse_qs(route.query,strict_parsing=True)
    require(route.scheme in ('postgres','postgresql') and route.hostname and not route.fragment)
    require(query == {'sslmode':['verify-full'],'sslrootcert':['/run/secrets/postgresql-ca.crt']})
    result = {'host':route.hostname,'port':route.port or 5432,'database':urllib.parse.unquote(route.path[1:]),'user':urllib.parse.unquote(route.username or ''),'password':urllib.parse.unquote(route.password or '')}
    require(re.fullmatch('[A-Za-z0-9_]{1,63}',result['database']))
    require(result['password'] and all(not any(char in str(value) for char in '\r\n\x00') for value in result.values()))
    return result

PRELUDE = """BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SET LOCAL statement_timeout='20s'; SET LOCAL lock_timeout='2s'; SET LOCAL idle_in_transaction_session_timeout='25s';
SELECT current_database()=:'expected_database' AND current_user=:'expected_owner'
 AND r.rolname=:'expected_owner' AND shobj_description(r.oid,'pg_authid')=:'expected_marker'
 AND NOT r.rolsuper AND NOT r.rolcreaterole AND NOT r.rolcreatedb AND NOT r.rolbypassrls
 AND (SELECT ssl FROM pg_stat_ssl WHERE pid=pg_backend_pid())
 AND current_setting('transaction_read_only')='on' AS identity_ok
 FROM pg_database d JOIN pg_roles r ON r.oid=d.datdba WHERE d.datname=current_database() \\gset
\\if :identity_ok
"""

def observe(tenant, secret, sql, manifest, ca):
    owner = 'so_'+tenant['id'].replace('-','')+'_owner'
    value = admin_get('/internal/schools/'+tenant['subdomain']+'/migration-connection?include_inactive=true',secret)
    shape(value, ('db_migration_connection_string',))
    c = connection(value['db_migration_connection_string']); runtime = connection(tenant['db_connection_string'])
    require(c['user'] == owner and runtime['user'] == owner.removesuffix('_owner')+'_runtime' and all(c[key] == runtime[key] for key in ('host','port','database')))
    runtime_directory = Path('/run/user')/str(os.getuid()); info = runtime_directory.lstat()
    require(stat.S_ISDIR(info.st_mode) and info.st_uid == os.getuid() and stat.S_IMODE(info.st_mode) == 0o700 and not runtime_directory.is_symlink())
    require(capture(['stat','-f','-c','%T',str(runtime_directory)],seconds=5).strip() == b'tmpfs')
    with tempfile.TemporaryDirectory(prefix='academic148-',dir=runtime_directory) as directory:
        work = Path(directory); work.chmod(0o700)
        escape = lambda value:str(value).replace('\\','\\\\').replace(':','\\:')
        (work/'pgpass').write_text(':'.join(escape(c[key]) for key in ('host','port','database','user','password'))+'\n'); (work/'pgpass').chmod(0o600)
        query = PRELUDE+'PREPARE academic148_observation(jsonb) AS\n'+sql.rstrip().rstrip(';')+";\nEXECUTE academic148_observation(:'manifest'::jsonb);\n\\endif\nROLLBACK;\n"
        (work/'query.sql').write_text(query); (work/'query.sql').chmod(0o600)
        environment = {key:value for key,value in os.environ.items() if key != 'INTERNAL_API_SECRET'}
        environment.update({'PGHOST':c['host'],'PGPORT':str(c['port']),'PGDATABASE':c['database'],'PGUSER':owner,'PGSSLMODE':'verify-full','PGSSLROOTCERT':'/run/secrets/postgresql-ca.crt','PGPASSFILE':'/run/academic148/pgpass','PGCONNECT_TIMEOUT':'5','PGAPPNAME':'academic148-readonly-observation','PGOPTIONS':'-c default_transaction_read_only=on -c statement_timeout=20000 -c idle_in_transaction_session_timeout=25000'})
        args = ['podman','run','--rm','--name',CLIENT,'--network','schoolorbit-web','--read-only','--cap-drop=all','--security-opt=no-new-privileges','--userns=keep-id','--user',f'{os.getuid()}:{os.getgid()}','-v',str(work)+':/run/academic148:ro','-v',str(ca)+':/run/secrets/postgresql-ca.crt:ro']
        for key in ('PGHOST','PGPORT','PGDATABASE','PGUSER','PGSSLMODE','PGSSLROOTCERT','PGPASSFILE','PGCONNECT_TIMEOUT','PGAPPNAME','PGOPTIONS'): args.extend(('--env',key))
        args.extend((PG_IMAGE,'psql','-w','-X','-A','-t','-q','-v','ON_ERROR_STOP=1','-v','VERBOSITY=sqlstate','-v','expected_database='+c['database'],'-v','expected_owner='+owner,'-v','expected_marker=schoolorbit:'+tenant['id']+':migration','-v','manifest='+json.dumps(manifest,separators=(',',':')),'-f','/run/academic148/query.sql'))
        return validate(decode(capture(args,env=environment,seconds=35)))

def aggregate(values):
    values = [validate(value) for value in values]
    result = {
        'operation':'academic148_curriculum_observation','schema_version':2,'tenants':len(values),
        'history_versions':[{'version':version,'count':sum(item['history_version']==version for item in values)} for version in (147,148)],
        'counts':{key:sum(item['counts'][key] for item in values) for key in COUNTS},
        'masks':[{'mask':mask,'count':sum(item['masks'][mask]['count'] for item in values)} for mask in range(16)],
        'timing':[{'kind':kind,**{key:sum(item['timing'][index][key] for item in values) for key in ('count','violating','zero_recorded_positive_created')}} for index,kind in enumerate(KINDS)],
        'checkpoints':checkpoint_counts(values),
        'history_observations':[]}
    for version in (147,148):
        group = [item for item in values if item['history_version'] == version]
        result['history_observations'].append({'version':version,'tenants':len(group),
            'divergence':{key:sum(item['divergence'][key] for item in group) for key in DIVERGENCE},
            'checkpoints':checkpoint_counts(group)})
    for count in result['counts'].values(): integer(count)
    validate_aggregate(result)
    return result

def checkpoint_counts(values):
    return [{'version':version,**{key+'_tenants':sum(item['checkpoints'][index][key] for item in values)
            for key in ('present','unique','count_match','hash_match')}} for index,version in enumerate((138,141))]

def validate_checkpoint_counts(checkpoints, tenants):
    require(type(checkpoints) is list and len(checkpoints) == 2)
    for version,item in zip((138,141),checkpoints):
        shape(item,('version','present_tenants','unique_tenants','count_match_tenants','hash_match_tenants'))
        require(type(item['version']) is int and item['version'] == version)
        for key in ('present_tenants','unique_tenants','count_match_tenants','hash_match_tenants'): integer(item[key])
        require(item['count_match_tenants'] <= item['unique_tenants'] <= item['present_tenants'] <= tenants)
        require(item['hash_match_tenants'] <= item['unique_tenants'])

def validate_aggregate(value):
    shape(value, ('operation','schema_version','tenants','history_versions','counts','masks','timing','checkpoints','history_observations'))
    require(value['operation'] == 'academic148_curriculum_observation' and type(value['schema_version']) is int and value['schema_version'] == 2)
    integer(value['tenants']); require(value['tenants'] <= 64)
    require(type(value['history_versions']) is list and len(value['history_versions']) == 2)
    for version,item in zip((147,148),value['history_versions']):
        shape(item,('version','count')); require(type(item['version']) is int and item['version'] == version); integer(item['count'])
    require(sum(item['count'] for item in value['history_versions']) == value['tenants'])
    validate_metrics(value)
    validate_checkpoint_counts(value['checkpoints'],value['tenants'])
    groups = value['history_observations']
    require(type(groups) is list and len(groups) == 2)
    for version,history,group in zip((147,148),value['history_versions'],groups):
        shape(group,('version','tenants','divergence','checkpoints'))
        require(type(group['version']) is int and group['version'] == version)
        integer(group['tenants']); require(group['tenants'] == history['count'])
        validate_divergence(group['divergence'])
        require(group['tenants'] > 0 or all(count == 0 for count in group['divergence'].values()))
        validate_checkpoint_counts(group['checkpoints'],group['tenants'])
    require(sum(group['tenants'] for group in groups) == value['tenants'])
    require(sum(group['divergence']['lower_receipts'] for group in groups) == value['counts']['matched_created_lower'])
    for index,checkpoint in enumerate(value['checkpoints']):
        for key in ('present_tenants','unique_tenants','count_match_tenants','hash_match_tenants'):
            require(sum(group['checkpoints'][index][key] for group in groups) == checkpoint[key])
    return value

def collect_tenants(secret, sql, manifest, ca):
    before = inventory(secret)
    values = [observe(tenant,secret,sql,manifest,ca) for tenant in before]
    require(before == inventory(secret))
    return aggregate(values)

def collect(digests):
    require(len(digests) == 3 and all(re.fullmatch('[0-9a-f]{64}',value) for value in digests))
    root = Path(__file__).resolve().parent
    require(root == Path('/opt/stack/deployment/academic148-observations'))
    require(root.lstat().st_uid == os.getuid() and stat.S_IMODE(root.lstat().st_mode) == 0o700 and not root.is_symlink())
    payloads = [regular(root/name) for name in ('pipeline-academic148-observations.py','pipeline-academic148-observations.sql','manifest.json')]
    require(all(hashlib.sha256(raw).hexdigest() == digest for raw,digest in zip(payloads,digests)))
    manifest = decode(payloads[2]); require(type(manifest) is list and len(manifest) == 148)
    for version,item in enumerate(manifest,1):
        shape(item,('version','checksum')); require(type(item['version']) is int and item['version'] == version and type(item['checksum']) is str and re.fullmatch('[0-9a-f]{96}',item['checksum']))
    helper = '/opt/stack/deployment/scripts/lib/pipeline-remote/runtime-env.sh'
    def setting(key):
        return capture(['bash','-euc','. "$1"; if [[ $2 == POSTGRES_CA_FILE ]] && ! grep -q "^POSTGRES_CA_FILE=" /opt/stack/.env; then exit 0; fi; schoolorbit_runtime_env_value "$2" /opt/stack/.env','academic148',helper,key],seconds=5).decode()
    secret = setting('INTERNAL_API_SECRET'); require(secret and not any(char in secret for char in '\r\n\x00'))
    ca = Path(setting('POSTGRES_CA_FILE') or '/opt/stack/postgresql/ca.crt')
    if not ca.is_absolute(): ca = Path('/opt/stack')/ca
    require(ca.is_file() and os.access(ca,os.R_OK))
    # A surviving rollout name is never removed or displaced by observation.
    for name in ('schoolorbit-school-rollout',CLIENT):
        result = subprocess.run(['podman','container','exists',name],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,timeout=5)
        require(result.returncode == 1)
    return collect_tenants(secret,payloads[1].decode(),manifest,ca)

if __name__ == '__main__':
    def interrupted(_signum, _frame):
        raise Refusal()
    for interruption in (signal.SIGTERM,signal.SIGINT,signal.SIGHUP):
        signal.signal(interruption,interrupted)
    try:
        if sys.argv[1:] == ['prepare']:
            prepare()
        else:
            require(len(sys.argv) == 5 and sys.argv[1] == 'collect')
            print(json.dumps(collect(sys.argv[2:]),separators=(',',':'),allow_nan=False))
    except Exception:
        print('academic148_observation=refused',file=sys.stderr)
        raise SystemExit(1)
