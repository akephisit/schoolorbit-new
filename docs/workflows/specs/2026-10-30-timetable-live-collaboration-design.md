# Timetable live collaboration

The user approved live collaboration within the selected timetable draft: an avatar roster beside
the version title, names on hover, authorized context jumps, semantic cursors and drag ghosts,
exclusive source/edit/target reservations, and changed-block updates without normal workspace reloads.
Tray configuration remains private. PostgreSQL remains authoritative for actual writes and revisions.

## Owners and contract

`school-academic-timetable` owns the typed collaboration protocol, canonical draft cache, shared
visibility projection, ephemeral reservation authority, mutation permits and ordered journal.
`school-academic-http` owns authenticated sockets and HTTP adapters. Application composition creates
one shared runtime. Staff supplies bounded authorized portrait references through its public port;
Files retains its existing authorization and private delivery. No schema, migration, outbox or
additional process is introduced.

Draft membership is keyed by immutable tenant, term and version UUIDs. Legacy term-wide Academic
Core and Delivery signals retain their existing consumers. New messages use PascalCase type names
and camelCase payloads, with separate client/server enums. Connection and actor identity are bound
by the server. The protocol source is `models/collaboration.rs`; generated OpenAPI/TypeScript are
derived from it. Roster users aggregate independently owned connection IDs.

Contexts use the existing homeroom, teacher, learning_group and wholeSchool views. Anchors use
cell day/period, block identity or tray source plus normalized finite u/v coordinates. The browser
samples at 20–30 Hz and interpolates with animation frames against its own layout and scroll.

## Authority, privacy and lifecycle

Joining loads one canonical cache and an authorized recipient projection using the same visibility
owner as HTTP. Block and offering visibility preserve the existing whole-resource semantics.
Source, target and resource conflicts use canonical cached identities, teachers, homerooms, rooms
and overlapping periods, including both swap directions. Claims, retargets, presence, renewal,
release and replay use memory only. Retarget is atomic and retains the source on denial.

Reservations expire after a bounded TTL and belong to one server connection, including separate
tabs of the same user. Private lease tokens and candidates never appear in peer events. Recipient
availability projects blocked cells for the recipient's own active source/candidate without
exposing another editor's tray selections. Hidden blocks, contexts, sources and portraits are
excluded from recipient events. Unauthorized portraits use initials.

Every actual write accepts a mandatory domain mutation context. A per-draft async gate and pinned
commit permit cover transaction acceptance, commit, canonical cache patch and journal publication.
Accepted work runs in a bounded owned task so an HTTP disconnect cannot silently suppress its
committed delta. Database revisions, lifecycle guards and constraints are checked again on save.
All block writes, series/bulk operations, template application/clear, teacher handoff, source
changes, clone-resume, publication and deletion use this authority. Transaction helpers require
the outer permit. Operator cleanup proves its offline/cancelled eligibility explicitly.

Disconnect, permission/session revocation, draft switch, expiry and lifecycle invalidation remove
the affected ephemeral state. A committing permit is pinned until its operation completes; TTL
expiry cannot release its resources during a transaction. Source/upstream/lifecycle changes
invalidate the relevant cache and require authoritative reconciliation.

## Publication and recovery

The domain serializes sequence allocation, canonical state updates and bounded journal insertion.
Accepted changes are recorded even with zero or one subscriber. Normal patches contain changed
blocks/deletions, affected demand projections and summary, with version/source metadata as needed.
Each recipient receives its authorized projection, including an empty patch when needed to advance
the shared sequence. Failed transactions publish nothing; transport failure never changes a
successful committed result.

State sync includes the authorized workspace, roster, reservations, epoch and sequence, closing
the HTTP bootstrap race. Replay is bounded by count and bytes. Gaps, expired rooms and process
restart require a new authoritative snapshot; a new epoch prevents stale state being accepted.
The supported topology remains exactly one School backend process. Existing shared authentication
heartbeat/cache, fail-closed permission/session signals, hidden-tab shutdown and reconnect remain.

## Acceptance

Verify atomic first claim, disjoint claims, cross-view conflicts and both swap directions; separate
tab ownership, expiry/renewal/disconnect/revocation; stale tokens and forged events; scoped recipient
privacy and portrait authorization; direct HTTP conflict enforcement across every write owner;
pinned commits, rollback and HTTP cancellation; zero-subscriber journal/replay and gaps; unchanged
blocks retained by normal patches; and browser alignment under scrolling, resizing and sidebar
changes. Use disposable database fixtures, direct backend and proxy paths, and the owning protocol,
service, authentication and frontend checks recorded in the implementation plan.
