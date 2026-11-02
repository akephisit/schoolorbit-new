# Direct file transfer streaming implementation plan

Approved design: [streaming and archive download](../specs/2026-11-02-file-transfer-streaming-design.md).
Base: `72664ddc2e641ef7434c0c1c88b66ac38c0a792c`; shared branch `feat/lan-file-transfer-streaming`.

## Working agreement and order

The Senior coordinates and owns Git, checkpoints, generators and final delivery. Backend and Frontend write only their assigned disjoint owners. Do not stage, commit, switch branch, overwrite another writer, run broad formatters or start concurrent Cargo processes. Design and plan are written before product changes; pause for Senior inspection before implementation. User-authorized scope and current `.rules` govern execution.

1. Backend establishes the authoritative protocol 3 DTOs and close lifecycle, adds focused regression coverage, and releases Cargo/generation dependencies to the Senior.
2. Frontend implements peer metadata, disk receiving, transport consent and archive/UI flows against the approved contract; dependency installation and package/lock changes stay with Frontend.
3. Senior regenerates shared API artifacts once the backend source is stable. Writers reconcile their consumers and focused checks.
4. Complete backend fixtures and all Docker lifecycle operations before native browser execution. Freeze source, obtain independent Rules/Security review and QA, then assign any repairs to the owning writer and repeat affected checks.

## Backend implementation

- [ ] Replace `FileTransferFile` and `files` with strict `FileTransferBatchSummary`/`batch`; validate nonzero safe count and safe total, advertise only six outgoing transfers, require protocol 3 and increase control queue to 32.
- [ ] Keep mutex admission, exact session/device/tenant ownership, exclusive incoming, host-only SDP, existing timeouts, code throttles and create-correlation behavior.
- [ ] Add bounded 1024-entry/120-second closed-pair receipts, verify owned device before receipt access, and route every removal through `remove_pair(id, now)` with deterministic time propagation. Preserve 404 for unknown and unauthorized callers and preserve acceptance/signaling refusal after close.
- [ ] Translate touched domain messages into actionable Thai without changing shared HTTP error envelopes.
- [ ] Update typed endpoint descriptions, root OpenAPI schema registration/contract tests and protected HTTP integration fixture. Do not edit generated files.
- [ ] Update `scripts/smoke_test.sh`, Node smoke tests, Bats orchestration snapshots and `docs/OPERATIONS.md` to protocol 3, summary privacy, six admission slots, queue size and bounded close receipts.

Backend owned paths: `backend-school/crates/school-staff/src/file_transfer/{models,runtime,http,tests}.rs`, `backend-school/src/api_contract.rs`, `backend-school/src/integration_tests/staff/file_transfer_integration_tests.rs`, the three smoke owners above and Operations. Root composition changes outside this map require Senior coordination.

## Frontend implementation

- [ ] Consume generated protocol 3 batch/limits DTOs, preserve unknown-create reservations and identity fences, admit six outgoing peers and leave the seventh queued.
- [ ] Implement ordered `schoolorbit-files-v3` metadata exchange with sixteen-entry acknowledgement windows and 4 KiB frames. Require list-preview consent, seal/count/total verification, then explicit full-batch consent before binary.
- [ ] Implement OPFS manifest/index and completed-prefix storage, paged list reads, session ownership, Web Lock/stale cleanup protection, unsupported receiving guidance, quota handling and async disposal.
- [ ] Implement disk-backed flow control: 1 MiB credit/native windows, 256 KiB awaited persisted acknowledgements/low-water mark, 16 KiB frames and bounded source reads. Preserve completed files on partial transfer failure/local stop and dispose owned data on identity/unmount cleanup.
- [ ] Add sampled decimal MB/s from persisted bytes and monotonic elapsed time; pause the transfer idle watchdog during bounded explicit list review.
- [ ] Install only approved `@zip.js/zip.js` 2.24.0, stream stored ZIP64 entries to an OPFS archive, expose one download and retain individual downloads; clean partial archives on cancellation/error and explain additional quota.
- [ ] Update helper/session/storage/archive and native browser fixtures plus `docs/TESTING.md`. Preserve 64-job history and existing correlation/cancellation/retry coverage.

Frontend owns its API/page/tools/tests, package/lock and Testing. Senior owns generated OpenAPI and TypeScript; no handwritten replacement wire schema.

## Verification and regression criteria

- [ ] Reproduce the old peer-close race in focused runtime coverage; prove both close orders and simultaneous closes succeed for owned members. Verify same-tenant unrelated device, wrong session, foreign tenant, missing device and unknown pair all remain 404; acceptance/signaling after close remain refused.
- [ ] Verify receipt expiry and FIFO capacity eviction, pending expiry and teardown receipts, six pending/accepted admissions under concurrent attempts, exclusive incoming, seventh refusal, 19-event undrained delivery and overflow cancellation. Preserve active request correlation and unknown-create semantics.
- [ ] Run `cargo test --manifest-path backend-school/Cargo.toml -p school-staff file_transfer --lib --locked`, protected `./scripts/test_backend_school.sh file_transfer_http -- --test-threads=1`, affected hint/auth checks, and root API contract tests with `api-docs`. Expand the existing focused runtime/API cases for the approved behavior change rather than removing denied coverage.
- [ ] Senior runs API generation/check/tests; compile affected owners in default and `api-docs` modes. Use `PATH=/workspace/.schoolorbit-tools/bin:/workspace/.schoolorbit-tools/node/bin:/home/agent/.cargo/bin:$PATH`, `CARGO_BUILD_JOBS=3`, `CARGO_INCREMENTAL=0`, `CARGO_PROFILE_DEV_DEBUG=0`, `CARGO_PROFILE_TEST_DEBUG=0`; one Cargo owner at a time.
- [ ] Run `node --test scripts/tests/pipeline-file-transfer-smoke.test.mjs`, `bats scripts/tests/installer/orchestration.bats`, scoped Rust formatting and applicable shell lint/format checks. Run documentation policy and `git diff --check` under Senior coordination.
- [ ] Run frontend type/lint/helper checks and native browser correctness cases from `docs/TESTING.md`; verify exact hashes/ZIP entries, many-file manifest paging, zero-byte and duplicate names, consent before binary, six independent peers/seventh queued, failure/partial retention, quota and cleanup, and actual close responses. These are correctness checks, not throughput benchmarks.
- [ ] Senior freezes the complete candidate; independent Reviewer and QA report exact inputs, checks, failures and limitations. Writers repair findings; re-freeze and repeat affected review/checks before readiness and authorized integration/deployment.

No database, migration, native Rust transfer, TURN/STUN/relay or unrelated dependency work is included. Record check results in chat/PR, not a new completion document. Remove these workflow artifacts after the implementation PR records the completed outcome, following `.rules`.
