# API Contract Parallelization Design

## Status

Approved on 2026-08-04.

## Problem

The first CI cache performance stage gives API Contract an exact Rust cache hit, but the warm
workflow still takes 7m 09s. The equivalent cold job takes 11m 09s, so dependency caching saves
about four minutes but does not meet the two-to-three-minute diagnostic target.

The warm log proves that the cache is working:

- the exact `backend-school-contracts` key is restored;
- `RUST_CACHE_HIT` is `true`;
- approximately 869 MB is restored in 19 seconds; and
- the post step reports the cache as up to date.

The remaining time comes from independent Rust and frontend gates running sequentially in one
job. OpenAPI generation, the backend binary test target, sanitized offline export, the static
architecture test target, `cargo check`, and frontend checking each extend the same critical
path. Permission Contract does not have this problem after caching: its warm job takes 2m 44s.

## Goals

- Reduce warm API Contract wall-clock time toward three to four minutes by running independent
  validation groups concurrently.
- Preserve every existing generator, generated-artifact, sanitized export, formatting, backend
  test, logging-boundary, `cargo check`, frontend static test, and frontend check command.
- Keep the shared Rust dependency cache exact-hit behavior from Stage 1.
- Give one API job cache-write ownership so parallel jobs cannot race to publish the same cache.
- Make job boundaries and the absence of artificial dependencies durable through static tests.

## Non-goals

- Do not remove, combine away, weaken, or conditionally skip a validation gate.
- Do not change generated OpenAPI output, Rust code, frontend code, permissions, migrations,
  runtime deployment, or production services.
- Do not change Permission Contract; its measured warm time already meets the target.
- Do not introduce `sccache`, a self-hosted runner, or a persistent build host in this stage.
- Do not optimize total GitHub runner-minutes at the expense of wall-clock time. Parallel jobs
  repeat setup and cache downloads by design.
- Do not promise a three-to-four-minute result for cold dependency caches or GitHub runner/cache
  service delays.

## Decision

Replace the single API Contract `verify` job with three independent jobs named `artifacts`,
`backend`, and `frontend`. None declares `needs`, so GitHub may schedule them concurrently. The
workflow remains successful only when all three jobs succeed.

### Job ownership

| Job | Toolchains and caches | Existing gates owned |
| --- | --- | --- |
| `artifacts` | Node/npm and Rust/shared cache | API generator test, generated API artifact comparison, sanitized offline OpenAPI export |
| `backend` | Rust/shared cache | backend formatting, backend API contract test, exporter logging boundary, backend `cargo check` |
| `frontend` | Node/npm | frontend API response contract test, frontend `npm run check` |

Each job checks out the same tracked commit. No generated output is passed between jobs because
the workflow verifies committed artifacts rather than creating release artifacts for downstream
consumers. This makes the groups independent and prevents a failed producer from hiding a
failure in another group.

The sanitized export remains exactly a fresh-environment execution using `env -i`, `PATH`, and
`HOME`. It stays separate from the normal generated-artifact comparison because it proves that
OpenAPI export requires no database credential, runtime secret, or running backend.

### Cache ownership

Both Rust jobs restore the same pinned `Swatinem/rust-cache` action, shared key
`backend-school-contracts`, and workspace mapping `backend-school -> target`.

The `artifacts` job retains the trusted-main save policy:

```yaml
save-if: ${{ github.ref == 'refs/heads/main' }}
```

The `backend` job is restore-only on every event:

```yaml
save-if: "false"
```

This gives the API workflow one cache writer. Permission Contract retains its existing
trusted-main writer because it is a separate workflow with the same dependency graph; GitHub's
immutable cache behavior safely handles two successful workflows observing the same exact key.
Pull request executions remain restore-only.

Both Rust jobs write their exact cache result to the job summary without exposing secrets. The
frontend job keeps the existing npm cache. The artifacts job also keeps npm caching because its
generator scripts run through the frontend-school package.

## Failure and Security Behavior

- Failure in any job fails API Contract even when the other two jobs succeed.
- A cache miss compiles normally and runs every command; it never skips or soft-fails a gate.
- One job being queued or slow does not block the other independent jobs from starting when a
  runner is available.
- Jobs exchange no files, credentials, URLs, database state, or runtime state.
- Workflow permissions remain `contents: read`.
- Cache inputs and summaries contain only public toolchain, dependency, platform, key, and hit
  metadata.
- The change does not trigger either backend deployment workflow. Updating the cross-stack
  static guard under `frontend-school/tests/static`, however, also matches the existing
  `frontend-school/**` production deployment path and therefore triggers the normal all-tenant
  frontend deployment after the implementation is pushed to `main`.

## Durable Guard

Extend `frontend-school/tests/static/deployment-installer.test.mjs` so it verifies:

- API Contract defines exactly the `artifacts`, `backend`, and `frontend` validation jobs;
- the jobs do not declare `needs` and therefore remain independent;
- every existing command belongs to its approved job;
- artifacts and frontend each retain npm caching;
- artifacts and backend each restore the pinned shared Rust cache;
- exactly one API job has the trusted-main save policy and the backend job is always
  restore-only;
- both Rust jobs publish cache-hit summaries; and
- Permission Contract retains its current single-job cache and complete command set.

Update `.rules` and `docs/TESTING.md` so future workflow maintenance preserves the parallel job
ownership and does not trade away a gate to improve timing.

## Verification and Rollout

Use test-driven development: first change the static guard and confirm it fails against the
single `verify` job, then implement the three jobs and confirm it passes. Run the deployment
workflow matrix required by `.rules`, including ShellCheck, shfmt, installer Bats, the deployment
static suite, Podman Compose dry-run, actionlint, documentation policy, `git diff --check`, and
status/diff review.

Push to `main` only after the local matrix is green or an unavailable local dependency is
explicitly delegated to Installer CI. Observe API Contract, Permission Contract, Installer,
Documentation, and the automatically triggered all-tenant frontend deployment. Do not trigger a
backend deployment for this change.

After the first successful API Contract run, rerun only API Contract once. Record each job's
duration and the workflow duration. The warm target is approximately three to four minutes, with
all Rust summaries reporting an exact cache hit.

If the workflow remains materially above four minutes, identify the longest job and its slowest
Cargo invocation from logs before designing Stage 3. `sccache` is considered only when repeated
workspace compilation, rather than queueing, npm, or cache transfer, remains the dominant cost.

## Success Criteria

- API Contract has three independent validation jobs and all existing commands execute.
- The artifacts job is the only API cache writer; the backend job is restore-only.
- Warm Rust jobs report exact hits for `backend-school-contracts`.
- Permission Contract remains unchanged and stays successful.
- Warm API Contract wall-clock time trends toward three to four minutes.
- Every automatically triggered workflow succeeds, including the frontend deployment caused by
  the static-test path.

## Rollback

Revert the API workflow, static guard, and cache-parallelism documentation to the single-job
layout. No application, VPS, database, migration, generated contract, permission, or data rollback
is required. Existing cache objects remain valid and expire normally.
