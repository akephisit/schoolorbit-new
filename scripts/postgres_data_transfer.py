#!/usr/bin/env python3
"""Private snapshot/restore reconciliation. URLs never enter argv or diagnostics.

Run on an operator host with PostgreSQL 18 client tools or the pinned Docker client.
The private JSON config has source_url, target_url, archive, and optional client_image.
Restore refuses nonempty targets. Reconcile must run before any target migrations.
"""
import argparse
import hashlib
import json
import os
import re
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import uuid
from urllib.parse import parse_qsl, unquote, urlparse, urlunparse

IMAGE = "public.ecr.aws/docker/library/postgres:18.6-alpine@sha256:d8703cd7fba306b9fec9268ecedfa8a966846c053036a60e3635791957eb2f66"


def connection_env(url):
    parsed = urlparse(url)
    if parsed.scheme not in ("postgres", "postgresql") or not parsed.hostname:
        raise ValueError("INVALID_CONNECTION")
    result = {"PGHOST": parsed.hostname, "PGPORT": str(parsed.port or 5432),
              "PGUSER": unquote(parsed.username or ""), "PGPASSWORD": unquote(parsed.password or ""),
              "PGDATABASE": unquote(parsed.path.removeprefix("/")), "PGCONNECT_TIMEOUT": "15"}
    names = {"sslmode": "PGSSLMODE", "sslrootcert": "PGSSLROOTCERT", "channel_binding": "PGCHANNELBINDING"}
    result.update({names[k]: v for k, v in parse_qsl(parsed.query) if k in names})
    return result


def execute(url, command, input_data=None):
    # Drop inherited PG options and secret-bearing runtime configuration.
    env = {k: v for k, v in os.environ.items() if not k.startswith("PG")}
    env.update(connection_env(url))
    result = subprocess.run(command, input=input_data, env=env, capture_output=True, check=False)
    if result.returncode:
        raise RuntimeError("DATABASE_OPERATION_FAILED")
    return result.stdout


def sql(url, statement):
    return execute(url, ["psql", "-X", "-qAt", "-v", "ON_ERROR_STOP=1"], statement.encode()).decode().strip()


def quote(identifier):
    return '"' + identifier.replace('"', '""') + '"'


def normalize_constraint(definition):
    # PostgreSQL's parser can move a varchar-literal-array cast to its elements
    # during dump/restore. Only this proven equivalent literal coercion is normalized;
    # all operators, parentheses, nonliteral expressions and length casts stay intact.
    literal = r"'(?:[^']|'')*'::character varying"
    array = rf"\(ARRAY\[({literal}(?:, {literal})*)\]\)::text\[\]"
    definition = re.sub(array, lambda match: "ARRAY[" + match[1].replace("::character varying", "::text") + "]", definition)
    definition = re.sub(rf"\(({literal})\)::text", lambda match: match[1].replace("::character varying", "::text"), definition)
    if definition.startswith("CHECK (") and definition.endswith(")"):
        return "CHECK " + json.dumps(boolean_expression(definition[6:]), separators=(",", ":"))
    return definition


def boolean_expression(expression):
    """Preserve atoms/order/precedence; flatten only associative AND/OR groups."""
    expression = expression.strip()
    depth = 0
    quoted = None
    operators = []
    outer_end = None
    index = 0
    while index < len(expression):
        char = expression[index]
        if quoted:
            if char == quoted:
                if index + 1 < len(expression) and expression[index + 1] == quoted:
                    index += 2
                    continue
                quoted = None
        elif char in ("'", '"'):
            quoted = char
        elif char in "([":
            depth += 1
        elif char in ")]":
            depth -= 1
            if depth == 0 and outer_end is None:
                outer_end = index
        elif depth == 0:
            for operator in (" AND ", " OR "):
                if expression.startswith(operator, index):
                    operators.append((index, operator.strip(), len(operator)))
        index += 1
    if quoted or depth != 0:
        raise ValueError("INVALID_CONSTRAINT_EXPRESSION")
    if expression.startswith("(") and outer_end == len(expression) - 1:
        return boolean_expression(expression[1:-1])
    # PostgreSQL's deparser brackets CASE/BETWEEN internals; refuse to guess if a
    # future expression exposes one at this level instead of normalizing it.
    if re.search(r"\b(CASE|BETWEEN|SELECT)\b", expression) and operators:
        return ["atom", expression]
    selected = [item for item in operators if item[1] == "OR"] or operators
    if not selected:
        return ["atom", expression]
    operator = selected[0][1]
    children = []
    start = 0
    for offset, _, length in [*selected, (len(expression), operator, 0)]:
        child = boolean_expression(expression[start:offset])
        if child[0] == operator:
            children.extend(child[1:])
        else:
            children.append(child)
        start = offset + length
    return [operator, *children]


def fingerprint(url):
    # One read-only, repeatable-read transaction produces bounded hashes, never rows.
    # Per-row SHA-256 protects column values/relationships including encrypted fields.
    tables = json.loads(sql(url, "SELECT COALESCE(json_agg(tablename ORDER BY tablename),'[]') FROM pg_tables WHERE schemaname='public';"))
    statements = ["BEGIN TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL statement_timeout='120s'; SET LOCAL timezone='UTC';"]
    for table in tables:
        literal = table.replace("'", "''")
        statements.append(f"SELECT json_build_object('table','{literal}','count',count(*),'digest',encode(sha256(convert_to(COALESCE(string_agg(h, '' ORDER BY h), ''),'UTF8')),'hex')) FROM (SELECT encode(sha256(convert_to(row_to_json(t)::text,'UTF8')),'hex') AS h FROM public.{quote(table)} t) q;")
    statements.append("ROLLBACK;")
    rows = [json.loads(line) for line in sql(url, "\n".join(statements)).splitlines() if line.startswith("{")]
    metadata = json.loads(sql(url, "SELECT json_build_object('encoding',pg_encoding_to_char(encoding),'effectiveCollation',CASE WHEN datlocprovider='b' THEN datlocale ELSE datcollate END,'ctype',datctype,'provider',datlocprovider,'locale',datlocale,'collationVersion',datcollversion) FROM pg_database WHERE datname=current_database();"))
    schema = json.loads(sql(url, """SELECT COALESCE(json_agg(row_to_json(q) ORDER BY table_name, ordinal_position),'[]') FROM
       (SELECT table_name,column_name,row_number() OVER (PARTITION BY table_name ORDER BY ordinal_position) AS ordinal_position,data_type,is_nullable,column_default
        FROM information_schema.columns WHERE table_schema='public') q;"""))
    constraints = json.loads(sql(url, """SELECT COALESCE(json_agg(row_to_json(q) ORDER BY table_name, name),'[]') FROM
       (SELECT conrelid::regclass::text AS table_name, conname AS name, pg_get_constraintdef(oid) AS definition
        FROM pg_constraint WHERE connamespace='public'::regnamespace AND conrelid<>0) q;"""))
    for constraint in constraints:
        constraint["definition"] = normalize_constraint(constraint["definition"])
    objects = schema_objects(url)
    evidence = {"tables": rows, "metadata": metadata, "schema": schema, "constraints": constraints, "objects": objects}
    digest = hashlib.sha256(json.dumps(evidence, sort_keys=True, separators=(",", ":")).encode()).hexdigest()
    return {"digest": digest, **evidence}


def schema_objects(url):
    indexes = json.loads(sql(url, """SELECT COALESCE(json_agg(row_to_json(q) ORDER BY table_name,name),'[]') FROM
      (SELECT t.relname AS table_name,c.relname AS name,a.amname AS method,
       i.indisunique AS unique,i.indisprimary AS primary,i.indisvalid AS valid,
       (SELECT json_agg(pg_get_indexdef(i.indexrelid,k,true) ORDER BY k) FROM generate_series(1,i.indnatts) k) AS keys,
       pg_get_expr(i.indpred,i.indrelid) AS predicate
       FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid JOIN pg_class t ON t.oid=i.indrelid
       JOIN pg_am a ON a.oid=c.relam WHERE t.relnamespace='public'::regnamespace) q;"""))
    for index in indexes:
        if not index["valid"]:
            raise ValueError("INVALID_DATABASE_INDEX")
        if index["predicate"]:
            index["predicate"] = normalize_constraint("CHECK (" + index["predicate"] + ")")
    functions = json.loads(sql(url, """SELECT COALESCE(json_agg(row_to_json(q) ORDER BY identity),'[]') FROM
      (SELECT p.oid::regprocedure::text AS identity,
       encode(sha256(convert_to(pg_get_functiondef(p.oid),'UTF8')),'hex') AS digest
       FROM pg_proc p WHERE p.pronamespace='public'::regnamespace AND p.prokind<>'a'
       AND NOT EXISTS(SELECT 1 FROM pg_depend d WHERE d.classid='pg_proc'::regclass AND d.objid=p.oid AND d.deptype='e')) q;"""))
    triggers = json.loads(sql(url, """SELECT COALESCE(json_agg(row_to_json(q) ORDER BY table_name,name),'[]') FROM
      (SELECT c.relname AS table_name,t.tgname AS name,t.tgenabled AS enabled,
       encode(sha256(convert_to(pg_get_triggerdef(t.oid,true),'UTF8')),'hex') AS digest
       FROM pg_trigger t JOIN pg_class c ON c.oid=t.tgrelid
       WHERE c.relnamespace='public'::regnamespace AND NOT t.tgisinternal) q;"""))
    views = json.loads(sql(url, """SELECT COALESCE(json_agg(row_to_json(q) ORDER BY name),'[]') FROM
      (SELECT c.relname AS name,encode(sha256(convert_to(pg_get_viewdef(c.oid,true),'UTF8')),'hex') AS digest
       FROM pg_class c WHERE c.relnamespace='public'::regnamespace AND c.relkind IN ('v','m')) q;"""))
    extensions = json.loads(sql(url, """SELECT COALESCE(json_agg(row_to_json(q) ORDER BY name),'[]') FROM
      (SELECT e.extname AS name,e.extversion AS version,n.nspname AS schema
       FROM pg_extension e JOIN pg_namespace n ON n.oid=e.extnamespace) q;"""))
    return {"indexes": indexes, "functions": functions, "triggers": triggers, "views": views, "extensions": extensions}


def run_archive_client(config, restore=False):
    archive = Path(config["archive"]).resolve()
    url = config["target_url" if restore else "source_url"]
    binary = "pg_restore" if restore else "pg_dump"
    if restore:
        if not archive.is_file() or archive.stat().st_mode & 0o077:
            raise ValueError("PRIVATE_ARCHIVE_REQUIRED")
        count = int(sql(url, "SELECT count(*) FROM pg_tables WHERE schemaname NOT IN ('pg_catalog','information_schema');"))
        if count:
            raise ValueError("TARGET_NOT_EMPTY")
        arguments = ["--exit-on-error", "--no-owner", "--no-privileges", "--dbname", connection_env(url)["PGDATABASE"], str(archive)]
    else:
        archive.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
        if archive.exists():
            raise ValueError("ARCHIVE_ALREADY_EXISTS")
        arguments = ["--format=custom", "--no-owner", "--no-privileges", "--file", str(archive)]
    if config.get("client_image"):
        # A private temporary env file keeps credentials out of Docker command arguments.
        with tempfile.TemporaryDirectory(prefix="schoolorbit-pg-client-") as temporary:
            env_file = Path(temporary) / "client.env"
            values = connection_env(url)
            if any("\n" in value or "\r" in value for value in values.values()):
                raise ValueError("INVALID_CONNECTION_VALUE")
            env_file.write_text("".join(f"{key}={value}\n" for key, value in values.items()))
            env_file.chmod(0o600)
            security = subprocess.run(["docker", "info", "--format", "{{json .SecurityOptions}}"], capture_output=True, check=False)
            if security.returncode:
                raise RuntimeError("LOCAL_DOCKER_UNAVAILABLE")
            # Root inside rootless Docker maps to the operator UID. A numeric host
            # UID inside that namespace would lose access to the private archive.
            client_uid = "0" if b"rootless" in security.stdout else str(os.getuid())
            command = ["docker", "run", "--rm", "--network", "host", "--user", client_uid,
                       "--env-file", str(env_file), "--volume", f"{archive.parent}:{archive.parent}"]
            ca = values.get("PGSSLROOTCERT")
            if ca:
                command.extend(["--volume", f"{ca}:{ca}:ro"])
            command.extend(["--entrypoint", binary, IMAGE, *arguments])
            result = subprocess.run(command, capture_output=True, check=False)
            if result.returncode:
                raise RuntimeError("ARCHIVE_CLIENT_FAILED")
    else:
        execute(url, [binary, *arguments])
    archive.chmod(0o600)
    if restore:
        restore_not_null_names(config)


def restore_not_null_names(config):
    query = """SELECT COALESCE(json_agg(row_to_json(q)),'[]') FROM
       (SELECT c.relname AS table_name, a.attname AS column_name, n.conname AS name
        FROM pg_constraint n JOIN pg_class c ON c.oid=n.conrelid
        JOIN pg_attribute a ON a.attrelid=c.oid AND a.attnum=n.conkey[1]
        WHERE n.contype='n' AND n.connamespace='public'::regnamespace) q;"""
    source = json.loads(sql(config["source_url"], query))
    target = json.loads(sql(config["target_url"], query))
    source_names = {(item["table_name"], item["column_name"]): item["name"] for item in source}
    if len(source_names) != len(source) or len(target) != len(source):
        raise ValueError("NOT_NULL_CONSTRAINT_MAPPING_FAILED")
    first, second = [], []
    suffix = uuid.uuid4().hex[:12]
    for index, item in enumerate(target):
        desired = source_names.get((item["table_name"], item["column_name"]))
        if desired is None:
            raise ValueError("NOT_NULL_CONSTRAINT_MAPPING_FAILED")
        if desired != item["name"]:
            table = 'public.' + quote(item["table_name"])
            temporary = "so_restore_nn_" + suffix + "_" + str(index)
            first.append(f"ALTER TABLE {table} RENAME CONSTRAINT {quote(item['name'])} TO {quote(temporary)};")
            second.append(f"ALTER TABLE {table} RENAME CONSTRAINT {quote(temporary)} TO {quote(desired)};")
    if first:
        sql(config["target_url"], "BEGIN;\n" + "\n".join(first + second) + "\nCOMMIT;")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("operation", choices=("snapshot", "restore", "reconcile", "bind-credentials"))
    parser.add_argument("--config", required=True)
    args = parser.parse_args()
    config_file = Path(args.config)
    if not stat.S_ISREG(config_file.stat().st_mode) or config_file.stat().st_mode & 0o077:
        raise ValueError("PRIVATE_CONFIG_REQUIRED")
    config = json.loads(config_file.read_text())
    if args.operation == "snapshot":
        has_plain_identity = sql(config["source_url"], "SELECT EXISTS(SELECT 1 FROM information_schema.columns WHERE table_schema='public' AND table_name='admin_users' AND column_name='national_id');") == "t"
        if has_plain_identity and int(sql(config["source_url"], "SELECT count(*) FROM admin_users WHERE national_id <> '1234567890123';")):
            # Current Admin source has only its documented synthetic fixture identity.
            # A different legacy identity needs an explicit encrypted mapping, never
            # a plaintext copy silently justified by database-provider migration.
            raise ValueError("LEGACY_ADMIN_IDENTITY_REMEDIATION_REQUIRED")
        source = fingerprint(config["source_url"])
        run_archive_client(config)
        # Caller must stop source writers for a final cutover snapshot. A rehearsal
        # may fail reconciliation if the live source changes, and cannot authorize cutover.
        manifest = Path(config["archive"] + ".source.json")
        with manifest.open("x") as output:
            json.dump(source, output)
        manifest.chmod(0o600)
        print(json.dumps({"operation": "snapshot", "tables": len(source["tables"])}))
    elif args.operation == "restore":
        run_archive_client(config, restore=True)
        print(json.dumps({"operation": "restore", "restored": True}))
    else:
        source = json.loads(Path(config["archive"] + ".source.json").read_text())
        target = fingerprint(config["target_url"])
        current_source = fingerprint(config["source_url"])
        passed = source == target == current_source
        print(json.dumps({"operation": "reconcile", "passed": passed, "tables": len(source["tables"])}))
        evidence = {"passed": passed, "sourceDigest": source["digest"], "targetDigest": target["digest"]}
        path = Path(config["archive"] + ".reconciliation.json")
        path.write_text(json.dumps(evidence))
        path.chmod(0o600)
        if not passed:
            raise ValueError("DATA_RECONCILIATION_FAILED")
        if args.operation == "bind-credentials":
            bind_credentials(config, evidence)


def bind_credentials(config, evidence):
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    tenant = str(uuid.UUID(config["tenant_id"]))
    key = bytes.fromhex(config["credentials_key"])
    if len(key) != 32:
        raise ValueError("INVALID_CREDENTIALS_KEY")
    encrypted = {}
    for purpose in ("runtime", "migration"):
        password = config[purpose + "_password"]
        if len(password) != 64 or any(c not in "0123456789abcdef" for c in password):
            raise ValueError("INVALID_GENERATED_PASSWORD")
        role = "so_" + tenant.replace("-", "") + ("_runtime" if purpose == "runtime" else "_owner")
        target = urlparse(config["target_url"])
        host = target.hostname
        if ":" in host:
            host = "[" + host + "]"
        netloc = f"{role}:{password}@{host}" + (f":{target.port}" if target.port else "")
        actual = sql(urlunparse(target._replace(netloc=netloc)), "SELECT current_user::text FROM pg_roles WHERE rolname=current_user AND NOT rolsuper AND NOT rolcreatedb AND NOT rolcreaterole AND NOT rolbypassrls AND NOT rolreplication;")
        if actual != role:
            raise ValueError("TENANT_CREDENTIAL_LOGIN_FAILED")
        nonce = os.urandom(12)
        aad = f"schoolorbit-db-credentials:v1:{tenant}:{purpose}".encode()
        encrypted[purpose] = (b"\x01" + nonce + AESGCM(key).encrypt(nonce, password.encode(), aad)).hex()
    # Only authenticated ciphertext enters SQL. The owner connection and master key
    # remain in the private operator config, never argv, stdout or source control.
    statement = f"""BEGIN;
      UPDATE schools SET db_runtime_password=decode('{encrypted['runtime']}','hex'),
        db_migration_password=decode('{encrypted['migration']}','hex'),
        db_credentials_reconciled_at=now(),
        db_credentials_reconciliation='{json.dumps(evidence)}'::jsonb
      WHERE id='{tenant}'::uuid RETURNING id;
      COMMIT;"""
    returned = sql(config["admin_target_url"], statement)
    if returned.strip() != tenant:
        raise ValueError("TENANT_CREDENTIAL_BINDING_FAILED")
    print(json.dumps({"operation": "bind-credentials", "bound": True}))


if __name__ == "__main__":
    os.umask(0o077)
    try:
        main()
    except Exception:
        # Never print exception text: driver/OS errors can contain credentials or source data.
        print("POSTGRES_TRANSFER_FAILED", file=sys.stderr)
        sys.exit(1)
