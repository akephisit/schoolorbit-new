# Correspondence directed routing

Approved product scope: the clerk registers a book and selects one first decision-maker. That person records instructions and selects the next people, units or positions. A recipient can acknowledge or execute, and an authorized group lead can delegate to staff while retaining parent tracking. Review is optional and defaults off; a reviewer is explicit when review is required. Required review inherited from an ancestor cannot be removed by a descendant. The director is an eligible configurable recipient, never an implicit mandatory stage.

## Owners and compatibility

`school-workflow` owns correspondence and process state, resource policy, immutable action/document/signature evidence and book numbers. Extend its process provider with a typed directed graph, keeping existing sequential template processing intact. This is one lifecycle owner and requires a module, not a new crate. Authenticated handlers continue through the existing session/tenant context and typed API envelope. Central Work and Notifications remain the persistence/delivery providers.

Legacy instances, template versions, document revisions, file links, book counters, signatures, events and mutation receipts are retained. New optional persisted fields deserialize with defaults and omit absent values when serializing existing mutation payloads. Directed instances are created explicitly; the migration must never reinterpret an existing fixed template instance as a directed instance. Existing incoming/outgoing/internal books and fixed template choices remain available.

## Directed lifecycle

Every assignment has a stable identity, parent identity, resolved recipient snapshot, purpose, required/copy distinction, onward-assignment permission, completion state and work linkage. The first decision-maker is one resolved active staff account. A unit or position selector resolves to named staff snapshots when a routing decision is accepted; future retries deliver to those snapshots.

Routing records the sender's instructions and creates child branches in the same transaction as annotations, signatures, evidence, immutable history and the process revision. A parent retains its place in the graph while its children work. A branch finishes after all its required children finish, followed by every explicit required review. The root finishes automatically after the required graph finishes. A read timestamp is never completion. Copies are visible participants but are excluded from completion and acquire no onward-assignment or review authority.

An acknowledged or executed leaf can finish without returning to its sender when review is off. When review is on, its completion activates an explicit reviewer assignment. A descendant can add review but cannot erase any ancestor's review obligation, change the ancestor's reviewer, or weaken required completion to a copy. Reject/revision decisions remain explicit recorded outcomes. Onward routing requires a current pending assignment with the corresponding grant; record visibility or being copied is insufficient.

## Settings and selection

Tenant settings configure eligible first decision-makers and defaults in the actual register/type context. Incoming, outgoing and internal remain the existing record register vocabulary. Settings are versioned, revision-checked and replay-safe. Administration uses the existing process management capability; ordinary staff can read the selection settings needed for an authorized correspondence operation.

Selectors expose only active staff, active units and current position assignments. Temporary acting authority must come from current dated organization assignments or existing valid organization permission delegation; a copied reader or arbitrary role label is not authority. The selected actual person and selection context are preserved in the accepted snapshot. Missing, inactive, ambiguous first recipients or reviewers are rejected before durable routing side effects.

## Safety and verification

Use a new migration after 124. No applied migration, immutable evidence or book identity is edited. Mutations lock correspondence and process rows in their existing order, check revisions, claim stable request receipts and roll back all side effects together on validation failure. Read/list/action-required/progress/upload predicates use the same current graph authority as actions. Tenant identity remains the selected tenant pool supplied by the central context.

Focused database tests cover default automatic completion, read versus acknowledge, multi-branch waiting, copies, delegation/parent tracking, explicit and inherited review, unauthorized copied readers, inactive/ambiguous selectors, stale writes, identical/divergent replay, tenant separation and unchanged sequential templates/book histories. Build both default and `api-docs`, register typed schemas and handlers, generate OpenAPI and frontend contracts, and run the owning contract tests.
