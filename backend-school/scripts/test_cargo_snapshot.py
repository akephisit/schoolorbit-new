#!/usr/bin/env python3
"""Filesystem regressions for the actual archive preflight and post-COPY fence."""
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('cargo_snapshot', Path(__file__).with_name('cargo_snapshot.py'))
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)
NATIVE = {'schemaVersion': 1, 'tool': 'fixed-test-identity'}


class SnapshotFilesystem(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='cargo-snapshot-test-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'source'
        self.source.mkdir()
        for name, value in {'Cargo.lock': 'version = 4\n', 'Cargo.toml': '[package]\nname="fixture"\nversion="0.1.0"\n',
                            'src/main.rs': 'fn main() {}\n', 'migrations/one.sql': 'SELECT 1;\n', 'src/old/nested.rs': '// old\n'}.items():
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(value)
        target = self.source / 'target/release'
        target.mkdir(parents=True)
        (target / 'fixture').write_bytes(b'compiled-bytes')
        os.link(target / 'fixture', target / 'same-inode-alias')
        self.incoming = self.root / 'incoming'
        shutil.copytree(self.source, self.incoming, ignore=shutil.ignore_patterns('target'))
        old = time.time_ns() - 30_000_000_000
        for path in [*self.source.rglob('*'), self.source]:
            os.utime(path, ns=(old, old))
        self.native = patch.object(cache, 'native_identity', return_value=NATIVE)
        self.native.start()
        self.addCleanup(self.native.stop)
        self.bundle = self.root / 'bundle'
        self.metadata = cache.create_archive(self.source, self.bundle, 'configuration')
        self.destination = self.root / 'app'

    def restore(self):
        return cache.preflight(self.bundle / 'snapshot.tar', self.bundle / 'snapshot.json', self.incoming,
                               self.destination, 'configuration', self.metadata['sourceManifestSha256'])

    def save_metadata(self):
        (self.bundle / 'snapshot.json').write_bytes(cache.canonical(self.metadata))

    def test_unchanged_preserves_files_and_materializes_aliases(self):
        result = self.restore()
        self.assertEqual(result['changedFiles'], 0)
        self.assertEqual(result['removedFiles'], 0)
        old = self.source / 'src/main.rs'
        restored = self.destination / 'src/main.rs'
        self.assertEqual(restored.stat().st_mtime_ns, old.stat().st_mtime_ns)
        self.assertEqual((self.destination / 'target/release/fixture').stat().st_nlink, 1)
        self.assertEqual((self.destination / 'target/release/same-inode-alias').stat().st_nlink, 1)
        self.assertTrue(cache.materialize(self.destination, self.root / 'prevalidated.json', 'configuration')['restored'])

    def test_changes_deletion_rename_mode_and_watched_directory_survive_copy(self):
        original = self.incoming / 'src/main.rs'
        original.write_text('fn main() { println!("changed"); }\n')
        os.utime(original, ns=(1, 1))  # Forged old source mtime cannot suppress the change.
        original.chmod(0o755)
        (self.incoming / 'migrations/one.sql').rename(self.incoming / 'migrations/renamed.sql')
        (self.incoming / 'src/empty').mkdir()
        shutil.rmtree(self.incoming / 'src/old')
        result = self.restore()
        self.assertIn('src/main.rs', result['changedPaths'])
        self.assertIn('migrations/one.sql', result['removedPaths'])
        self.assertTrue({'.', 'src', 'migrations', 'src/empty'} <= set(result['changedDirectories']))
        copied = self.root / 'copy'
        shutil.copytree(self.destination, copied, copy_function=shutil.copy)
        cache.materialize(copied, self.root / 'prevalidated.json', 'configuration')
        for name in result['changedPaths'] + result['changedDirectories']:
            path = copied if name == '.' else copied / name
            self.assertGreater(path.stat().st_mtime_ns, result['snapshotMaximumNs'])
        self.assertEqual((copied / 'src/main.rs').stat().st_mode & 0o777, 0o755)

    def test_future_equal_and_coarse_clocks_refuse_before_materialization(self):
        for delta in (60_000_000_000, 0, -1_000_000_000):
            with self.subTest(delta=delta):
                # Keep headers in sync so this exercises the timestamp trust fence.
                future = time.time_ns() + delta
                for path in [*self.source.rglob('*'), self.source]:
                    os.utime(path, ns=(future, future))
                shutil.rmtree(self.bundle)
                self.metadata = cache.create_archive(self.source, self.bundle, 'configuration')
                with self.assertRaisesRegex(cache.SnapshotRefused, 'snapshot_clock'):
                    self.restore()
                self.assertFalse(self.destination.exists())

    def test_coarse_filesystem_timestamp_readback_refuses_without_partial_state(self):
        original_utime = os.utime
        def coarse(path, *, ns, follow_symlinks=True):
            rounded = tuple(value // 1_000_000_000 * 1_000_000_000 for value in ns)
            return original_utime(path, ns=rounded, follow_symlinks=follow_symlinks)
        with patch.object(cache.os, 'utime', side_effect=coarse):
            with self.assertRaisesRegex(cache.SnapshotRefused, 'metadata_clock_precision'):
                self.restore()
        self.assertFalse(self.destination.exists())
        self.assertEqual(list(self.root.glob('cargo-restore-*')), [])

    def test_incoming_symlink_never_followed(self):
        (self.incoming / 'src/main.rs').unlink()
        (self.incoming / 'src/main.rs').symlink_to('/etc/passwd')
        with self.assertRaises(cache.SnapshotRefused):
            self.restore()
        self.assertFalse(self.destination.exists())

    def test_missing_source_declaration_refuses_before_any_extraction(self):
        self.metadata['sourceManifest'] = [record for record in self.metadata['sourceManifest']
                                           if record['path'] != 'src/main.rs']
        self.metadata['sourceManifestSha256'] = hashlib.sha256(cache.canonical(self.metadata['sourceManifest'])).hexdigest()
        self.save_metadata()
        with patch.object(cache.tempfile, 'mkdtemp', side_effect=AssertionError('extraction attempted')):
            with self.assertRaisesRegex(cache.SnapshotRefused, 'source_metadata_complete'):
                self.restore()
        self.assertFalse(self.destination.exists())

    def test_empty_target_inventory_refuses_before_any_extraction(self):
        self.metadata['fileMetadata'] = [record for record in self.metadata['fileMetadata']
                                         if not record['path'].startswith('target/')]
        self.save_metadata()
        with patch.object(cache.tempfile, 'mkdtemp', side_effect=AssertionError('extraction attempted')):
            with self.assertRaisesRegex(cache.SnapshotRefused, 'target_inventory'):
                self.restore()
        self.assertFalse(self.destination.exists())

    def test_archive_header_attacks_refuse_before_extraction(self):
        original = (self.bundle / 'snapshot.tar').read_bytes()
        for attack in ('traversal', 'duplicate', 'hardlink', 'symlink', 'fifo', 'privileged', 'file-directory-alias'):
            with self.subTest(attack=attack):
                archive = self.bundle / 'snapshot.tar'
                with tarfile.open(fileobj=io.BytesIO(original)) as source, tarfile.open(archive, 'w') as output:
                    duplicate = None
                    for item in source:
                        payload = source.extractfile(item) if item.isfile() else None
                        if item.name == 'app/src/main.rs':
                            if attack == 'traversal': item.name = 'app/../escaped'
                            elif attack == 'hardlink': item.type = tarfile.LNKTYPE; item.linkname = '/etc/passwd'
                            elif attack == 'symlink': item.type = tarfile.SYMTYPE; item.linkname = '/etc/passwd'
                            elif attack == 'fifo': item.type = tarfile.FIFOTYPE
                            elif attack == 'privileged': item.mode = 0o4755
                            elif attack == 'file-directory-alias': item.type = tarfile.DIRTYPE
                            elif attack == 'duplicate': duplicate = item
                        output.addfile(item, payload if item.isfile() else None)
                    if duplicate:
                        output.addfile(duplicate, io.BytesIO(b'fn main() {}\n'))
                self.metadata['archiveBytes'] = archive.stat().st_size
                self.metadata['archiveSha256'] = cache.hash_file(archive)
                self.save_metadata()
                with self.assertRaises(cache.SnapshotRefused):
                    self.restore()
                self.assertFalse(self.destination.exists())
                self.assertFalse((self.root / 'escaped').exists())

    def test_native_mismatch_is_bool_and_late_change_is_fatal(self):
        self.restore()
        with patch.object(cache, 'native_identity', return_value={'schemaVersion': 1, 'tool': 'different'}):
            result = cache.native_gate(self.root / 'prevalidated.json', self.root / 'gate.json')
            self.assertIs(result['compatible'], False)
            with self.assertRaisesRegex(RuntimeError, 'identity changed'):
                cache.materialize(self.destination, self.root / 'prevalidated.json', 'configuration')
        with patch.object(cache, 'native_identity', side_effect=RuntimeError('tool failed')):
            with self.assertRaisesRegex(RuntimeError, 'tool failed'):
                cache.native_gate(self.root / 'prevalidated.json', self.root / 'gate.json')

    def test_final_materialization_refusal_discards_state_and_returns_fallback_bool(self):
        self.restore()
        (self.destination / 'src/main.rs').write_text('corrupted after COPY')
        output = self.root / 'materialization.json'
        result = cache.materialize_gate(self.destination, self.root / 'prevalidated.json', 'configuration', output)
        self.assertIs(result['compatible'], False)
        self.assertFalse(self.destination.exists())
        self.assertEqual(json.loads(output.read_text()), result)

    def test_late_native_failure_is_fatal_in_materialization_gate(self):
        self.restore()
        with patch.object(cache, 'native_identity', return_value={'schemaVersion': 1, 'tool': 'different'}):
            with self.assertRaisesRegex(RuntimeError, 'identity changed'):
                cache.materialize_gate(self.destination, self.root / 'prevalidated.json', 'configuration', self.root / 'gate.json')
        self.assertFalse((self.root / 'gate.json').exists())

    def test_authoritative_recipe_selector_executes_refusal_and_success_paths(self):
        native = self.root / 'native.json'
        materialized = self.root / 'materialized.json'
        for outcome in ('failure', 'skipped'):
            self.assertEqual(cache.select_recipe(outcome, native, materialized), 'fresh-builder')
        native.write_text('{"compatible":false}')
        self.assertEqual(cache.select_recipe('success', native, materialized), 'fresh-builder')
        native.write_text('{"compatible":true}')
        materialized.write_text('{"compatible":false}')
        self.assertEqual(cache.select_recipe('success', native, materialized), 'fresh-builder')
        materialized.write_text('{"compatible":true}')
        self.assertEqual(cache.select_recipe('success', native, materialized), 'snapshot-builder')
        for path in (native, materialized):
            path.write_text('{"compatible":"true"}')
            with self.assertRaises(cache.SnapshotRefused):
                cache.select_recipe('success', native, materialized)
            path.write_text('{"compatible":true}')
        native.unlink()
        with self.assertRaises(FileNotFoundError):
            cache.select_recipe('success', native, materialized)
        with self.assertRaises(cache.SnapshotRefused):
            cache.select_recipe('unexpected', native, materialized)

    def test_application_compiler_failure_propagates_through_tee(self):
        tools = self.root / 'tools'
        tools.mkdir()
        cargo = tools / 'cargo'
        cargo.write_text('#!/bin/sh\nexit 23\n')
        cargo.chmod(0o755)
        environment = {**os.environ, 'PATH': str(tools) + ':' + os.environ['PATH'], 'SCHOOL_COMPILER_CACHE': 'off'}
        process = subprocess.run(['bash', str(Path(__file__).with_name('build_release.sh'))], cwd=self.incoming,
                                 env=environment, capture_output=True)
        self.assertEqual(process.returncode, 23)
        self.assertFalse((self.incoming / 'target/cargo-timings/build-metrics.json').exists())


if __name__ == '__main__':
    unittest.main()
