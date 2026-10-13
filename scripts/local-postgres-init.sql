-- Disposable/local Docker bootstrap only. Production passwords come from the secret store.
CREATE ROLE schoolorbit_admin_owner LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE PASSWORD 'local-migration-password';
CREATE ROLE schoolorbit_admin_runtime LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE PASSWORD 'local-runtime-password';
ALTER DATABASE schoolorbit_admin OWNER TO schoolorbit_admin_owner;
REVOKE ALL ON DATABASE schoolorbit_admin FROM PUBLIC;
GRANT CONNECT ON DATABASE schoolorbit_admin TO schoolorbit_admin_runtime;
ALTER ROLE schoolorbit_provisioner NOSUPERUSER CREATEDB CREATEROLE;
