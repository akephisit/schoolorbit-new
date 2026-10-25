"""Disposable PostgreSQL 18.6: real owner/TEMP/DML/TLS client transport, no production."""
import importlib.util
import json
import os
from pathlib import Path
import secrets
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]

def load(name, file):
    spec = importlib.util.spec_from_file_location(name, ROOT / file)
    module = importlib.util.module_from_spec(spec); sys.modules[name] = module
    spec.loader.exec_module(module)
    return module

load('owned_cleanup_proof', 'scripts/lib/owned-load-cleanup-proof.py')
controller = load('owned_cleanup_controller_transport', 'scripts/lib/pipeline-remote/owned-load-cleanup.py')
CAT = json.loads((ROOT / 'scripts/lib/owned-load-cleanup/catalogue.json').read_text())
RUN = subprocess.run


class OwnerTlsTransport(unittest.TestCase):
    @classmethod
    def invoke(cls, args, *, data=None, env=None, ok=True):
        result = RUN(args, input=data, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
        if ok and result.returncode:
            raise RuntimeError('Disposable transport setup refused: ' + args[0])
        return result

    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory(prefix='owned-cleanup-owner-tls-')
        cls.directory = Path(cls.temp.name)
        cls.prefix = 'owned-cleanup-tls-' + secrets.token_hex(5)
        cls.network = cls.prefix + '-net'; cls.server = cls.prefix + '-db'
        cls.owner = 'so_' + CAT['tenantId'].replace('-', '') + '_owner'
        cls.runtime = cls.owner.removesuffix('_owner') + '_runtime'
        cls.password = secrets.token_hex(24)
        cls.admin_password = secrets.token_hex(24)
        try:
            cls.invoke(['docker', 'network', 'create', cls.network])
            ca = cls.directory / 'ca'; ca.mkdir()
            cls.ca = ca
            cls.invoke(['openssl', 'req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(ca / 'ca.key'), '-out', str(ca / 'ca.crt'), '-days', '1', '-subj', '/CN=Disposable cleanup CA'])
            cls.invoke(['openssl', 'req', '-newkey', 'rsa:2048', '-nodes', '-keyout', str(ca / 'server.key'), '-out', str(ca / 'server.csr'), '-subj', '/CN=owned-cleanup-postgres'])
            (ca / 'extensions').write_text('subjectAltName=DNS:owned-cleanup-postgres\nextendedKeyUsage=serverAuth\n')
            cls.invoke(['openssl', 'x509', '-req', '-in', str(ca / 'server.csr'), '-CA', str(ca / 'ca.crt'), '-CAkey', str(ca / 'ca.key'), '-CAcreateserial', '-out', str(ca / 'server.crt'), '-days', '1', '-extfile', str(ca / 'extensions')])
            for p in ca.iterdir(): p.chmod(0o600)
            environment = {**os.environ, 'POSTGRES_PASSWORD': cls.admin_password}
            setup = 'mkdir /run/owned-pg-ssl; cp /fixtures/server.key /fixtures/server.crt /run/owned-pg-ssl/; chown -R postgres:postgres /run/owned-pg-ssl; chmod 600 /run/owned-pg-ssl/*; exec docker-entrypoint.sh postgres -c ssl=on -c ssl_cert_file=/run/owned-pg-ssl/server.crt -c ssl_key_file=/run/owned-pg-ssl/server.key'
            cls.invoke(['docker', 'run', '--detach', '--rm', '--name', cls.server, '--network', cls.network, '--network-alias', 'owned-cleanup-postgres', '--network-alias', 'wrong-name', '--env', 'POSTGRES_PASSWORD', '-v', str(ca) + ':/fixtures:ro', '--entrypoint', 'sh', controller.PG_IMAGE, '-ceu', setup], env=environment)
            for attempt in range(100):
                if cls.invoke(['docker', 'exec', cls.server, 'pg_isready', '-h', '127.0.0.1', '-U', 'postgres'], ok=False).returncode == 0:
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError('Disposable PostgreSQL readiness refused')
            sql = f"""CREATE ROLE {cls.owner} LOGIN PASSWORD '{cls.password}';
COMMENT ON ROLE {cls.owner} IS 'schoolorbit:{CAT['tenantId']}:migration';
CREATE ROLE {cls.runtime} LOGIN PASSWORD '{cls.password}';
CREATE DATABASE {CAT['databaseName']} OWNER {cls.owner};
REVOKE ALL ON DATABASE {CAT['databaseName']} FROM PUBLIC;
GRANT CONNECT ON DATABASE {CAT['databaseName']} TO {cls.owner},{cls.runtime};
\\connect {CAT['databaseName']}
SET ROLE {cls.owner};
CREATE TABLE cleanup_transport_probe(id integer PRIMARY KEY,value text NOT NULL);
INSERT INTO cleanup_transport_probe VALUES(1,'preserved');
GRANT USAGE ON SCHEMA public TO {cls.runtime};
GRANT SELECT,INSERT,UPDATE,DELETE ON cleanup_transport_probe TO {cls.runtime};
"""
            cls.invoke(['docker', 'exec', '-i', cls.server, 'psql', '-X', '-q', '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=sqlstate', '-U', 'postgres'], data=sql.encode())
        except Exception:
            cls.tearDownClass()
            raise

    @classmethod
    def tearDownClass(cls):
        if hasattr(cls, 'server'): cls.invoke(['docker', 'rm', '-f', '-v', cls.server], ok=False)
        if hasattr(cls, 'network'): cls.invoke(['docker', 'network', 'rm', cls.network], ok=False)
        if hasattr(cls, 'temp'): cls.temp.cleanup()

    def job(self, work, user=None, host='owned-cleanup-postgres'):
        job = controller.OwnerJob.__new__(controller.OwnerJob)
        job.cat = CAT
        job.connection = {'host': host, 'port': 5432, 'user': user or self.owner, 'database': CAT['databaseName'], 'password': self.password}
        escape = lambda value: str(value).replace('\\', '\\\\').replace(':', '\\:')
        (work / 'pgpass').write_text(':'.join(escape(job.connection[key]) for key in ['host', 'port', 'database', 'user', 'password']) + '\n')
        (work / 'pgpass').chmod(0o600)
        return job

    def local_transport(self, args, **kwargs):
        self.assertEqual(args[:2], ['podman', 'run'])
        self.assertNotIn(self.password, str(args))
        self.assertNotIn(self.password, str(kwargs.get('env')))
        converted = list(args); converted[0] = 'docker'
        converted[converted.index('--name') + 1] = self.prefix + '-client'
        converted[converted.index('--network') + 1] = self.network
        ca_mount = converted.index('/opt/stack/postgresql/ca.crt:/run/secrets/postgresql-ca.crt:ro')
        converted[ca_mount] = str(self.ca / 'ca.crt') + ':/run/secrets/postgresql-ca.crt:ro'
        return RUN(converted, **kwargs)

    def test_real_owner_identity_tls_and_temp_function(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as directory:
            work = Path(directory); job = self.job(work)
            (work / 'probe.sql').write_text("BEGIN; CREATE TEMP TABLE owned_temp(id int); CREATE FUNCTION pg_temp.probe() RETURNS integer LANGUAGE sql AS 'SELECT 1'; UPDATE cleanup_transport_probe SET value='temporary'; SELECT pg_temp.probe(); ROLLBACK; SELECT value FROM cleanup_transport_probe WHERE id=1;")
            (work / 'probe.sql').chmod(0o600)
            with patch.object(controller.subprocess, 'run', side_effect=self.local_transport):
                job.database_identity(work)
                output = job.psql(work, 'probe.sql')
            self.assertIn(b'preserved', output)
            self.assertNotIn(self.password.encode(), output)

    def test_canonical_runtime_dml_role_has_no_temp_privilege(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as directory:
            work = Path(directory); job = self.job(work, self.runtime)
            (work / 'probe.sql').write_text('BEGIN; CREATE TEMP TABLE forbidden_temp(id int); ROLLBACK;')
            (work / 'probe.sql').chmod(0o600)
            with patch.object(controller.subprocess, 'run', side_effect=self.local_transport):
                with self.assertRaises(controller.ControllerRefusal): job.psql(work, 'probe.sql')

    def test_verify_full_refuses_wrong_hostname(self):
        with tempfile.TemporaryDirectory(dir=self.directory) as directory:
            work = Path(directory); job = self.job(work, host='wrong-name')
            (work / 'probe.sql').write_text('SELECT 1;'); (work / 'probe.sql').chmod(0o600)
            with patch.object(controller.subprocess, 'run', side_effect=self.local_transport):
                with self.assertRaises(controller.ControllerRefusal): job.psql(work, 'probe.sql')


if __name__ == '__main__':
    unittest.main()
