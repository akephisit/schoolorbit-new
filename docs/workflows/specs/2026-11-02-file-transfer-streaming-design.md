# Direct file transfer streaming and archive download

Date: 2026-11-02. Approved scope: Senior agreement within the user's requested implementation.
Base: `72664ddc2e641ef7434c0c1c88b66ac38c0a792c` on `feat/lan-file-transfer-streaming`.

## Outcome and boundaries

Staff can send large files and many files to more recipients, inspect the incoming list before accepting file bytes, see transfer rates in MB/s, and download received files together as one ZIP. Remove the existing twenty-file, 64 MiB individual and 128 MiB batch business limits. Actual browser memory, storage quota, integer representation and direct network connectivity remain physical constraints and must be explained truthfully in Thai.

Keep the existing authenticated same-school discovery, session-bound device identity, explicit consent, host-only WebRTC signaling, SSE lifecycle and single backend process. Do not add STUN, TURN, coturn, a relay, server file storage, database state, native Rust transfer code or performance benchmarks. File contents and the ordered manifest travel only between browsers. This is an architectural extension of the existing `school-staff::file_transfer` owner, not a new service or crate.

## Verified cause of close errors

The current runtime removes a pair on the first close and requires that live pair for every subsequent close. The initiating browser closes on completion; a completed receiving browser can also close when its channel closes. The second authorized request therefore receives 404 after successful transfer. Fix this at the runtime lifecycle owner rather than treating all frontend 404 responses as success.

## Backend contract and admission

Protocol version is 3. `RegisterFileTransferDevice.protocolVersion` remains optional for typed deserialization, but registration accepts only `Some(3)` and otherwise returns the existing Thai refresh guidance with 400. There is no legacy bridge.

Rust owns `FileTransferBatchSummary { file_count: u64, total_bytes: u64 }`, serialized as `{"fileCount": ..., "totalBytes": ...}` using camelCase and rejecting unknown input fields. `FileTransferPair` and `CreateFileTransferPair` replace `files` with required `batch: FileTransferBatchSummary`. Require `1 <= fileCount <= 9007199254740991` and `0 <= totalBytes <= 9007199254740991`. These are JavaScript representability bounds, not content or batch business caps. Remove `FileTransferFile`, server file validators and their schema registrations; the browser owns peer file metadata. Keep `clientRequestId`, target selection, pair identity and existing active-correlation semantics. Unknown create outcomes still reserve admission until authoritative cleanup; do not add automatic mutation retries or create replay receipts.

`FileTransferLimits` contains only `maxOutgoingTransfers: 6`. Under the existing mutex, pending and accepted pairs both count toward six outgoing transfers, or a device may reserve one exclusive incoming pair and cannot send concurrently. Remaining recipient jobs stay queued in the browser. Existing device bounds, presence coalescing, pairing-code throttling and timeouts remain. Increase the bounded control queue to 32: an undrained normal sender burst needs 19 entries (ready, six pending, six accepted, six answers). Overflow still invalidates the device and cancels every associated pair. The accepted ten-minute deadline continues to bound signaling, not the lifetime of an established file transfer.

## Authorized idempotent close

Use one `Registry::remove_pair(id, now)` owner for all pair removal. Explicit close, pending expiry, device removal, SSE drop, revocation and overflow must pass through it. Record a RAM-only receipt containing pair ID, tenant ID, initiating device ID, receiving device ID and an expiry 120 seconds after removal. Retain at most 1024 receipts with deterministic oldest-first eviction and expiry cleanup; never retain names, manifest entries, SDP or file bytes.

Close first verifies that the caller owns the exact live session-bound device. A live pair then requires normal tenant and device membership. A removed pair can return success only for a matching unexpired receipt with the same tenant and member device. Unknown pairs, foreign tenants, sessions and devices retain the same 404 behavior. Receipts do not resurrect pairs, authorize signaling or acceptance, or replay create operations. An expired or evicted receipt can again return 404. Pass operation time through dispatch and teardown for deterministic lifecycle checks; stream guard teardown uses current UTC.

Domain errors touched by this feature use actionable Thai messages. Preserve shared HTTP envelope semantics and generic transport error identifiers. Rust DTOs and endpoint declarations remain authoritative; the designated generator owner regenerates OpenAPI and TypeScript, and frontend consumers use those generated types.

## Peer protocol and consent

Use an ordered reliable `schoolorbit-files-v3` data channel. The recipient's initial “ดูรายการไฟล์” action accepts the backend pairing and permits metadata exchange only. Stream `manifest_start`, individual file metadata and `manifest_end` frames. Each metadata frame is at most 4 KiB; at most sixteen metadata entries are outstanding. Use cumulative acknowledgement and `manifest_ready` after the receiver has persisted and sealed the complete manifest and verified its count and total against the backend summary.

Only after the complete manifest is available can the recipient explicitly choose “รับไฟล์ทั้งชุด”, sending `receive_ready`. No binary file bytes precede that consent. Pause the normal thirty-second idle watchdog while the user reviews the manifest and use a bounded consent wait. Reject malformed order, totals, file metadata and protocol state through the transport owner. Preserve existing correlation, uncertain-response, cancellation, identity-generation and bounded-history fences.

## Browser storage, flow control and lifecycle

The receiver persists the manifest in OPFS with an eight-byte offset index and a completed-file prefix. Render approximately fifty manifest entries per page rather than retaining the full receiving list in UI memory. Namespace storage by the owning session, use Web Locks, and acquire a stale-collection lock with `ifAvailable` so cleanup cannot remove another live tab's data. Feature-detect the receiving APIs and show Thai unsupported guidance while keeping sending usable.

Binary flow uses a 1 MiB credit window, cumulative persisted acknowledgements after awaited 256 KiB writes and file boundaries, a native data-channel window of 1 MiB with a 256 KiB low-water mark, 16 KiB frames and at most 1 MiB source reads. Sender progress and rate reflect remotely persisted bytes; receiver progress reflects disk-accepted bytes. Sample monotonic time at about 500 ms and display decimal MB/s. Correctness checks must not claim benchmark throughput.

The session retains completed received files through partial network failure or a local stop. Authentication disposal, unmount and explicit disposal remove owned files, URLs and partial archives. Preserve the existing bounded history of 64 jobs. Quota and write errors release transport resources, expose actionable Thai feedback and preserve already completed files where the session remains valid.

## One ZIP download

Use frontend-only `@zip.js/zip.js` version 2.24.0 to produce a ZIP64 archive with stored entries (no compression). Stream file contents from OPFS into an OPFS archive, then expose one `getFile()` object URL for a single browser download. Keep individual downloads available. Archive cancellation and quota errors remove the partial archive without losing completed received files. Explain that the ZIP can require additional disk quota. The library's central-directory metadata grows with file count; this physical browser resource bound is accepted. Do not handwrite ZIP or CRC code to force constant metadata memory.

## Ownership, rollout and verification

Backend writer owns Rust models/runtime/HTTP/tests, root schema registration and integration fixture, canonical smoke and its Node/Bats fixtures, and `docs/OPERATIONS.md`. Frontend writer owns API consumers, page/session/transport/storage/archive helpers and tests, frontend package/lock and `docs/TESTING.md`. The Senior owns Git, shared generators and integration; generated OpenAPI/TypeScript have a single designated owner. No migration or permission changes are required. Deploy matching backend, generated contracts and frontend together; refresh both browsers after cutover. Restart clears ephemeral discovery and receipts, requiring explicit re-pairing.

Acceptance covers safe numeric summary validation, six admitted recipients and a seventh queued, exclusive incoming admission, normal 19-event bursts, overflow teardown, authorized close races and bounded receipts, strict unknown/foreign refusal, and unchanged create-correlation behavior. Browser acceptance covers manifest review and consent before binary, disk flow control and quota failure, large and many-file correctness, rates, one-click ZIP integrity, partial failure retention and cleanup. Use existing focused backend HTTP/auth, API-contract, smoke Node/Bats, frontend helper/type/lint and native browser fixtures. Finish disposable Docker fixtures before browser execution; use one Cargo owner with three jobs and debug/incremental disabled. Independent Reviewer and QA assess the frozen integrated candidate.
