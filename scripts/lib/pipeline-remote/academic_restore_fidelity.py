"""Pure, bounded PostgreSQL18.6 restore-fidelity protocol. No IO or CLI."""
import hashlib
import json
import re
from functools import wraps

POLICY = 'pg18.6-exact-expression-replay-nn-v1'
DOMAIN = 'schoolorbit.academic-current.restore-evidence.v2'
MAX_METADATA = 32 * 1024 * 1024
MAX_DDL = 4 * 1024 * 1024
MAX_EXPRESSION = 256 * 1024
MAX_OBJECTS = 20000
CHECKS = sorted(['CURRENT_DELIVERY_GRAPH_RECONCILED', 'CURRENT_TEACHERS_RECONCILED', 'CURRENT_TIMETABLE_IDS_RECONCILED', 'CURRENT_FACTS_PRESERVED', 'CURRENT_SUPERVISION_PRESERVED', 'CURRENT_CATALOG_PINS_PRESERVED', 'CURRENT_LEGACY_SCHEMA_REMOVED', 'CURRENT_SCOPE_GUARDS_ENABLED'])

DIAGNOSTIC_STAGES = ('unknown','source_profile','source_catalog','copy_setup','copy_profile','copy_catalog','nn_reconcile','expression_pairing','oracle','source_stream','copy_stream','canonical_compare','retained_validation')
# Property position is the stable u32 bit position. Never emit supplied keys.
DIAGNOSTIC_PROPERTIES = {
    'unknown': (),
    'profile_shape': ('roles','memberships','shape'),
    'role_identity': ('owner','runtime','provisioner','unknown'),
    'owner_capabilities': ('super','createDb','createRole','replication','bypass'),
    'runtime_capabilities': ('super','createDb','createRole','replication','bypass'),
    'provisioner_capabilities': ('super','createDb','createRole','replication','bypass'),
    'memberships': tuple(prefix+category for prefix in ['role_','member_','grantor_'] for category in ['owner','runtime','provisioner','postgres','unknown'])+('admin','inherit','set','missing_edge','extra_edge','count'),
    'catalog_budget': ('objects','expressionBytes','dependencies','bytes','encodedBytes'),
    'metadata_shape': ('shape',),
    'database': ('owner','tablespace','acl','settings','metadata.datname','metadata.encoding','metadata.datlocprovider','metadata.datlocale','metadata.datcollate','metadata.datctype','metadata.datcollversion','metadata.datconnlimit','metadata.datistemplate','metadata.datallowconn','metadata.dathasloginevt','metadata.daticurules'),
    'relation': ('inventory','schema','name','kind','partition','inheritance'),
    'columns': ('inventory','name','notnull','typeSchema','typeName','typeKind','typeCategory','typeMod','dimensions','identity','generated','collationSchema','collationName','collationEncoding','collationProvider','collationDeterministic','collationLocale','collationCollate','collationCType','collationRules','collationVersion','elementSchema','elementName','elementKind'),
    'nn': ('inventory','keys','tree','attnotnull','metadata','target_collision','ambiguity'),
    'constraints': ('inventory','kind','domain','parent','index','foreignRelation','flags','comment','keys','dependencies','definition','expression'),
    'indexes': ('inventory','schema','table','name','expression','flags','owner','method','kind','partition','persistence','options','tablespace','comment','keys','dependencies','definition','predicate'),
    'expression_context': ('size','relation','column','type','element','dependency'),
    'oracle': ('replyCount','rollbackConfirmed','remainingReferences','role','number','columns','dependencies','tree','shape'),
    'framing': ('syntax','limit','binding','duplicate','expression','unused_evidence','copy_termination'),
    'canonical': ('source_raw','canonical'),
    'retained': ('shape','binding','hash','profile','metadata','map','oracle','count'),
}


def diagnostic(facet='unknown', properties=(), count=None):
    if facet not in DIAGNOSTIC_PROPERTIES:
        facet='unknown'
    allowed=DIAGNOSTIC_PROPERTIES[facet]
    mask=sum(1<<i for i,key in enumerate(allowed) if key in properties)
    if count is None:count=mask.bit_count() if mask else (0 if facet=='unknown' else 1)
    if type(count) is not int or not 0<=count<=MAX_OBJECTS:count=0
    return {'stage':'unknown','facet':facet,'mismatchCount':count,'propertyMask':mask}


def validate_diagnostic(value):
    return (isinstance(value,dict) and set(value)=={'stage','facet','mismatchCount','propertyMask'}
            and isinstance(value['stage'],str) and isinstance(value['facet'],str) and value['stage'] in DIAGNOSTIC_STAGES and value['facet'] in DIAGNOSTIC_PROPERTIES
            and type(value['mismatchCount']) is int and 0<=value['mismatchCount']<=MAX_OBJECTS
            and type(value['propertyMask']) is int and 0<=value['propertyMask']<2**len(DIAGNOSTIC_PROPERTIES[value['facet']])
            and (value['facet']!='unknown' or value['mismatchCount']==0))


def facet_context(facet):
    def decorate(function):
        @wraps(function)
        def wrapped(*args,**kwargs):
            try:return function(*args,**kwargs)
            except FidelityError as error:
                if error.diagnostic['facet']=='unknown':error.diagnostic=diagnostic(facet)
                raise
        return wrapped
    return decorate


def difference_properties(left,right,facet):
    def get(value,path):
        for key in path.split('.'):
            if not isinstance(value,dict) or key not in value:return (False,None)
            value=value[key]
        return (True,value)
    return tuple(key for key in DIAGNOSTIC_PROPERTIES[facet] if not exact(get(left,key),get(right,key)))


def require_match(left,right,facet):
    if not exact(left,right):
        properties=difference_properties(left,right,facet)
        raise FidelityError(facet,properties,len(properties) if properties else 0)


class FidelityError(ValueError):
    """Static reason only; never attach source SQL or rows."""
    def __init__(self,facet='unknown',properties=(),count=None):
        super().__init__('restored_source_mismatch')
        self.diagnostic=diagnostic(facet,properties,count)


def require(condition,facet='unknown',properties=(),count=None):
    if not condition:
        raise FidelityError(facet,properties,count)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def valid_hash(value):
    return isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value) is not None


def exact(left, right):
    """JSON equality must distinguish true from 1."""
    return encoded(left) == encoded(right)


@facet_context('profile_shape')
def validate_profiles(roles, memberships):
    require(isinstance(roles,list) and len(roles)<=MAX_OBJECTS and isinstance(memberships,list) and len(memberships)<=MAX_OBJECTS)
    require(len({r['name'] for r in roles})==len(roles))
    for role in roles:
        require(set(role)=={'name','login','super','createDb','createRole','inherit','replication','bypass','limit','comment'})
        require(isinstance(role['name'],str) and all(type(role[k]) is bool for k in ['login','super','createDb','createRole','inherit','replication','bypass']))
        require(type(role['limit']) is int and (role['comment'] is None or isinstance(role['comment'],str)))
    seen=set()
    for membership in memberships:
        require(set(membership)=={'role','member','grantor','admin','inherit','set'})
        require(all(isinstance(membership[k],str) for k in ['role','member','grantor']) and all(type(membership[k]) is bool for k in ['admin','inherit','set']))
        key=tuple(membership[k] for k in ['role','member','grantor'])
        require(key not in seen)
        seen.add(key)


def supported_profile(owner, roles, memberships):
    """Finite PG18.6 provider/fixture graphs, not a generic role restorer."""
    validate_profiles(roles, memberships)
    require(isinstance(owner,str) and re.fullmatch('so_[0-9a-f]{32}_owner',owner) is not None)
    runtime=owner[:-6]+'_runtime'
    provisioner='schoolorbit_provisioner'
    by_name={r['name']:r for r in roles}
    require(set(by_name)=={owner,runtime,provisioner},'role_identity',
            tuple(category for name,category in [(owner,'owner'),(runtime,'runtime'),(provisioner,'provisioner')] if name not in by_name)+(('unknown',) if set(by_name)-{owner,runtime,provisioner} else ()))
    for name in [owner,runtime]:
        role=by_name[name]
        require(not any(role[k] for k in ['super','createDb','createRole','replication','bypass']),
                'owner_capabilities' if name==owner else 'runtime_capabilities',
                tuple(k for k in ['super','createDb','createRole','replication','bypass'] if role[k]))
    require(not any(by_name[provisioner][k] for k in ['super','replication','bypass']),
            'provisioner_capabilities',tuple(k for k in ['super','replication','bypass'] if by_name[provisioner][k]))
    require(by_name[provisioner]['createDb'] and by_name[provisioner]['createRole'],
            'provisioner_capabilities',tuple(k for k in ['createDb','createRole'] if not by_name[provisioner][k]))
    actual=sorted(tuple(row[k] for k in ['role','member','grantor','admin','inherit','set']) for row in memberships)
    fixture=[(owner,provisioner,'postgres',False,False,True)]
    provider=sorted([(owner,provisioner,'postgres',True,False,False),(owner,provisioner,provisioner,False,False,True),(runtime,provisioner,'postgres',True,False,False)])
    if exact(actual,provider):return 'provider3'
    if not exact(actual,fixture):
        # Closest profile is diagnostic categorization ONLY, never acceptance.
        expected=min([provider,fixture],key=lambda rows:len(set(rows)^set(actual)))
        categories={owner:'owner',runtime:'runtime',provisioner:'provisioner','postgres':'postgres'}
        observed={row[:3]:row[3:] for row in actual};wanted={row[:3]:row[3:] for row in expected}
        properties=set();mismatches=0
        for edge in observed.keys()|wanted.keys():
            if edge in observed and edge in wanted and observed[edge]==wanted[edge]:continue
            mismatches+=1
            for prefix,name in zip(['role_','member_','grantor_'],edge):properties.add(prefix+categories.get(name,'unknown'))
            if edge not in observed:properties.add('missing_edge')
            elif edge not in wanted:properties.add('extra_edge')
            else:
                for key,left,right in zip(['admin','inherit','set'],observed[edge],wanted[edge]):
                    if left!=right:properties.add(key)
        if len(actual)!=len(expected):properties.add('count')
        raise FidelityError('memberships',properties,mismatches)
    return 'postgresFixture1'


@facet_context('database')
def database_locale(owner, database, source):
    """Preserve bounded source locale names; PostgreSQL validates support."""
    require(isinstance(owner,str) and re.fullmatch('so_[0-9a-f]{32}_owner',owner) is not None)
    require(isinstance(database,str) and re.fullmatch('[A-Za-z_][A-Za-z0-9_]{0,62}',database) is not None)
    require(isinstance(source,dict) and set(source)=={'metadata','owner','tablespace','acl','settings'})
    require(source['owner']==owner and isinstance(source['metadata'],dict),'database',('owner',))
    metadata=source['metadata']
    require(metadata.get('datname')==database and type(metadata.get('encoding')) is int and metadata['encoding']==6,
            'database',tuple(k for k,bad in [('metadata.datname',metadata.get('datname')!=database),('metadata.encoding',type(metadata.get('encoding')) is not int or metadata.get('encoding')!=6)] if bad))
    require(metadata.get('datlocprovider')=='b' and metadata.get('datlocale')=='C.UTF-8',
            'database',tuple(k for k,bad in [('metadata.datlocprovider',metadata.get('datlocprovider')!='b'),('metadata.datlocale',metadata.get('datlocale')!='C.UTF-8')] if bad))
    pair=(metadata.get('datcollate'),metadata.get('datctype'))
    invalid=tuple(key for key,value in zip(('metadata.datcollate','metadata.datctype'),pair) if not (isinstance(value,str) and 0<len(value)<=255 and re.fullmatch('[A-Za-z0-9_.@+-]+',value) and len(value.encode())<=255))
    require(not invalid,'database',invalid)
    return pair


@facet_context('metadata_shape')
def validate_metadata(document):
    require(isinstance(document,dict) and set(document)=={'catalog','database','indexes'})
    require(isinstance(document['catalog'],dict) and set(document['catalog'])=={'relations','constraints','constraintNames'})
    names=inventory(document['catalog']['constraintNames'],qualified)
    for value in names.values():
        require(set(value)=={'schema','table','name','kind'})
    relations=inventory(document['catalog']['relations'],lambda x:(x['schema'],x['name']))
    for r in relations.values():
        require(set(r)=={'schema','name','kind','partition','inheritance','columns'})
        require(type(r['partition']) is bool and type(r['inheritance']) is int and 0<=r['inheritance']<=MAX_OBJECTS)
        for a in r['columns'] or []:
            require(set(a)=={'num','name','dropped','notnull','typeSchema','typeName','typeKind','typeCategory','typeMod','dimensions','identity','generated','collationSchema','collationName','collationEncoding','collationProvider','collationDeterministic','collationLocale','collationCollate','collationCType','collationRules','collationVersion','elementSchema','elementName','elementKind'})
            require(type(a['num']) is int and type(a['dropped']) is bool and type(a['notnull']) is bool)
            require(type(a['typeMod']) is int and type(a['dimensions']) is int)
            require(a['collationEncoding'] is None or type(a['collationEncoding']) is int)
            require(a['collationDeterministic'] is None or type(a['collationDeterministic']) is bool)
    for c in document['catalog']['constraints']:
        require(set(c)=={'schema','table','name','kind','domain','parent','index','foreignRelation','flags','comment','keys','rawKeys','expr','definition','tree','dependencies'})
        require(all(type(c[k]) is bool for k in ['domain','parent','index','foreignRelation']))
        require(c['kind'] in ['c','n'])
        require(qualified(c) in names and names[qualified(c)]['kind']==c['kind'])
        flags=c['flags']
        expected={'contype','condeferrable','condeferred','conenforced','convalidated','confupdtype','confdeltype','confmatchtype','conislocal','coninhcount','connoinherit','conpfeqop','conppeqop','conffeqop','confdelsetcols','conexclop','conperiod'}
        require(isinstance(flags,dict) and set(flags)==expected)
        require(all(type(flags[k]) is bool for k in ['condeferrable','condeferred','conenforced','convalidated','conislocal','connoinherit','conperiod']))
        require(type(flags['coninhcount']) is int and flags['contype']==c['kind'])
        relation=relations.get((c['schema'],c['table']))
        if not c['domain']:
            require(relation is not None)
            nums={a['num']:a['name'] for a in relation['columns'] or [] if not a['dropped']}
            require(c['rawKeys'] is None or (isinstance(c['rawKeys'],list) and all(type(x) is int and x in nums for x in c['rawKeys']) and [nums[x] for x in c['rawKeys']]==c['keys']))
    for row in document['indexes']:
        require(set(row)=={'schema','table','name','definition','predicate','tree','expression','flags','owner','method','kind','partition','persistence','options','tablespace','comment','keys','dependencies'})
        require(type(row['expression']) is bool and type(row['partition']) is bool)
        flags=row['flags']
        bools={'indisunique','indnullsnotdistinct','indisprimary','indisexclusion','indimmediate','indisclustered','indisvalid','indcheckxmin','indisready','indislive','indisreplident'}
        require(set(flags)==bools|{'indnatts','indnkeyatts','indoption'} and all(type(flags[x]) is bool for x in bools))
        require(type(flags['indnatts']) is int and type(flags['indnkeyatts']) is int)
        for key in row['keys']:
            require(set(key)=={'column','number','opclassSchema','opclassName','opclassDefault','opclassInput','opclassFamily','collationSchema','collationName','option','attributeOptions'})
            require(type(key['number']) is int and type(key['opclassDefault']) is bool and (key['option'] is None or type(key['option']) is int))
    require(set(document['database'])=={'metadata','owner','tablespace','acl','settings'})
    for row in [*document['catalog']['constraints'],*document['indexes']]:
        for dependency in row['dependencies'] or []:
            require(set(dependency)=={'kind','class','object','column'})
            require(all(isinstance(dependency[k],str) for k in ['kind','class','object']))
            require(dependency['column'] is None or isinstance(dependency['column'],str))
    for setting in document['database']['settings'] or []:
        require(set(setting)=={'global','role','config'} and type(setting['global']) is bool and isinstance(setting['role'],str) and isinstance(setting['config'],list) and all(isinstance(x,str) for x in setting['config']))
    for privilege in document['database']['acl'] or []:
        require(isinstance(privilege,list) and len(privilege)==4 and all(isinstance(x,str) for x in privilege[:3]) and type(privilege[3]) is bool)
    # Every remaining scalar/property is retained and compared, never dropped.
    def scalar(value):
        if isinstance(value,dict):
            require(all(isinstance(k,str) for k in value))
            for v in value.values():scalar(v)
        elif isinstance(value,list):
            require(len(value)<=MAX_OBJECTS)
            for v in value:scalar(v)
        else:require(value is None or type(value) in [bool,int,str])
    scalar(document)


def evidence_digest(bundle, item):
    fidelity = item['restoreFidelity']
    require(fidelity['policy'] == POLICY)
    checks = item['checks']
    require(isinstance(checks,list) and len(checks) == 8 and all(isinstance(c,dict) and set(c)=={'code','passed','mismatchCount'} for c in checks) and sorted(c['code'] for c in checks) == CHECKS)
    require(item['decryptionVerified'] is True)
    counts = [fidelity[k] for k in ['checkCount', 'partialIndexPredicateCount', 'nnRenameCount', 'unclassifiedDifferenceCount']]
    require(all(type(x) is int and 0 <= x <= MAX_OBJECTS for x in counts) and counts[-1] == 0)
    fields = [DOMAIN, POLICY, bundle['releaseSha'], bundle['imageDigest'], bundle['inventoryFingerprint'],
              item['schoolId'], item['subdomain'], item['databaseName'], item['archiveSha256'],
              item['sourceFingerprint'], item['restoredSourceFingerprint'], item['restoredReconciledFingerprint'],
              fidelity['sourceCanonicalFingerprint'], fidelity['restoredCanonicalFingerprint'],
              fidelity['metadataSha256'], fidelity['nnIdentityMapSha256'], item['cutoverDate'],
              item['selectionFingerprint'], str(item['historyVersion']), 'true', *map(str, counts)]
    for check in sorted(checks, key=lambda c: c['code']):
        require(check['passed'] is True and type(check['mismatchCount']) is int and check['mismatchCount'] == 0)
        fields.extend([check['code'], 'true', '0'])
    digest = hashlib.sha256()
    for field in fields:
        require(isinstance(field, str))
        raw = field.encode()
        digest.update(len(raw).to_bytes(8, 'big'))
        digest.update(raw)
    return digest.hexdigest()


def qualified(row):
    return row['schema'], row['table'], row['name']


def inventory(rows, key):
    require(isinstance(rows, list) and len(rows) <= MAX_OBJECTS)
    result = {}
    for row in rows:
        require(isinstance(row, dict))
        identity = key(row)
        require(identity not in result)
        result[identity] = row
    return result


@facet_context('columns')
def logical_columns(relation):
    rows = relation['columns'] or []
    require(len(rows) <= MAX_OBJECTS)
    live = [a for a in rows if not a['dropped']]
    require(len({a['name'] for a in live}) == len(live))
    require(all(type(a['num']) is int and a['num'] > 0 for a in live))
    return [{k: v for k, v in a.items() if k not in ['num', 'dropped']} for a in live]


def constraint_meta(row):
    return {k: row[k] for k in ['kind', 'domain', 'parent', 'index', 'foreignRelation', 'flags', 'comment', 'keys', 'dependencies']}


@facet_context('expression_context')
def ordinary(constraint, relation):
    require(not any(constraint[k] for k in ['domain', 'parent', 'index', 'foreignRelation']))
    require(relation['kind'] == 'r' and not relation['partition'] and relation['inheritance'] == 0)
    require(constraint['flags']['conislocal'] is True and constraint['flags']['coninhcount'] == 0)


@facet_context('nn')
def plan_nn(source, restored):
    validate_metadata(source)
    validate_metadata(restored)
    sr = inventory(source['catalog']['relations'], lambda r: (r['schema'], r['name']))
    dr = inventory(restored['catalog']['relations'], lambda r: (r['schema'], r['name']))
    require(sr.keys() == dr.keys(),'relation',('inventory',))
    require_match(source['database'],restored['database'],'database')
    for key in sr:
        require_match({k: v for k, v in sr[key].items() if k != 'columns'},{k: v for k, v in dr[key].items() if k != 'columns'},'relation')
        left,right=logical_columns(sr[key]),logical_columns(dr[key])
        if not exact(left,right):
            properties=set()
            for a,b in zip(left,right):properties.update(difference_properties(a,b,'columns'))
            if len(left)!=len(right):properties.add('inventory')
            raise FidelityError('columns',properties,sum(not exact(a,b) for a,b in zip(left,right))+abs(len(left)-len(right)))
    maps = []
    copy_names = inventory(restored['catalog']['constraintNames'], qualified)
    for metadata, relations in [(source, sr), (restored, dr)]:
        values = {}
        for c in metadata['catalog']['constraints']:
            if c['kind'] != 'n':
                continue
            r = relations[(c['schema'], c['table'])]
            ordinary(c, r)
            require(isinstance(c['keys'], list) and len(c['keys']) == 1 and c['tree'] is None)
            key = c['schema'], c['table'], c['keys'][0]
            require(key not in values,'nn',('ambiguity',))
            require(any(a['name'] == key[2] and not a['dropped'] and a['notnull'] for a in r['columns']),'nn',('attnotnull',))
            values[key] = c
        maps.append(values)
    source_nn, copy_nn = maps
    require(source_nn.keys() == copy_nn.keys(),'nn',('inventory',))
    renames = []
    for key in sorted(source_nn):
        s, d = source_nn[key], copy_nn[key]
        require_match(constraint_meta(s),constraint_meta(d),'constraints')
        if s['name'] != d['name']:
            require(qualified(s) not in copy_names,'nn',('target_collision',))
            renames.append({'schema': key[0], 'table': key[1], 'column': key[2], 'from': d['name'], 'to': s['name'], 'metadata': constraint_meta(s)})
    return renames


@facet_context('expression_context')
def supported_expression(row, relation, text):
    require(isinstance(text, str) and 0 < len(text.encode()) <= MAX_EXPRESSION)
    require(relation['kind'] == 'r' and not relation['partition'] and relation['inheritance'] == 0)
    columns = {a['name']: a for a in relation['columns'] if not a['dropped']}
    dependency_names = []
    for dependency in row['dependencies'] or []:
        if dependency['object'] == '<own-relation>':
            if dependency['kind'] == 'n' or dependency['column'] is not None:
                require(dependency['column'] in columns)
                dependency_names.append(dependency['column'])
    names = row.get('keys') or []
    if names and isinstance(names[0], dict):
        names = [a['column'] for a in names]
    for name in set(names + dependency_names):
        require(name in columns)
        a = columns[name]
        require(a['typeKind'] == 'b' and a['typeSchema'] == 'pg_catalog')
        require(not a['elementKind'] or (a['elementKind'] == 'b' and a['elementSchema'] == 'pg_catalog'))


@facet_context('constraints')
def expression_pairs(source, restored):
    require(plan_nn(source, restored) == [])
    require(exact(source['catalog']['constraintNames'],restored['catalog']['constraintNames']),'constraints',('inventory',))
    sr = inventory(source['catalog']['relations'], lambda r: (r['schema'], r['name']))
    dr = inventory(restored['catalog']['relations'], lambda r: (r['schema'], r['name']))
    sc = inventory(source['catalog']['constraints'], qualified)
    dc = inventory(restored['catalog']['constraints'], qualified)
    require(sc.keys() == dc.keys(),'constraints',('inventory',))
    pairs = []
    for key in sorted(sc):
        s, d = sc[key], dc[key]
        require_match(constraint_meta(s),constraint_meta(d),'constraints')
        if s['definition'] == d['definition']:
            continue
        require(s['kind'] == 'c' and d['kind'] == 'c')
        for c, r in [(s, sr[key[:2]]), (d, dr[key[:2]])]:
            ordinary(c, r)
            require(c['flags']['conenforced'] is True and not c['flags']['condeferrable'] and not c['flags']['condeferred'])
            supported_expression(c, r, c['expr'])
            require(c['definition'].startswith('CHECK (') and c['definition'][7:].startswith(c['expr']))
            require(c['definition'] == 'CHECK (' + c['expr'] + ')' + (' NO INHERIT' if c['flags']['connoinherit'] else '') + ('' if c['flags']['convalidated'] else ' NOT VALID'))
        pairs.append({'kind': 'check', 'key': list(key), 'source': s, 'restored': d})
    si = inventory(source['indexes'], qualified)
    di = inventory(restored['indexes'], qualified)
    require(si.keys() == di.keys(),'indexes',('inventory',))
    physical = 0
    for key in sorted(si):
        s, d = si[key], di[key]
        sm = {k: v for k, v in s.items() if k not in ['definition', 'predicate', 'tree']}
        dm = {k: v for k, v in d.items() if k not in ['definition', 'predicate', 'tree']}
        # PostgreSQL heap-build MVCC bookkeeping only, never rewritten.
        require(type(sm['flags'].get('indcheckxmin')) is bool and type(dm['flags'].get('indcheckxmin')) is bool)
        physical += sm['flags']['indcheckxmin'] != dm['flags']['indcheckxmin']
        sm['flags'] = {k: v for k, v in sm['flags'].items() if k != 'indcheckxmin'}
        dm['flags'] = {k: v for k, v in dm['flags'].items() if k != 'indcheckxmin'}
        require_match(sm,dm,'indexes')
        if s['definition'] == d['definition']:
            continue
        for c, r in [(s, sr[key[:2]]), (d, dr[key[:2]])]:
            f = c['flags']
            require(not c['expression'] and c['method'] == 'btree' and c['kind'] == 'i' and not c['partition'] and c['options'] is None)
            require(f['indnatts'] == f['indnkeyatts'] and not f['indisunique'] and not f['indisprimary'] and not f['indisexclusion'])
            require(f['indisvalid'] and f['indisready'] and f['indislive'])
            for a in c['keys']:
                require(a['column'] is not None and a['attributeOptions'] is None and a['opclassSchema'] == 'pg_catalog' and a['opclassDefault'] and a['option'] in [0, 1, 2, 3])
            supported_expression(c, r, c['predicate'])
            require(c['definition'].endswith(' WHERE ' + c['predicate']))
        require(s['definition'][:-len(s['predicate'])] == d['definition'][:-len(d['predicate'])],'indexes',('definition',))
        pairs.append({'kind': 'index', 'key': list(key), 'source': s, 'restored': d})
    require(len(pairs) <= MAX_OBJECTS)
    return pairs, physical


class Incomplete(Exception):
    pass


def sql_tokens(raw):
    """Offsets into one bounded statement; strings/comments are never keywords."""
    i, n = 0, len(raw)
    while i < n:
        c = raw[i]
        if c in b' \t\r\n\f':
            i += 1
            continue
        if raw[i:i+2] == b'--':
            end = raw.find(b'\n', i+2)
            if end < 0:
                raise Incomplete()
            i = end+1
            continue
        if raw[i:i+2] == b'/*':
            depth, j = 1, i+2
            while depth:
                if j+1 >= n:
                    raise Incomplete()
                pair = raw[j:j+2]
                if pair == b'/*':
                    depth += 1
                    j += 2
                elif pair == b'*/':
                    depth -= 1
                    j += 2
                else:
                    j += 1
            i = j
            continue
        begin = i
        escaped = c in b'eE' and raw[i+1:i+2] == b"'"
        if c in b'"\'' or escaped:
            quote = 39 if escaped else c
            i += 2 if escaped else 1
            while True:
                if i >= n:
                    raise Incomplete()
                if escaped and raw[i] == 92:
                    i += 2
                elif raw[i] == quote:
                    if i+1 >= n:
                        raise Incomplete()
                    if raw[i+1] == quote:
                        i += 2
                    else:
                        i += 1
                        break
                else:
                    i += 1
            if quote == 34:
                yield ('id', raw[begin+1:i-1].replace(b'""', b'"').decode(), begin, i)
            else:
                yield ('string', '', begin, i)
            continue
        if c == 36:
            match = re.match(rb'\$(?:[A-Za-z_][A-Za-z_0-9]*)?\$', raw[i:])
            if match:
                tag = match[0]
                require(len(tag) <= 128)
                end = raw.find(tag, i+len(tag))
                if end < 0:
                    raise Incomplete()
                i = end+len(tag)
                yield ('string', '', begin, i)
                continue
            if re.fullmatch(rb'\$[A-Za-z_0-9]*', raw[i:]):
                require(n-i <= 128)
                raise Incomplete()
        if c in b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_':
            i += 1
            while i < n and raw[i] in b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_0123456789$':
                i += 1
            yield ('word', raw[begin:i].decode().lower(), begin, i)
        else:
            i += 1
            yield ('symbol', chr(c), begin, i)


def statement_end(raw):
    depth = 0
    try:
        for kind, value, begin, end in sql_tokens(raw):
            if kind != 'symbol':
                continue
            if value == '(':
                depth += 1
            elif value == ')':
                depth -= 1
                require(depth >= 0)
            elif value == ';' and depth == 0:
                return end
    except Incomplete:
        return None
    return None


def parse_name(tokens, position):
    require(position < len(tokens) and tokens[position][0] in ['id', 'word'])
    first = tokens[position][1]
    position += 1
    if position < len(tokens) and tokens[position][1] == '.':
        require(position+1 < len(tokens) and tokens[position+1][0] in ['id', 'word'])
        return (first, tokens[position+1][1]), position+2
    return (None, first), position


class DumpFingerprint:
    """Chunk-independent raw and framed canonical digests; COPY rows never buffer."""
    def __init__(self, evidence, side):
        require(side in ['source', 'restored'] and len(evidence) <= MAX_OBJECTS)
        self.evidence = {}
        for item in evidence:
            key = (item['kind'], *item['key'])
            require(key not in self.evidence and valid_hash(item[side+'TreeSha256']))
            self.evidence[key] = item
        self.side = side
        self.used = set()
        self.raw = hashlib.sha256()
        self.canonical = hashlib.sha256(b'schoolorbit.pg18.6.framed-dump.v1\0')
        self.run = hashlib.sha256()
        self.run_size = 0
        self.buffer = bytearray()
        self.copy = False
        self.copy_newline = False
        self.line_start = True
        self.copy_prefix = bytearray()
        self.statement_count = 0

    def raw_run(self, raw):
        self.run.update(raw)
        self.run_size += len(raw)

    @facet_context('framing')
    def expression(self, key, raw):
        require(key not in self.used and key in self.evidence,'framing',('duplicate' if key in self.used else 'binding',))
        entry = self.evidence[key]
        expected = entry[self.side]['expr' if key[0] == 'check' else 'predicate'].encode()
        require(raw == expected,'framing',('expression',))
        self.canonical.update(b'R'+self.run_size.to_bytes(8, 'big')+self.run.digest())
        identity = encoded(list(key))
        self.canonical.update(b'E'+len(identity).to_bytes(8, 'big')+identity+bytes.fromhex(entry[self.side+'TreeSha256']))
        self.run = hashlib.sha256()
        self.run_size = 0
        self.used.add(key)

    @facet_context('framing')
    def statement(self, raw):
        tokens = list(sql_tokens(raw))
        words = [t[1] if t[0] == 'word' else None for t in tokens]
        spans = []
        table = None
        if words[:2] == ['create', 'table']:
            table, _ = parse_name(tokens, 2)
        elif words[:2] == ['alter', 'table']:
            pos = 3 if words[2:3] == ['only'] else 2
            table, _ = parse_name(tokens, pos)
        if table and table[0] is not None:
            for i, token in enumerate(tokens):
                if token[0:2] != ('word', 'constraint') or i+3 >= len(tokens):
                    continue
                if tokens[i+1][0] not in ['id', 'word'] or words[i+2] != 'check' or tokens[i+3][1] != '(':
                    continue
                key = ('check', *table, tokens[i+1][1])
                if key not in self.evidence:
                    continue
                depth, end = 1, i+4
                while depth and end < len(tokens):
                    if tokens[end][0] == 'symbol':
                        depth += (tokens[end][1] == '(') - (tokens[end][1] == ')')
                    end += 1
                require(depth == 0)
                spans.append((tokens[i+3][3], tokens[end-1][2], key))
        if words[:2] == ['create', 'index']:
            index, pos = parse_name(tokens, 2)
            require(words[pos:pos+1] == ['on'])
            table, pos = parse_name(tokens, pos+1)
            key = ('index', table[0], table[1], index[1])
            if key in self.evidence:
                require(index[0] in [None, table[0]])
                depth, where = 0, []
                for i in range(pos, len(tokens)):
                    kind, value, _, _ = tokens[i]
                    if kind == 'symbol':
                        depth += (value == '(') - (value == ')')
                    elif kind == 'word' and value == 'where' and depth == 0:
                        where.append(i)
                require(len(where) == 1 and tokens[-1][1] == ';')
                begin, end = tokens[where[0]][3], tokens[-1][2]
                while begin < end and raw[begin] in b' \t\r\n':
                    begin += 1
                while end > begin and raw[end-1] in b' \t\r\n':
                    end -= 1
                spans.append((begin, end, key))
        position = 0
        for begin, end, key in sorted(spans):
            require(position <= begin < end)
            self.raw_run(raw[position:begin])
            self.expression(key, raw[begin:end])
            position = end
        self.raw_run(raw[position:])
        return words[:1] == ['copy'] and words[-3:-1] == ['from', 'stdin']

    @facet_context('framing')
    def feed(self, chunk):
        require(isinstance(chunk, bytes) and len(chunk) <= 1024*1024)
        self.raw.update(chunk)
        position = 0
        while position < len(chunk):
            if self.copy_newline:
                require(chunk[position] == 10)
                self.raw_run(chunk[position:position+1])
                position += 1
                self.copy_newline = False
                self.copy = True
                continue
            if self.copy:
                if self.line_start:
                    self.copy_prefix.append(chunk[position])
                    position += 1
                    prefix = bytes(self.copy_prefix)
                    if prefix == b'\\.\n':
                        self.raw_run(prefix)
                        self.copy_prefix.clear()
                        self.copy = False
                    elif b'\\.\n'.startswith(prefix):
                        continue
                    else:
                        self.raw_run(prefix)
                        self.copy_prefix.clear()
                        self.line_start = prefix.endswith(b'\n')
                    continue
                end = chunk.find(b'\n', position)
                end = len(chunk) if end < 0 else end+1
                self.raw_run(chunk[position:end])
                self.line_start = chunk[end-1:end] == b'\n'
                position = end
                continue
            # Scan once per input chunk (including dollar bodies with many
            # semicolons), never once per semicolon inside a quoted body.
            self.buffer.extend(chunk[position:])
            position = len(chunk)
            stop = statement_end(self.buffer)
            if stop is not None:
                require(stop <= MAX_DDL,'framing',('limit',))
                self.statement_count += 1
                require(self.statement_count <= MAX_OBJECTS,'framing',('limit',))
                raw = bytes(self.buffer[:stop])
                chunk = bytes(self.buffer[stop:])
                position = 0
                self.buffer.clear()
                if self.statement(raw):
                    self.copy_newline = True
            else:
                require(len(self.buffer) <= MAX_DDL,'framing',('limit',))

    @facet_context('framing')
    def finish(self):
        require(not self.copy and not self.copy_newline and not self.copy_prefix,'framing',('copy_termination',))
        # The pinned dump's closing meta-command/comments contain no statement.
        tail = bytes(self.buffer)
        try:
            tokens = list(sql_tokens(tail+b'\n'))
        except Incomplete:
            raise FidelityError('framing',('syntax',)) from None
        require(not tokens or (len(tokens) >= 3 and tokens[0][0:2] == ('symbol', '\\') and tokens[1][0:2] == ('word', 'unrestrict') and re.fullmatch(rb'[A-Za-z0-9_]{1,128}', tail[tokens[2][2]:tokens[-1][3]])))
        self.raw_run(tail)
        self.canonical.update(b'R'+self.run_size.to_bytes(8, 'big')+self.run.digest())
        require(self.used == self.evidence.keys(),'framing',('unused_evidence',),len(self.evidence.keys()-self.used))
        return {'raw': self.raw.hexdigest(), 'canonical': self.canonical.hexdigest(), 'expressions': len(self.used)}
