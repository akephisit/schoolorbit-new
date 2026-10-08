# Backend build performance experiments

The user authorized experimenting with all proposed backend build improvements and refreshing
GitHub first. Base: `c1134a68`, including the unrelated timetable PDF fix. Use Docker for session
experiments as explicitly requested. Production remains in the existing canonical topology.

Measure identical source-changing release builds on pinned Rust 1.98.1, distinguishing Cargo,
link, binary size, cache transport, persistent target reuse, CPU quota, and total release phases.
Compare GNU, lld, application-only opt-level 2, application-only 64 codegen units, and 1 versus
2 CPUs. Preserve optimized dependency settings and compare OpenAPI output exactly. Benchmark
jobs never publish images or deploy. Paid/hosted runner provisioning is not available through
the current integration; quota experiments measure scaling without changing account settings.

Prototype moving exam scheduling models and services into the existing assessment crate. This
is one cohesive capability that currently lives in the application binary, already depends only
on core/error contracts, and receives PostgreSQL transactions rather than full AppState. Keep
HTTP handlers, router, authorization adapters, OpenAPI composition, and lifecycle adapters in
the application. Preserve operations, errors, transactions, schemas, and existing integration
tests. Compare compile invalidation before and after on the same runner and Cargo state.

Adopt only changes supported by repeatable build evidence and verification. Do not lower
production optimization based only on compile speed; runtime correctness and a representative
performance comparison are necessary. Keep migration/readiness/smoke/origin/acceptance gates.
Use bounded deployment phase logs to locate remaining waits rather than shorten readiness
timeouts speculatively. Record results and unselected experiments in the PR and canonical
verification/operations documentation, then remove these temporary workflow artifacts.

Recovery: revert selected compiler flags or the source relocation; no database migration,
runtime environment, external runner, or dependency-version change is required.
