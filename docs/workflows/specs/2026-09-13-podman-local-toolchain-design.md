# Podman Local Toolchain Design

## Summary

SchoolOrbit will standardize Podman as the container runtime for local development, disposable
database tests, deployment verification, and the production VPS. GitHub Actions will continue to
use Docker Buildx actions only to build and publish OCI-compatible backend images; those images
remain runnable by Podman in production.

This is a hard cutover for repository-owned local runtime commands. It will not add a Docker
fallback, a `CONTAINER_ENGINE` abstraction, a Docker-compatible Podman socket, or a
`docker=podman` alias.

## Goals

- Use the same rootless container runtime for local execution, database-backed tests, and the VPS.
- Preserve separate local and production topologies so local PostgreSQL and source builds cannot
  be confused with production Neon and GHCR images.
- Keep disposable database-test cleanup exact, automatic, and safe on success and handled signals.
- Make repository verification fail clearly when Podman or Podman Compose is unavailable.
- Keep the existing Buildx cache, build-secret, timing, and image-publication behavior in GitHub
  Actions.

## Non-goals

- Changing application behavior, API contracts, permissions, migrations, or production secrets.
- Combining the local and production Compose files.
- Replacing Docker Buildx actions in image-build jobs.
- Supporting both Docker and Podman for repository-owned local commands.
- Removing pulled images or globally pruning containers, networks, volumes, or image caches.

## Runtime and Compose Ownership

The current local `docker-compose.yml` will become `compose.local.yml`. It will retain local-only
PostgreSQL, source builds, development defaults, and published development ports. Local commands
will invoke it explicitly:

```bash
podman-compose -f compose.local.yml up --build
```

The production `podman-compose.yml` remains the sole production Compose owner. It continues to use
Neon, prebuilt GHCR images, loopback backend bindings, production networks, and the existing
deployment workflow. Registry-qualified image names beginning with `docker.io/` remain unchanged;
`docker.io` identifies the registry and does not select the Docker runtime.

`podman-compose` is the explicit provider. Repository commands will not use `podman compose`, whose
provider selection can choose another installed Compose implementation.

## Disposable Backend-School Database Tests

`scripts/test_backend_school.sh` will call `podman` directly. Before creating resources it will:

1. require the Podman CLI;
2. reject configured remote Podman connections so test data stays on the developer machine;
3. require a reachable rootless local Podman engine; and
4. preserve the existing rule that an inherited `TEST_DATABASE_URL` is never used.

Each run will create a uniquely named PostgreSQL container backed by an anonymous volume. It will
publish PostgreSQL only on a random `127.0.0.1` port, install the required extensions inside that
container, pass the resulting local URL only to the Cargo child, and forward all test arguments.

Cleanup remains armed before container creation. On success, Cargo failure, readiness failure,
`INT`, `TERM`, or `HUP`, the runner will inspect and remove only its exact generated container with
`podman rm --force --volumes`. This removes the anonymous database volume attached to that
container. It will not remove the PostgreSQL image, Podman build cache, named volumes, networks, or
unrelated resources. As before, an uncatchable `SIGKILL` or host crash can require exact-target
manual cleanup.

## Static and Deployment Verification

Tests that currently resolve Compose through `docker compose config --format json` will instead
execute `podman-compose config`. Because that command emits YAML, `frontend-school` will declare a
direct YAML parser development dependency and parse the resolved topology before making the same
service, network, port, volume, and resource-limit assertions.

The backend static architecture tests will refer to `compose.local.yml` and
`podman-compose.yml`. Installer and topology verification will use `podman-compose` and
`podman run`, including the pinned actionlint container. Local runtime-image verification will use
`podman build` and `podman image inspect`.

GitHub backend image jobs will retain `docker/login-action`, `docker/setup-buildx-action`, and
`docker/build-push-action`. These actions are the CI image builder, not the application runtime,
and their output is consumed as OCI-compatible images by the production Podman host.

## Error Handling and Safety

- Missing Podman and Podman Compose errors will name the exact required command.
- A configured remote connection, a non-rootless engine, an unreachable engine, container startup
  failure, readiness timeout, extension failure, or invalid port binding will fail closed before
  Cargo uses a database URL.
- Diagnostics may contain generated container names, bounded status text, and local port numbers.
  They must not print inherited database URLs, production secrets, credentials, or container
  environments.
- Cleanup failures will preserve the original test failure status while reporting the exact
  generated container that could not be removed.
- No migration files or generated permission/API contracts will change.

## Documentation and Development Rules

The durable standard in `.rules` will state that:

- Podman is the local, test, deployment-verification, and production runtime;
- `compose.local.yml` owns local topology;
- `podman-compose.yml` remains the sole production topology owner;
- Docker Buildx is retained only for GitHub Actions image builds; and
- verification commands use Podman-native commands.

`README.md`, `docs/OPERATIONS.md`, and `docs/TESTING.md` will be updated in the same change. The
production bootstrap procedure in `docs/PODMAN_SETUP.md` remains authoritative for VPS setup and
will only be adjusted if a cross-reference or command has become inconsistent.

## Test Strategy

Implementation will start by updating tests to express Podman-native behavior and observing the
expected failures before changing runtime scripts or configuration. Verification will include:

- focused Node tests for the disposable PostgreSQL runner, including exact cleanup and signal paths;
- deployment/topology static tests using the renamed local Compose file and Podman Compose output;
- backend static architecture tests;
- shell formatting and lint checks for changed scripts;
- Podman Compose configuration/dry-run verification for local and production files;
- actual disposable PostgreSQL execution through Podman when the local rootless engine is
  available;
- the frontend and backend checks required by `.rules`; and
- `git diff --check`, final diff review, and `git status --short`.

Checks requiring Podman cannot be reported as passing until Podman is installed and reachable.
Unavailable external credentials and deployment targets remain explicitly unrun.

## Rollout and Recovery

This change affects development and CI verification only; it does not recreate production
containers or deploy an application. Developers install `podman` and `podman-compose`, then replace
local Docker commands with the documented Podman commands. The first full verification confirms
rootless networking, anonymous-volume cleanup, Compose resolution, and backend image builds.

If the cutover must be reverted, restore the former local Compose filename, Docker database-test
runner, verification commands, and documentation together. Production `podman-compose.yml` and
running VPS resources are not part of that rollback.
