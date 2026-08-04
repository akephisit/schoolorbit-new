# Neon Scale-to-Zero Healthcheck Design

## Status

Approved on 2026-08-04.

## Problem

Production Compose probes `backend-admin /ready` and `backend-school /ready` every 30 seconds. The admin readiness handler executes `SELECT 1`, while school readiness calls admin readiness. These recurring dependency probes keep the admin Neon compute active even when no user is using SchoolOrbit. If the admin and tenant databases share a Neon branch compute, the probes keep that shared compute active as well.

SchoolOrbit already exposes dependency-free `/health` endpoints and dependency-aware `/ready` endpoints. Deployment workflows also call `/ready` explicitly, so recurring container liveness and release readiness do not need to use the same endpoint.

## Goals

- Allow Neon computes to scale to zero after the configured inactivity delay when there is no user or scheduled work.
- Preserve strict dependency checks before a backend deployment is declared successful.
- Keep process liveness visible to Compose without querying Neon.
- Prevent static architecture tests and operational documentation from reverting to recurring database readiness probes.

## Non-goals

- Do not remove or weaken `/ready` dependency checks.
- Do not change the public response contracts of `/health` or `/ready`.
- Do not remove required File Platform reconciliation or calendar reminder jobs.
- Do not change database schemas, permissions, migrations, or generated API contracts.
- Do not promise a permanently flat zero graph: real requests and required scheduled jobs may wake Neon temporarily.

## Decision

Use `/health` for recurring backend container healthchecks in both local and production Compose. Continue using `/ready` for deployment gates, smoke tests, direct operational verification, and intentional dependency diagnostics.

This separation gives each endpoint one responsibility:

- `/health`: the backend process is listening and can serve a response; no dependency access.
- `/ready`: backend dependencies are currently usable; admin checks PostgreSQL, and school checks the admin control plane plus the File Platform.

External uptime monitors must use `/health`. Calling `/ready` on a short interval would recreate the Neon keep-awake behavior outside the repository.

## Runtime Flow

1. Compose calls each backend's `/health` endpoint every 30 seconds.
2. The liveness handlers respond without acquiring a database connection or checking R2 or clamd.
3. With no user traffic or scheduled job, SQLx pools can release idle connections and Neon can apply its configured scale-to-zero policy.
4. A real request or scheduled job reconnects to Neon. The first operation after inactivity may incur an accepted cold-start delay.
5. File Platform reconciliation still visits active tenants at the top of each hour. Calendar reminders still visit active tenants at 07:00 Asia/Bangkok. Neon may be active briefly for those required jobs, then become idle again.

## Deployment and Failure Behavior

Backend deployment workflows continue to call `/ready` before restoring or declaring service availability. A PostgreSQL, backend-admin, R2, or clamd failure therefore fails the applicable release gate.

Changing Compose to `/health` means `depends_on: condition: service_healthy` establishes process availability rather than full dependency readiness. This is acceptable because backend-school starts without opening every tenant database, and the deployment workflow performs the authoritative `/ready` check before success. Clamd retains its own dependency-aware container healthcheck.

If a dependency fails after deployment, the backend process remains live while dependency-backed requests return their existing errors. Restarting a healthy process would not repair an external database or storage outage. Operators can call `/ready` intentionally for diagnosis, understanding that an admin readiness call wakes the relevant Neon compute.

## Repository Changes

- Change backend-admin and backend-school healthcheck commands in `podman-compose.yml` from `/ready` to `/health`.
- Make the matching change in `docker-compose.yml` so local and production topology remain aligned.
- Update the backend static architecture guard to require Compose `/health` and continue requiring workflow, smoke-test, and explicit deployment checks to use `/ready`.
- Update `.rules` to make the recurring-liveness versus deployment-readiness ownership explicit.
- Update `docs/OPERATIONS.md` with the same operational contract and the external-monitor warning.
- Leave the health handlers, readiness handlers, deployment workflows, smoke-test calls, schedulers, migrations, permissions, and generated contracts unchanged unless a focused test exposes a mismatch.

## Verification

Focused verification must prove:

- both Compose definitions call `/health` for backend-admin and backend-school;
- neither Compose definition calls backend `/ready` as its recurring healthcheck;
- deployment workflows and smoke tests still call `/ready`;
- backend `/health` handlers remain dependency-free;
- backend `/ready` handlers retain their current dependency checks.

Run the applicable production-topology matrix from `.rules`, including shell formatting/linting, installer Bats tests, deployment static tests, Compose dry-run validation, and actionlint. Run the backend-school static architecture test that owns these durable boundaries. Review `git diff --check`, the final diff, and `git status --short`.

After deployment, verify the rendered container health commands, call each `/ready` endpoint once to confirm the release gate, and observe that the recurring `SELECT 1` pattern disappears. Neon should transition to idle/zero after its configured inactivity delay except during genuine traffic or scheduled jobs.

## Success Criteria

- No repository-owned 30-second probe reaches Neon.
- Production deploys still fail when required dependencies are unavailable.
- With no users, external `/ready` monitor, or scheduled job running, Neon reaches zero after the configured inactivity period.
- The first request after idle succeeds within existing connection-acquisition timeouts, with the accepted cold-start delay.
