# Clamd Runtime Recreation Design

## Context

The canonical production Compose file now sets `schoolorbit-clamd` to `mem_limit: 3g`, but the
running VPS container still reports a `1.61 GB` ceiling. That value corresponds to the previous
`1536m` limit. The backend-school deployment uploads and promotes the new Compose file, then calls
`podman-compose up -d clamd` without explicitly replacing the existing container. On the current
VPS this leaves the old container configuration in place.

The workflow subsequently checks only clamd health. An already-running container can therefore
become healthy and let deployment succeed even though its memory limit is stale.

## Decision

The backend-school deployment will explicitly replace `schoolorbit-clamd` before waiting for
scanner health:

1. Pull the pinned ClamAV image before interrupting the scanner.
2. If `schoolorbit-clamd` exists, stop it and remove only that container.
3. Start the `clamd` service from the promoted canonical `podman-compose.yml`.
4. Read the running container memory limit with `podman inspect` and require exactly
   `3221225472` bytes (3 GiB).
5. Wait for the existing clamd healthcheck to report `healthy` before continuing the
   backend-school deployment.

The replacement must not remove the named `schoolorbit-clamav-signatures` volume, restart
backend-admin, or implicitly recreate backend-school before its existing controlled deployment
sequence.

## Alternatives Rejected

- `podman-compose up -d --force-recreate clamd` is shorter, but Compose may include running
  dependents in its recreation set. The workflow needs explicit service ownership and ordering.
- `podman update --memory 3g schoolorbit-clamd` changes the current container in place but does
  not prove that future deployments can reconstruct the runtime from the canonical Compose file.
- Continuing to rely on implicit Compose change detection preserves the production failure mode
  and cannot distinguish a healthy stale container from a correctly recreated one.

## Failure and Recovery Behavior

Image pull and Compose validation occur before the scanner is stopped. After removal, any stop,
remove, create, inspect, or health failure fails the deployment visibly. Backend-school may remain
live during the scanner interruption, but File Platform readiness and file uploads can be
temporarily unavailable until clamd is healthy again.

The signature database remains on its named volume, so container recreation does not intentionally
discard downloaded signatures. A failed deployment is recovered by fixing the reported runtime
error and rerunning the backend-school workflow; the workflow must never report success when the
observed memory limit differs from 3 GiB.

Cockpit displays memory using decimal GB, so the expected 3 GiB limit may appear as approximately
`3.22 GB`.

## Verification

1. Add a focused deployment static test that requires the ordered sequence: existing-container
   check, stop, remove, Compose start, exact runtime memory assertion, then health wait.
2. Confirm the new test fails against the current workflow before implementation.
3. Implement the minimal backend-school workflow change and confirm the focused test passes.
4. Run the frontend and deployment-workflow verification matrices from `.rules`, including the
   installer tests, Podman Compose dry-run, and Actionlint.
5. Run the smoke test because readiness and cross-service deployment behavior changes.
6. After deployment, verify both clamd health and the observed `3221225472`-byte memory limit on
   the VPS before declaring the runtime update complete.

## Scope

This change touches only the backend-school deployment workflow and its static regression test.
It does not change Compose topology, image versions, CPU/PID limits, backend code, database schema,
API contracts, permissions, realtime behavior, or sensitive-data handling.
