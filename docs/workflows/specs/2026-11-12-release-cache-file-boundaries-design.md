# Release transport, cache export and Workflow boundaries

Approved scope: one immutable remote release bundle, an isolated full-cost BuildKit exporter experiment, and removal of the demonstrated File Platform → Workflow → Budget/Staff Leave implementation dependency. The Senior approved this direction in task state. Production caches remain unchanged until measured evidence supports a separate decision.

## Release ownership

The public, secret-free release preflight packages a fixed executable/template allowlist from its exact checkout. Its public SHA256 and release SHA identify one data-only archive. Private preflight reconstructs the same exact Git-blob archive and verifies that public digest; there is no additional Actions artifact transfer or private identity output. Private preflight validates the reconciled plan and observes the existing owned-cleanup interlock before its first host upload. One SCP replaces the private/Admin/maintenance/School repeated uploads. A trusted checkout bootstrap installs the archive inside the existing account-check SSH when Admin backend is selected, otherwise inside the existing maintenance-enter SSH. No scope adds an SSH connection. Charge both archive reconstructions, transfer and bootstrap verification to release latency alongside the removed SCP steps.

Published code lives at `/opt/stack/deployment/release-bundles/<sha>/<sha256>/`. Publication is exclusive and atomic; retries verify the complete existing bundle. No mutable code selector, arbitrary extraction, archive hook or caller-selected executable is introduced. Validate archive SHA, release identity, exact allowlist, file modes, sizes, content hashes, duplicates, traversal and link/special types before materializing regular files into private empty staging. Existing Python 3 is required explicitly, never installed during deployment.

Every coordinated release entrypoint and its helper/template reads use that exact code root. The six School jobs and staging → backend → promotion order remain intact. Runtime state stays with its existing owner: stack environment/CA/Compose/nginx and deployment pending/accepted markers, release probe tokens, storage namespace journals, cleanup and TLS/DNS state. Other operations and provisioning retain their existing shared-tree lifecycle; path helpers distinguish immutable code from shared runtime state without moving journals or changing their protocols. Secrets never enter bundles, caches or public outputs.

## Export experiment

Compare current runtime `mode=max` registry export with a dependency-target `mode=min` export followed by runtime compilation without application-graph export. Dependency `mode=max` is unsuitable because it also exports the changing planner source intermediate. Use committed experiment source through Operations, CPU4 Ubuntu runners, pinned BuildKit, fresh builders, isolated scratch refs, digest-pinned imports, identical frozen compiler-cache bytes and unchanged opt3/CGU16/final-bin-LTO-off settings. Preserve production cache selection and disabled target snapshots.

Measure producer export plus fresh consumer build, including candidate dependency solve/export, image export and declared transport costs. Counterbalance at least two source-edit and two workspace-edit pairs; distinguish seed, source and workspace costs. Retain phase/cache/error/network/size evidence and verify runtime binary, migrations, UID, command and canonical offline OpenAPI equivalence. Any build or evidence failure invalidates the comparison and propagates. Cleanup addresses only run-owned builders/images/package. No correctness tests are added to Actions.

## Backend boundary

The Backend writer owns one new `school-workflow-http` crate containing current HTTP transport and actual PDF rendering. Workflow core owns the document-validation port, signing-label semantics and transactional business operations; HTTP/rendering depends on core and File Platform. Budget and Staff Leave continue using core operations without inheriting renderer implementation. Preserve file version/checksum checks, permission and tenant boundaries, preflight/locked revalidation, PDF behavior and byte-identical API contracts. Other genuine File Platform consumers retain their edges. No migrations or product behavior changes are intended.

## Verification and delivery

Pipeline owns deployment/bootstrap/archive fixtures, exporter fixtures, deployment verification and operational docs. Backend owns Rust checks/fixtures, architecture and consumer inventories, API equivalence and repeated warm implementation-change measurements. Senior serializes Git, shared tools and heavy Rust work. Freeze the integrated candidate for independent review and QA. Actual migration/readiness/browser/proxy acceptance remains required on deployment; local fixtures do not certify production. Record completed outcome in the PR and retire these workflow artifacts after it records the implementation.
