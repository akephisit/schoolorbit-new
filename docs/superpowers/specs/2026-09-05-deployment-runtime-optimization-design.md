# Deployment Runtime Optimization Design

## Status

Approved in chat on 2026-09-05.

## Problem

Backend-school deployment duration increased from 5 minutes 31 seconds in workflow run
`30425390697` to 12 minutes 16 seconds in workflow run `33895666508`. The current BuildKit
dependency cache is healthy, but the changed application crate takes substantially longer to compile
and the production deployment performs more readiness, migration, maintenance, scanner, and
authenticated-smoke gates than the older workflow.

The production host also retains every commit-SHA image pulled by the backend workflows. A current
backend-school image is approximately 83.5 MB compressed, of which approximately 49.5 MB is unique
to a release. Sixty-five distinct successful backend-school SHAs since 2026-07-29 therefore represent
roughly 3.2 GB of potentially retained compressed layers before failed deployments, backend-admin,
and unpacked storage are counted. `podman image prune -f` is insufficient because it removes dangling
images, while the retained releases still have SHA tags.

The runtime image has another avoidable cost: it copies the binary and migrations, then runs
`chown -R /app` in a later layer. The ownership layer copies up approximately 25 MB of application
content in the current backend-school image.

Finally, backend-school currently recreates ClamAV on every deployment even when the pinned image,
resource limits, network isolation, signature volume, and health are already correct. That preserves
configuration correctness but adds an avoidable stop/start and signature-readiness delay to an
otherwise unchanged scanner.

## Goals

- Bound local production image growth without deleting the active image, rollback image, or any
  volume.
- Make runtime disk consumption and deployment phase durations directly diagnosable without
  exposing environment values, application logs, object keys, or credentials.
- Reduce both backend runtime image sizes by avoiding the ownership copy-up layer.
- Reuse a healthy, exactly configured ClamAV container and recreate it on any relevant drift.
- Make the Rust compiler and cargo-chef versions deterministic.
- Measure Cargo work and add a safe compiler-output cache for source-changing builds while retaining
  the existing dependency-oriented BuildKit cache.
- Bound GHCR history to the 30 latest releases of each backend package.
- Preserve every migration, readiness, proxy, R2, ClamAV, origin, and authenticated-smoke gate.

## Non-goals

- Do not change application APIs, permissions, database schemas, migrations, session behavior,
  realtime behavior, R2 data, or frontend behavior.
- Do not use a paid larger runner or introduce a self-hosted build service.
- Do not prune Podman volumes, containers owned by another service, general host images, or BuildKit
  cache on the VPS.
- Do not make production correctness depend on a cache hit.
- Do not deploy, push, delete a live package version, or mutate the VPS as part of local verification.
- Do not remove deployment safety gates to meet a timing target.

## Decision

Implement the work as independently testable stages on one branch. Each stage keeps the current
production behavior valid if a later stage is reverted.

### Runtime disk observability

Extend the manually dispatched runtime diagnostics workflow with bounded, non-sensitive output:

- filesystem capacity for the filesystem containing Podman's graph root;
- `podman system df` totals and `podman system df -v` image/container/volume accounting;
- counts of commit-SHA tags for the two SchoolOrbit backend repositories;
- no `podman inspect` environment output, container logs, volume contents, or file listings.

Backend deployment workflows print machine-readable phase timing lines in the form
`deployment_timing phase=<name> seconds=<integer>`. Backend-school measures scanner convergence,
image pull, backend replacement/readiness, tenant migration, migration status/audit, authenticated
smoke, and proxy opening where those phases run. Backend-admin measures image pull, backend
replacement/readiness, and origin verification. GitHub step timings remain authoritative for runner
checkout, BuildKit import/export, image push, SCP, and SSH action overhead.

### Production image retention

Add one reusable Bash script under `scripts/` that accepts an exact repository name and a retention
count. It lists only tags belonging to that repository, recognizes release tags only when they are
exactly 40 lowercase hexadecimal characters, sorts them newest first using Podman's creation sort,
and retains the newest three release tags.

Before removing any tag, the script resolves and protects:

- every image ID used by an existing container;
- the image IDs addressed by `<repository>:latest` and `<repository>:rollback` when present;
- the newest three commit-SHA tags.

Every older unprotected SHA reference is removed with `podman image rm <exact-reference>`. A failure
to enumerate or protect images fails closed and removes nothing. A removal race or an image that
became active is reported and fails the cleanup instead of forcing deletion. After targeted tag
removal, `podman image prune -f` may remove newly dangling layers. The script never calls
`podman system prune`, `podman volume prune`, `podman container prune`, or a broad
`podman image prune -a`.

Both backend workflows upload the script with the canonical deployment assets and invoke it only
after the workflow's current release-acceptance boundary. Backend-school cleanup follows readiness,
all active tenant migrations, and migration-status/audit verification; a deployment that fails before
that boundary retains the prior rollback tag and performs no cleanup. Backend-admin cleanup follows
readiness and selected-origin verification. The current `rollback` advancement semantics remain
unchanged.

The cleanup reports before/after release-tag counts and `podman system df` totals without dumping
image configuration. It is idempotent and a second invocation with no new deployment removes
nothing.

### Runtime image construction

Pin both builder images to `rust:1.98.0-slim-bookworm` and install `cargo-chef` version `0.1.78`
with Cargo's locked dependency resolution. The version pin prevents a new stable compiler release
from silently selecting a different dependency-cache identity; normal Debian base-image refreshes
remain possible through the tag.

Create the numeric runtime user before application copies and use `COPY --chown=1000:1000` for the
binary and migrations. Remove the later recursive ownership mutation. Runtime UID, paths, commands,
ports, installed runtime packages, and migration contents remain unchanged.

The image build must still work outside GitHub Actions. Compiler caching is optional at build time,
and absent GitHub cache credentials cause a direct Cargo build rather than a failure.

### ClamAV convergence

Replace unconditional scanner recreation with an explicit convergence decision. The existing
container is reusable only when every inspection succeeds and all of the following match the
canonical Compose definition:

- running image ID equals the locally pulled `clamav-debian:1.5.3` image ID;
- memory is exactly 3 GiB, CPU limit is 1.0, PID limit is 256, and restart policy is
  `unless-stopped`;
- `no-new-privileges` is present;
- there are no published scanner ports;
- the exact `schoolorbit-clamav-signatures` named volume is mounted at `/var/lib/clamav`;
- the container belongs to the file-platform internal and ClamAV egress networks;
- health status is `healthy`.

If any value is absent, unreadable, mismatched, or unhealthy, deployment stops and removes only
`schoolorbit-clamd`, recreates it from the canonical Compose file, reasserts the 3 GiB limit, and
waits for the existing health gate. A matching healthy scanner is left running and still passes
through the health assertion. No path removes or prunes the signature volume.

The workflow emits `clamd_action=reused` or `clamd_action=recreated reason=<bounded-code>` without
printing raw inspect JSON or logs on the successful path. The existing bounded final log tail remains
available only when scanner readiness fails.

### Cargo timing and compiler cache

Retain the current distinct `type=gha,mode=max` BuildKit scopes for dependency and image-layer
caching. Add Cargo `--timings` to the final application build and a non-runtime Docker target that
exports only Cargo's timing HTML. A second Buildx invocation reuses the already-built builder layer,
exports the small timing target locally, and uploads it as a short-retention workflow artifact. The
runtime image contains no timing report.

Install the official sccache `0.17.0` x86-64 musl release in the builder from its versioned GitHub
release URL and require SHA-256
`67c4a96dd237c1f518f6b36083f270f9976d516f1e57fce891755ea782e50006` before installation.
The compiler wrapper is enabled only when both GitHub Actions cache runtime values are present.

Trusted backend image workflows expose `ACTIONS_RESULTS_URL` and `ACTIONS_RUNTIME_TOKEN` to the
specific Cargo build instruction through BuildKit secret environment mounts. They are never build
arguments, image environment variables, copied files, cache-key material, or workflow output. The
two backends use distinct sccache keys so their result sets cannot overwrite one another. Cache
read/write errors fall back to compilation and do not bypass or fail the build. The build prints
bounded sccache statistics such as compile requests, hits, misses, errors, and non-cacheable calls;
it never prints cache credentials.

sccache is expected to reuse cacheable Rust compilation outputs, not the final executable link.
The Cargo timing artifact and sccache statistics determine whether it is beneficial. If two ordinary
warm builds show no useful hits or longer total build time, the sccache portion is reverted while
retaining version pins, timings, and BuildKit caching.

### GHCR retention

Add a weekly scheduled and manually dispatchable workflow with only `contents: read` and
`packages: write`. It processes exactly `schoolorbit-backend-admin` and
`schoolorbit-backend-school`, using the repository `GITHUB_TOKEN`; it accepts no VPS, application,
database, R2, SSH, or Cloudflare secrets.

For each package, the workflow paginates every package version and identifies releases by an exact
40-character lowercase hexadecimal container tag. It keeps the 30 newest release versions and any
version carrying `latest`. Versions without a recognized release tag are not counted as releases
and are not deleted by the first implementation, avoiding accidental deletion of OCI provenance or
attestation manifests. Deletion candidates are re-read immediately before mutation; if pagination,
metadata, package authorization, or protected-tag validation fails, that package receives no
deletions.

Manual dispatch defaults to dry-run and prints candidate version IDs, creation times, and safe tag
names. The weekly schedule performs deletion. The workflow handles the GitHub API's pagination and
per-run limits explicitly, deleting oldest candidates first and leaving excess candidates for the
next scheduled run. It never deletes all versions of a package.

GitHub package deletion is recoverable for GitHub's documented restoration window, but the design
does not rely on recovery: protected/current versions are excluded before every delete request.

## Data Flow and Failure Boundaries

```text
build source
  -> pinned Rust/cargo-chef + optional secret-mounted sccache
  -> full Cargo release build + timing artifact
  -> smaller runtime image
  -> GHCR current SHA/latest
  -> serialized VPS deployment
  -> ClamAV reuse or exact recreation
  -> backend readiness and existing release gates
  -> rollback tag advancement
  -> targeted local SHA cleanup

weekly package inventory
  -> protect latest + 30 newest SHA releases
  -> re-read candidates
  -> delete only confirmed older release versions
```

A cache miss performs the full build. A failed image cleanup fails the deployment after the new
backend has already passed its acceptance boundary, but it cannot stop or delete that backend. A
failed GHCR cleanup affects only its maintenance workflow and never triggers a deployment. A failed
ClamAV drift inspection recreates the scanner and uses the existing health gate.

## Security and Privacy

- Runtime diagnostics expose capacity and object counts only.
- Build cache runtime tokens use BuildKit secret mounts and remain absent from layers, image config,
  cache metadata, artifacts, and logs.
- Cleanup matches exact repository names and exact SHA syntax; user-supplied arbitrary repository
  patterns are rejected.
- No workflow prints container environments, application logs on success, database URLs, object
  keys, credentials, or request bodies.
- Production cleanup never touches databases, R2 objects, migration files, certificates, application
  files, container writable layers, or volumes.

## Testing

Test-first implementation adds focused shell/Bats coverage for image-retention selection,
protection, idempotency, malformed Podman output, and fail-closed behavior. Deployment static tests
enforce cleanup placement, the absence of broad/volume prune commands, deterministic tool versions,
secret-mounted cache credentials, preserved cache scopes and release gates, ClamAV reuse/recreation
conditions, diagnostics redaction, and GHCR permissions/protection.

Dockerfile verification builds both runtime images, checks the configured UID and executable/migration
ownership, and compares image history to prove the recursive ownership layer is gone. Local tests do
not contact production or delete package versions.

The final verification follows `.rules` and `docs/TESTING.md`: focused tests, ShellCheck, shfmt,
installer Bats, deployment static tests, Podman Compose dry-run, actionlint, applicable frontend
checks, Docker builds, `git diff --check`, final diff review, and `git status --short`.

After review and merge, rollout observation uses the next ordinary backend deployments. It confirms
runtime before/after storage, retained tags, ClamAV action, timing artifact, sccache statistics,
readiness, migrations, authenticated smoke, and rollback availability. Production is not redeployed
solely to benchmark cache performance.

## Success Criteria

- Runtime diagnostics identify image, container, and volume consumption without exposing sensitive
  runtime data.
- Each backend retains at most three local SHA release tags after a successful deployment, while
  active, `latest`, and `rollback` image IDs remain available.
- The ClamAV signature volume is unchanged by every cleanup and scanner path.
- A healthy exact ClamAV runtime is reused; any relevant drift recreates only ClamAV and reaches
  healthy before backend-school replacement.
- Both runtime images run as UID 1000 with correctly owned application content and no recursive
  post-copy ownership layer.
- Rust 1.98.0, cargo-chef 0.1.78, and sccache 0.17.0 are deterministic and verified.
- Cache failure cannot skip compilation or fail an otherwise valid build.
- Cargo timing artifacts and bounded cache/deployment metrics are available for ordinary builds.
- GHCR retains at least the 30 newest SHA releases plus the current `latest` version of each backend,
  and unknown/attestation versions are left untouched.
- Every existing deployment gate remains present and ordered safely.

## Rollback

Each stage is independently revertible:

- remove diagnostics/timing output without affecting runtime;
- remove cleanup calls while leaving retained images untouched;
- restore the prior Dockerfile ownership layer if image verification finds incompatibility;
- restore unconditional ClamAV recreation without touching its named volume;
- remove sccache and secret mounts while retaining the ordinary Cargo build and BuildKit cache;
- disable or revert the GHCR maintenance workflow, after which no further versions are deleted.

No rollback edits an applied migration or restores application/database data. A backend image needed
for an application rollback is pulled by its retained GHCR SHA and deployed through the documented
procedure.
