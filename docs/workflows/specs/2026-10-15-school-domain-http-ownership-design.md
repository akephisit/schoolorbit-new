# Backend School domain ownership

The requested scope is a behavior-preserving reorganization of every production capability in
`backend-school/src/modules`, using existing domain owners before introducing crates.

## Before and after

| Current application module | Owner after migration |
| --- | --- |
| students, parents | school-students (separate parent module) |
| staff | school-staff |
| calendar | school-calendar, with delivery/scheduler dependencies injected where needed |
| admission | school-admission |
| supervision | school-supervision |
| question_bank | school-question-bank |
| attendance | school-attendance; application retains cross-domain background adapters |
| behavior | school-behavior; application injects Attendance integration |
| workflow, work | school-workflow |
| menu | school-navigation |
| notification | school-notifications |
| school_fonts | school-fonts |
| auth, consent | school-auth and school-auth-http |
| academic | existing academic domain crates and school-academic-http |
| certificates | existing school-certificates and school-certificates-http |
| files | school-file-platform and school-files-http for domain-aware transport |
| school, facility, supply_catalog, lookup | cohesive school administration owner, internally separated |
| system | system composition plus existing tenancy/navigation provider operations |
| achievement | school-staff, as a separate achievement module sharing staff_achievements persistence |

## Boundaries

Concrete `http/state.rs`, `http/routes.rs`, and `http/handlers.rs` belong beside domain code.
Existing dedicated HTTP crates remain separate when they compose multiple domain owners.
The executable retains startup, full AppState construction, FromRef state selection, global
prefixes, public/protected router composition, system middleware and cross-domain adapters.
No domain accepts full AppState or depends on the executable. AuthRuntime and AuthenticatedSession
remain canonical; school-auth-http alone resolves session/tenant request context and cookies.
Shared resource authorization primitives move to school-authorization; resource-specific policies
move with their owner. File infrastructure never depends on a feature that consumes it: provider
ports preserve authorization and relationship/deletion transactions across that boundary.

No migration, endpoint, permission, session, tenant-origin, CSRF, or wire-contract change is intended.
OpenAPI and generated TypeScript must remain identical. Existing application integration fixtures
may remain test-only while concrete production handlers move.

## Verification and evidence

Use pinned Rust 1.98.1 and the same host/profile/dependency state for baseline and final compile
measurements, including edits in main and each migrated category. Report Cargo check timings as
check timings, not release-build or deployment speed. Check dependency invalidation as well as wall
time. Run focused owning-crate checks after each category, denied/session/tenant integration tests,
normal and api-docs workspace checks, architecture guards, byte-identical API generation, and the
repository-owned `./scripts/pipeline verify --scope auto`. Delivery is through a feature-branch PR;
incomplete required checks keep it in draft. The stable Pipeline gate owns integration.
