# Component-aware delivery pipeline

Approved in chat on 2026-10-08. Every production deployment enters maintenance, including frontend-only and Admin. PRs from write collaborators auto-merge after CI without mandatory approval.

## Owners

Four public entry points own pipeline, merge, operations and retention. Reusable workflows have no independent push or PR triggers. Executable change policy separates verification, build and deployment. Components are School/Admin backend/frontend. Unknown inputs expand verification rather than silently skipping it. Automatic releases compare every component with its accepted baseline, including queued changes.

## Verification and preparation

Use the same commands locally and in CI; Docker owns local/CI fixtures and production retains rootless Podman and the sole production Compose file. Consolidate duplicated Rust workspace/architecture and frontend checks. Preserve API/permission artifacts, negative/tenant tests, database preservation/refusal and deployment guards. Build immutable OCI images and frontend bundles before maintenance. Keys include source, toolchain, profile and compiled public configuration. Main writes shared caches; PRs restore. Cargo cache snapshots include source evidence: only content-identical inputs have checkout timestamps normalized, while edits are forced dirty. Main backend owners execute fresh fixtures and refresh those snapshots; equivalent static/frontend PR evidence can be reused only with matching tree, tools, runner image and attempt. Cache restore is not verification evidence. Debug/test output cannot replace release output.

## Integration

Each developer owns a branch/worktree and opens a PR. One stable required gate validates the merged candidate against current main. A serialized trusted controller scans eligible collaborator PRs, updates stale bases and squash-merges passing candidates without approvals. External PRs are never auto-merged. No direct/force push to main; enable strict required checks where repository permissions allow.

## Release

Preparation and CI may run concurrently. A single production mutation lock covers School, Admin and provisioning. Under the lock, reread accepted state and reject older candidates. Maintenance begins only after selected builds and checks succeed. Deploy selected immutable images/Worker versions, run migrations and audits, validate readiness, tenant browser mount and authenticated proxy smoke, persist accepted baselines, then leave maintenance. Failure retains maintenance and recovery evidence; retries must not regress newer releases. Protect accepted, rollback and in-flight artifacts from retention.

Installer bootstrap respects Admin-before-School dependencies. Provisioning correlates exact request identifiers and consumes the accepted frontend artifact rather than rebuilding current main. Cloudflare production promotion has one owner.

## Transition and evidence

Remove automatic production writers before activating the new entry point. The first rollout deliberately reconciles all four components; legacy state cannot bypass the new acceptance gates. Update implementation-source guards rather than weakening invariants. Validate component scopes, cold/warm cache, two PRs, failures/retries/stale candidates and provisioning. Record actual CI/build/deploy/cache timings; do not promise speed percentages. Operational compatibility decisions and schema rollback restrictions remain unchanged.
