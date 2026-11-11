#!/usr/bin/env python3
"""Validate and reconcile a same-path Cargo source/target snapshot; never edit fingerprints."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tarfile
import tempfile
import unicodedata
import re
import time
import tomllib

METADATA = '.cargo-snapshot.json'
SAFETY_NS = 2_000_000_000
SOURCE_ROOTS = {'.deployment-trigger', '.dockerignore', '.env.example', 'Cargo.lock', 'Cargo.toml',
                'Dockerfile', 'build.rs', 'crates', 'migrations', 'migrations_legacy', 'scripts', 'src'}
TARGET_ROOTS = {'release', 'cargo-timings', 'dependency-timings', '.rustc_info.json', 'CACHEDIR.TAG'}


class SnapshotRefused(ValueError):
    pass


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':')).encode()


def hash_file(path):
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def tree(root):
    if root.is_symlink() or not root.is_dir():
        raise SnapshotRefused('root_type')
    entries = {}
    # os.walk never follows links; reject them rather than resolving outside the root.
    for parent, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(parent) / name
            info = path.lstat()
            parts = path.relative_to(root).parts
            if any(part in {'.git', '.ssh', '.env'} or part.endswith('.log') for part in parts):
                raise SnapshotRefused('private_path')
            if not (stat.S_ISDIR(info.st_mode) or stat.S_ISREG(info.st_mode)):
                raise SnapshotRefused('unsupported_file_type')
            entries[path.relative_to(root).as_posix()] = (path, info)
    return entries


def source_manifest(root, entries=None):
    entries = tree(root) if entries is None else entries
    result = []
    for relative, (path, info) in sorted(entries.items()):
        first = relative.split('/', 1)[0]
        if first in {'target', METADATA, 'recipe.json'}:
            continue
        if first not in SOURCE_ROOTS:
            raise SnapshotRefused('source_allowlist')
        if stat.S_ISREG(info.st_mode):
            if info.st_nlink != 1:
                raise SnapshotRefused('source_hardlink')
            result.append({'path': relative, 'sha256': hash_file(path), 'mode': stat.S_IMODE(info.st_mode)})
    if not any(record['path'] == 'Cargo.lock' for record in result):
        raise SnapshotRefused('missing_source_identity')
    return result


def validate_target(entries):
    count = 0
    for relative, (_, info) in entries.items():
        parts = relative.split('/')
        if parts[0] == 'target' and len(parts) > 1:
            if parts[1] not in TARGET_ROOTS:
                raise SnapshotRefused('target_allowlist')
            if stat.S_ISREG(info.st_mode):
                count += 1
    if count == 0:
        raise SnapshotRefused('empty_target')


def identity(root, cache_mode='gha'):
    lock = tomllib.loads((root / 'Cargo.lock').read_text())
    external = sorted((p['name'], p['version'], p['source'], p.get('checksum'))
                      for p in lock['package'] if p.get('source'))
    profiles = {}
    for path in sorted(root.rglob('Cargo.toml')):
        if 'target' in path.relative_to(root).parts:
            continue
        profile = tomllib.loads(path.read_text()).get('profile')
        if profile:
            profiles[path.relative_to(root).as_posix()] = profile
    files = ['Dockerfile', 'scripts/build_release.sh', 'scripts/release_linker.sh',
             'scripts/cargo_snapshot.py', 'scripts/sccache-gha-retry.patch', 'scripts/sccache-startup.json']
    root_manifest = tomllib.loads((root / 'Cargo.toml').read_text())
    configuration = {'schemaVersion': 1, 'platform': 'linux/amd64', 'sourcePath': '/app',
                     'productionDefaultFeatures': sorted(root_manifest.get('features', {}).get('default', [])),
                     'resolver': root_manifest.get('workspace', {}).get('resolver', '2'),
                     'compilerCacheMode': cache_mode, 'profiles': profiles,
                     'publicBuildEnvironment': {},
                     'files': {name: hash_file(root / name) for name in files}}
    return {'buildConfigurationHash': hashlib.sha256(canonical(configuration)).hexdigest(),
            'externalLockIdentityHash': hashlib.sha256(canonical(external)).hexdigest()}


def native_identity():
    """Public executable/package/sysroot identity from the current compiler stage."""
    def query(arguments):
        return subprocess.run(arguments, check=True, capture_output=True, text=True,
                              timeout=30).stdout.strip()
    def binary(name):
        path = shutil.which(name)
        if path is None:
            raise RuntimeError('Required compiler tool unavailable: ' + name)
        resolved = Path(path).resolve(strict=True)
        return {'path': str(resolved), 'sha256': hash_file(resolved)}
    rustc_version = query(['rustc', '-vV'])
    cargo_version = query(['cargo', '-Vv'])
    host = next(line[6:] for line in rustc_version.splitlines() if line.startswith('host: '))
    sysroot = Path(query(['rustc', '--print', 'sysroot'])).resolve(strict=True)
    # Bound the closure to compiler runtime libraries, host standard libraries and
    # shipped link tools; never walk arbitrary HOME/configuration/environment.
    libraries = {}
    for directory in (sysroot / 'lib', sysroot / 'lib/rustlib' / host / 'lib',
                      sysroot / 'lib/rustlib' / host / 'bin'):
        if not directory.is_dir():
            raise RuntimeError('Required Rust sysroot directory unavailable')
        for path in sorted(directory.rglob('*') if directory.name == 'bin' else directory.iterdir()):
            if path.is_file():
                libraries[path.relative_to(sysroot).as_posix()] = hash_file(path)
    if not libraries:
        raise RuntimeError('Empty Rust sysroot identity')
    packages = query(['dpkg-query', '-W', '-f=${Package}=${Version}\\n'])
    return {'schemaVersion': 1, 'rustcVersion': rustc_version, 'cargoVersion': cargo_version,
            'sysroot': str(sysroot), 'rustLibraries': libraries,
            'rustExecutables': {name: hash_file(sysroot / 'bin' / name) for name in ('rustc', 'cargo')},
            'tools': {name: binary(name) for name in
                      ('rustc', 'cargo', 'cargo-chef', 'sccache', 'rustc-cache', 'cc', 'gcc', 'ld', 'ar', 'pkg-config', 'openssl')},
            'nativePackages': sorted(packages.splitlines()),
            'opensslVersion': query(['pkg-config', '--modversion', 'openssl']),
            'opensslConfiguration': query(['pkg-config', '--cflags', '--libs', 'openssl'])}


def native_gate(metadata_path, output):
    metadata = read_metadata(metadata_path)
    expected = metadata.get('nativeIdentity')
    if not isinstance(expected, dict) or expected.get('schemaVersion') != 1:
        raise SnapshotRefused('native_identity_missing')
    actual = native_identity()  # Execution/tool build failure is fatal, never a refusal bool.
    result = {'compatible': actual == expected,
              'actualIdentitySha256': hashlib.sha256(canonical(actual)).hexdigest(),
              'expectedIdentitySha256': hashlib.sha256(canonical(expected)).hexdigest()}
    output.write_bytes(canonical(result) + b'\n')
    return result


def saved_metadata(root, entries, sealed_at):
    records = [{'path': '.', 'type': 'directory', 'mode': stat.S_IMODE(root.lstat().st_mode), 'mtimeNs': root.lstat().st_mtime_ns}]
    for relative, (_, info) in sorted(entries.items()):
        if relative == METADATA:
            continue
        records.append({'path': relative, 'type': 'directory' if stat.S_ISDIR(info.st_mode) else 'file',
                        'mode': stat.S_IMODE(info.st_mode), 'mtimeNs': info.st_mtime_ns,
                        'bytes': info.st_size if stat.S_ISREG(info.st_mode) else 0})
    return records


def restore_saved_metadata(root, entries, metadata):
    records = metadata.get('fileMetadata')
    expected = {'.', *(path for path in entries if path != METADATA)}
    if not isinstance(records, list) or len(records) != len(expected):
        raise SnapshotRefused('metadata_inventory')
    seen = set()
    for item in records:
        if not isinstance(item, dict):
            raise SnapshotRefused('metadata_entry')
        relative = item.get('path')
        if relative not in expected or relative in seen:
            raise SnapshotRefused('metadata_path')
        seen.add(relative)
        timestamp = item.get('mtimeNs')
        mode = item.get('mode')
        if type(timestamp) is not int or not 0 <= timestamp <= 2**63 - 1 or type(mode) is not int or not 0 <= mode <= 0o777:
            raise SnapshotRefused('metadata_timestamp')
        path = root if relative == '.' else entries[relative][0]
        info = path.lstat()
        kind = 'directory' if stat.S_ISDIR(info.st_mode) else 'file'
        if item.get('type') != kind or stat.S_IMODE(info.st_mode) != mode:
            raise SnapshotRefused('metadata_type')
        if kind == 'file' and (item.get('bytes') != info.st_size or info.st_nlink != 1):
            raise SnapshotRefused('metadata_file')
    if seen != expected:
        raise SnapshotRefused('metadata_missing')
    # Restore file times, then directories after all children. Never derive trust from extraction time.
    for item in sorted(records, key=lambda item: (item['type'] == 'directory', 1 if item['path'] == '.' else -item['path'].count('/'))):
        path = root if item['path'] == '.' else root / item['path']
        if path.lstat().st_mtime_ns != item['mtimeNs']:
            os.utime(path, ns=(item['mtimeNs'], item['mtimeNs']), follow_symlinks=False)
        if path.lstat().st_mtime_ns != item['mtimeNs']:
            raise SnapshotRefused('metadata_clock_precision')
    sealed_at = metadata.get('sealedAtNs')
    if type(sealed_at) is not int or not 0 <= sealed_at <= 2**63 - 1:
        raise SnapshotRefused('metadata_seal_time')
    if (root / METADATA).exists():
        os.utime(root / METADATA, ns=(sealed_at, sealed_at), follow_symlinks=False)


def restore(incoming, root, configuration_hash, expected_manifest_hash, now_ns=None, metadata=None):
    now_ns = time.time_ns() if now_ns is None else now_ns
    old_entries = tree(root)
    new_entries = tree(incoming)
    validate_target(old_entries)
    metadata_path = root / METADATA
    if metadata is None:
        metadata = read_metadata(metadata_path)
    old_source = source_manifest(root, old_entries)
    if (metadata.get('schemaVersion') != 1 or metadata.get('sourcePath') != '/app'
            or metadata.get('configurationHash') != configuration_hash
            or metadata.get('sourceManifest') != old_source
            or metadata.get('sourceManifestSha256') != expected_manifest_hash
            or hashlib.sha256(canonical(old_source)).hexdigest() != expected_manifest_hash):
        raise SnapshotRefused('snapshot_identity')
    restore_saved_metadata(root, old_entries, metadata)
    old_entries = tree(root)
    maximum = max([root.lstat().st_mtime_ns, *(info.st_mtime_ns for _, info in old_entries.values())])
    if maximum < 0 or maximum >= now_ns - SAFETY_NS:
        raise SnapshotRefused('snapshot_clock')
    new_source = source_manifest(incoming, new_entries)
    old_files = {record['path']: record for record in old_source}
    new_files = {record['path']: record for record in new_source}
    changed_directories = set()
    def mark_ancestors(path):
        while True:
            if path.exists():
                changed_directories.add(path)
            if path == root:
                break
            path = path.parent
    if stat.S_IMODE(root.lstat().st_mode) != stat.S_IMODE(incoming.lstat().st_mode):
        os.chmod(root, stat.S_IMODE(incoming.lstat().st_mode))
        changed_directories.add(root)
    # First remove obsolete paths and incompatible directory/file types, deepest first.
    # Protect only target and owned metadata; no cached source is an authority over incoming bytes.
    for relative, (path, info) in sorted(old_entries.items(), key=lambda item: (-item[0].count('/'), item[0]), reverse=False):
        first = relative.split('/', 1)[0]
        if first in {'target', METADATA}:
            continue
        incoming_entry = new_entries.get(relative)
        incompatible = incoming_entry is not None and stat.S_ISDIR(info.st_mode) != stat.S_ISDIR(incoming_entry[1].st_mode)
        if incoming_entry is None or incompatible:
            mark_ancestors(path.parent)
            if stat.S_ISDIR(info.st_mode):
                path.rmdir()
            else:
                path.unlink()
    changed = []
    for relative, (source, info) in sorted(new_entries.items(), key=lambda item: (item[0].count('/'), item[0])):
        if relative.split('/', 1)[0] not in SOURCE_ROOTS:
            raise SnapshotRefused('incoming_allowlist')
        destination = root / relative
        if stat.S_ISDIR(info.st_mode):
            directory_changed = relative not in old_entries or stat.S_IMODE(old_entries[relative][1].st_mode) != stat.S_IMODE(info.st_mode)
            destination.mkdir(exist_ok=True)
            if directory_changed:
                mark_ancestors(destination)
            os.chmod(destination, stat.S_IMODE(info.st_mode))
            continue
        if old_files.get(relative) == new_files[relative] and destination.exists():
            continue
        temporary = destination.with_name(destination.name + '.cargo-copy')
        with source.open('rb') as src, temporary.open('xb') as dst:
            shutil.copyfileobj(src, dst)
        os.chmod(temporary, stat.S_IMODE(info.st_mode))
        os.utime(temporary, ns=(now_ns, now_ns), follow_symlinks=False)
        os.replace(temporary, destination)
        if destination.lstat().st_mtime_ns <= maximum:
            raise SnapshotRefused('written_clock')
        changed.append(relative)
    # Directory mtimes are watched by build.rs. Explicitly fence changed/deleted children too.
    for relative in set(old_files) ^ set(new_files) | set(changed):
        mark_ancestors((root / relative).parent)
    changed_directories = {path for path in changed_directories if path.is_dir() and not path.is_symlink()}
    for directory in changed_directories:
        os.utime(directory, ns=(now_ns, now_ns), follow_symlinks=False)
        if directory.lstat().st_mtime_ns <= maximum:
            raise SnapshotRefused('directory_clock')
    if source_manifest(root) != new_source:
        raise SnapshotRefused('source_reconciliation')
    metadata_path.unlink(missing_ok=True)
    return {'schemaVersion': 1, 'sourceFiles': len(new_source), 'changedFiles': len(changed),
            'removedFiles': len(set(old_files) - set(new_files)), 'snapshotMaximumNs': maximum,
            'preflightNowNs': now_ns, 'changedPaths': changed,
            'removedPaths': sorted(set(old_files) - set(new_files)),
            'changedDirectories': sorted(p.relative_to(root).as_posix() for p in changed_directories)}


def read_metadata(path):
    if path.is_symlink() or path.stat().st_size > 4 * 1024 * 1024:
        raise SnapshotRefused('metadata_size_type')
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise SnapshotRefused('metadata_duplicate_key')
            result[key] = value
        return result
    return json.loads(path.read_text(), object_pairs_hook=unique)


def metadata_for(root, configuration_hash, native=None):
    entries = {name: value for name, value in tree(root).items() if name not in {METADATA, 'recipe.json'}}
    validate_target(entries)
    source = source_manifest(root, entries)
    now = time.time_ns()
    return {'schemaVersion': 1, 'configurationHash': configuration_hash, 'sourcePath': '/app',
            'sourceManifest': source, 'sourceManifestSha256': hashlib.sha256(canonical(source)).hexdigest(),
            'sealedAtNs': now, 'fileMetadata': saved_metadata(root, entries, now),
            'nativeIdentity': native_identity() if native is None else native}


def relative_path(value):
    if not isinstance(value, str) or not value or value.startswith('/') or any(ord(c) < 32 for c in value) or chr(92) in value:
        raise SnapshotRefused('archive_path')
    normalized = unicodedata.normalize('NFC', value)
    if normalized != value or any(part in {'', '.', '..'} for part in value.split('/')):
        raise SnapshotRefused('archive_path')
    if any(part in {'.git', '.ssh', '.env'} or part.endswith('.log') for part in value.split('/')):
        raise SnapshotRefused('private_path')
    return value


def metadata_inventory(metadata):
    records = metadata.get('fileMetadata')
    if not isinstance(records, list) or not 1 <= len(records) <= 100000:
        raise SnapshotRefused('metadata_inventory')
    result = {}
    total = 0
    for record in records:
        if not isinstance(record, dict):
            raise SnapshotRefused('metadata_entry')
        name = record.get('path')
        if name != '.':
            relative_path(name)
        if name in result or record.get('type') not in {'file', 'directory'}:
            raise SnapshotRefused('metadata_duplicate_type')
        mode = record.get('mode'); timestamp = record.get('mtimeNs'); size = record.get('bytes', 0)
        if type(mode) is not int or not 0 <= mode <= 0o777 or type(timestamp) is not int or not 0 <= timestamp < 2**63:
            raise SnapshotRefused('metadata_mode_time')
        if type(size) is not int or not 0 <= size <= 4 * 1024**3 or (record['type'] == 'directory' and size != 0):
            raise SnapshotRefused('metadata_size')
        total += size
        if name != '.':
            parts = name.split('/')
            if parts[0] == 'target':
                if len(parts) > 1 and parts[1] not in TARGET_ROOTS:
                    raise SnapshotRefused('target_allowlist')
            elif parts[0] not in SOURCE_ROOTS:
                raise SnapshotRefused('source_allowlist')
        result[name] = record
    if result.get('.', {}).get('type') != 'directory' or total > 16 * 1024**3:
        raise SnapshotRefused('metadata_root_size')
    for name in result:
        if name == '.':
            continue
        parts = name.split('/')
        for count in range(1, len(parts)):
            if result.get('/'.join(parts[:count]), {}).get('type') != 'directory':
                raise SnapshotRefused('metadata_parent_conflict')
    return result


def create_archive(root, destination, configuration_hash):
    if destination.exists():
        raise SnapshotRefused('export_destination')
    destination.mkdir(parents=True)
    metadata = metadata_for(root, configuration_hash)
    inventory = metadata_inventory(metadata)
    archive = destination / 'snapshot.tar'
    with tarfile.open(archive, 'w', format=tarfile.PAX_FORMAT) as bundle:
        for relative, record in sorted(inventory.items(), key=lambda item: (item[0] != '.', item[0])):
            name = 'app' if relative == '.' else 'app/' + relative
            info = tarfile.TarInfo(name)
            info.mode = record['mode']; info.uid = 0; info.gid = 0; info.uname = ''; info.gname = ''
            info.mtime = record['mtimeNs'] // 1000000000
            if record['type'] == 'directory':
                info.type = tarfile.DIRTYPE; bundle.addfile(info)
            else:
                # Explicit regular records materialize each Cargo alias independently; no global link dereference.
                info.type = tarfile.REGTYPE; info.size = record['bytes']
                descriptor = os.open(root / relative, os.O_RDONLY | os.O_NOFOLLOW)
                with os.fdopen(descriptor, 'rb') as stream:
                    current = os.fstat(stream.fileno())
                    if not stat.S_ISREG(current.st_mode) or current.st_size != info.size:
                        raise SnapshotRefused('export_race')
                    bundle.addfile(info, stream)
    metadata['archiveBytes'] = archive.stat().st_size
    metadata['archiveSha256'] = hash_file(archive)
    (destination / 'snapshot.json').write_bytes(canonical(metadata) + b'\n')
    return metadata


def validate_archive(archive, metadata):
    if archive.is_symlink() or not stat.S_ISREG(archive.lstat().st_mode) or archive.stat().st_size != metadata.get('archiveBytes'):
        raise SnapshotRefused('archive_size_type')
    if not re.fullmatch('[0-9a-f]{64}', metadata.get('archiveSha256', '')) or hash_file(archive) != metadata['archiveSha256']:
        raise SnapshotRefused('archive_hash')
    expected = metadata_inventory(metadata)
    source_records = {}
    if not isinstance(metadata.get('nativeIdentity'), dict) or metadata['nativeIdentity'].get('schemaVersion') != 1:
        raise SnapshotRefused('native_identity_missing')
    if metadata.get('schemaVersion') != 1 or metadata.get('sourcePath') != '/app' or not isinstance(metadata.get('sourceManifest'), list):
        raise SnapshotRefused('metadata_schema')
    for record in metadata['sourceManifest']:
        if not isinstance(record, dict) or set(record) != {'path', 'sha256', 'mode'}:
            raise SnapshotRefused('source_metadata')
        name = relative_path(record['path'])
        if name in source_records or name.split('/')[0] not in SOURCE_ROOTS or name not in expected:
            raise SnapshotRefused('source_metadata_path')
        if expected[name]['type'] != 'file' or expected[name]['mode'] != record['mode'] or not re.fullmatch('[0-9a-f]{64}', record['sha256']):
            raise SnapshotRefused('source_metadata_type')
        source_records[name] = record
    if 'Cargo.lock' not in source_records or 'Cargo.toml' not in source_records:
        raise SnapshotRefused('source_metadata_identity')
    if hashlib.sha256(canonical(metadata.get('sourceManifest'))).hexdigest() != metadata.get('sourceManifestSha256'):
        raise SnapshotRefused('source_manifest_hash')
    members = []
    seen = set()
    with tarfile.open(archive, 'r:') as bundle:
        for info in bundle:
            name = relative_path(info.name)
            if name == 'app':
                relative = '.'
            elif name.startswith('app/'):
                relative = name[4:]
            else:
                raise SnapshotRefused('archive_root')
            if relative in seen or relative not in expected or len(seen) >= 100000:
                raise SnapshotRefused('archive_duplicate_inventory')
            seen.add(relative)
            record = expected[relative]
            kind = 'directory' if info.type == tarfile.DIRTYPE else 'file' if info.type == tarfile.REGTYPE else 'unsafe'
            if info.uid != 0 or info.gid != 0 or set(info.pax_headers) - {'path', 'mtime'}:
                raise SnapshotRefused('archive_owner_extensions')
            if kind != record['type'] or info.mode & ~0o777 or info.mode != record['mode'] or info.size != record.get('bytes', 0):
                raise SnapshotRefused('archive_type_mode_size')
            if info.linkname or info.mtime != record['mtimeNs'] // 1000000000:
                raise SnapshotRefused('archive_link_time')
            if relative in source_records:
                stream = bundle.extractfile(info)
                digest = hashlib.sha256()
                for chunk in iter(lambda: stream.read(1024 * 1024), b''):
                    digest.update(chunk)
                if digest.hexdigest() != source_records[relative]['sha256']:
                    raise SnapshotRefused('archive_source_hash')
            members.append(info)
    if seen != set(expected):
        raise SnapshotRefused('archive_missing_inventory')
    return members


def preflight(archive, metadata_path, incoming, destination, configuration_hash, expected_source_hash):
    if destination.exists():
        raise SnapshotRefused('staging_destination')
    metadata = read_metadata(metadata_path)
    if not isinstance(metadata, dict):
        raise SnapshotRefused('metadata_schema')
    if metadata.get('configurationHash') != configuration_hash or metadata.get('sourceManifestSha256') != expected_source_hash:
        raise SnapshotRefused('snapshot_identity')
    source_manifest(incoming)  # Reject unsupported incoming paths before materializing any cached state.
    members = validate_archive(archive, metadata)  # Complete header/inventory/hash pass before extraction.
    now = time.time_ns()
    if max(record['mtimeNs'] for record in metadata['fileMetadata']) >= now - SAFETY_NS:
        raise SnapshotRefused('snapshot_clock')
    temporary = Path(tempfile.mkdtemp(prefix='cargo-restore-', dir=destination.parent))
    try:
        with tarfile.open(archive, 'r:') as bundle:
            for info in members:
                path = temporary / info.name
                if info.isdir():
                    path.mkdir(parents=True, exist_ok=True)
                else:
                    with bundle.extractfile(info) as source, path.open('xb') as output:
                        shutil.copyfileobj(source, output, length=1024 * 1024)
                os.chmod(path, info.mode)
        staged = temporary / 'app'
        result = restore(incoming, staged, configuration_hash, expected_source_hash, now_ns=now, metadata=metadata)
        report = staged / 'target/cargo-timings/snapshot-restore.json'
        report.parent.mkdir(parents=True, exist_ok=True)
        report.write_bytes(canonical({'restored': True, **result}) + b'\n')
        sealed = metadata_for(staged, configuration_hash, metadata.get('nativeIdentity'))
        sealed['preflight'] = result
        # Keep metadata outside /app so merely carrying the descriptor cannot dirty watched source directories.
        result_path = destination.parent / 'prevalidated.json'
        result_path.write_bytes(canonical(sealed) + b'\n')
        os.rename(staged, destination)
        return result
    finally:
        shutil.rmtree(temporary)


def materialize(root, metadata_path, configuration_hash):
    metadata = read_metadata(metadata_path)
    entries = tree(root)
    validate_target(entries)
    if metadata.get('configurationHash') != configuration_hash or source_manifest(root, entries) != metadata.get('sourceManifest'):
        raise SnapshotRefused('materialized_source')
    if metadata.get('nativeIdentity') != native_identity():
        raise RuntimeError('Current compiler/native identity changed after preflight')
    proof = metadata.get('preflight', {})
    maximum = proof.get('snapshotMaximumNs'); validated_at = proof.get('preflightNowNs')
    if type(maximum) is not int or type(validated_at) is not int or maximum >= validated_at - SAFETY_NS or time.time_ns() < validated_at:
        raise SnapshotRefused('materialized_fence')
    restore_saved_metadata(root, entries, metadata)
    for relative in [*proof.get('changedPaths', []), *proof.get('changedDirectories', [])]:
        path = root if relative == '.' else root / relative_path(relative)
        if path.lstat().st_mtime_ns <= maximum:
            raise SnapshotRefused('materialized_changed_clock')
    if source_manifest(root) != metadata['sourceManifest']:
        raise SnapshotRefused('materialized_final_source')
    return {'restored': True, **proof}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('operation', choices=['identity', 'archive', 'preflight', 'native-gate', 'materialize'])
    parser.add_argument('root', type=Path)
    parser.add_argument('--incoming', type=Path)
    parser.add_argument('--destination', type=Path)
    parser.add_argument('--archive', type=Path)
    parser.add_argument('--metadata', type=Path)
    parser.add_argument('--configuration-hash', default='')
    parser.add_argument('--source-manifest-hash', default='')
    parser.add_argument('--cache-mode', choices=['gha', 'local', 'off'], default='gha')
    args = parser.parse_args()
    try:
        if args.operation == 'identity':
            result = identity(args.root, args.cache_mode)
        elif args.operation == 'archive':
            result = create_archive(args.root, args.destination, args.configuration_hash)
        elif args.operation == 'native-gate':
            result = native_gate(args.metadata, args.destination)
        elif args.operation == 'preflight':
            result = preflight(args.archive, args.metadata, args.incoming, args.root,
                               args.configuration_hash, args.source_manifest_hash)
        else:
            result = materialize(args.root, args.metadata, args.configuration_hash)
        print(json.dumps(result, sort_keys=True))
    except (OSError, ValueError, KeyError, TypeError, tarfile.TarError) as error:
        reason = str(error) if isinstance(error, SnapshotRefused) else 'invalid_snapshot'
        print('cargo_snapshot_refused=' + reason, file=sys.stderr)
        sys.exit(86)

if __name__ == '__main__':
    main()
