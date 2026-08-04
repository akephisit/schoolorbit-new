# CI Cache Performance Design

## Status

Draft for final review on 2026-08-04. The design outline and warm-run targets have been
approved.

## Problem

Four GitHub Actions workflows take longer than necessary even when the relevant dependency
inputs have not changed:

| Workflow | Observed successful run | Total time | Main avoidable work |
| --- | --- | ---: | --- |
| Deploy Backend Admin | `30915220317` | 5m 21s | Rust dependency rebuild and 63s cache export despite no admin source change |
| Deploy Backend School | `30915222877` | 3m 25s | Build was warm; most time was required deployment verification |
| API Contract | `30915220858` | 10m 48s | Repeated Rust compile work across export, tests, and `cargo check` |
| Permission Contract | `30915220078` | 6m 45s | Rust compilation in `cargo check` and static architecture tests |

The backend image workflows both use the unnamed GitHub Actions BuildKit cache scope. The GHA
backend defaults that scope to `buildkit`, so the two images can replace one another's cache.
The admin run imported a cache but rebuilt its Rust dependency layer, while the school run was
fully cached. This is consistent with cross-image cache scope collision rather than a required
source rebuild. Docker documents the default scope and overwrite behavior in the
[GitHub Actions cache backend documentation](https://docs.docker.com/build/cache/backends/gha/).

The API and permission contract workflows cache npm dependencies but do not restore Cargo's
registry, Git sources, or the `backend-school/target` dependency artifacts. The latest runs
therefore spend several minutes recompiling the same Rust dependency graph on fresh hosted
runners.

## Goals

- Bring ordinary warm-cache backend deployment workflows to approximately three minutes or
  less without removing deployment, migration, readiness, proxy, R2, or ClamAV checks.
- Bring ordinary warm-cache API and permission contract workflows to approximately two to three
  minutes while preserving every contract-generation and validation command.
- Make cache ownership explicit so one backend image cannot evict the other backend's active
  BuildKit cache key.
- Share safe Rust dependency artifacts between backend-school contract workflows without
  allowing pull requests to publish trusted caches.
- Make cache configuration and outcomes diagnosable from each workflow run.
- Add a static repository guard against accidentally returning to colliding or missing cache
  configuration.

## Non-goals

- Do not skip, merge, weaken, or reorder correctness and security gates merely to meet a timing
  target.
- Do not change backend runtime behavior, APIs, permissions, database schemas, migrations, or
  generated contracts.
- Do not parallelize production mutations or remove the shared deployment concurrency group.
- Do not introduce a self-hosted runner, persistent build host, or new production service.
- Do not introduce `sccache` in the first implementation. It remains a follow-up option only if
  measured warm runs still miss the target after dependency caching works.
- Do not promise the warm target for a deliberately cold cache, Rust/toolchain or lockfile
  change, Neon cold start, tenant migration, registry latency, or external service delay.

## Decision

Use separate GHA BuildKit scopes for the two deployable backend images and add one shared,
dependency-oriented Rust cache to the two backend-school contract workflows. Keep the existing
npm cache and every current command and release gate.

### Backend image cache ownership

`deploy-backend-admin.yml` uses the scope `backend-admin`, and
`deploy-backend-school.yml` uses the scope `backend-school`. Each workflow supplies its own scope
to both `cache-from` and `cache-to`, retaining `mode=max` on cache export.

The cache identity therefore becomes:

```text
backend-admin image  -> GHA BuildKit scope backend-admin
backend-school image -> GHA BuildKit scope backend-school
```

The first build after changing scopes can be cold because the new scope has no history. Later
builds reuse only the matching image's layers. Normal Dockerfile inputs continue to invalidate
the applicable layers; the design does not conceal source or dependency changes.

The existing deployment concurrency group remains unchanged. Build jobs may execute normally,
but VPS runtime activation, migrations, proxy replacement, and readiness verification must stay
serialized.

### Rust contract cache ownership

`api-contract.yml` and `permission-contract.yml` restore a Rust cache immediately after the Rust
toolchain is installed and before the first Cargo command. The cache covers Cargo download
state and dependency-oriented artifacts for `backend-school -> target`.

Both workflows use the same explicit shared key because they compile the same backend-school
workspace. The cache action also includes the Rust toolchain, operating system, and Cargo
manifest/lockfile inputs in its effective identity. A Rust version, `Cargo.toml`, or `Cargo.lock`
change consequently creates or selects a different cache rather than reusing incompatible
artifacts.

The implementation uses `Swatinem/rust-cache` pinned to a reviewed immutable commit. Its default
dependency-focused cleanup is preferable to caching the entire target directory mechanically,
and one shared key avoids duplicating large Rust caches within GitHub's repository cache quota.
See the [rust-cache project documentation](https://github.com/Swatinem/rust-cache).

Pull request and other untrusted runs may restore the default branch cache but do not save or
replace it. Only trusted `main` runs are allowed to publish updated Rust cache entries. Cache
contents must not contain workflow secrets, generated environment files, application logs, or
runtime credentials.

The existing npm setup and cache remain unchanged. The workflows continue to run their current
OpenAPI export, sanitized-environment export, generated-file comparison, backend tests, static
architecture tests, `cargo check`, and frontend checks.

## Observability

Each contract workflow gives the Rust cache step a stable identifier and writes its reported
cache hit/miss result and shared cache name to `$GITHUB_STEP_SUMMARY`. This makes a slow run
distinguishable from an intentional cold cache without exposing secrets.

Each backend workflow records its configured BuildKit scope in the workflow summary. The Docker
build action's build record and log remain the authoritative layer-level evidence because
BuildKit does not provide one reliable aggregate hit/miss value for a multi-layer build. Reviewers
can verify imported scope and `CACHED` layer lines from that record instead of relying on an
invented boolean.

GitHub's job and step timestamps remain the timing source. Timing is evaluated separately for
build/contract work and production deployment work so a Neon wake-up or tenant migration is not
misdiagnosed as a cache miss.

## Failure and Security Behavior

- A cache miss is not a workflow failure. The workflow performs the full build or compilation,
  then continues through all existing gates.
- A corrupt or incompatible restored artifact must be recoverable by normal Cargo/BuildKit
  invalidation or by reverting the cache configuration; correctness never depends on the cache.
- No cache step receives VPS, database, R2, Cloudflare, SSH, or application secrets as inputs.
- The shared Rust cache is writable only by trusted `main` executions. Pull requests remain
  read-only cache consumers.
- GitHub Actions cache storage is finite. The initial change does not delete caches manually;
  obsolete entries expire under GitHub's cache retention behavior.
- The implementation must not print cache keys containing secret-derived data. Keys are based
  only on public workflow, toolchain, platform, and dependency metadata.

## Repository Changes

- Update `.github/workflows/deploy-backend-admin.yml` with the `backend-admin` BuildKit scope and
  a non-secret cache summary.
- Update `.github/workflows/deploy-backend-school.yml` with the `backend-school` BuildKit scope
  and a non-secret cache summary.
- Update `.github/workflows/api-contract.yml` with the shared backend-school Rust cache and cache
  summary.
- Update `.github/workflows/permission-contract.yml` with the same shared Rust cache contract and
  cache summary.
- Extend `frontend-school/tests/static/deployment-installer.test.mjs` to enforce distinct backend
  scopes, matching import/export scopes, Rust cache placement and ownership, trusted save policy,
  and preservation of the important contract and deployment gates.
- Update operational/testing documentation only where the new cache policy needs a durable owner.

## Verification

Focused static verification must prove:

- the admin and school BuildKit scopes are explicit, distinct, and used consistently by both
  `cache-from` and `cache-to`;
- both contract workflows install Rust before restoring cache and restore cache before any Cargo
  command;
- both contract workflows use the same backend-school workspace/shared cache contract;
- only `main` may save the Rust cache;
- the npm cache and current API, permission, migration, readiness, proxy, R2, and ClamAV gates
  remain present;
- summaries expose only non-sensitive scope and cache-result metadata.

Run the deployment-workflow matrix required by `.rules`: ShellCheck, shfmt, installer Bats tests,
the deployment static test, Podman Compose dry-run validation, and actionlint. Also run the
documentation policy test, `git diff --check`, review the final diff, and inspect
`git status --short`.

After pushing `main`, observe the naturally triggered backend, API contract, and permission
contract runs. The first new Docker-scope and Rust-cache runs establish their caches and are not
the warm benchmark. Re-run only the API and permission contract workflows to measure a second
warm execution. Do not redeploy either production backend solely to create a benchmark; use the
next ordinary backend deploy to measure warm image reuse.

## Success Criteria

- Backend Admin and Backend School import and export different BuildKit cache scopes.
- An unrelated school build no longer causes Admin's unchanged Rust dependency layer to rebuild,
  and vice versa.
- Warm API and permission contract runs report Rust cache hits and retain identical validation
  behavior and generated outputs.
- Ordinary warm backend runs complete in approximately three minutes or less, excluding a
  legitimate migration, Neon cold start, or external delay.
- Ordinary warm API and permission contract runs complete in approximately two to three minutes.
- A missed timing target produces enough build/cache evidence to select a focused second-stage
  optimization without weakening a gate.

## Rollback

Revert the four workflow cache changes and their static assertions. This requires no application,
database, migration, VPS, or data rollback. Existing cache objects are harmless and can expire
naturally; rollback does not delete repository caches automatically.
