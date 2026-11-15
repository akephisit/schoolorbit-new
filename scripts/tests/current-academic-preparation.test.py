#!/usr/bin/env python3
"""Pure/child-process tests; no database, provider or container is contacted."""
import contextlib
import copy
import datetime as dt
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import shutil
import sys
import tempfile
import unittest
from unittest import mock

spec=importlib.util.spec_from_file_location('current_preparation',Path(__file__).resolve().parents[1]/'lib/pipeline-remote/current-academic-preparation.py')
m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
ID='11111111-1111-4111-8111-111111111111'
OTHER='22222222-2222-4222-8222-222222222222'


def row(identifier=ID):
    return {'id':identifier,'subdomain':'tenant-'+identifier[0],'status':'active','db_connection_string':'postgresql://runtime:private@db.example:5432/tenant_'+identifier.replace('-','')+'?sslmode=verify-full'}


def audit():
    return {'version':157,'passed':True,'selectionFingerprint':'a'*64,'cutoverDate':'2026-11-14','runtimeGrants':True,'checks':[{'code':code,'passed':True,'mismatchCount':0} for code in m.CHECKS]}


def role_profile(identifier=ID,kind='postgresFixture1'):
    owner='so_'+identifier.replace('-','')+'_owner';runtime=owner[:-6]+'_runtime';provisioner='schoolorbit_provisioner'
    roles=[]
    for name,purpose in [(owner,'migration'),(runtime,'runtime'),(provisioner,None)]:
        roles.append({'name':name,'login':True,'super':False,'createDb':name==provisioner,'createRole':name==provisioner,'inherit':name!=provisioner,'replication':False,'bypass':False,'limit':7 if name==provisioner else -1,'comment':None if purpose is None else 'schoolorbit:'+identifier+':'+purpose})
    memberships=[{'role':owner,'member':provisioner,'grantor':'postgres','admin':False,'inherit':False,'set':True}]
    if kind=='provider3':
        memberships=[{'role':owner,'member':provisioner,'grantor':'postgres','admin':True,'inherit':False,'set':False},{'role':owner,'member':provisioner,'grantor':provisioner,'admin':False,'inherit':False,'set':True},{'role':runtime,'member':provisioner,'grantor':'postgres','admin':True,'inherit':False,'set':False}]
    return {'roles':sorted(roles,key=lambda x:x['name']),'memberships':memberships}


def database_role(name='database-reader'):
    return {'name':name,'login':False,'super':False,'createDb':False,'createRole':False,'inherit':False,'replication':False,'bypass':False,'limit':7,'comment':"private comment 'quoted' \\ retained"}


def identity_profile(identifier=ID,kind='postgresFixture1'):
    return dict(role_profile(identifier,kind),databaseRoles=[])


def database_profile(owner,database,pair=('en_US.utf8','en_US.utf8')):
    return {'metadata':{'datname':database,'encoding':6,'datlocprovider':'b','datlocale':'C.UTF-8','datcollate':pair[0],'datctype':pair[1]},'owner':owner,'tablespace':'pg_default','acl':None,'settings':None}


class Fake(m.Controller):
    def __init__(self,point,versions):
        self.point=point;self.root=point;self.release='a'*40;self.image_digest='sha256:'+'b'*64
        self.rows=m.inventory({'schools':[row(ID),row(OTHER)],'total':2});self.inventory_hash=m.inventory_hash(self.rows)
        self.key=point/'identity.agekey';self.proof=point/'proof.json';self.state=point/'state.json'
        self.client='source';self.copy='copy';self.volume='volume';self.verify='verify';self.versions=versions;self.restored=[];self.checked=False;self.recovery_validations=0
        self.compiled=[[157,True,'a'*96]]
        self.database_roles=[]
        self.source_database=database_profile('so_'+ID.replace('-','')+'_owner','tenant_'+ID.replace('-',''))
    def labels(self):return []
    def exists(self,*args):return False
    def assert_quiesced(self):pass
    def unchanged_inventory(self):pass
    @contextlib.contextmanager
    def source(self,row):
        yield {'id':row['id'],'database':'tenant_'+row['id'].replace('-',''),'user':'so_'+row['id'].replace('-','')+'_owner'},['source'],{}
    def identity(self,connection,*args):return dict(role_profile(connection['id']),databaseRoles=copy.deepcopy(self.database_roles),date='2026-11-14',version=self.versions[connection['id']])
    def source_database_roles(self,database,owner,*args):
        return copy.deepcopy(m.validate_database_roles(self.database_roles,database,owner))
    def age(self,*arguments):return ["synthetic-age",*arguments]
    def ensure_key(self):
        self.key.write_text('synthetic key');self.key.chmod(0o600);return 'synthetic recipient'
    def fingerprint(self,*args):return 'c'*64
    def command(self,*args,**kwargs):return b'1'
    def bounded_capture(self,args,**kwargs):
        if args[0]=='source':return json.dumps(self.source_database).encode()
        return m.Controller.bounded_capture(self,args,**kwargs)
    def stream(self,*args,**kwargs):
        if 'destination' in kwargs:kwargs['destination'].write(b'synthetic encrypted archive')
        else:
            self.recovery_validations+=1
            if kwargs['source'].read()!=b'synthetic encrypted archive':m.refuse('stream_failed')
    def restore_and_migrate(self,row,*args):
        self.restored.append(row['id'])
        archive=args[2]
        metadata={'catalog':{'relations':[],'constraints':[],'constraintNames':[]},'database':copy.deepcopy(self.source_database),'indexes':[]}
        name=row['id'].replace('-','');archive_hash=m.digest_file(archive)
        profile=role_profile(row['id'])
        md={'schemaVersion':2,'policy':m.fidelity.POLICY,'schoolId':row['id'],'archiveSha256':archive_hash,'roleMetadata':profile['roles'],'membershipMetadata':profile['memberships'],'restoredRoleMetadata':copy.deepcopy(profile['roles']),'restoredMembershipMetadata':copy.deepcopy(profile['memberships']),'databaseRoleMetadata':copy.deepcopy(self.database_roles),'restoredDatabaseRoleMetadata':copy.deepcopy(self.database_roles),'source':metadata,'restoredBefore':metadata,'restored':metadata,'oracle':[],'physicalIndexBookkeepingDifferenceCount':0}
        nm={'schemaVersion':2,'policy':m.fidelity.POLICY,'schoolId':row['id'],'archiveSha256':archive_hash,'renames':[]}
        md_path=self.point/(name+'.restore-metadata.json');nm_path=self.point/(name+'.nn-identities.json')
        m.durable(md_path,md);m.durable(nm_path,nm)
        return dict(audit(),restoredSourceFingerprint='f'*64,restoredReconciledFingerprint='d'*64,restoreFidelity={'policy':m.fidelity.POLICY,'sourceCanonicalFingerprint':'e'*64,'restoredCanonicalFingerprint':'e'*64,'metadataSha256':m.digest_file(md_path),'nnIdentityMapSha256':m.digest_file(nm_path),'checkCount':0,'partialIndexPredicateCount':0,'nnRenameCount':0,'unclassifiedDifferenceCount':0})
    def check(self):self.checked=True


def completion_fixture(point,database_roles=None):
    fake=Fake(point,{ID:157,OTHER:157})
    if database_roles:
        fake.database_roles=copy.deepcopy(database_roles)
        fake.source_database['acl']=[[role['name'],'so_'+ID.replace('-','')+'_owner','CONNECT',False] for role in database_roles]
    fake.key.write_bytes(b'synthetic key');fake.key.chmod(0o600)
    archive=point/(ID.replace('-','')+'.dump.age')
    archive.write_bytes(b'synthetic encrypted archive');archive.chmod(0o600)
    restored=fake.restore_and_migrate(fake.rows[0],None,None,archive,None,'c'*64)
    item={'schoolId':ID,'subdomain':'tenant-1','databaseName':'tenant_'+ID.replace('-',''),'cutoverDate':'2026-11-14','selectionFingerprint':'a'*64,'checks':audit()['checks'],'archiveSha256':m.digest_file(archive),'sourceFingerprint':'c'*64,'restoredSourceFingerprint':restored['restoredSourceFingerprint'],'restoredReconciledFingerprint':restored['restoredReconciledFingerprint'],'restoreFidelity':restored['restoreFidelity'],'decryptionVerified':True,'historyVersion':157}
    bundle={'schemaVersion':2,'operation':m.OPERATION,'releaseSha':fake.release,'imageDigest':fake.image_digest,'inventoryFingerprint':fake.inventory_hash,'tenants':[item]}
    item['restoreFidelity']['evidenceSha256']=m.fidelity.evidence_digest(bundle,item)
    m.durable(fake.proof,bundle)
    profile=role_profile()
    m.durable(point/(ID.replace('-','')+'.json'),{'roleMetadata':profile['roles'],'membershipMetadata':profile['memberships'],'databaseRoleMetadata':copy.deepcopy(fake.database_roles)})
    m.durable(fake.state,{'state':'prepared','retainUntil':None})
    fake.command=lambda *a,**k:json.dumps(audit()).encode()
    return fake,archive


def refresh_archive_binding(fake,archive):
    bundle=m.read_json(fake.proof);item=bundle['tenants'][0];item['archiveSha256']=m.digest_file(archive)
    for suffix,field in [('.restore-metadata.json','metadataSha256'),('.nn-identities.json','nnIdentityMapSha256')]:
        path=fake.point/(ID.replace('-','')+suffix)
        document=m.read_json(path);document['archiveSha256']=item['archiveSha256'];m.durable(path,document)
        item['restoreFidelity'][field]=m.digest_file(path)
    item['restoreFidelity']['evidenceSha256']=m.fidelity.evidence_digest(bundle,item)
    m.durable(fake.proof,bundle)


class PreparationTests(unittest.TestCase):
    def test_canonical_failure_enriches_only_with_transient_completed_observations(self):
        profiles=[
            (b'18.6','source_header_plain_18_6'),
            (b'18.6 (Debian 18.6-1.pgdg13+1)','source_header_debian_18_6_1'),
            (b'18.6 (Debian 18.6-1.pgdg13+2)','source_header_debian_18_6_2'),
            (b'18.6 (private-sentinel)','source_header_unclassified'),
            (b'18.6',None),
        ]
        original_observer=m.fidelity.DumpDiagnostics
        class Poisoned(original_observer):
            def complete(self):
                super().complete();self.source_header_profile='postgresql://private-sentinel'
        for version,profile in profiles:
            with self.subTest(profile=profile),tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:155});fake.exits=0
                owner='so_'+ID.replace('-','')+'_owner';connection={'user':owner,'database':fake.source_database['metadata']['datname']}
                archive=fake.point/'source.dump.age';archive.write_bytes(b'synthetic encrypted archive')
                prefix=b'--\n-- PostgreSQL database dump\n--\n\n\\restrict stableNonce\n-- Dumped from database version '
                tail=b"\n-- Dumped by pg_dump version 18.6\n\nSET statement_timeout = 0;\nSELECT 'private-sentinel';\n\\unrestrict stableNonce\n"
                source=prefix+version+tail;restored=prefix+b'18.6 (other-target-package)'+tail
                metadata={'catalog':{'relations':[],'constraints':[],'constraintNames':[]},'indexes':[],'database':copy.deepcopy(fake.source_database)}
                fake.catalog=lambda *a:copy.deepcopy(metadata);fake.oracle=lambda *a:[]
                fake.copy_sql=lambda *a:b'';fake.copy_role_profile=lambda *a:role_profile()
                fake.migrate_copy=mock.Mock(side_effect=AssertionError('CLI must remain unreachable'))
                def original(data,evidence=None,side='source'):
                    base=[sys.executable,'-c','import sys;sys.stdout.buffer.write('+repr(data)+')']
                    return m.Controller.fingerprint(fake,base,None,'stableNonce',evidence,side)
                fake.fingerprint=lambda base,environment,nonce,evidence=None,side='source':original(source,evidence,side)
                fake.restored_fingerprint=lambda connection,nonce,evidence=None:original(restored,evidence,'restored')
                observation=mock.patch.object(m.fidelity,'DumpDiagnostics',Poisoned) if profile is None else contextlib.nullcontext()
                with observation,mock.patch.object(m.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                    with self.assertRaises(m.fidelity.FidelityError) as caught:m.Controller.restore_and_migrate(fake,fake.rows[0],connection,identity_profile(),archive,'stableNonce',m.digest(source))
                properties=('canonical','dumped_from_header')+((profile,) if profile is not None else ())
                expected=m.fidelity.diagnostic('canonical',properties,count=2);expected['stage']='unknown'
                self.assertEqual(caught.exception.diagnostic,expected)
                self.assertEqual(fake._canonical_observations,{});self.assertEqual(fake._canonical_header_profiles,{})
                fake.migrate_copy.assert_not_called();self.assertFalse(fake.proof.exists())
                for path in fake.point.iterdir():self.assertNotIn(b'private-sentinel',path.read_bytes())

    def test_fingerprint_observer_failure_and_malformed_context_do_not_change_result_shape(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155});fake.exits=0
            data=b'SELECT 1;\n\\unrestrict stableNonce\n'
            base=[sys.executable,'-c','import sys;sys.stdout.buffer.write('+repr(data)+')']
            expected=m.fidelity.DumpFingerprint([],'source');expected.feed(data);expected=expected.finish()
            class Broken:
                def begin_statement(self,*a):raise RuntimeError('private-sentinel')
            with mock.patch.object(m.fidelity,'DumpDiagnostics',Broken):
                actual=m.Controller.fingerprint(fake,base,None,'stableNonce',[],side='source')
            self.assertEqual(actual,expected);self.assertEqual(set(actual),{'raw','canonical','expressions'})
            self.assertIsNone(fake._canonical_observations['source'])
            with mock.patch.object(m.fidelity,'DumpDiagnostics',side_effect=ValueError('private-sentinel')):
                self.assertEqual(m.Controller.fingerprint(fake,base,None,'stableNonce',[],side='source'),expected)
            self.assertEqual(m.Controller.fingerprint(fake,base,None,'stableNonce'),m.digest(data))

    def test_main_refusal_receipt_is_private_static_and_never_masks_refusal(self):
        with tempfile.TemporaryDirectory() as directory:
            point=Path(directory);point.chmod(0o700)
            class Failing:
                def __init__(self):self.point=point;self.diagnostic_stage='copy_stream'
                def prepare(self):
                    self.diagnostic_stage='source_profile'
                    raise m.fidelity.FidelityError('memberships',('grantor_unknown',),1)
            with mock.patch.object(m,'Controller',Failing),mock.patch.object(sys,'argv',['controller','prepare']):
                with self.assertRaises(m.fidelity.FidelityError) as caught:m.main()
            expected=dict(m.fidelity.diagnostic('memberships',('grantor_unknown',),1),stage='source_profile')
            self.assertEqual(caught.exception.diagnostic,expected)
            receipt=m.private(point/'last-fidelity-refusal.json')
            self.assertEqual(m.read_json(receipt),{'schemaVersion':1,'error':'restored_source_mismatch','diagnostic':expected})
            before=receipt.read_bytes()
            with mock.patch.object(m,'Controller',Failing),mock.patch.object(sys,'argv',['controller','prepare']),mock.patch.object(m,'durable',side_effect=OSError('private-sentinel')):
                with self.assertRaises(m.fidelity.FidelityError) as caught:m.main()
            self.assertEqual(caught.exception.diagnostic,expected);self.assertEqual(receipt.read_bytes(),before)
            receipt.unlink();outside=point/'outside';outside.write_text('untouched');receipt.symlink_to(outside)
            with mock.patch.object(m,'Controller',Failing),mock.patch.object(sys,'argv',['controller','prepare']):
                with self.assertRaises(m.fidelity.FidelityError):m.main()
            self.assertEqual(outside.read_text(),'untouched');self.assertTrue(receipt.is_symlink())

    def test_main_resets_stage_and_never_reuses_receipt_as_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            point=Path(directory);point.chmod(0o700)
            class Failing:
                def __init__(self):self.point=point;self.diagnostic_stage='copy_stream'
                def check(self):raise m.fidelity.FidelityError()
            with mock.patch.object(m,'Controller',Failing),mock.patch.object(sys,'argv',['controller','check']):
                with self.assertRaises(m.fidelity.FidelityError) as caught:m.main()
            self.assertEqual(caught.exception.diagnostic,m.fidelity.diagnostic())

    def test_supported_role_profiles_are_finite_and_initial_unsafe_graph_never_creates_recovery(self):
        owner='so_'+ID.replace('-','')+'_owner'
        for kind in ['provider3','postgresFixture1']:
            profile=role_profile(kind=kind)
            self.assertEqual(m.fidelity.supported_profile(owner,profile['roles'],profile['memberships']),kind)
            provisioner=next(r for r in profile['roles'] if r['name']=='schoolorbit_provisioner')
            provisioner['login']=False;provisioner['inherit']=False
            self.assertEqual(m.fidelity.supported_profile(owner,profile['roles'],profile['memberships']),kind)
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155,OTHER:157});original=fake.identity
            def unsafe(connection,*args):
                identity=original(connection,*args)
                identity['memberships'].append({'role':connection['user'],'member':connection['user'][:-6]+'_runtime','grantor':'postgres','admin':False,'inherit':True,'set':True})
                return identity
            fake.identity=unsafe
            with self.assertRaises(m.fidelity.FidelityError):fake.prepare()
            self.assertFalse(fake.key.exists());self.assertFalse(fake.proof.exists())
            self.assertEqual(list(fake.point.glob('*.dump.age')),[]);self.assertEqual(fake.restored,[])
            fake.versions={ID:157,OTHER:157}
            fake.prepare()
            self.assertEqual(m.read_json(fake.state)['state'],'already-current')
            self.assertFalse(fake.key.exists())

    def test_setup_preserves_role_authority_and_leaves_database_to_archive(self):
        owner='so_'+ID.replace('-','')+'_owner'
        for kind,pair in [(kind,pair) for kind in ['provider3','postgresFixture1'] for pair in [('C','C.UTF-8'),('en_US.utf8','en_US.utf8'),('C.UTF-8','C.UTF-8'),('C','C')]]:
            with self.subTest(kind=kind,pair=pair),tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:155})
                profile=identity_profile(kind=kind);profile['roles'][0]['login']=False
                calls=[]
                fake.copy_sql=lambda sql,*a,**k:calls.append(sql) or b''
                fake.copy_role_profile=lambda *a:{key:copy.deepcopy(profile[key]) for key in ['roles','memberships']}
                with mock.patch.object(m.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                    with fake.isolated({'user':owner,'database':'tenant'},profile,database_profile(owner,'tenant',pair)):
                        pass
                self.assertIn('CREATE ROLE "schoolorbit_provisioner"',calls[0])
                self.assertEqual('SET ROLE schoolorbit_provisioner;' in calls[1],kind=='provider3')
                self.assertIn('RESET ROLE;',calls[1]) if kind=='provider3' else None
                self.assertIn('NOLOGIN',calls[0])
                self.assertEqual(len(calls),2)
                self.assertFalse(any('DATABASE ' in sql for sql in calls))
                self.assertFalse(any('GRANTED BY' in x or 'pg_auth_members' in x for x in calls))

    def test_invalid_locale_binding_refuses_before_creating_owned_resources(self):
        owner='so_'+ID.replace('-','')+'_owner';connection={'user':owner,'database':'tenant'}
        for field,value in [('datcollate',''),('datctype',"C'; DROP DATABASE tenant;--"),('datcollate','C\\'),('datctype','x'*256),('datlocprovider','c'),('datlocale','C'),('encoding',8),('datname','other')]:
            with self.subTest(field=field),tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:155});calls=[]
                fake.command=lambda *a,**k:calls.append(a) or b''
                source=database_profile(owner,'tenant');source['metadata'][field]=value
                with self.assertRaises(m.fidelity.FidelityError):
                    with fake.isolated(connection,identity_profile(),source):pass
                self.assertEqual(calls,[])

    def test_archive_restore_owns_database_and_refusal_never_reaches_cli(self):
        # Use the actual restore coordinator and isolated() lifecycle. Only the
        # external transport is substituted; native stock restore is separate.
        for failure in ['producer','consumer','metadata']:
            with self.subTest(failure=failure),tempfile.TemporaryDirectory() as directory:
                point=Path(directory);fake=Fake(point,{ID:155});fake.exits=0
                owner='so_'+ID.replace('-','')+'_owner';connection={'user':owner,'database':'tenant'}
                archive=point/'source.dump.age';archive.write_bytes(b'private encrypted archive sentinel')
                source={'catalog':{'relations':[],'constraints':[],'constraintNames':[]},'indexes':[],'database':database_profile(owner,'tenant')}
                restored=copy.deepcopy(source);restored['database']['acl']=[['unexpected','unexpected','CONNECT',False]]
                fake.catalog=mock.Mock(side_effect=[source,restored])
                fake.copy_sql=mock.Mock(return_value=b'');fake.copy_role_profile=lambda *a:role_profile()
                fake.restored_fingerprint=lambda *a:'f'*64
                fake.migrate_copy=mock.Mock(side_effect=AssertionError('CLI must be unreachable'))
                commands=[];fake.command=lambda args,**kwargs:commands.append(args) or b''
                observed=[]
                def restore_stream(first,second,**kwargs):
                    observed.append((first,second))
                    self.assertEqual(kwargs['stage'],'restore_archive')
                    self.assertEqual(second,['podman','exec','-i','copy','pg_restore','--create','--clean','--if-exists','--exit-on-error','-U','postgres','-d','postgres'])
                    self.assertEqual(kwargs['source'].read(),archive.read_bytes());kwargs['source'].seek(0)
                    if failure!='metadata':
                        producer=[sys.executable,'-c', 'import sys;sys.stdout.buffer.write(sys.stdin.buffer.read());raise SystemExit('+('5' if failure=='producer' else '0')+')']
                        consumer=[sys.executable,'-c','import sys;sys.stdin.buffer.read();raise SystemExit('+('7' if failure=='consumer' else '0')+')']
                        m.Controller.stream(fake,producer,consumer,**kwargs)
                fake.stream=restore_stream
                expected=m.fidelity.FidelityError if failure=='metadata' else m.Refusal
                with mock.patch.object(m.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                    with self.assertRaises(expected) as caught:m.Controller.restore_and_migrate(fake,fake.rows[0],connection,identity_profile(),archive,'nonce','c'*64)
                if failure=='metadata':
                    self.assertEqual(caught.exception.diagnostic['facet'],'database')
                else:
                    self.assertEqual(str(caught.exception),'stream_failed')
                    receipt=m.read_json(point/'last-stream-exit.json')
                    self.assertEqual(receipt['producerExit'],5 if failure=='producer' else 0)
                    self.assertEqual(receipt['consumerExit'],7 if failure=='consumer' else 0)
                self.assertEqual(len(observed),1);fake.migrate_copy.assert_not_called()
                self.assertEqual(commands[-2:],[['podman','rm','-f','copy'],['podman','volume','rm','volume']])
                self.assertFalse(fake.proof.exists());self.assertFalse(fake.state.exists())
                self.assertEqual(archive.read_bytes(),b'private encrypted archive sentinel')
                self.assertEqual(list(point.glob('*.restore-metadata.json')),[])

    def test_database_role_inventory_is_exact_bound_quoted_and_scoped(self):
        owner='so_'+ID.replace('-','')+'_owner';quoted='data "quoted" role-ก\\name'
        roles=sorted([database_role(quoted),database_role('settings-only')],key=lambda role:role['name'])
        database=database_profile(owner,'tenant');database['acl']=[[quoted,owner,'CONNECT',True],['<PUBLIC>',owner,'CONNECT',False]]
        database['settings']=[{'global':False,'role':'settings-only','config':['application_name=fixture']},{'global':True,'role':owner,'config':['lock_timeout=1s']}]
        self.assertEqual(m.database_role_names(database,owner),[role['name'] for role in roles])
        self.assertEqual(m.database_role_identifier(quoted),'"'+quoted.replace('"','""')+'"')
        self.assertEqual(m.database_role_identifier('ก'*21),'"'+'ก'*21+'"')
        for name in ['', 'x'*64, 'ก'*22, 'a\x00b', '\ud800']:
            with self.assertRaises(m.fidelity.FidelityError):m.database_role_identifier(name)
        self.assertTrue(m.database_role_literal(quoted).startswith("E'"));self.assertIn('\\\\name',m.database_role_literal(quoted))
        controller=m.Controller.__new__(m.Controller);calls=[]
        def capture(arguments,**kwargs):
            sql=arguments[-1];calls.append(sql)
            self.assertTrue(sql.startswith('SET search_path=pg_catalog; '))
            self.assertIn('setdatabase=(SELECT oid FROM selected)',sql)
            self.assertIn('a.grantee<>0',sql);self.assertIn('a.grantor',sql)
            return json.dumps({'count':2,'comments':64} if 'comments' in sql else roles).encode()
        controller.bounded_capture=capture
        self.assertEqual(controller.source_database_roles(database,owner,['source']),roles)
        self.assertEqual(len(calls),2)
        controller.bounded_capture=lambda *a,**k:json.dumps({'count':65,'comments':0}).encode()
        with self.assertRaises(m.fidelity.FidelityError):controller.source_database_roles(database,owner,['source'])
        for bad in [roles[:1],roles+[database_role('unreferenced')],list(reversed(roles)),[dict(roles[0],comment='x'*65537),roles[1]]]:
            with self.assertRaises(m.fidelity.FidelityError):m.validate_database_roles(bad,database,owner)

    def test_identity_keeps_local_object_guard_strict_and_scopes_shared_database_roles(self):
        owner='so_'+ID.replace('-','')+'_owner';connection={'user':owner,'database':'tenant'}
        for version,local_unknown,selected in [(155,1,False),(155,0,False),(157,0,False),(157,0,True)]:
            with self.subTest(version=version,local_unknown=local_unknown,selected=selected),tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:version});fake.source_history=lambda *a:version
                role=database_role();fake.database_roles=[role];fake.source_database=database_profile(owner,'tenant');fake.source_database['acl']=[[role['name'],owner,'CONNECT',False]]
                if selected:fake._recovery_profiles={ID:{}}
                primary=role_profile();calls=[]
                def capture(arguments,**kwargs):
                    sql=arguments[-1];calls.append(sql)
                    if 'unknownRoles' in sql:
                        self.assertNotIn(role['name'],sql)
                        return json.dumps(dict(primary,user=owner,database='tenant',owner=owner,marker='schoolorbit:'+ID+':migration',super=False,createDb=False,createRole=False,bypass=False,ssl=True,server='180006',unknownRoles=local_unknown,otherConnections=0)).encode()
                    if sql==m.DATABASE_SQL:return json.dumps(fake.source_database).encode()
                    if 'pg_shdepend x' in sql:
                        self.assertTrue(sql.startswith('SET search_path=pg_catalog;'))
                        self.assertIn("x.dbid=0 AND x.classid='pg_database'::regclass",sql);self.assertIn('x.objid=(SELECT oid FROM pg_database WHERE datname=current_database())',sql)
                        return b'0'
                    return json.dumps(primary['memberships']).encode()
                fake.bounded_capture=capture
                if local_unknown:
                    with self.assertRaises(m.Refusal) as caught:m.Controller.identity(fake,connection,['source'],{})
                    self.assertEqual(str(caught.exception),'source_identity_invalid');self.assertEqual(len(calls),1)
                else:
                    value=m.Controller.identity(fake,connection,['source'],{})
                    expected=[role] if version<157 or selected else []
                    self.assertEqual(value['databaseRoles'],expected)
                    self.assertEqual(any('pg_shdepend x' in sql for sql in calls),bool(expected))

    def test_database_role_setup_attests_existing_builtins_and_never_repairs_them(self):
        owner='so_'+ID.replace('-','')+'_owner';role=database_role('data "quote" role')
        for existing,expected_failure in [([],False),([role],False),([dict(role,comment='changed')],True)]:
            with tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:155});identity=identity_profile();identity['databaseRoles']=[role]
                database=database_profile(owner,'tenant');database['acl']=[[role['name'],owner,'CONNECT',False]]
                current=copy.deepcopy(existing);statements=[]
                fake.copy_database_roles=lambda expected:copy.deepcopy(current)
                fake.copy_role_profile=lambda *a:role_profile()
                def sql(statement,*args):
                    statements.append(statement)
                    if statement.startswith('CREATE ROLE '+m.database_role_identifier(role['name'])):current.append(copy.deepcopy(role))
                    return b''
                fake.copy_sql=sql
                with mock.patch.object(m.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                    if expected_failure:
                        with self.assertRaises(m.fidelity.FidelityError):
                            with fake.isolated({'user':owner,'database':'tenant'},identity,database):pass
                    else:
                        with fake.isolated({'user':owner,'database':'tenant'},identity,database):pass
                extra=[statement for statement in statements if statement.startswith('CREATE ROLE '+m.database_role_identifier(role['name']))]
                self.assertEqual(bool(extra),not existing)
                if extra:self.assertIn('NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT NOREPLICATION NOBYPASSRLS CONNECTION LIMIT 7',extra[0])
                self.assertFalse(any('ALTER ROLE' in statement or 'GRANTED BY' in statement for statement in statements))
        for name in ['postgres','pg_monitor']:
            with tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:155});identity=identity_profile();identity['databaseRoles']=[database_role(name)]
                database=database_profile(owner,'tenant');database['acl']=[[name,owner,'CONNECT',False]]
                fake.copy_role_profile=lambda *a:role_profile();fake.copy_database_roles=lambda *a:[];fake.copy_sql=mock.Mock(return_value=b'')
                with mock.patch.object(m.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                    with self.assertRaises(m.fidelity.FidelityError):
                        with fake.isolated({'user':owner,'database':'tenant'},identity,database):pass
                self.assertFalse(any(('CREATE ROLE "'+name+'"') in call.args[0] for call in fake.copy_sql.call_args_list))

    def test_database_role_readback_is_required_before_and_after_cli(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155});identity=identity_profile();identity['databaseRoles']=[database_role()]
            fake.worker='worker';fake.image='immutable';fake.copy_role_profile=lambda *a:role_profile()
            current=copy.deepcopy(identity['databaseRoles']);fake.copy_database_roles=lambda *a:copy.deepcopy(current)
            calls=[]
            def command(*args,**kwargs):calls.append(args);current[0]['inherit']=True;return b'child result'
            fake.command=command
            with self.assertRaises(m.fidelity.FidelityError):fake.migrate_copy({'user':'so_'+ID.replace('-','')+'_owner'},identity,{})
            self.assertEqual(len(calls),1)
            with self.assertRaises(m.fidelity.FidelityError):fake.migrate_copy({'user':'so_'+ID.replace('-','')+'_owner'},identity,{})
            self.assertEqual(len(calls),1)

    def test_database_role_retained_tamper_and_retry_drift_refuse_without_recovery_changes(self):
        for field in ['databaseRoleMetadata','restoredDatabaseRoleMetadata']:
            with tempfile.TemporaryDirectory() as directory:
                fake,archive=completion_fixture(Path(directory),[database_role()]);path=fake.point/(ID.replace('-','')+'.restore-metadata.json')
                document=m.read_json(path);document[field][0]['comment']='changed';m.durable(path,document)
                bundle=m.read_json(fake.proof);bundle['tenants'][0]['restoreFidelity']['metadataSha256']=m.digest_file(path);bundle['tenants'][0]['restoreFidelity']['evidenceSha256']=m.fidelity.evidence_digest(bundle,bundle['tenants'][0]);m.durable(fake.proof,bundle)
                before=archive.read_bytes()
                with self.assertRaises(m.fidelity.FidelityError):fake.retained_recovery()
                self.assertEqual(archive.read_bytes(),before)
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155,OTHER:157});fake.database_roles=[database_role()];fake.source_database['acl']=[['database-reader','so_'+ID.replace('-','')+'_owner','CONNECT',False]]
            fake.restore_and_migrate=lambda *a:m.refuse('stream_failed')
            with mock.patch.object(m.shutil,'disk_usage',return_value=type('Space',(),{'free':10**15})()):
                with self.assertRaises(m.Refusal):fake.prepare()
                archive=fake.point/(ID.replace('-','')+'.dump.age');before=archive.read_bytes();fake.database_roles[0]['comment']='source drift'
                with self.assertRaises(m.Refusal) as caught:fake.prepare()
            self.assertEqual(str(caught.exception),'source_roles_changed');self.assertEqual(archive.read_bytes(),before);self.assertFalse(fake.proof.exists())

    def test_final_copy_profile_readback_blocks_cli_after_valid_initial_creation(self):
        owner='so_'+ID.replace('-','')+'_owner';connection={'user':owner}
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155});identity=identity_profile()
            fake.worker='worker';fake.image='immutable'
            current=copy.deepcopy(identity);fake.copy_role_profile=lambda *a:{key:copy.deepcopy(current[key]) for key in ['roles','memberships']}
            self.assertEqual(fake.verify_copy_profile(connection,identity),{key:identity[key] for key in ['roles','memberships']})
            calls=[];fake.command=lambda *a,**k:calls.append(a) or b'child result'
            raw,profile=fake.migrate_copy(connection,identity,{})
            self.assertEqual(raw,b'child result');self.assertEqual(profile,{key:identity[key] for key in ['roles','memberships']});self.assertEqual(len(calls),1)
            for fault in ['flags','comment','limit','membership']:
                current=copy.deepcopy(identity)
                if fault=='flags':current['roles'][0]['inherit']=not current['roles'][0]['inherit']
                elif fault=='comment':current['roles'][0]['comment']='changed after restore'
                elif fault=='limit':current['roles'][0]['limit']+=1
                else:current['memberships'][0]['set']=False
                before=len(calls)
                with self.assertRaises(m.fidelity.FidelityError):fake.migrate_copy(connection,identity,{})
                self.assertEqual(len(calls),before)

    def test_retained_actual_restored_profile_cannot_be_replaced_even_with_updated_file_hash(self):
        for field in ['restoredRoleMetadata','restoredMembershipMetadata']:
            with self.subTest(field=field),tempfile.TemporaryDirectory() as directory:
                fake,archive=completion_fixture(Path(directory));before=fake.state.read_bytes()
                path=fake.point/(ID.replace('-','')+'.restore-metadata.json');doc=m.read_json(path)
                if field=='restoredRoleMetadata':doc[field][0]['limit']+=1
                else:doc[field][0]['inherit']=True
                m.durable(path,doc);refresh_archive_binding(fake,archive)
                with self.assertRaises(m.fidelity.FidelityError):fake.complete()
                self.assertEqual(fake.state.read_bytes(),before)

    def test_isolated_readiness_waits_for_final_tcp_server_not_temporary_socket(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155,OTHER:157})
            fake.copy_sql=lambda *a,**k:b''
            fake.copy_role_profile=lambda *a:role_profile()
            probes=[]
            def ready(arguments,**kwargs):
                probes.append(arguments)
                self.assertEqual(arguments[arguments.index('-h')+1],'127.0.0.1')
                return subprocess.CompletedProcess(arguments,1 if len(probes)==1 else 0)
            with mock.patch.object(m.subprocess,'run',side_effect=ready),mock.patch.object(m.time,'sleep'):
                owner='so_'+ID.replace('-','')+'_owner'
                with fake.isolated({'database':'tenant','user':owner},identity_profile(),database_profile(owner,'tenant')):
                    self.assertEqual(len(probes),2)

    def test_bounded_metadata_reader_limits_output_and_records_real_child_failures(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155,OTHER:157});fake.exits=0
            raw=fake.bounded_capture([sys.executable,'-c','import sys;sys.stdout.buffer.write(sys.stdin.buffer.read())'],data=b'x'*100000,limit=100001)
            self.assertEqual(raw,b'x'*100000)
            for code,limit in [('import sys;sys.stdout.buffer.write(b"x"*100001)',100000),('raise SystemExit(7)',100)]:
                with self.assertRaises(m.Refusal):fake.bounded_capture([sys.executable,'-c',code],limit=limit)
            self.assertEqual(m.read_json(fake.point/'last-metadata-exit.json')['exitCode'],7)

    def test_retained_metadata_map_role_context_and_strict_schema_are_completion_dependencies(self):
        for fault in ['missing_metadata','missing_map','tamper_metadata','tamper_roles','tamper_memberships','unknown_proof','unknown_fidelity','boolean_count','duplicate_json','nonfinite']:
            with self.subTest(fault=fault),tempfile.TemporaryDirectory() as directory:
                fake,_=completion_fixture(Path(directory));before=fake.state.read_bytes()
                metadata=fake.point/(ID.replace('-','')+'.restore-metadata.json')
                mapping=fake.point/(ID.replace('-','')+'.nn-identities.json')
                if fault=='missing_metadata':metadata.unlink()
                elif fault=='missing_map':mapping.unlink()
                elif fault=='tamper_metadata':metadata.write_bytes(metadata.read_bytes()+b' ')
                elif fault in ['tamper_roles','tamper_memberships']:
                    path=fake.point/(ID.replace('-','')+'.json');state=m.read_json(path)
                    state['roleMetadata' if fault=='tamper_roles' else 'membershipMetadata']=[{'changed':True}];m.durable(path,state)
                elif fault=='duplicate_json':fake.proof.write_text('{"schemaVersion":2,"schemaVersion":2}')
                elif fault=='nonfinite':fake.proof.write_text('{"schemaVersion":NaN}')
                else:
                    proof=m.read_json(fake.proof)
                    if fault=='unknown_proof':proof['tenants'][0]['skip']=True
                    elif fault=='unknown_fidelity':proof['tenants'][0]['restoreFidelity']['ignored']=True
                    else:proof['tenants'][0]['restoreFidelity']['checkCount']=False
                    m.durable(fake.proof,proof)
                with self.assertRaises((m.Refusal,m.fidelity.FidelityError)):fake.complete()
                self.assertEqual(fake.state.read_bytes(),before)

    def test_live_role_membership_or_relevant_database_setting_drift_blocks_completion(self):
        for fault in ['roles','memberships','database']:
            with self.subTest(fault=fault),tempfile.TemporaryDirectory() as directory:
                fake,_=completion_fixture(Path(directory));before=fake.state.read_bytes()
                if fault=='database':fake.source_database['settings']=[{'global':True,'role':'synthetic','config':['search_path=unsafe']}]
                else:
                    previous=fake.identity
                    def changed(*args):
                        identity=previous(*args);identity[fault]=[{'changed':True}];return identity
                    fake.identity=changed
                with self.assertRaises(m.fidelity.FidelityError):fake.complete()
                self.assertEqual(fake.state.read_bytes(),before)

    def test_oracle_requires_exact_trees_logical_dependencies_and_completed_owner_rollback(self):
        fake=Fake(Path('.'),{ID:155})
        pair={'kind':'check','key':['public','t','valid_v'],'source':{'schema':'public','table':'t','expr':'(v > 0)','flags':{'connoinherit':False}},'restored':{'tree':'actual-tree','dependencies':[]}}
        good={'number':0,'tree':'actual-tree','columns':[[1,'v',23,-1,0]],'actualColumns':[[1,'v',23,-1,0]],'dependencies':[]}
        rollback={'rollbackConfirmed':True,'remainingReferences':0,'role':'owner'}
        def reply(sql,connection):
            self.assertIn('BEGIN; SET LOCAL search_path=pg_catalog;',sql)
            self.assertIn('ROLLBACK;',sql)
            return [copy.deepcopy(good),copy.deepcopy(rollback)]
        fake.copy_write=reply
        self.assertEqual(fake.oracle([pair],{'user':'owner'})[0]['sourceTreeSha256'],m.digest(b'actual-tree'))
        for fault in ['tree','number','columns','dependencies','rollback','remaining']:
            def bad(sql,connection):
                rows=reply(sql,connection)
                if fault=='tree':rows[0]['tree']='different-tree'
                elif fault=='number':rows[0]['number']=False
                elif fault=='columns':rows[0]['columns'][0][0]=True
                elif fault=='dependencies':rows[0]['dependencies']=[{'changed':True}]
                elif fault=='rollback':rows[-1]['rollbackConfirmed']=False
                else:rows[-1]['remainingReferences']=1
                return rows
            fake.copy_write=bad
            with self.assertRaises(m.fidelity.FidelityError):fake.oracle([pair],{'user':'owner'})

    def test_database_settings_query_excludes_unrelated_global_roles_but_keeps_target_database_scope(self):
        # Fixed SQL ownership guard: all settings for the exact DB, but only the
        # owner/runtime/provisioner/<ALL> globals. Real SQL parity is native QA.
        self.assertIn('s.setdatabase=d.oid OR (s.setdatabase=0 AND (s.setrole=0 OR s.setrole IN',m.DATABASE_SQL)
        self.assertIn("regexp_replace(pg_get_userbyid(d.datdba),'_owner$','_runtime')",m.DATABASE_SQL)
        self.assertNotIn('WHERE s.setdatabase IN(0,d.oid)',m.DATABASE_SQL)

    def test_inventory_is_complete_unique_and_bound_to_namespace(self):
        rows=m.inventory({'schools':[row(OTHER),row(ID)],'total':2})
        self.assertEqual([r['id'] for r in rows],[ID,OTHER])
        original=m.inventory_hash(rows)
        for field,value in [('id','33333333-3333-4333-8333-333333333333'),('subdomain','changed'),('status','inactive'),('url',row()['db_connection_string'].replace('db.example','other.example'))]:
            changed=copy.deepcopy(rows);changed[0][field]=value
            self.assertNotEqual(original,m.inventory_hash(changed))
        for raw in [{'schools':[row()], 'total':2},{'schools':[row(),row()],'total':2},{'schools':[row()],'total':True}]:
            with self.assertRaises(m.Refusal):m.inventory(raw)

    def test_owner_connection_rejects_tls_identity_namespace_and_url_drift(self):
        source=m.inventory({'schools':[row()],'total':1})[0]
        url='postgresql://so_'+ID.replace('-','')+'_owner:private@db.example:5432/tenant_'+ID.replace('-','')+'?sslmode=verify-full&sslrootcert=%2Frun%2Fsecrets%2Fpostgresql-ca.crt'
        self.assertEqual(m.owner_connection(url,source)['password'],'private')
        for bad in [url.replace('verify-full','require'),url.replace('_owner:','_runtime:'),url.replace('/tenant_','/foreign_'),url.replace('db.example','foreign.example'),url.replace(':5432/',':6432/'),url.replace(':5432/',':invalid/'),url+'&options=anything',url.replace(':private@',':bad%0Asecret@')]:
            with self.assertRaises(m.Refusal):m.owner_connection(bad,source)

    def test_eight_exact_receipt_checks_and_runtime_grants_are_required(self):
        self.assertEqual(m.verify_audit(audit()),audit())
        for mutate in [lambda a:a.update(passed=False),lambda a:a.update(runtimeGrants=False),lambda a:a['checks'].pop(),lambda a:a['checks'].__setitem__(0,a['checks'][1]),lambda a:a['checks'][0].update(mismatchCount=True),lambda a:a['checks'][0].update(mismatchCount=1),lambda a:a.update(selectionFingerprint='unknown')]:
            value=audit();mutate(value)
            with self.assertRaises(m.Refusal):m.verify_audit(value)

    def test_private_files_reject_symlinks_permissions_and_ownerless_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);target=root/'proof';m.durable(target,{'known':True})
            self.assertEqual(m.read_json(target),{'known':True});self.assertEqual(target.stat().st_mode&0o777,0o600)
            link=root/'link';link.symlink_to(target)
            with self.assertRaises(m.Refusal):m.read_json(link)
            target.chmod(0o644)
            with self.assertRaises(m.Refusal):m.read_json(target)

    def test_already_converted_inventory_does_not_create_new_backup_or_rehearsal(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:157,OTHER:157});fake.prepare()
            self.assertEqual(fake.restored,[]);self.assertFalse(fake.key.exists());self.assertFalse(fake.proof.exists())
            self.assertEqual(m.read_json(fake.state)['state'],'already-current')

    def test_mixed_inventory_rehearses_only_pre157_and_binds_full_inventory(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155,OTHER:157});fake.prepare()
            self.assertEqual(fake.restored,[ID]);proof=m.read_json(fake.proof)
            self.assertEqual([t['schoolId'] for t in proof['tenants']],[ID]);self.assertEqual(proof['inventoryFingerprint'],m.inventory_hash(fake.rows));self.assertTrue(fake.checked)
            self.assertIsNone(m.read_json(fake.state)['retainUntil'])

    def test_fingerprint_preserves_every_byte_including_data_lines_starting_with_comment_marker(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            raw=b'--actual data\nCOPY x FROM stdin;\n--row-data\n\\.\n\0binary'
            script='import sys;sys.stdout.buffer.write('+repr(raw)+')'
            actual=controller.fingerprint([sys.executable,'-c',script],os.environ.copy(),'fixedNonce')
            self.assertEqual(actual,m.digest(raw));self.assertEqual(m.read_json(controller.point/'last-fingerprint-exit.json')['exitCode'],0)
            with self.assertRaises(m.Refusal):controller.fingerprint([sys.executable,'-c','raise SystemExit(9)'],os.environ.copy(),'fixedNonce')
            self.assertEqual(m.read_json(controller.point/'last-fingerprint-exit.json')['exitCode'],9)

    def test_stream_diagnostics_have_closed_shapes_and_static_classification(self):
        detail={'stage':'restore_archive','producerExit':0,'consumerExit':1,'producerCode':'unknown','consumerCode':'missing_role','producerStderrTruncated':False,'consumerStderrTruncated':False}
        self.assertTrue(m.valid_stream_failure(detail))
        for key,values in [('stage',[None,True,'private-sentinel']),('producerExit',[None,True,'1',1.5,-256,256]),('consumerExit',[None,False,'1',1.5,-256,256]),('producerCode',[None,True,'private-sentinel']),('consumerCode',[None,True,'private-sentinel']),('producerStderrTruncated',[None,0,1,'true']),('consumerStderrTruncated',[None,0,1,'false'])]:
            for value in values:self.assertFalse(m.valid_stream_failure(dict(detail,**{key:value})))
        self.assertFalse(m.valid_stream_failure(dict(detail,consumerExit=0)))
        self.assertFalse(m.valid_stream_failure(dict(detail,raw='private-sentinel')))
        self.assertEqual(m.stream_error_code(b'COPY row private-sentinel permission denied\nCONTEXT: role "private" does not exist\n'),'unknown')
        recipes={
            'authentication':b'pg_dump: error: connection to server at "private" failed: FATAL: password authentication failed',
            'missing_role':b'pg_restore: error: could not execute query: ERROR: role "private-sentinel" does not exist',
            'database_in_use':b'pg_restore: error: database "private-sentinel" is being accessed by other users',
            'database_exists':b'pg_restore: error: database "private-sentinel" already exists',
            'unsupported_parameter':b'pg_restore: error: unrecognized configuration parameter "private"',
            'locale':b'pg_restore: error: invalid locale name: "private-sentinel"',
            'archive_format':b'pg_restore: error: input file does not appear to be a valid archive',
            'decryption':b'age: error: no identity matched any of the recipients',
            'disk_space':b'age: error: failed to write output: No space left on device',
            'permission':b'pg_dump: error: query failed: ERROR: permission denied for table private',
            'connection':b'pg_dump: error: connection to server at "private" failed',
            'acl':b'pg_restore: error: could not set session authorization',
            'database_create':b'pg_restore: error: could not create database',
            'broken_pipe':b'age: error: Broken pipe',
            'tool_runtime':b'Error: unable to find image private-sentinel',
        }
        self.assertEqual(set(recipes),set(m.STREAM_CODES)-{'unknown'})
        for code,sample in recipes.items():self.assertEqual(m.stream_error_code(sample),code)
        self.assertEqual(m.stream_error_code(recipes['permission']+b'\n'+recipes['locale']),'permission')
        for message in [b'column "permission denied" does not exist',b'unknown error: private-sentinel permission denied',b'column "role fake does not exist" does not exist']:
            self.assertEqual(m.stream_error_code(b'pg_restore: error: could not execute query: ERROR: '+message+b'\nCommand was: permission denied\nERROR: permission denied'),'unknown')

    def test_stream_concurrent_stderr_flood_is_bounded_private_and_keeps_real_exits(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            producer=[sys.executable,'-c','import sys;sys.stderr.buffer.write(b"pg_dump: error: permission denied for table private-sentinel\\n"+b"\\xff"+b"p"*2000000);sys.stderr.flush();sys.stdout.buffer.write(b"original stream bytes"*200000);raise SystemExit(5)']
            consumer=[sys.executable,'-c','import sys;sys.stderr.buffer.write(b\'pg_restore: error: could not execute query: ERROR: role "private-sentinel" does not exist\\n\'+b"c"*2000000);sys.stderr.flush();assert sys.stdin.buffer.read()==b"original stream bytes"*200000;raise SystemExit(7)']
            sizes=[];classify=m.stream_error_code
            def bounded(sample):sizes.append(len(sample));return classify(sample)
            with mock.patch.object(m,'stream_error_code',side_effect=bounded):
                with self.assertRaises(m.Refusal) as caught:controller.stream(producer,consumer,stage='restore_archive')
            self.assertEqual(str(caught.exception),'stream_failed')
            expected={'stage':'restore_archive','producerExit':5,'consumerExit':7,'producerCode':'permission','consumerCode':'missing_role','producerStderrTruncated':True,'consumerStderrTruncated':True}
            self.assertEqual(caught.exception.stream_failure,expected);self.assertEqual(sizes,[16384,16384])
            path=m.private(controller.point/'last-stream-exit.json');receipt=m.read_json(path)
            self.assertEqual(receipt['streamFailure'],expected);self.assertEqual(path.stat().st_uid,os.getuid())
            for raw in [b'private-sentinel',b'pg_restore:',b'original stream bytes',b'permission denied',b'postgresql://']:self.assertNotIn(raw,path.read_bytes())

    def test_stream_success_preserves_payload_and_original_receipt_even_with_noisy_stderr(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            producer=[sys.executable,'-c','import sys;sys.stderr.buffer.write(b"private-sentinel"*20000);sys.stdout.buffer.write(bytes(range(256))*16384)']
            consumer=[sys.executable,'-c','import sys;sys.stderr.buffer.write(b"\\xffprivate-sentinel"*20000);sys.stdout.buffer.write(sys.stdin.buffer.read())']
            output=controller.point/'output'
            with output.open('wb') as destination:controller.stream(producer,consumer,destination=destination,stage='backup_archive')
            self.assertEqual(output.read_bytes(),bytes(range(256))*16384)
            self.assertEqual(m.read_json(controller.point/'last-stream-exit.json'),{'schemaVersion':1,'sequence':1,'producerExit':0,'consumerExit':0})

    def test_stream_exact_stderr_cap_and_original_signal_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            consumer=[sys.executable,'-c','import sys;sys.stdin.buffer.read()']
            for length in [16384,16385]:
                producer=[sys.executable,'-c','import sys;sys.stderr.buffer.write(b"x"*'+str(length)+');raise SystemExit(2)']
                with self.assertRaises(m.Refusal) as caught:controller.stream(producer,consumer,stage='backup_archive')
                detail=caught.exception.stream_failure
                self.assertEqual(detail['producerExit'],2);self.assertEqual(detail['consumerExit'],0)
                self.assertEqual(detail['producerStderrTruncated'],length>16384);self.assertFalse(detail['consumerStderrTruncated'])
                self.assertEqual(detail['producerCode'],'unknown')
            producer=[sys.executable,'-c','import os,signal;os.kill(os.getpid(),signal.SIGTERM)']
            with self.assertRaises(m.Refusal) as caught:controller.stream(producer,consumer)
            self.assertEqual(caught.exception.stream_failure['producerExit'],-15)
            producer=[sys.executable,'-c','import signal,sys;signal.signal(signal.SIGPIPE,signal.SIG_DFL);sys.stdout.buffer.write(b"x"*10000000)']
            consumer=[sys.executable,'-c','import sys;sys.stderr.write("pg_restore: error: invalid locale name\\n");raise SystemExit(7)']
            with self.assertRaises(m.Refusal) as caught:controller.stream(producer,consumer,stage='restore_archive')
            self.assertEqual(caught.exception.stream_failure['producerExit'],-13)
            self.assertEqual(caught.exception.stream_failure['consumerExit'],7)
            self.assertEqual(caught.exception.stream_failure['consumerCode'],'locale')

    def test_stream_receipt_failure_does_not_mask_refusal_or_follow_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            producer=[sys.executable,'-c','import sys;sys.stdout.buffer.write(b"safe");raise SystemExit(3)']
            consumer=[sys.executable,'-c','import sys;sys.stdin.buffer.read()']
            with mock.patch.object(m,'durable',side_effect=OSError('private-sentinel')):
                with self.assertRaises(m.Refusal) as caught:controller.stream(producer,consumer,stage='retained_validation')
            self.assertEqual(str(caught.exception),'stream_failed');self.assertTrue(m.valid_stream_failure(caught.exception.stream_failure))
            target=controller.point/'target';target.write_text('untouched');(controller.point/'last-stream-exit.json').symlink_to(target)
            with self.assertRaises(m.Refusal):controller.stream(producer,consumer)
            self.assertEqual(target.read_text(),'untouched');self.assertTrue((controller.point/'last-stream-exit.json').is_symlink())

    def test_stream_descendant_held_stderr_cannot_change_known_client_outcomes(self):
        for code in [0,3]:
            with self.subTest(code=code),tempfile.TemporaryDirectory() as directory:
                controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
                pidfile=controller.point/'descendant.pid'
                program='import os,sys,time;pid=os.fork();'+'\nif pid==0:\n os.close(1);time.sleep(5);os._exit(0)\n'+ 'open(sys.argv[1],"w").write(str(pid));sys.stdout.buffer.write(b"payload");raise SystemExit('+str(code)+')'
                producer=[sys.executable,'-c',program,str(pidfile)]
                consumer=[sys.executable,'-c','import sys;assert sys.stdin.buffer.read()==b"payload"']
                try:
                    start=m.time.monotonic()
                    if code:
                        with self.assertRaises(m.Refusal) as caught:controller.stream(producer,consumer,stage='backup_archive')
                        detail=caught.exception.stream_failure
                        self.assertEqual(detail['producerExit'],3);self.assertEqual(detail['consumerExit'],0);self.assertTrue(detail['producerStderrTruncated'])
                    else:
                        controller.stream(producer,consumer)
                        self.assertNotIn('streamFailure',m.read_json(controller.point/'last-stream-exit.json'))
                    self.assertLess(m.time.monotonic()-start,1)
                finally:
                    if pidfile.exists():
                        try:os.kill(int(pidfile.read_text()),9)
                        except ProcessLookupError:pass

    def test_stream_timeout_and_launch_failure_never_report_cleanup_kills_as_original_exits(self):
        actual=m.subprocess.Popen;children=[]
        def launch(*args,**kwargs):
            child=actual(*args,**kwargs);children.append(child);return child
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            sleeping=[sys.executable,'-c','import time;time.sleep(5)']
            with mock.patch.object(m.subprocess,'Popen',side_effect=launch),mock.patch.object(m,'MAX_SECONDS',0.05):
                with self.assertRaises(m.Refusal) as caught:controller.stream(sleeping,sleeping,stage='restore_archive')
            self.assertEqual(str(caught.exception),'stream_outcome_unknown');self.assertFalse(hasattr(caught.exception,'stream_failure'))
            self.assertTrue(all(child.poll() is not None for child in children));self.assertFalse((controller.point/'last-stream-exit.json').exists())
            children.clear()
            def fail_second(*args,**kwargs):
                if children:raise FileNotFoundError('private-sentinel')
                return launch(*args,**kwargs)
            with mock.patch.object(m.subprocess,'Popen',side_effect=fail_second):
                with self.assertRaises(FileNotFoundError):controller.stream(sleeping,sleeping)
            self.assertEqual(len(children),1);self.assertIsNotNone(children[0].poll());self.assertFalse((controller.point/'last-stream-exit.json').exists())

    def test_stream_requires_both_actual_child_exit_codes(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            source=[sys.executable,'-c','import sys;sys.stdout.buffer.write(b"synthetic archive")']
            sink=[sys.executable,'-c','import sys;assert sys.stdin.buffer.read()==b"synthetic archive"']
            controller.stream(source,sink)
            self.assertEqual(m.read_json(controller.point/'last-stream-exit.json')['consumerExit'],0)
            with self.assertRaises(m.Refusal):controller.stream(source,[sys.executable,'-c','raise SystemExit(4)'])
            self.assertEqual(m.read_json(controller.point/'last-stream-exit.json')['consumerExit'],4)

    def test_completion_binds_exact_eight_checks_date_hash_and_never_shortens_retention(self):
        with tempfile.TemporaryDirectory() as directory:
            fake,_=completion_fixture(Path(directory))
            future=(dt.datetime.now(dt.timezone.utc)+dt.timedelta(days=20)).isoformat()
            m.durable(fake.state,{'state':'prepared','retainUntil':future})
            fake.fingerprint=lambda *a,**k:self.fail('completion must not compare the contracted live source with its old hash')
            fake.complete();self.assertGreaterEqual(dt.datetime.fromisoformat(m.read_json(fake.state)['retainUntil']),dt.datetime.fromisoformat(future))
            self.assertEqual(fake.recovery_validations,1)
            bad=audit();bad['selectionFingerprint']='d'*64;fake.command=lambda *a,**k:json.dumps(bad).encode()
            before=fake.state.read_bytes()
            with self.assertRaises(m.Refusal):fake.complete()
            self.assertEqual(fake.state.read_bytes(),before)

    def test_check_and_completion_refuse_missing_replaced_or_unvalidated_recovery_without_starting_retention(self):
        for operation in ['check','complete']:
            for fault in ['missing_key','public_key','missing_archive','replaced_archive','truncated_archive']:
                with self.subTest(operation=operation,fault=fault),tempfile.TemporaryDirectory() as directory:
                    fake,archive=completion_fixture(Path(directory))
                    if fault=='missing_key':fake.key.unlink()
                    elif fault=='public_key':fake.key.chmod(0o644)
                    elif fault=='missing_archive':archive.unlink()
                    else:
                        archive.write_bytes(b'wrong' if fault=='replaced_archive' else b'synthetic encrypted archiv')
                        if fault=='truncated_archive':
                            # Even a hash-valid private proof cannot bless a
                            # consumer refusing the complete retained stream.
                            refresh_archive_binding(fake,archive)
                    before=fake.state.read_bytes()
                    with self.assertRaises(m.Refusal):getattr(m.Controller,operation)(fake)
                    self.assertEqual(fake.state.read_bytes(),before)

    def test_no_cleanup_or_arbitrary_command_input_is_supported(self):
        with mock.patch.object(sys,'argv',['fixed-controller','cleanup']):
            with self.assertRaises(m.Refusal):m.main()
        with mock.patch.object(sys,'argv',['fixed-controller','prepare','--sql','anything']):
            with self.assertRaises(m.Refusal):m.main()

    def test_cli_refusal_has_exact_version_code_schema_and_never_adopts_raw_errors(self):
        diagnostic={'schemaVersion':1,'operation':'migrate_tenant_schema','status':'refused','version':157,'sqlstate':'23514','code':'ACADEMIC_CURRENT_PREPARATION_MISMATCH'}
        self.assertEqual(m.copy_refusal(json.dumps(diagnostic).encode()),diagnostic)
        for value in [dict(diagnostic,payload='private sentinel'),dict(diagnostic,version=151),dict(diagnostic,code='postgresql://private'),dict(diagnostic,code=[]),dict(diagnostic,sqlstate='private')]:
            self.assertIsNone(m.copy_refusal(json.dumps(value).encode()))
        self.assertIsNone(m.copy_refusal(b'COPY error private row sentinel'))
        self.assertIsNone(m.copy_refusal(json.dumps(diagnostic).encode()+b'\nraw extra output'))

    def test_actual_source_history_requires_exact_contiguous_checksums(self):
        controller=m.Controller.__new__(m.Controller)
        controller.compiled=[[1,True,'a'*96],[2,True,'b'*96]]
        for rows,valid in [([],True),(controller.compiled[:1],True),(controller.compiled,True),([[2,True,'b'*96]],False),([[1,False,'a'*96]],False),([[1,True,'c'*96]],False),(controller.compiled+[[3,True,'d'*96]],False)]:
            replies=iter([b't',json.dumps(rows).encode()]);controller.command=lambda *a,**k:next(replies)
            if valid:self.assertEqual(controller.source_history([],{}),len(rows))
            else:
                with self.assertRaises(m.Refusal):controller.source_history([],{})

    def test_future_compiled_history_remains_valid_without_requiring_new157_rehearsal(self):
        controller=m.Controller.__new__(m.Controller);controller.worker='copy';controller.image='immutable';controller.labels=lambda:[]
        controller.command=lambda *a,**k:('\n'.join('a'*96+'  /app/migrations/'+str(i).zfill(3)+'_fixture.sql' for i in range(1,159))).encode()
        self.assertEqual(controller.compiled_history()[-1][0],158)

    def test_fingerprint_timeout_records_unknown_outcome_and_never_returns_a_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.exits=0
            with mock.patch.object(m,'MAX_SECONDS',0.03):
                with self.assertRaises(m.Refusal):controller.fingerprint([sys.executable,'-c','import time;time.sleep(1)'],os.environ.copy(),'fixed')
            self.assertEqual(m.read_json(controller.point/'last-fingerprint-exit.json')['outcome'],'unknown')

    def test_insufficient_disk_space_cannot_publish_or_rehearse_an_archive(self):
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155,OTHER:157})
            with mock.patch.object(m.shutil,'disk_usage',return_value=type('Space',(),{'free':0})()):
                with self.assertRaises(m.Refusal):fake.prepare()
            self.assertFalse(fake.proof.exists());self.assertEqual(fake.restored,[])

    def test_interrupted_controller_only_reclaims_exact_nonce_bound_local_or_readonly_resources(self):
        with tempfile.TemporaryDirectory() as directory:
            controller=m.Controller.__new__(m.Controller);controller.point=Path(directory);controller.release='a'*40;controller.image_digest='sha256:'+'b'*64
            controller.client='source';controller.copy='copy';controller.crypto='crypto';controller.worker='worker';controller.verify='verify';controller.volume='volume'
            old={'releaseSha':controller.release,'imageDigest':controller.image_digest,'pid':999999999,'startIdentity':'unknown','nonce':'c'*64}
            m.durable(controller.point/'controller.json',old)
            controller.exists=lambda kind,name:name=='source'
            calls=[]
            def command(args,**kwargs):
                calls.append(args)
                return json.dumps({'org.schoolorbit.preparation.release':controller.release,'org.schoolorbit.preparation.nonce':old['nonce']}).encode() if 'inspect' in args else b''
            controller.command=command;controller.reconcile_owned()
            self.assertIn(['podman','rm','-f','source'],calls)
            self.assertNotIn(['podman','rm','-f','schoolorbit-backend-school'],calls)
            old['pid']=999999999;m.durable(controller.point/'controller.json',old)
            controller.command=lambda *a,**k:b'{}'
            with self.assertRaises(m.Refusal):controller.reconcile_owned()
            old['pid']=os.getpid();old['startIdentity']=Path('/proc/self/stat').read_text().split(') ',1)[1].split()[19];m.durable(controller.point/'controller.json',old)
            with self.assertRaises(m.Refusal):controller.reconcile_owned()

    def test_pinned_no_egress_copy_and_source_acl_hash_contract_is_not_weakened(self):
        text=(Path(__file__).resolve().parents[1]/'lib/pipeline-remote/current-academic-preparation.py').read_text()
        self.assertIn("'--network', 'none'",text);self.assertIn("'--network', 'container:' + self.copy",text)
        self.assertNotIn("'--no-owner'",text);self.assertNotIn("'--no-acl'",text);self.assertNotIn("'--no-comments'",text)
        self.assertEqual(m.OPERATION,'academic-current-156157')
        self.assertEqual(text.count("['podman', 'run',"),text.count("['podman', 'run', '--log-driver=none',"))


class CryptoTests(unittest.TestCase):
    def test_completion_revalidates_real_age_key_and_entire_ciphertext_before_retention(self):
        age=os.environ.get('AGE_BINARY') or shutil.which('age')
        keygen=os.environ.get('AGE_KEYGEN_BINARY') or shutil.which('age-keygen')
        self.assertTrue(age and keygen,'provision pinned age/age-keygen on PATH or their explicit binary variables')
        self.assertEqual(subprocess.check_output([age,'--version']).strip(),b'v1.2.1')
        self.assertEqual(subprocess.check_output([keygen,'--version']).strip(),b'v1.2.1')
        identity=subprocess.check_output([keygen],stderr=subprocess.DEVNULL)
        other_identity=subprocess.check_output([keygen],stderr=subprocess.DEVNULL)
        payload=b'synthetic recovery stream; PostgreSQL archive validation belongs to native qualification\n'
        for fault in ['intact','missing_key','wrong_key','truncated_archive','corrupt_archive']:
            with self.subTest(fault=fault),tempfile.TemporaryDirectory() as directory:
                fake,archive=completion_fixture(Path(directory))
                fake.key.write_bytes(identity)
                recipient=subprocess.check_output([keygen,'-y',str(fake.key)]).strip().decode()
                ciphertext=subprocess.run([age,'-r',recipient],input=payload,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,check=True).stdout
                if fault=='truncated_archive':ciphertext=ciphertext[:-8]
                elif fault=='corrupt_archive':ciphertext=ciphertext[:40]+b'corrupt'+ciphertext[47:]
                archive.write_bytes(ciphertext)
                refresh_archive_binding(fake,archive)
                if fault=='missing_key':fake.key.unlink()
                elif fault=='wrong_key':fake.key.write_bytes(other_identity)
                fake.exits=0
                fake.age=lambda *arguments:[age,*[str(fake.key) if value=='/run/recovery/identity.agekey' else value for value in arguments[1:]]]
                def stream(producer,consumer,**kwargs):
                    self.assertEqual(consumer[-2:],['pg_restore','--file=/dev/null'])
                    self.assertEqual(kwargs['stage'],'retained_validation')
                    self.assertIn('none',consumer)
                    # Real age and child exit accounting, but no fake claim of
                    # PostgreSQL restore: that requires the separate native run.
                    validator=[sys.executable,'-c','import sys;assert sys.stdin.buffer.read()=='+repr(payload)]
                    return m.Controller.stream(fake,producer,validator,**kwargs)
                fake.stream=stream
                fake.fingerprint=lambda *a,**k:self.fail('post-cutover source hashes intentionally differ')
                before=fake.state.read_bytes()
                if fault=='intact':
                    fake.complete();self.assertEqual(m.read_json(fake.state)['state'],'completed')
                else:
                    with self.assertRaises(m.Refusal):fake.complete()
                    self.assertEqual(fake.state.read_bytes(),before)

    def test_real_verified_age_roundtrip_refuses_wrong_key_and_truncated_ciphertext(self):
        age_path=os.environ.get('AGE_BINARY') or shutil.which('age')
        self.assertTrue(age_path,'set AGE_BINARY or provision pinned age on PATH')
        age=Path(age_path)
        keygen_path=os.environ.get('AGE_KEYGEN_BINARY') or shutil.which('age-keygen')
        self.assertTrue(keygen_path,'set AGE_KEYGEN_BINARY or provision pinned age-keygen on PATH')
        keygen=Path(keygen_path)
        self.assertTrue(age.is_file() and keygen.is_file(),'approved local age fixture tools must be provisioned')
        self.assertEqual(subprocess.check_output([str(age),'--version']).strip(),b'v1.2.1')
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory);key=root/'identity';other=root/'other'
            for path in [key,other]:
                path.write_bytes(subprocess.check_output([str(keygen)],stderr=subprocess.DEVNULL));path.chmod(0o600)
            recipient=subprocess.check_output([str(keygen),'-y',str(key)]).strip().decode()
            payload=b'synthetic custom archive boundary\x00--actual row\n'
            encrypted=subprocess.run([str(age),'-r',recipient],input=payload,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,check=True).stdout
            decrypted=subprocess.run([str(age),'-d','-i',str(key)],input=encrypted,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,check=True).stdout
            self.assertEqual(decrypted,payload)
            for path,raw in [(other,encrypted),(key,encrypted[:-8]),(key,encrypted[:40])]:
                self.assertNotEqual(subprocess.run([str(age),'-d','-i',str(path)],input=raw,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode,0)
            key.unlink()
            self.assertNotEqual(subprocess.run([str(age),'-d','-i',str(key)],input=encrypted,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL).returncode,0)


if __name__=='__main__':unittest.main()
