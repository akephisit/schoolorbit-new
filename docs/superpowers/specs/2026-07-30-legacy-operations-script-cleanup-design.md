# Legacy Operations Script Cleanup Design

## Goal

Remove obsolete one-off and compatibility scripts after the production cutovers,
while preserving the deployment, rollback, provisioning, migration, discovery,
and smoke-test tooling that defines the current operating model.

## Scope

Delete these obsolete or unsafe utilities:

- `nginx-configs/quick-fix.sh`
- `backend-admin/scripts/create_prod_admin.sh`
- `backend-admin/scripts/deploy_tenant.sh`
- `backend-admin/scripts/diagnose_login.sh`
- `backend-admin/scripts/verify_admin.sh`
- `backend-school/scripts/set_encryption_role.sh`
- `backend-school/scripts/upgrade_file_platform_env.sh`

The Backend Admin utilities are superseded by current workflows and include
plaintext identity or bootstrap credential handling that conflicts with current
security rules. The Nginx quick fix targets the retired host-Nginx layout.
The encryption-role utility is unused and conflicts with the container runtime
configuration model. The File Platform environment upgrader was needed only for
the completed legacy-to-public/private bucket cutover.

Keep these operational boundaries:

- tracked Nginx proxy definitions and their test/reload/rollback deployment flow;
- `backend-school/scripts/setup_r2.sh` for intentional new-environment setup;
- tenant discovery, smoke tests, sandbox seeding, and migration/cutover tools;
- runtime readiness gates and File Platform bucket/scanner checks already
  performed by the application and deployment.

## Deployment Behavior

The Backend School workflow will stop uploading and executing
`upgrade_file_platform_env.sh`. It will use the already-cut-over
`R2_PUBLIC_BUCKET_NAME` and `R2_PRIVATE_BUCKET_NAME` values directly.
No replacement preflight will be added for those values; missing configuration
will fail through the existing deployment or application readiness path.

The workflow will remove the known stale staged copy at
`/opt/stack/file-platform-runtime/scripts/upgrade_file_platform_env.sh` so the
retired compatibility script does not remain on the VPS. No directory-wide or
pattern-based deletion is allowed.

## Tests and Documentation

Replace the compatibility-upgrade test with a static deployment contract that
rejects references to `upgrade_file_platform_env.sh` and the legacy
`R2_BUCKET_NAME`. Remove documentation describing the first-rollout upgrader.
Repository searches must confirm that deleted utility names and plaintext
bootstrap credentials are not retained by the deleted scripts.

No applied migration will be changed.

## Success Criteria

- The seven retired scripts are absent.
- Backend School deployment does not upload or execute the compatibility
  upgrader and removes only its exact stale VPS path.
- Current deployment, rollback, migration, discovery, setup, and smoke tooling
  remains intact.
- Focused architecture tests, workflow/static contracts, formatting, and the
  applicable project verification matrix pass.
