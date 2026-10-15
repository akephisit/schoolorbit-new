# Backend School HTTP ownership implementation

1. Inspect all modules, crate edges, policy/contract consumers and existing tests; collect baseline
   compile timings before modifying production source.
2. Move shared resource authorization primitives to school-authorization and lifecycle/error
   operations to their existing provider. Reuse AuthRuntime rather than creating shared AppState.
3. Move HTTP handlers, route factories and specific policies category by category into existing
   crates, with concrete narrow state. Check each owner before the next dependent migration.
4. Complete small related modules under cohesive owners; preserve Academic and Certificate splits.
   Inject application-owned cross-domain adapters for files, lifecycle, realtime and background work.
5. Rewire app.rs and FromRef construction, remove production facades, and update architecture and
   frontend source guards to follow actual owners without weakening behavior assertions.
6. Measure main/category invalidation on the same toolchain and configuration. Run focused tests,
   all applicable repository-owned verification, and final rules/security/diff review.
7. Refresh main, record actual checks and compile evidence in the PR, push the branch and attach
   the PR. Follow repository integration/acceptance workflow; do not claim unrun checks passed.
