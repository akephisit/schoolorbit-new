#!/usr/bin/env bash
# Read-only guard shared by every production mutation entry point and cleanup.
set -euo pipefail
root="${SCHOOLORBIT_STACK_ROOT:-/opt/stack}/deployment/owned-load-cleanup"
# An orphan SQL client can survive controller loss even when its journal is damaged.
if podman container exists schoolorbit-owned-load-cleanup-3962938a >/dev/null 2>&1; then
    echo 'Production mutation refused: owned cleanup client exists.' >&2
    exit 78
else
    status=$?
    if [[ $status != 1 ]]; then
        echo 'Production mutation refused: cleanup client state is unavailable.' >&2
        exit 78
    fi
fi
python3 - "$root" <<'PY'
import hashlib
import json
import os
import re
from datetime import datetime
from pathlib import Path
import stat
import sys

root = Path(sys.argv[1])
operation = 'cleanup-3962938a-54d0fe6f-73f5-49be-908f-4ec14aee486b'
inputs = '9649a38dd08f91bcd1fbcb899a5fda28e7f0689cb0f06c55f77040b3177b33ae'
image = 'sha256:f983b8e4593e2a7b480b13643c9778e7b79a55c6fa7eb72f6987a8562fd89043'

def refuse():
    raise ValueError('interlock')

def exists(path):
    return os.path.lexists(path)

def directory(path, private=False):
    st = path.lstat()
    if not stat.S_ISDIR(st.st_mode) or not os.access(path, os.R_OK | os.X_OK):
        refuse()
    if private and (stat.S_IMODE(st.st_mode) != 0o700 or st.st_uid != os.getuid()):
        refuse()

def record(path, protocol=True):
    st = path.lstat()
    if not stat.S_ISREG(st.st_mode) or st.st_uid != os.getuid() or stat.S_IMODE(st.st_mode) != 0o600 or st.st_size > 2 * 1024 * 1024:
        refuse()
    raw = path.read_bytes()
    value = json.loads(raw)
    if not isinstance(value, dict) or (protocol and (type(value.get('schemaVersion')) is not int or value['schemaVersion'] != 1 or value.get('operationId') != operation)):
        refuse()
    return value, hashlib.sha256(raw).hexdigest()

def integer(value, minimum):
    return type(value) is int and value >= minimum

def dead_controller(identity):
    if not isinstance(identity, dict) or not integer(identity.get('pid'), 1) or not integer(identity.get('startTicks'), 1) or not re.fullmatch('[0-9a-f-]{36}', identity.get('bootId', '')):
        refuse()
    boot = Path('/proc/sys/kernel/random/boot_id').read_text().strip()
    if boot != identity['bootId']:
        return True
    try:
        raw = Path('/proc/' + str(identity['pid']) + '/stat').read_text()
    except FileNotFoundError:
        return True
    fields = raw.rsplit(')', 1)[1].split()
    return int(fields[19]) != identity['startTicks']


try:
    for ancestor in reversed(root.parents):
        if exists(ancestor):
            directory(ancestor)
    if exists(root):
        directory(root, True)
        if exists(root / 'active'):
            refuse()
        aborted = root / 'aborted'
        if exists(aborted):
            directory(aborted, True)
            if len(list(aborted.iterdir())) > 100:
                refuse()
            for path in aborted.iterdir():
                if not re.fullmatch('[0-9]+-[0-9]+\.json', path.name):
                    refuse()
                receipt, _ = record(path)
                run = path.stem
                origin = root / 'staging' / run
                directory(root / 'staging', True)
                directory(origin, True)
                reservation, reserved_hash = record(origin / 'aborted-reservation.json')
                if receipt.get('status') != 'aborted-before-dispatch' or receipt.get('run') != run or receipt.get('reservationSha256') != reserved_hash or receipt.get('noDispatchIntent') is not True or reservation.get('status') not in ['prepared', 'reserved'] or reservation.get('run') != run or receipt.get('inputSha256') != inputs or reservation.get('inputSha256') != inputs or not dead_controller(reservation.get('controller')):
                    refuse()
                for key in ['catalogueSha256', 'controllerSha256', 'proofSha256']:
                    if not re.fullmatch('[0-9a-f]{64}', receipt.get(key, '')) or receipt[key] != reservation.get(key):
                        refuse()
                archive = origin / 'aborted-journal'
                if exists(archive):
                    directory(archive, True)
                    if any(p.name not in ['actual-binding.json', 'approved-snapshot.json'] for p in archive.iterdir()):
                        refuse()
        operations = root / 'operations'
        if exists(operations):
            directory(operations, True)
            journal = operations / operation
            if exists(journal):
                directory(journal, True)
                if any(journal.iterdir()) and not exists(journal / 'dispatch-intent.json'):
                    refuse()
                if exists(journal / 'dispatch-intent.json'):
                    intent, intent_hash = record(journal / 'dispatch-intent.json')
                    binding, binding_hash = record(journal / 'actual-binding.json')
                    exited, exit_hash = record(journal / 'postgres-exit.json')
                    commit, commit_hash = record(journal / 'commit-receipt.json')
                    watch, watch_hash = record(journal / 'watch-receipt.json')
                    completed, _ = record(journal / 'completed.json')
                    approved, approved_hash = record(journal / 'approved-snapshot.json', False)
                    backend = binding.get('backend', {})
                    if binding.get('tenantId') != '0e297ca4-0809-4aab-a03f-1915045257b8' or binding.get('subdomain') != 'snwsb' or type(binding.get('databaseSchemaVersion')) is not int or binding['databaseSchemaVersion'] != 138 or backend.get('image') != image:
                        refuse()
                    if not re.fullmatch('[0-9a-f]{40}', binding.get('acceptedRelease', '')) or not re.fullmatch('[0-9a-f]{64}', backend.get('containerId', '')) or not integer(backend.get('pid'), 1) or type(backend.get('restartCount')) is not int or backend['restartCount'] != 0:
                        refuse()
                    started = datetime.fromisoformat(backend.get('startedAt', '').replace('Z', '+00:00'))
                    cutoff = datetime.fromisoformat('2026-10-25T09:56:38.687447+00:00')
                    if started < cutoff:
                        refuse()
                    for item in [intent, exited, commit, watch, completed]:
                        if item.get('inputSha256') != inputs or item.get('actualBindingSha256') != binding_hash or item.get('approvedSnapshotSha256') != approved_hash:
                            refuse()
                    if intent.get('status') != 'dispatching' or exited.get('status') != 'exited' or type(exited.get('exitCode')) is not int or exited['exitCode'] != 0:
                        refuse()
                    if commit.get('status') != 'committed' or commit.get('postgresExitSha256') != exit_hash or commit.get('postCommitVerified') is not True:
                        refuse()
                    if watch.get('status') != 'preserved' or not integer(watch.get('watchSeconds'), 300) or not integer(watch.get('samples'), 2) or watch.get('stableBackend') is not True or watch.get('preserved') is not True:
                        refuse()
                    if completed.get('status') != 'completed' or not integer(completed.get('watchSeconds'), 300) or completed.get('dispatchIntentSha256') != intent_hash or completed.get('commitReceiptSha256') != commit_hash or completed.get('watchReceiptSha256') != watch_hash or completed.get('watchSeconds') != watch['watchSeconds']:
                        refuse()
    print('Owned cleanup interlock is clear.')
except (OSError, ValueError, TypeError, KeyError, AttributeError, IndexError):
    print('Production mutation refused: owned cleanup requires reconciliation.', file=sys.stderr)
    sys.exit(78)
PY
