# Attendance

The user approved implementation of the complete design in chat on 2026-10-09.

One attendance owner supports arrival, flag ceremony, dated lessons and named special rounds. Results are unchecked, present, late, absent, leave or activity. Teachers save directly; no draft/confirmation lifecycle is exposed. Saving a nonempty group converts remaining unchecked students to explicit inferred absences; scans never do so. Cancelled occurrences are excluded, and arrival never proves class participation.

Term settings own processing weekdays, date overrides, late cutoff, teacher digest times and evidence retention. Uncounted days remain writable and individually notify; processing/digests use the configured dates. Special rounds own mutually exclusive student assignments, teacher assignments and reusable audience groups. Membership is frozen per occurrence using date-effective enrollment providers.

Committed changes create durable per-recipient notification work. Daily arrival/flag initial notices share a deduplication key; changed results use correction revisions. Lessons and special rounds have independent keys. Active linked guardians all receive notices. Provider deliveries are retryable; in-app storage and notification identity are idempotent. Private evidence is read through current student/guardian/assigned-teacher authorization, never exposed in push payloads.

A supervised browser kiosk processes webcam images locally, loads only authorized enrollment descriptors, submits a recognized descriptor and one private evidence image, and acknowledges only committed server results. Models and descriptors are versioned, descriptors encrypted at rest, enrollment separately permissioned, and manual attendance remains available. This is supervised face matching, not a claim of spoof-proof liveness. Camera/lighting accuracy needs field acceptance.

Closed-term purge freezes separate per-student category/offering summaries, validates record totals, blocks further attendance writes and processing-setting changes, and durably requests evidence deletion in the same transaction before removing details. Summary and purge receipt survive. Retention can delete evidence independently. No production data is purged by this implementation task.

Architecture admission: a school-attendance domain crate owns models, pure rules, persistence and policy, following school-calendar's existing satellite boundary. Root owns thin HTTP, dated academic adapters, file-policy integration and the all-tenant scheduler. Dependencies point at foundation/provider crates only. A second HTTP crate is deferred because this new capability has no existing before/after split benchmark; no compilation improvement is claimed. Capture focused and workspace compile timings on the same installed toolchain.

The additive migration is safe for older binaries, which ignore attendance. New writes require compatible attendance binaries. Generated permissions and OpenAPI are mandatory. Verification includes pure rules, disposable PostgreSQL concurrency/auth/notification/purge cases, Svelte checks, generated-contract checks, central pipeline and intercepted browser workflow tests. Production proxy smoke and physical-camera acceptance are separately reported if credentials/hardware are unavailable.
