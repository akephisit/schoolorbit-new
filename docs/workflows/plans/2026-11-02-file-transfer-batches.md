# Implement staff file-transfer batches

Design: [approved batch design](../specs/2026-11-02-file-transfer-batches-design.md). Base: `0bb5425d512ff755719befa4f2a315af9c5ab238`, shared branch `feat/lan-file-transfer-batches`.

1. Backend writer defines protocol 2, batch/limits/correlation DTOs and the narrow Staff admission policy. Preserve existing authentication, typed envelopes, signaling and lifecycle owners. Add focused batch, capacity, correlation, isolation and cleanup tests. The Coordinator regenerates OpenAPI/TypeScript after the writer returns the single Cargo slot.
2. Frontend writer consumes generated DTOs, implements one immutable ordered batch, sequential files per recipient, a three-slot recipient queue and per-pair connection/progress state. Improve the actual send/read/backpressure path within native browser WebRTC. Add functional consent, cancellation, stale-response and multi-recipient tests; stop all performance-measurement work.
3. Backend writer updates canonical smoke registration and snapshot validation, its real HTTP/SSE Node fixture, Bats fixture and Operations. Keep existing deployment wrappers, jobs and proxy configuration intact.
4. Writers provide scoped formatting and functional checks; pause for Coordinator checkpoints. The Coordinator freezes the complete candidate and assigns independent Rules/Security review and QA. Repairs return to the owning writer and affected checks repeat.

Planned backend checks: `cargo test -p school-staff file_transfer --lib --locked`; protected root file-transfer fixture through `scripts/test_backend_school.sh`; default and `api-docs` compilation and API-contract tests; contract regeneration/check under Coordinator ownership. Use the shared target with three build jobs, no incremental state and no debug information. Serialize Cargo and Docker lifecycle with browser work.

Planned script checks: owning ShellCheck/shfmt for `scripts/smoke_test.sh`, `node --test scripts/tests/pipeline-file-transfer-smoke.test.mjs`, and `bats scripts/tests/installer/orchestration.bats`. The existing deployed authenticated smoke performs its own temporary device registration, bounded typed ready check and cleanup.

Frontend selects necessary type/lint/static and native browser functional checks. Keep the 64 MiB per-file and 128 MiB batch limits explicit. Do not run quantitative performance benchmarks. Record actual results in chat/PR, then remove these workflow artifacts after the completed PR records the design and outcome; Git retains their history.
