#!/usr/bin/env python3
"""Bounded, local PostgreSQL host status; runs as postgres through systemd/SSH."""
import argparse
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time


def run(command):
    result = subprocess.run(command, capture_output=True, timeout=30, check=False)
    if result.returncode:
        raise RuntimeError("STATUS_DEPENDENCY_UNAVAILABLE")
    return result.stdout.decode()


def purge_closed_logs(pgdata):
    root = (pgdata / "log").resolve()
    active = set()
    # Refuse cleanup if active-log metadata is unavailable.
    for line in (pgdata / "current_logfiles").read_text().splitlines():
        active.add((pgdata / line.split(" ", 1)[1]).resolve())
    files = sorted((p for p in root.glob("*.log") if p.is_file() and not p.is_symlink()), key=lambda p: p.stat().st_mtime)
    total = sum(p.stat().st_size for p in files)
    for path in files:
        if path.resolve() in active or path.resolve().parent != root:
            continue
        if path.stat().st_mtime < time.time() - 14 * 86400 or total > 256 * 1024 * 1024:
            size = path.stat().st_size
            path.unlink()
            total -= size


def purge_closed_backup_logs():
    root = Path("/var/log/pgbackrest")
    files = sorted((p for p in root.glob("*.log.*") if p.is_file() and not p.is_symlink()), key=lambda p: p.stat().st_mtime)
    total = sum(p.stat().st_size for p in root.iterdir() if p.is_file() and not p.is_symlink())
    for path in files:
        if path.stat().st_mtime < time.time() - 14 * 86400 or total > 64 * 1024 * 1024:
            size = path.stat().st_size
            path.unlink()
            total -= size


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--cleanup", action="store_true")
    parser.add_argument("--pgdata", default="/var/lib/postgresql/18/main")
    args = parser.parse_args()
    pgdata = Path(args.pgdata)
    memory = {line.split(":")[0]: int(line.split()[1]) for line in Path("/proc/meminfo").read_text().splitlines()}
    disk = shutil.disk_usage(pgdata)
    statement = """SELECT json_build_object(
      'connections',(SELECT count(*) FROM pg_stat_activity WHERE backend_type='client backend'),
      'maxConnections',current_setting('max_connections')::int,
      'archiveFailureOutstanding',last_failed_time IS NOT NULL AND (last_archived_time IS NULL OR last_failed_time>last_archived_time),
      'lastArchivedAt',last_archived_time,
      'databaseBytes',(SELECT sum(pg_database_size(oid)) FROM pg_database WHERE NOT datistemplate))
      FROM pg_stat_archiver;"""
    database = json.loads(run(["psql", "-X", "-qAt", "-v", "ON_ERROR_STOP=1", "-d", "postgres", "-c", statement]))
    backups = json.loads(run(["pgbackrest", "--stanza=schoolorbit", "--output=json", "info"]))
    stanza = backups[0]
    completed = [item["timestamp"]["stop"] for item in stanza.get("backup", [])]
    backup_age = time.time() - max(completed) if completed else None
    warnings = []
    if memory["MemAvailable"] < 512 * 1024:
        warnings.append("LOW_AVAILABLE_RAM")
    if disk.free / disk.total < 0.15:
        warnings.append("LOW_DISK_SPACE")
    if database["connections"] / database["maxConnections"] > 0.8:
        warnings.append("HIGH_CONNECTION_USE")
    if stanza.get("status", {}).get("code") != 0 or backup_age is None or backup_age > 36 * 3600:
        warnings.append("BACKUP_NOT_HEALTHY")
    if database["archiveFailureOutstanding"]:
        warnings.append("WAL_ARCHIVE_FAILURE")
    cert = Path("/etc/postgresql/18/main/server.crt")
    if cert.exists():
        result = subprocess.run(["openssl", "x509", "-checkend", str(30 * 86400), "-noout", "-in", str(cert)], capture_output=True, check=False)
        if result.returncode:
            warnings.append("TLS_CERTIFICATE_RENEWAL_DUE")
    if args.cleanup:
        purge_closed_logs(pgdata)
        purge_closed_backup_logs()
    print(json.dumps({"timestamp": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                      "availableRamMiB": memory["MemAvailable"] // 1024,
                      "diskFreeGiB": round(disk.free / 1024**3, 2), "load": os.getloadavg(),
                      "database": database, "backupAgeHours": None if backup_age is None else round(backup_age / 3600, 2),
                      "warnings": warnings}))
    return bool(warnings)


if __name__ == "__main__":
    try:
        sys.exit(int(main()))
    except Exception:
        print('{"warnings":["POSTGRES_STATUS_UNAVAILABLE"]}', file=sys.stderr)
        sys.exit(2)
