"""Local confidentiality, transport and crash-boundary fixtures; no production access."""
import contextlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]

def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / file)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module

load('owned_cleanup_proof', 'scripts/lib/owned-load-cleanup-proof.py')
controller = load('owned_cleanup_controller', 'scripts/lib/pipeline-remote/owned-load-cleanup.py')
CAT = json.loads((ROOT / 'scripts/lib/owned-load-cleanup/catalogue.json').read_text())


class ControllerTests(unittest.TestCase):
    def job(self, directory):
        job = controller.OwnerJob.__new__(controller.OwnerJob)
        job.cat = CAT
        job.root = Path(directory)
        (job.root / 'staging').mkdir(mode=0o700)
        (job.root / 'operations').mkdir(mode=0o700)
        job.stage = job.root / 'staging' / '123-1'
        job.inputs = job.root / 'inputs'
        job.active = job.root / 'active'
        job.journal = job.root / 'operations' / CAT['operationId']
        job.stage.mkdir(mode=0o700)
        job.inputs.mkdir(mode=0o700)
        job.common = {'schemaVersion': 1, 'operationId': CAT['operationId'], 'inputSha256': CAT['inputSha256']}
        job.floor = {}
        job.catalogue_hash = 'a' * 64
        job.env = {'CLEANUP_CONTROLLER_SHA256': 'b' * 64, 'CLEANUP_PROOF_SHA256': 'c' * 64}
        job.run = '123-1'
        job.identity = {'pid': 123, 'startTicks': 1, 'bootId': 'd' * 36}
        job.receipt_common = {**job.common, 'actualBindingSha256': 'e' * 64}
        job.connection = {'host': 'db.example.invalid', 'port': 5432, 'user': 'owner', 'database': 'fixture', 'password': 'sentinel-secret'}
        return job

    def test_nanosecond_cutoff_is_exact(self):
        cutoff = controller.timestamp_ns(CAT['authCutoff'])
        self.assertEqual(controller.timestamp_ns('2026-10-25T09:56:38.687447863Z'), cutoff + 1)
        self.assertEqual(controller.timestamp_ns('2026-10-25T09:56:38.687447861Z'), cutoff - 1)

    def test_actual_offset_timestamp_preserves_nanoseconds(self):
        actual = '2026-10-25T16:56:38.687447862+07:00'
        self.assertEqual(controller.timestamp_ns(actual), controller.timestamp_ns(CAT['authCutoff']))
        self.assertEqual(controller.utc_timestamp(actual), CAT['authCutoff'])

    def test_actual_podman_registry_digest_and_offset_shape(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory)
            job.root = Path(directory) / 'deployment' / 'owned-load-cleanup'
            job.root.mkdir(parents=True)
            (job.root.parent / 'accepted-release').write_text('7' * 40)
            job.env.update(CLEANUP_ACCEPTED_SHA='7' * 40, CLEANUP_BACKEND_INPUT_HASH='d' * 64)
            config = 'e8e44ca6e32b5c1e698116651a82ceca9cde9fe79da8d1941c8c6f01cbb1b3bb'
            actual = {'Image': config, 'ImageDigest': CAT['image'], 'ImageName': 'ghcr.io/akephisit/schoolorbit-backend-school@' + CAT['image'], 'Id': 'a' * 64,
                'State': {'Running': True, 'Pid': 123, 'StartedAt': '2026-10-25T16:56:38.687447862+07:00'}, 'RestartCount': 0, 'Config': {'Env': []}}
            peer = {'Names': ['schoolorbit-backend-school'], 'ImageID': config}
            with patch.object(job, 'legacy_clear'), patch.object(job, 'public_ready'), patch.object(job, 'command', side_effect=[json.dumps([actual]).encode(), json.dumps([peer]).encode()]), patch.object(controller, 'process_identity', return_value=job.identity):
                binding = job.actual_binding()
            self.assertEqual(binding['backend']['image'], CAT['image'])
            self.assertEqual(binding['backend']['configId'], config)
            self.assertEqual(binding['backend']['startedAt'], CAT['authCutoff'])

    def test_redirects_never_forward_internal_credentials(self):
        with self.assertRaises(controller.ControllerRefusal) as error:
            controller.NoRedirect().redirect_request(None, None, 302, '', {}, 'https://outside.invalid')
        self.assertEqual(error.exception.code, 'admin_redirect_refused')

    def test_confirmed_absent_pid_is_distinct_from_unknown(self):
        with patch.object(Path, 'read_text', side_effect=FileNotFoundError):
            self.assertIsNone(controller.process_identity(123))
        for failure in [PermissionError(), OSError('EIO')]:
            with patch.object(Path, 'read_text', side_effect=failure):
                with self.assertRaises(controller.ControllerRefusal):
                    controller.process_identity(123)
        with patch.object(Path, 'read_text', return_value='malformed'):
            with self.assertRaises(controller.ControllerRefusal):
                controller.process_identity(123)

    def test_live_controller_is_not_dead(self):
        identity = {'pid': 123, 'startTicks': 1, 'bootId': '00000000-0000-4000-8000-000000000001'}
        with patch.object(controller, 'process_identity', return_value=identity):
            self.assertFalse(controller.controller_dead(identity))
        with patch.object(controller, 'process_identity', return_value={**identity, 'startTicks': 2}):
            self.assertTrue(controller.controller_dead(identity))

    def test_private_inputs_reject_symlinks(self):
        with tempfile.TemporaryDirectory() as directory:
            real = Path(directory) / 'real'
            real.write_text('{}'); real.chmod(0o600)
            link = Path(directory) / 'link'; link.symlink_to(real)
            with self.assertRaises(controller.ControllerRefusal):
                controller.private_bytes(link)

    def test_dispatch_exit_is_durable_before_output_parser(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory); job.journal.mkdir(mode=0o700)
            def process(args, **kwargs):
                self.assertNotIn('sentinel-secret', str(args))
                self.assertNotIn('sentinel-secret', str(kwargs['env']))
                return subprocess.CompletedProcess(args, 0, b'bad output', b'')
            with patch.object(controller.subprocess, 'run', side_effect=process):
                raw = job.psql(job.stage, 'cleanup-commit.sql', dispatch=True)
            exited = controller.json_record(job.journal / 'postgres-exit.json')
            self.assertEqual(exited['exitCode'], 0)
            with self.assertRaises(controller.ControllerRefusal):
                controller.extract_object(raw, 'committedCandidate')
            self.assertFalse((job.journal / 'commit-receipt.json').exists())

    def test_nonzero_exit_cannot_become_a_commit_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory); job.journal.mkdir(mode=0o700)
            with patch.object(controller.subprocess, 'run', return_value=subprocess.CompletedProcess([], 3, b'{"committedCandidate":true}', b'provider-private-error')):
                with self.assertRaises(controller.ControllerRefusal):
                    job.psql(job.stage, 'cleanup-commit.sql', dispatch=True)
            self.assertEqual(controller.json_record(job.journal / 'postgres-exit.json')['exitCode'], 3)
            self.assertFalse((job.journal / 'commit-receipt.json').exists())

    def test_timeout_preserves_unknown_without_replay(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory); job.journal.mkdir(mode=0o700)
            with patch.object(controller.subprocess, 'run', side_effect=subprocess.TimeoutExpired('podman', 120)):
                with self.assertRaises(controller.ControllerRefusal):
                    job.psql(job.stage, 'cleanup-commit.sql', dispatch=True)
            self.assertEqual(controller.json_record(job.journal / 'postgres-exit.json')['status'], 'unknown')

    def test_owner_credential_uses_canonical_active_provider(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory)
            owner = 'so_' + CAT['tenantId'].replace('-', '') + '_owner'
            metadata = {'id': CAT['tenantId'], 'subdomain': CAT['subdomain'], 'status': 'active', 'migration_version': 138, 'migration_status': 'migrated'}
            url = f'postgresql://{owner}:sentinel-secret@db.example.invalid:5432/{CAT["databaseName"]}?sslmode=verify-full&sslrootcert=%2Frun%2Fsecrets%2Fpostgresql-ca.crt'
            with patch.object(job, 'admin_get', side_effect=[metadata, {'db_migration_connection_string': url}]) as api:
                job.owner_connection()
            self.assertEqual(api.call_args_list[1].args[0], '/internal/schools/snwsb/migration-connection')
            self.assertEqual(job.connection['user'], owner)
            with patch.object(job, 'admin_get', side_effect=[metadata, {'db_migration_connection_string': url.replace('verify-full', 'require')}]):
                with self.assertRaises(controller.ControllerRefusal):
                    job.owner_connection()

    def test_prepare_cannot_dispatch_business_dml(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory)
            binding = {'backend': {'image': CAT['image']}}
            snapshot = {'outside': []}
            @contextlib.contextmanager
            def work():
                yield job.stage
            with patch.object(job, 'legacy_clear'), patch.object(job, 'actual_binding', return_value=binding), patch.object(job, 'same_binding'), patch.object(job, 'owner_connection'), patch.object(job, 'work', work), patch.object(job, 'database_identity'), patch.object(job, 'psql', return_value=json.dumps(snapshot).encode()) as psql, patch.object(controller, 'validate_snapshot'):
                job.prepare()
            self.assertEqual(psql.call_args.args[1], 'preflight.sql')
            self.assertEqual(psql.call_count, 1)
            self.assertFalse(job.journal.exists())
            self.assertEqual(controller.json_record(job.active)['status'], 'prepared')

    def test_existing_journal_prevents_execute_replay(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory); job.journal.mkdir(mode=0o700)
            with self.assertRaises(controller.ControllerRefusal) as error:
                job.commit()
            self.assertEqual(error.exception.code, 'cleanup_already_dispatched')

    def recovery_fixture(self, directory, *, committed=False):
        job = self.job(directory); job.journal.mkdir(mode=0o700)
        fixtures = load('owned_cleanup_fixtures', 'scripts/tests/owned-load-cleanup-proof.test.py')
        approved, _, _ = fixtures.fixtures(); observed = fixtures.cleaned(approved)
        binding = {'backend': {'image': CAT['image']}}
        controller.durable(job.journal / 'actual-binding.json', binding)
        controller.durable(job.journal / 'approved-snapshot.json', approved)
        common = {**job.common, 'actualBindingSha256': controller.sha(controller.canonical(binding)), 'approvedSnapshotSha256': controller.sha(controller.canonical(approved)), 'sqlSha256': CAT['files']['cleanup-commit.sql']}
        controller.durable(job.journal / 'dispatch-intent.json', {**common, 'status': 'dispatching'})
        exit_hash = controller.durable(job.journal / 'postgres-exit.json', {**common, 'status': 'exited', 'exitCode': 0})
        if committed:
            controller.durable(job.journal / 'commit-receipt.json', {**common, 'status': 'committed', 'postgresExitSha256': exit_hash, 'postCommitVerified': True})
        controller.durable(job.active, {**job.common, 'status': 'reserved', 'run': job.run, 'controller': job.identity, 'catalogueSha256': job.catalogue_hash, 'controllerSha256': job.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': job.env['CLEANUP_PROOF_SHA256']})
        job.receipt_common = common
        return job, approved, observed

    def test_exit_zero_recovery_derives_receipt_only_after_fresh_readback(self):
        with tempfile.TemporaryDirectory() as directory:
            job, approved, observed = self.recovery_fixture(directory)
            @contextlib.contextmanager
            def work(_): yield job.stage
            with patch.object(controller, 'controller_dead', return_value=True), patch.object(job, 'client_absent'), patch.object(job, 'same_binding'), patch.object(job, 'owner_connection'), patch.object(job, 'work', work), patch.object(job, 'database_identity'), patch.object(job, 'psql', return_value=json.dumps(observed).encode()) as psql, patch.object(job, 'watch') as watch:
                job.reconcile()
            self.assertEqual(psql.call_args.args[1], 'reconcile.sql')
            self.assertEqual(psql.call_count, 1)
            self.assertTrue(controller.json_record(job.journal / 'commit-receipt.json')['derivedFromExitAndReadback'])
            watch.assert_called_once()

    def test_resumed_counter_floor_cannot_be_reset_to_original(self):
        with tempfile.TemporaryDirectory() as directory:
            job, approved, observed = self.recovery_fixture(directory, committed=True)
            floor = json.loads(json.dumps(approved)); floor['watchCounters'][0]['row_revision'] += 5
            controller.durable(job.journal / 'observation-1.json', {**job.receipt_common, 'watchCounters': floor['watchCounters'], 'behaviorRevision': floor['behaviorRevision']})
            observed['watchCounters'][0]['row_revision'] += 3
            with self.assertRaises(controller.CleanupRefusal):
                job.observe_floor(job.observation_floor(approved), approved, observed)

    def test_completed_watch_can_finalize_after_controller_loss_without_dml(self):
        with tempfile.TemporaryDirectory() as directory:
            job, approved, observed = self.recovery_fixture(directory, committed=True)
            controller.durable(job.journal / 'watch-receipt.json', {**job.receipt_common, 'status': 'preserved', 'watchSeconds': 300, 'samples': 2, 'stableBackend': True, 'preserved': True, 'watchCounters': observed['watchCounters'], 'behaviorRevision': observed['behaviorRevision']})
            with patch.object(job, 'same_binding'), patch.object(job, 'client_absent'):
                job.finalize(observed)
            self.assertFalse(job.active.exists())
            self.assertEqual(controller.json_record(job.journal / 'completed.json')['status'], 'completed')

    def test_unknown_controller_identity_does_not_abort_reserved_work(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory)
            controller.durable(job.active, {**job.common, 'status': 'prepared', 'run': job.run, 'controller': job.identity, 'catalogueSha256': job.catalogue_hash, 'controllerSha256': job.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': job.env['CLEANUP_PROOF_SHA256']})
            with patch.object(controller, 'controller_dead', side_effect=controller.ControllerRefusal('process_identity_unavailable')):
                with self.assertRaises(controller.ControllerRefusal): job.reconcile()
            self.assertTrue(job.active.exists())

    def test_proven_no_dispatch_abort_is_distinct_from_success(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory)
            controller.durable(job.active, {**job.common, 'status': 'prepared', 'run': job.run, 'controller': job.identity, 'catalogueSha256': job.catalogue_hash, 'controllerSha256': job.env['CLEANUP_CONTROLLER_SHA256'], 'proofSha256': job.env['CLEANUP_PROOF_SHA256']})
            with patch.object(controller, 'controller_dead', return_value=True), patch.object(job, 'client_absent'):
                job.reconcile()
            self.assertFalse(job.active.exists())
            receipt = controller.json_record(job.root / 'aborted' / '123-1.json')
            self.assertEqual(receipt['status'], 'aborted-before-dispatch')
            self.assertNotIn('exitCode', receipt)
            self.assertFalse((job.journal / 'commit-receipt.json').exists())

    def test_watch_requires_full_300_seconds_and_persists_counter_floors(self):
        with tempfile.TemporaryDirectory() as directory:
            job, approved, observed = self.recovery_fixture(directory, committed=True)
            with patch.object(job, 'same_binding'), patch.object(job, 'client_absent'), patch.object(job, 'psql', return_value=json.dumps(observed).encode()), patch.object(controller.time, 'monotonic', side_effect=[10, 10, 309.9, 310]), patch.object(controller.time, 'monotonic_ns', side_effect=[1, 2, 3]), patch.object(controller.time, 'sleep') as sleep:
                job.watch(job.stage, approved, {})
            receipt = controller.json_record(job.journal / 'watch-receipt.json')
            self.assertEqual(receipt['watchSeconds'], 300)
            self.assertEqual(receipt['samples'], 3)
            self.assertEqual(receipt['watchCounters'], observed['watchCounters'])
            self.assertEqual(len(list(job.journal.glob('observation-*.json'))), 3)
            self.assertEqual(sleep.call_count, 2)
            self.assertFalse(job.active.exists())

    def test_serving_change_during_watch_retains_interlock(self):
        with tempfile.TemporaryDirectory() as directory:
            job, approved, observed = self.recovery_fixture(directory, committed=True)
            with patch.object(job, 'same_binding', side_effect=[None, controller.ControllerRefusal('serving_binding_changed')]), patch.object(job, 'psql', return_value=json.dumps(observed).encode()):
                with self.assertRaises(controller.ControllerRefusal):
                    job.watch(job.stage, approved, {})
            self.assertTrue(job.active.exists())
            self.assertFalse((job.journal / 'watch-receipt.json').exists())

    def test_admin_proxy_environment_is_disabled(self):
        with tempfile.TemporaryDirectory() as directory:
            job = self.job(directory); job.backend_env = ['INTERNAL_API_SECRET=sentinel-secret']
            response = unittest.mock.MagicMock()
            response.__enter__.return_value.read.return_value = b'{}'
            opener = unittest.mock.MagicMock(); opener.open.return_value = response
            with patch.object(controller.urllib.request, 'build_opener', return_value=opener) as factory:
                job.admin_get('/internal/schools/snwsb')
            self.assertEqual(factory.call_args.args[0].proxies, {})
            self.assertIsInstance(factory.call_args.args[1], controller.NoRedirect)


if __name__ == '__main__':
    unittest.main()
