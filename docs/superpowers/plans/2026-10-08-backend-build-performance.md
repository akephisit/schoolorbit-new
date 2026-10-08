# Backend build performance implementation

1. Refresh origin/main and branch from the latest tree. Read .rules and canonical operations.
2. Run Docker benchmark jobs for both backends: GNU/lld, opt-level 2, 64 codegen units, CPU quotas,
   repeated source changes, unchanged persistent target, linker duration and binary size.
3. Prototype the exam scheduling assessment boundary. Retain existing service tests and compare
   source-changing build time/invalidation against the baseline on the same machine.
4. Inspect deployment phase timing. Apply only measured compiler, crate, or deployment gains.
5. Verify cargo fmt, focused assessment tests, static architecture, workspace all-target checks,
   API export equality and generator checks. For workflows run actionlint, shellcheck/shfmt,
   installer Bats, deployment static guards and Compose dry run using Docker session fixtures.
6. Create a PR with the measurements and limitations. Refresh the remote base before squash
   integration, verify the exact tested tree, and observe CI/deployment acceptance.
