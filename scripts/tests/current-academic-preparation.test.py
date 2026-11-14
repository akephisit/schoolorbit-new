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


class Fake(m.Controller):
    def __init__(self,point,versions):
        self.point=point;self.root=point;self.release='a'*40;self.image_digest='sha256:'+'b'*64
        self.rows=m.inventory({'schools':[row(ID),row(OTHER)],'total':2});self.inventory_hash=m.inventory_hash(self.rows)
        self.key=point/'identity.agekey';self.proof=point/'proof.json';self.state=point/'state.json'
        self.client='source';self.copy='copy';self.volume='volume';self.versions=versions;self.restored=[];self.checked=False
    def exists(self,*args):return False
    def assert_quiesced(self):pass
    def unchanged_inventory(self):pass
    @contextlib.contextmanager
    def source(self,row):
        yield {'id':row['id'],'database':'tenant_'+row['id'].replace('-',''),'user':'so_'+row['id'].replace('-','')+'_owner'},['source'],{}
    def identity(self,connection,*args):return {'date':'2026-11-14','version':self.versions[connection['id']],'roles':[]}
    def age(self,*arguments):return ["synthetic-age",*arguments]
    def ensure_key(self):
        self.key.write_text('synthetic key');self.key.chmod(0o600);return 'synthetic recipient'
    def fingerprint(self,*args):return 'c'*64
    def command(self,*args,**kwargs):return b'1'
    def stream(self,*args,**kwargs):kwargs['destination'].write(b'synthetic encrypted archive')
    def restore_and_migrate(self,row,*args):self.restored.append(row['id']);return audit()
    def check(self):self.checked=True


class PreparationTests(unittest.TestCase):
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
            fake=Fake(Path(directory),{ID:157,OTHER:157})
            item={'schoolId':ID,'subdomain':'tenant-1','cutoverDate':'2026-11-14','selectionFingerprint':'a'*64,'checks':audit()['checks']}
            m.durable(fake.proof,{'releaseSha':fake.release,'imageDigest':fake.image_digest,'inventoryFingerprint':fake.inventory_hash,'tenants':[item]})
            future=(dt.datetime.now(dt.timezone.utc)+dt.timedelta(days=20)).isoformat()
            m.durable(fake.state,{'state':'prepared','retainUntil':future})
            fake.command=lambda *a,**k:json.dumps(audit()).encode()
            fake.complete();self.assertGreaterEqual(dt.datetime.fromisoformat(m.read_json(fake.state)['retainUntil']),dt.datetime.fromisoformat(future))
            bad=audit();bad['selectionFingerprint']='d'*64;fake.command=lambda *a,**k:json.dumps(bad).encode()
            before=fake.state.read_bytes()
            with self.assertRaises(m.Refusal):fake.complete()
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
