#!/usr/bin/env python3
"""Pure adversarial tests. No database, container, or external provider."""
import copy
import hashlib
import importlib.util
import sys
import unittest
from pathlib import Path

sys.dont_write_bytecode = True
spec=importlib.util.spec_from_file_location('academic_restore_fidelity',Path(__file__).resolve().parents[1]/'lib/pipeline-remote/academic_restore_fidelity.py')
f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)


SOURCE="((v)::text = ANY ((ARRAY['a'::character varying, 'b'::character varying])::text[]))"
RESTORED="((v)::text = ANY (ARRAY[('a'::character varying)::text, ('b'::character varying)::text]))"
TREE='{SCALARARRAYOPEXPR :opno 98 :location -1 :args ({VAR :varattno 1})}'


def expression(kind='check'):
    src={'expr':SOURCE} if kind=='check' else {'predicate':SOURCE}
    dst={'expr':RESTORED} if kind=='check' else {'predicate':RESTORED}
    return {'kind':kind,'key':['public','t','valid_v' if kind=='check' else 'idx_v'],'source':src,'restored':dst,'sourceTreeSha256':f.sha(TREE.encode()),'restoredTreeSha256':f.sha(TREE.encode())}


def sql(expr,rows=b'one\ttwo\n',suffix=b''):
    return ("-- CHECK (not code)\nCREATE TABLE public.t (\n v character varying(10) CONSTRAINT t_v_not_null NOT NULL,\n CONSTRAINT valid_v CHECK ("+expr+")\n);\n").encode()+b"CREATE FUNCTION public.note() RETURNS text LANGUAGE sql AS $body$ SELECT '; CONSTRAINT valid_v CHECK (fake); --WHERE'; /* nested /* note */ */ $body$;\nCOPY public.t (v) FROM stdin;\n"+rows+b"\\.\n"+suffix+b"\\unrestrict 123abcdef\n"


def fingerprint(raw,evidence,side,width=4096):
    scanner=f.DumpFingerprint(evidence,side)
    for start in range(0,len(raw),width):
        scanner.feed(raw[start:start+width])
    return scanner.finish()


def column(number=1,dropped=False):
    return dict(num=number,name='gone' if dropped else 'v',dropped=dropped,notnull=not dropped,typeSchema='pg_catalog',typeName='varchar',typeKind='b',typeCategory='S',typeMod=14,dimensions=0,identity='',generated='',collationSchema='pg_catalog',collationName='default',collationEncoding=-1,collationProvider='d',collationDeterministic=True,collationLocale=None,collationCollate=None,collationCType=None,collationRules=None,collationVersion=None,elementSchema=None,elementName=None,elementKind=None)


def constraint(kind='c',number=1):
    flags=dict(contype=kind,condeferrable=False,condeferred=False,conenforced=True,convalidated=True,confupdtype=' ',confdeltype=' ',confmatchtype=' ',conislocal=True,coninhcount=0,connoinherit=False,conpfeqop=None,conppeqop=None,conffeqop=None,confdelsetcols=None,conexclop=None,conperiod=False)
    return dict(schema='public',table='t',name='valid_v' if kind=='c' else 't_v_not_null',kind=kind,domain=False,parent=False,index=False,foreignRelation=False,flags=flags,comment=None,keys=['v'],rawKeys=[number],expr=SOURCE if kind=='c' else None,definition='CHECK ('+SOURCE+')' if kind=='c' else 'NOT NULL v',tree=TREE if kind=='c' else None,dependencies=[{'kind':'a','class':'pg_class','object':'<own-relation>','column':None},{'kind':'n','class':'pg_class','object':'<own-relation>','column':'v'}])


def index():
    flags=dict(indnatts=1,indnkeyatts=1,indisunique=False,indnullsnotdistinct=False,indisprimary=False,indisexclusion=False,indimmediate=True,indisclustered=False,indisvalid=True,indcheckxmin=False,indisready=True,indislive=True,indisreplident=False,indoption='0')
    return dict(schema='public',table='t',name='idx_v',definition='CREATE INDEX idx_v ON public.t USING btree (v) WHERE '+SOURCE,predicate=SOURCE,tree=TREE,expression=False,flags=flags,owner='owner',method='btree',kind='i',partition=False,persistence='p',options=None,tablespace=None,comment=None,keys=[dict(column='v',number=1,opclassSchema='pg_catalog',opclassName='text_ops',opclassDefault=True,opclassInput='text',opclassFamily='pg_catalog.text_ops',collationSchema='pg_catalog',collationName='default',option=0,attributeOptions=None)],dependencies=constraint()['dependencies'])


def metadata(number=1):
    return {'catalog':{'relations':[{'schema':'public','name':'t','kind':'r','partition':False,'inheritance':0,'columns':[column(number)]}],'constraints':[constraint('n',number),constraint('c',number)],'constraintNames':[{'schema':'public','table':'t','name':'t_v_not_null','kind':'n'},{'schema':'public','table':'t','name':'valid_v','kind':'c'}]},'indexes':[index()],'database':{'metadata':{'datname':'tenant','encoding':6},'owner':'owner','tablespace':'pg_default','acl':[['owner','owner','CONNECT',False]],'settings':None}}


class FidelityTests(unittest.TestCase):
    def test_diagnostics_are_closed_static_integer_masks(self):
        self.assertEqual(len(f.DIAGNOSTIC_STAGES),len(set(f.DIAGNOSTIC_STAGES)))
        for facet,properties in f.DIAGNOSTIC_PROPERTIES.items():
            self.assertEqual(len(properties),len(set(properties)))
            self.assertLessEqual(len(properties),32)
            detail=f.diagnostic(facet,properties)
            self.assertTrue(f.validate_diagnostic(detail))
            for key,value in [('stage','private-sentinel'),('facet','private-sentinel'),('mismatchCount',True),('mismatchCount',None),('mismatchCount',-1),('mismatchCount',20001),('mismatchCount',1.5),('propertyMask',True),('propertyMask',1.5),('propertyMask',2**len(properties))]:
                self.assertFalse(f.validate_diagnostic(dict(detail,**{key:value})))
        self.assertFalse(f.validate_diagnostic(dict(f.diagnostic(),payload='private-sentinel')))
        self.assertEqual(str(f.FidelityError('database',('owner',))),'restored_source_mismatch')

    def test_membership_and_capability_diagnostics_use_categories_not_identities(self):
        owner='so_11111111111141118111111111111111_owner';runtime=owner[:-6]+'_runtime';prov='schoolorbit_provisioner'
        roles=[{'name':name,'login':True,'super':False,'createDb':name==prov,'createRole':name==prov,'inherit':True,'replication':False,'bypass':False,'limit':-1,'comment':None} for name in [prov,owner,runtime]]
        graph=[{'role':owner,'member':prov,'grantor':'postgres','admin':True,'inherit':False,'set':False},{'role':owner,'member':prov,'grantor':prov,'admin':False,'inherit':False,'set':True},{'role':runtime,'member':prov,'grantor':'postgres','admin':True,'inherit':False,'set':False}]
        cases=[(graph[1:],'missing_edge'),(graph+[dict(graph[0],member=runtime)],'extra_edge')]
        for field,value,bit in [('admin',False,'admin'),('inherit',True,'inherit'),('set',True,'set'),('grantor','postgresql://private-sentinel','grantor_unknown')]:
            changed=copy.deepcopy(graph);changed[0][field]=value;cases.append((changed,bit))
        for changed,bit in cases:
            with self.assertRaises(f.FidelityError) as caught:f.supported_profile(owner,roles,changed)
            detail=caught.exception.diagnostic
            self.assertEqual(detail['facet'],'memberships')
            self.assertTrue(detail['propertyMask'] & (1<<f.DIAGNOSTIC_PROPERTIES['memberships'].index(bit)))
            self.assertGreater(detail['mismatchCount'],0)
            self.assertNotIn(owner,str(detail));self.assertNotIn('private-sentinel',str(detail))
        roles[0]['createDb']=False
        with self.assertRaises(f.FidelityError) as caught:f.supported_profile(owner,roles,graph)
        self.assertEqual(caught.exception.diagnostic,f.diagnostic('provisioner_capabilities',('createDb',)))

    def test_property_facets_locate_comparison_without_values(self):
        left=metadata();right=copy.deepcopy(left);right['database']['metadata']['encoding']=8
        with self.assertRaises(f.FidelityError) as caught:f.plan_nn(left,right)
        self.assertEqual(caught.exception.diagnostic,f.diagnostic('database',('metadata.encoding',)))
        right=copy.deepcopy(left);right['catalog']['relations'][0]['columns'][0]['typeMod']=15
        with self.assertRaises(f.FidelityError) as caught:f.plan_nn(left,right)
        self.assertEqual(caught.exception.diagnostic,f.diagnostic('columns',('typeMod',)))
        right=copy.deepcopy(left);right['indexes'][0]['owner']='private-sentinel'
        with self.assertRaises(f.FidelityError) as caught:f.expression_pairs(left,right)
        self.assertEqual(caught.exception.diagnostic,f.diagnostic('indexes',('owner',)))
        with self.assertRaises(f.FidelityError) as caught:fingerprint(b'SELECT 1;\n',[expression()], 'restored')
        self.assertEqual(caught.exception.diagnostic['facet'],'framing')

    def test_database_locale_preserves_only_both_proven_profiles_and_exact_binding(self):
        owner='so_11111111111141118111111111111111_owner';database='tenant'
        baseline={'metadata':{'datname':database,'encoding':6,'datlocprovider':'b','datlocale':'C.UTF-8','datcollate':'C','datctype':'C.UTF-8'},'owner':owner,'tablespace':'pg_default','acl':None,'settings':None}
        self.assertEqual(f.database_locale(owner,database,baseline),('C','C.UTF-8'))
        fixture=copy.deepcopy(baseline);fixture['metadata'].update(datcollate='en_US.utf8',datctype='en_US.utf8')
        self.assertEqual(f.database_locale(owner,database,fixture),('en_US.utf8','en_US.utf8'))
        for field,value in [('datname','wrong'),('encoding',8),('encoding',True),('datlocprovider','c'),('datlocale','C'),('datcollate','C.UTF-8'),('datcollate',"C'; DROP DATABASE tenant;--"),('datctype','en_US.utf8')]:
            changed=copy.deepcopy(baseline);changed['metadata'][field]=value
            with self.assertRaises(f.FidelityError):f.database_locale(owner,database,changed)
        changed=copy.deepcopy(baseline);changed['owner']='foreign'
        with self.assertRaises(f.FidelityError):f.database_locale(owner,database,changed)
        with self.assertRaises(f.FidelityError):f.database_locale(owner,"tenant';DROP",baseline)
        with self.assertRaises(f.FidelityError):f.database_locale('other',database,baseline)

    def test_role_profiles_reject_structurally_valid_graph_and_capability_deviations(self):
        owner='so_11111111111141118111111111111111_owner';runtime=owner[:-6]+'_runtime';prov='schoolorbit_provisioner'
        roles=[{'name':name,'login':True,'super':False,'createDb':name==prov,'createRole':name==prov,'inherit':True,'replication':False,'bypass':False,'limit':-1,'comment':None} for name in [prov,owner,runtime]]
        graph=[{'role':owner,'member':prov,'grantor':'postgres','admin':True,'inherit':False,'set':False},{'role':owner,'member':prov,'grantor':prov,'admin':False,'inherit':False,'set':True},{'role':runtime,'member':prov,'grantor':'postgres','admin':True,'inherit':False,'set':False}]
        self.assertEqual(f.supported_profile(owner,roles,graph),'provider3')
        fixture=[dict(graph[0],admin=False,set=True)]
        self.assertEqual(f.supported_profile(owner,roles,fixture),'postgresFixture1')
        for base in [graph,fixture]:
            for field,value in [('grantor','unrecognized'),('admin',not base[0]['admin']),('inherit',True),('set',not base[0]['set']),('member',runtime)]:
                changed=copy.deepcopy(base);changed[0][field]=value
                with self.assertRaises(f.FidelityError):f.supported_profile(owner,roles,changed)
            with self.assertRaises(f.FidelityError):f.supported_profile(owner,roles,base[1:])
            changed=copy.deepcopy(base)+[{'role':owner,'member':runtime,'grantor':'postgres','admin':False,'inherit':True,'set':True}]
            with self.assertRaises(f.FidelityError):f.supported_profile(owner,roles,changed)
        for name,field,value in [(owner,'super',True),(runtime,'bypass',True),(runtime,'createRole',True),(runtime,'replication',True),(prov,'createRole',False),(prov,'createDb',False),(prov,'super',True)]:
            changed=copy.deepcopy(roles);next(x for x in changed if x['name']==name)[field]=value
            with self.assertRaises(f.FidelityError):f.supported_profile(owner,changed,graph)

    def test_streaming_exact_expression_substitution_is_chunk_independent_and_retains_true_raw_hashes(self):
        left=sql(SOURCE);right=sql(RESTORED)
        for width in [1,2,7,31,4096,1024*1024]:
            with self.subTest(width=width):
                a=fingerprint(left,[expression()],'source',width);b=fingerprint(right,[expression()],'restored',width)
                self.assertNotEqual(a['raw'],b['raw'])
                self.assertEqual(a['raw'],hashlib.sha256(left).hexdigest())
                self.assertEqual(b['raw'],hashlib.sha256(right).hexdigest())
                self.assertEqual(a['canonical'],b['canonical'])
                self.assertEqual(a['expressions'],1)

    def test_copy_payload_is_streamed_unchanged_including_very_long_lines_and_fake_sql(self):
        rows=b'-- CONSTRAINT valid_v CHECK ('+SOURCE.encode()+b')\n\\.not-the-terminator\n'+b'x'*(3*1024*1024)+b'\tWHERE v; CHECK()\n'
        a=fingerprint(sql(SOURCE,rows),[expression()],'source',8191)
        b=fingerprint(sql(RESTORED,rows),[expression()],'restored',65535)
        self.assertEqual(a['canonical'],b['canonical'])
        changed=fingerprint(sql(RESTORED,rows.replace(b'not-the',b'not-XX',1)),[expression()],'restored',8191)
        self.assertNotEqual(a['canonical'],changed['canonical'])

    def test_every_outside_span_byte_remains_compared(self):
        source=fingerprint(sql(SOURCE),[expression()],'source')
        original=sql(RESTORED)
        for old,new in [(b'character varying(10)',b'character varying(11)'),(b't_v_not_null NOT NULL',b't_v_not_null NULL'),(b'RETURNS text',b'RETURNS varchar'),(b'-- CHECK (not code)',b'-- changed'),(b'one\ttwo',b'one\txxx')]:
            with self.subTest(old=old):
                changed=fingerprint(original.replace(old,new,1),[expression()],'restored')
                self.assertNotEqual(source['canonical'],changed['canonical'])
        extra=b"ALTER TABLE public.t OWNER TO owner;\nGRANT SELECT ON public.t TO runtime;\nSELECT pg_catalog.setval('public.seq',100,true);\n"
        source=fingerprint(sql(SOURCE,suffix=extra),[expression()],'source')
        for old,new in [(b'OWNER TO owner',b'OWNER TO wrong'),(b'GRANT SELECT',b'GRANT DELETE'),(b',100,true',b',101,true')]:
            changed=fingerprint(sql(RESTORED,suffix=extra.replace(old,new)),[expression()],'restored')
            self.assertNotEqual(source['canonical'],changed['canonical'])

    def test_flags_name_wrapper_and_operator_changes_cannot_be_substituted(self):
        for changed in [sql(RESTORED).replace(b'valid_v CHECK',b'other_v CHECK'),sql(RESTORED.replace(' = ',' <> ')),sql(RESTORED).replace(b'CHECK (',b'CHECK ((')]:
            with self.assertRaises(f.FidelityError):fingerprint(changed,[expression()],'restored')
        with self.assertRaises(f.FidelityError):fingerprint(sql(SOURCE),[expression(),expression()],'source')
        with self.assertRaises(f.FidelityError):fingerprint(sql(SOURCE).replace(b'CONSTRAINT valid_v',b'CONSTRAINT "CONSTRAINT valid_v"'),[expression()],'source')
        duplicate=sql(SOURCE).replace(b');\nCREATE FUNCTION',(', CONSTRAINT valid_v CHECK ('+SOURCE+'));\nCREATE FUNCTION').encode(),1)
        with self.assertRaises(f.FidelityError):fingerprint(duplicate,[expression()],'source')

    def test_partial_index_predicate_only_and_quoted_where_framing(self):
        evidence=[expression(),expression('index')]
        src=('CREATE INDEX idx_v ON public.t USING btree (v) WHERE '+SOURCE+';\n').encode()
        dst=('CREATE INDEX idx_v ON public.t USING btree (v) WHERE '+RESTORED+';\n').encode()
        a=fingerprint(sql(SOURCE,suffix=src),evidence,'source',7)
        b=fingerprint(sql(RESTORED,suffix=dst),evidence,'restored',3)
        self.assertEqual(a['canonical'],b['canonical'])
        for old,new in [(b'btree (v)',b'btree (v DESC)'),(b'btree',b'hash'),(b'CREATE INDEX',b'CREATE UNIQUE INDEX'),(b'idx_v',b'idx_wrong')]:
            raw=sql(RESTORED,suffix=dst.replace(old,new))
            try:
                changed=fingerprint(raw,evidence,'restored')
            except f.FidelityError:
                continue
            self.assertNotEqual(a['canonical'],changed['canonical'])

    def test_dollar_strings_escaped_quotes_nested_comments_and_limits_fail_closed(self):
        extras=b"SELECT E'escaped\\'quote; WHERE'; SELECT 'it''s; CHECK'; /* a /* nested; */ b */ SELECT 1;\n"
        self.assertEqual(fingerprint(sql(SOURCE,suffix=extras),[expression()],'source',1)['canonical'],fingerprint(sql(RESTORED,suffix=extras),[expression()],'restored',2)['canonical'])
        for raw in [b"COPY public.t (v) FROM stdin;\nunterminated",b"SELECT $bad$unclosed;",b"SELECT /* unclosed;",b'SELECT "'+b'x'*(f.MAX_DDL+1)]:
            with self.assertRaises(f.FidelityError):fingerprint(raw,[],'source',65536)

    def test_nn_maps_live_columns_across_dropped_history_but_refuses_collision_and_ambiguity(self):
        source=metadata(3);source['catalog']['relations'][0]['columns'].insert(0,column(1,True))
        restored=metadata(1);restored['catalog']['constraints'][0]['name']='t_v_not_null1';restored['catalog']['constraintNames'][0]['name']='t_v_not_null1'
        plan=f.plan_nn(source,restored)
        self.assertEqual([(x['column'],x['from'],x['to']) for x in plan],[('v','t_v_not_null1','t_v_not_null')])
        other_kind=copy.deepcopy(restored)
        other_kind['catalog']['constraintNames'].append({'schema':'public','table':'t','name':'t_v_not_null','kind':'f'})
        with self.assertRaises(f.FidelityError):f.plan_nn(source,other_kind)
        for mutate in [lambda d:d['catalog']['constraints'].append(copy.deepcopy(d['catalog']['constraints'][0])),lambda d:d['catalog']['constraints'][1].update(name='t_v_not_null'),lambda d:d['catalog']['relations'][0]['columns'][0].update(notnull=False),lambda d:d['catalog']['constraints'][0]['flags'].update(convalidated=False),lambda d:d['catalog']['relations'][0].update(inheritance=1)]:
            bad=copy.deepcopy(restored);mutate(bad)
            with self.assertRaises(f.FidelityError):f.plan_nn(source,bad)

    def test_typed_check_index_database_and_dependency_changes_refuse(self):
        source=metadata();restored=metadata()
        restored['catalog']['constraints'][1].update(expr=RESTORED,definition='CHECK ('+RESTORED+')')
        restored['indexes'][0].update(predicate=RESTORED,definition='CREATE INDEX idx_v ON public.t USING btree (v) WHERE '+RESTORED)
        pairs,physical=f.expression_pairs(source,restored)
        self.assertEqual([p['kind'] for p in pairs],['check','index']);self.assertEqual(physical,0)
        physical_only=copy.deepcopy(restored);physical_only['indexes'][0]['flags']['indcheckxmin']=True
        self.assertEqual(f.expression_pairs(source,physical_only)[1],1)
        for mutate in [lambda d:d['database'].update(owner='wrong'),lambda d:d['database']['acl'].append(['PUBLIC','owner','TEMP',False]),lambda d:d['database'].update(settings=[{'role':'other','config':['search_path=unsafe']}]),lambda d:d['catalog']['constraints'][1]['flags'].update(convalidated=False),lambda d:d['catalog']['constraints'][1]['dependencies'][1].update(column='gone'),lambda d:d['indexes'][0]['keys'][0].update(opclassName='varchar_pattern_ops'),lambda d:d['indexes'][0]['keys'][0].update(collationName='C'),lambda d:d['indexes'][0]['keys'][0].update(option=1),lambda d:d['indexes'][0].update(options=['fillfactor=80']),lambda d:d['indexes'][0].update(expression=True),lambda d:d['indexes'][0]['flags'].update(indisunique=True),lambda d:d['indexes'][0]['flags'].update(indisvalid=False),lambda d:d['indexes'][0]['flags'].update(indnkeyatts=0),lambda d:d['catalog']['relations'][0].update(partition=True),lambda d:d['catalog']['relations'][0]['columns'][0].update(typeKind='c'),lambda d:d['catalog']['constraints'][1]['dependencies'][1].update(column=None),lambda d:d['catalog']['constraints'][1]['flags'].update(convalidated=1),lambda d:d['catalog']['relations'][0].update(unknown='ignored')]:
            bad=copy.deepcopy(restored);mutate(bad)
            with self.assertRaises(f.FidelityError):f.expression_pairs(source,bad)

    def test_digest_vector_types_and_zero_mismatch_are_exact(self):
        bundle={'releaseSha':'a'*40,'imageDigest':'sha256:'+'b'*64,'inventoryFingerprint':'c'*64}
        item={'schoolId':'11111111-1111-4111-8111-111111111111','subdomain':'tenant-1','databaseName':'tenant_11111111111141118111111111111111','archiveSha256':'d'*64,'sourceFingerprint':'e'*64,'restoredSourceFingerprint':'f'*64,'restoredReconciledFingerprint':'0'*64,'cutoverDate':'2026-11-14','selectionFingerprint':'1'*64,'historyVersion':157,'decryptionVerified':True,'checks':[{'code':c,'passed':True,'mismatchCount':0} for c in f.CHECKS],'restoreFidelity':{'policy':f.POLICY,'sourceCanonicalFingerprint':'2'*64,'restoredCanonicalFingerprint':'2'*64,'metadataSha256':'3'*64,'nnIdentityMapSha256':'4'*64,'checkCount':43,'partialIndexPredicateCount':3,'nnRenameCount':16,'unclassifiedDifferenceCount':0}}
        self.assertEqual(f.evidence_digest(bundle,item),'049caf36905ac15c1581194ab09f00763892167c1ac3d01c9a22f83705cbe48a')
        for key in ['checkCount','partialIndexPredicateCount','nnRenameCount','unclassifiedDifferenceCount']:
            for value in [True,-1,20001,1.5]:
                bad=copy.deepcopy(item);bad['restoreFidelity'][key]=value
                with self.assertRaises(f.FidelityError):f.evidence_digest(bundle,bad)


if __name__=='__main__':unittest.main()
