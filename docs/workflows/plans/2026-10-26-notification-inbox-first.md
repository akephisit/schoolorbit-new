# Notification inbox-first implementation

Approved scope and ownership follow the companion specification. Senior owns Git, environment and shared generator/build scheduling; Backend owns backend/schema/docs; Frontend owns browser APIs, account lifecycle and service worker. One shared checkout on `codex/notification-inbox-first`.

1. Trace full migration history and source lifecycle contracts. Add sequential migration 139 with preservation/refusal gates, legacy suppression, durable one-shot presentation and versioned session-bound devices.
2. Implement typed idempotent transactional bulk acceptance. Cut over Attendance, Workflow, Student Care and Calendar mutations/reminders to the authoritative API and preserve source-owned reset/cancellation.
3. Replace transport lease retries with guarded irreversible presentation consumption before external I/O. Separate SSE from Push preparation failures, enforce freshness, bound endpoint concurrency and exact-version cleanup.
4. Expose generated authenticated device/config APIs; remove arbitrary notification creation and obsolete subscription route. Validate endpoint/crypto input, session ownership, key identity and stale result fences.
5. Update architecture rules, operating cutover/recovery and focused test recipes. Remove verified SEC-008 backlog item after endpoint removal.
6. Run owner unit checks, migration preservation/refusal and application DB contracts, static architecture and API generation/consumer checks with Senior scheduling. Freeze for separate Reviewer and QA; repair reported defects and repeat affected checks.

Focused evidence includes acceptance replay/conflict/rollback, concurrent consume and restart/finish uncertainty, cancelled/missing source or inbox, legacy no-resurrection, session/device ownership and exact-version cleanup, malformed/global VAPID classifications, bounded freshness and all-device admission, Attendance reset/progress and Calendar atomic reminder acceptance. Required Rust formatting/workspace/api-docs checks and root policy tests follow `.rules` section 11. Database fixtures use only owned disposable databases through the canonical runner.
