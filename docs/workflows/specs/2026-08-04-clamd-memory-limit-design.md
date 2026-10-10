# Clamd Memory Limit Design

## Context

The production `schoolorbit-clamd` container became unhealthy immediately after `freshclam`
downloaded and tested a new daily signature database. Backend-school remained live, but its
`/ready` endpoint returned `filePlatform: unavailable`, which blocked the tenant frontend
deployment readiness gate.

Both local and production Compose definitions currently cap clamd at `1536m`. ClamAV documents
that concurrent database reload temporarily keeps the old and new engines in memory while
`freshclam` also tests the downloaded database. This creates a predictable memory peak during
daily signature updates.

## Decision

- Set the clamd `mem_limit` to `3g` in both `docker-compose.yml` and `podman-compose.yml`.
- Keep concurrent database reload enabled so scans can continue while a new signature database
  loads.
- Add a static deployment invariant that requires the same `3g` limit in both Compose owners.
- Do not change CPU, PID, healthcheck, network, volume, scanner timeout, or ClamAV image settings.

## Runtime and Deployment Impact

The change raises only the maximum memory available to clamd; it does not reserve 3 GB
continuously. Local development and production retain matching scanner limits.

Pushing the production Compose change to `main` matches the backend deployment workflow path
filters. The normal backend-school deployment starts clamd, waits for `clamdcheck.sh` to report
healthy, recreates backend-school, and verifies `/ready`. Because the canonical production
Compose file is shared, the backend-admin workflow is also eligible to run from the same push.

No database, API contract, permission, frontend behavior, realtime event, or sensitive-data flow
changes.

## Verification

1. Add the `3g` invariant to the existing deployment static test and confirm it fails while the
   Compose files still contain `1536m`.
2. Update both Compose files to `3g` and confirm the focused test passes.
3. Run the deployment verification matrix from `.rules`, including shell checks, installer tests,
   Compose dry-run validation, deployment static tests, and `actionlint`.
4. Review `git diff --check`, the final diff, and `git status --short`.

## Recovery Expectation

After deployment recreates clamd with the larger limit, `clamdcheck.sh` must become healthy and
backend-school `/ready` must report `filePlatform: ready`. If the failure recurs at 3 GB, inspect
the clamd process and host memory evidence before considering `ConcurrentDatabaseReload no`.
