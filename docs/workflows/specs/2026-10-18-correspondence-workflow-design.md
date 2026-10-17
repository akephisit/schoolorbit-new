# Correspondence and reusable processes

Approved phase-one scope: users register actual incoming, outgoing or internal correspondence, attach private documents, edit a draft annotation layer and send it through a versioned sequential process. Runtime steps and canonical My Work assignments are created automatically. Existing windows and legacy work remain functional; any windows created by the engine are internal projections, never a user prerequisite.

## Ownership and dependency direction

`school-workflow` owns cohesive process, correspondence, document revision, annotation and signing-evidence modules. Its existing work lifecycle and new runtime lifecycle have the same owner, so a separate crate would add public plumbing without reducing invalidation of its consumers. The provider exposes typed transaction-aware start/action/completion operations. Other domains retain source validation and use application composition adapters; the engine contains no correspondence lifecycle decisions. Source identity is typed module/resource identity with replay protection.

File Platform remains the only storage/lifecycle owner. Correspondence document/signature purposes are private, scanned, resource-authorized and attached under the same transaction as domain records. Attached files cannot be generically deleted. The composition-bound Files policy delegates to Workflow; Workflow does not depend on Files HTTP. Signature assets are owner-only; signed evidence references the immutable private image or drawn strokes without public URLs or base64 storage.

## Process semantics

Templates are versioned; runtime instances snapshot steps and resolved active recipients. A template edit never mutates accepted instances. Steps use acknowledge, review, approve, execute and sign, all/any completion policies, relative due days, explicit runtime deadlines and optional comment/evidence/signature/delegated-authority requirements. Recipient selectors resolve direct users, exact organization units and exact unit positions without inferred inheritance. Every required selector must resolve before sending; membership changes do not rewrite accepted recipients.

Read is distinct from acknowledgement. Reject is terminal; return-for-revision pauses the instance and creates a source revision opportunity. Report, reassignment, review and close append history. Row locking, monotonic resource revision and UUID idempotency with payload hash prevent stale overwrite and divergent key replay. Actions are a closed typed set. Optional delegated authority uses current explicit grants/delegations, never assumed hierarchy.

Existing work items, counts and realtime remain the inbox projection. Active process steps create direct per-user work assignments; completion closes their projection. Persisted notifications use `school-notifications` producer-resolved recipient snapshot API in the acceptance transaction; no independent dispatch queue.

## Correspondence and signing

Registers preserve sender/date/number/urgency, source links, original/attachments and immutable content revisions. Creator, participant and exact authorized manager scopes combine as a union. Every signature requires an explicit action bound to document revision, immutable original/content/annotation hash, actor, timestamp and method. Signature and action evidence are append-only; edits create a new revision while retaining earlier signatures/history. Annotation coordinates use normalized page positions and typed text/ink/highlight/circle layers; edits remain drafts until a revision is committed. Approval may carry a signature independently of step kind.

Exports are server-authoritative PDFs assembled from private original PDF/image content and persisted committed annotations/signatures. Export never accepts a client-rendered final. Unicode text uses an embedded compatible font when required; unavailable rendering must fail explicitly rather than silently corrupt Thai. This is electronic signing evidence, not PKI or certificate-backed digital signing.

## Rollout and verification

The migration is additive after 120 and preserves all existing windows/work. Previous binaries remain schema-compatible until feature use; after use, roll forward to retain file-purpose and process ownership. Deploy backend, permission/API contracts and frontend together under existing maintenance and migration procedures. Phase two excludes external webhooks, visual drag-flow and PKI.

Focused pure and disposable PostgreSQL tests cover recipient resolution/snapshots, all/any progression, allowed/denied scope union, rollback, stale/concurrent/replay decisions, source-provider reuse, immutable revisions/signatures and file retention. Both default and api-docs feature modes plus generated API/permission contracts and architecture checks are required; frontend and independent final review/QA are separate owners.
