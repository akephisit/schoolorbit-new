# Attendance reliability

Approved scope: repair parent provisioning, session conflict recovery, scan replay, bounded reports and mobile workflows. The user accepts losing unsaved inputs on explicit reload/session changes; no durable drafts or draft table.

Owners: school-students provisions users/roles transactionally; application handlers invalidate identity/permission caches before permission_changed. school-attendance owns scoped summaries and scan acceptance. Rust DTO/OpenAPI owns API shapes; generated TypeScript is consumed by attendance.ts and feature components. No new crate is warranted: all capabilities share existing domain lifecycles.

Add a forward migration creating the missing PARENT role and its attendance.read.own grant only when the role is newly created. Backfill linked active parents with no role history, preserving customized roles, inactive defaults and explicit ended assignments. Required roles must exist and match user type before creation commits. Existing accounts must be active parents; only accounts without any role history are repaired on reuse.

Session edits keep one in-memory baseline while editing. On 409 load current detail, merge nonoverlapping result/note edits, and require explicit choice for overlapping fields. Never automatically resubmit a mutation. Explicit reload replaces both detail and baseline; switching sessions still discards edits.

Scan replay authenticates operator/device/session, then reconciles an accepted event before fresh-capture/model checks. Bind event ID to immutable session/student/device/evidence/time. New events retain timestamp, enrollment and private-file checks and transactional uniqueness. Terminal errors release the pending capture; ambiguous failures retain the original event for retry. Accepted evidence cannot be deleted by cleanup.

Report API defaults to bounded pages with deterministic student/category/scope order and server-side search/category. Apply resource scope before filtering/pagination; aggregate complete facts before paging summaries. CSV export explicitly retrieves all matching pages with bounded requests, cancellation and progress, never only the visible page. Existing archived teacher-scoped semantics remain intact. Mobile presentation prioritizes name/status/counts without horizontal scrolling; desktop retains tables. Roster rendering is bounded while saving and mark-all continue to cover the entire roster.

Rollout: existing binaries can read the added role data; updated frontend/backend are deployed together for report pagination under Maintenance Mode. Preserve existing migrations. Recovery uses roll-forward fixes; source revert is safe for the additive role migration, but old frontend must not consume paginated reports. No changes to real teacher grants without verified intent.

Verification: focused student provisioning/migration and attendance database tests; pure merge and query tests; browser concurrency/retry/pagination/export/mobile workflows; generated contracts; ./scripts/pipeline verify --scope auto. Deployed authenticated read smoke remains separate from local fixtures.
