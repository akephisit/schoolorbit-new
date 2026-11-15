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


def database_profile(owner,database,pair=('en_US.utf8','en_US.utf8')):
    return {'metadata':{'datname':database,'encoding':6,'datlocprovider':'b','datlocale':'C.UTF-8','datcollate':pair[0],'datctype':pair[1]},'owner':owner,'tablespace':'pg_default','acl':None,'settings':None}


class Fake(m.Controller):
    def __init__(self,point,versions):
        self.point=point;self.root=point;self.release='a'*40;self.image_digest='sha256:'+'b'*64
        self.rows=m.inventory({'schools':[row(ID),row(OTHER)],'total':2});self.inventory_hash=m.inventory_hash(self.rows)
        self.key=point/'identity.agekey';self.proof=point/'proof.json';self.state=point/'state.json'
        self.client='source';self.copy='copy';self.volume='volume';self.verify='verify';self.versions=versions;self.restored=[];self.checked=False;self.recovery_validations=0
        self.compiled=[[157,True,'a'*96]]
        self.source_database=database_profile('so_'+ID.replace('-','')+'_owner','tenant_'+ID.replace('-',''))
    def labels(self):return []
    def exists(self,*args):return False
    def assert_quiesced(self):pass
    def unchanged_inventory(self):pass
    @contextlib.contextmanager
    def source(self,row):
        yield {'id':row['id'],'database':'tenant_'+row['id'].replace('-',''),'user':'so_'+row['id'].replace('-','')+'_owner'},['source'],{}
    def identity(self,connection,*args):return dict(role_profile(connection['id']),date='2026-11-14',version=self.versions[connection['id']])
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
        md={'schemaVersion':2,'policy':m.fidelity.POLICY,'schoolId':row['id'],'archiveSha256':archive_hash,'roleMetadata':profile['roles'],'membershipMetadata':profile['memberships'],'restoredRoleMetadata':copy.deepcopy(profile['roles']),'restoredMembershipMetadata':copy.deepcopy(profile['memberships']),'source':metadata,'restoredBefore':metadata,'restored':metadata,'oracle':[],'physicalIndexBookkeepingDifferenceCount':0}
        nm={'schemaVersion':2,'policy':m.fidelity.POLICY,'schoolId':row['id'],'archiveSha256':archive_hash,'renames':[]}
        md_path=self.point/(name+'.restore-metadata.json');nm_path=self.point/(name+'.nn-identities.json')
        m.durable(md_path,md);m.durable(nm_path,nm)
        return dict(audit(),restoredSourceFingerprint='f'*64,restoredReconciledFingerprint='d'*64,restoreFidelity={'policy':m.fidelity.POLICY,'sourceCanonicalFingerprint':'e'*64,'restoredCanonicalFingerprint':'e'*64,'metadataSha256':m.digest_file(md_path),'nnIdentityMapSha256':m.digest_file(nm_path),'checkCount':0,'partialIndexPredicateCount':0,'nnRenameCount':0,'unclassifiedDifferenceCount':0})
    def check(self):self.checked=True


def completion_fixture(point):
    fake=Fake(point,{ID:157,OTHER:157})
    fake.key.write_bytes(b'synthetic key');fake.key.chmod(0o600)
    archive=point/(ID.replace('-','')+'.dump.age')
    archive.write_bytes(b'synthetic encrypted archive');archive.chmod(0o600)
    restored=fake.restore_and_migrate(fake.rows[0],None,None,archive,None,'c'*64)
    item={'schoolId':ID,'subdomain':'tenant-1','databaseName':'tenant_'+ID.replace('-',''),'cutoverDate':'2026-11-14','selectionFingerprint':'a'*64,'checks':audit()['checks'],'archiveSha256':m.digest_file(archive),'sourceFingerprint':'c'*64,'restoredSourceFingerprint':restored['restoredSourceFingerprint'],'restoredReconciledFingerprint':restored['restoredReconciledFingerprint'],'restoreFidelity':restored['restoreFidelity'],'decryptionVerified':True,'historyVersion':157}
    bundle={'schemaVersion':2,'operation':m.OPERATION,'releaseSha':fake.release,'imageDigest':fake.image_digest,'inventoryFingerprint':fake.inventory_hash,'tenants':[item]}
    item['restoreFidelity']['evidenceSha256']=m.fidelity.evidence_digest(bundle,item)
    m.durable(fake.proof,bundle)
    profile=role_profile()
    m.durable(point/(ID.replace('-','')+'.json'),{'roleMetadata':profile['roles'],'membershipMetadata':profile['memberships']})
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

    def test_role_creation_uses_original_effective_authority_and_reads_actual_copy(self):
        owner='so_'+ID.replace('-','')+'_owner'
        for kind,pair in [(kind,pair) for kind in ['provider3','postgresFixture1'] for pair in [('C','C.UTF-8'),('en_US.utf8','en_US.utf8'),('C.UTF-8','C.UTF-8'),('C','C')]]:
            with self.subTest(kind=kind,pair=pair),tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:155})
                profile=role_profile(kind=kind);profile['roles'][0]['login']=False
                calls=[]
                fake.copy_sql=lambda sql,*a,**k:calls.append(sql) or b''
                fake.copy_role_profile=lambda *a:copy.deepcopy(profile)
                with mock.patch.object(m.subprocess,'run',return_value=subprocess.CompletedProcess([],0)):
                    with fake.isolated({'user':owner,'database':'tenant'},profile,database_profile(owner,'tenant',pair)):
                        pass
                self.assertIn('CREATE ROLE "schoolorbit_provisioner"',calls[0])
                self.assertEqual('SET ROLE schoolorbit_provisioner;' in calls[1],kind=='provider3')
                self.assertIn('RESET ROLE;',calls[1]) if kind=='provider3' else None
                self.assertIn('NOLOGIN',calls[0])
                created=next(sql for sql in calls if sql.startswith('CREATE DATABASE '))
                self.assertIn("LC_COLLATE '"+pair[0]+"' LC_CTYPE '"+pair[1]+"'",created)
                self.assertFalse(any('GRANTED BY' in x or 'pg_auth_members' in x for x in calls))

    def test_invalid_locale_binding_refuses_before_creating_owned_resources(self):
        owner='so_'+ID.replace('-','')+'_owner';connection={'user':owner,'database':'tenant'}
        for field,value in [('datcollate',''),('datctype',"C'; DROP DATABASE tenant;--"),('datcollate','C\\'),('datctype','x'*256),('datlocprovider','c'),('datlocale','C'),('encoding',8),('datname','other')]:
            with self.subTest(field=field),tempfile.TemporaryDirectory() as directory:
                fake=Fake(Path(directory),{ID:155});calls=[]
                fake.command=lambda *a,**k:calls.append(a) or b''
                source=database_profile(owner,'tenant');source['metadata'][field]=value
                with self.assertRaises(m.fidelity.FidelityError):
                    with fake.isolated(connection,role_profile(),source):pass
                self.assertEqual(calls,[])

    def test_final_copy_profile_readback_blocks_cli_after_valid_initial_creation(self):
        owner='so_'+ID.replace('-','')+'_owner';connection={'user':owner}
        with tempfile.TemporaryDirectory() as directory:
            fake=Fake(Path(directory),{ID:155});identity=role_profile()
            fake.worker='worker';fake.image='immutable'
            current=copy.deepcopy(identity);fake.copy_role_profile=lambda *a:copy.deepcopy(current)
            self.assertEqual(fake.verify_copy_profile(connection,identity),identity)
            calls=[];fake.command=lambda *a,**k:calls.append(a) or b'child result'
            raw,profile=fake.migrate_copy(connection,identity,{})
            self.assertEqual(raw,b'child result');self.assertEqual(profile,identity);self.assertEqual(len(calls),1)
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
                with fake.isolated({'database':'tenant','user':owner},role_profile(),database_profile(owner,'tenant')):
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
