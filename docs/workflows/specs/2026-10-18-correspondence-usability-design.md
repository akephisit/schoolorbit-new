# Correspondence usability design

Approved scope: the user approved the complete correspondence UX proposal and instructed implementation on 2026-10-18. The coordinated implementation uses the existing Workflow, File Platform and generated API owners.

## User experience

- Show action required, sent, drafts and all views with authoritative server counts, the current step, frozen pending recipient names, due date and a concrete primary action. Search includes registration number/year independently of the source document number.
- Guide creation and sending through three steps: document details, recipients/process, review/send. Keep draft edits, warn before abandoning unsaved changes, and retain failed upload items for explicit retry through the existing upload owner.
- Allow multiple recipients with clear selection summaries. Present process history as a readable timeline and keep the main action usable on mobile.
- Place a signature directly on the document using the existing normalized document coordinate contract. Provide starter process presets and copying through the current versioned template owner.

## Owners and contract

The existing `school-workflow` correspondence service remains the authority for read scope, actions and persistence. The current HTTP/session/tenant boundary remains in place. This bounded extension shares that owner's lifecycle and does not justify a new crate, table, queue, worker or permission.

`CorrespondenceList.items` remains `CorrespondenceSummary[]`. Add `view=action_required|sent|drafts|all`, defaulting to `all`, and two response fields:

- `counts: {actionRequired, sent, drafts, all}` counts the complete authorized result after search/register/status filters and before view/cursor/page limits.
- `progress: CorrespondenceListProgress[]` has one entry per listed item: `correspondenceId`, nullable `currentStepIndex`, `stepCount`, nullable `currentStepTitle`, `pendingRecipients: {id,displayName}[]`, nullable `dueAt`, nullable `myAction`, and `canRevise`.

Progress derives only from persisted `ProcessInstance` snapshots. Active steps expose incomplete recipients and the step deadline falling back to the instance deadline. Inactive/terminal steps expose null current index/title/deadline and no pending recipients, retaining historical step count. `myAction` reuses existing available completion/close actions, excluding read/report/reassign-only access. `canRevise` requires the creator, create-own permission and draft/returned status.

Action required means an available completion/close action or an editable returned record. Sent means the actor created a non-draft record; drafts means the actor created a draft. Future recipients, completed recipients, reassigned recipients, read-only access and manager-only reassignment do not constitute action required. Every view and count remains inside the existing read scope. The list uses one SQL snapshot, bounded returned process projections and no per-row detail calls. SQL eligibility is checked against the existing action owner in focused tests.

A saved draft must remain editable throughout the wizard. Add optional `ReviseCorrespondenceRequest.metadata: CorrespondenceDraftMetadata` containing title, sender, nullable document number/date and urgency. Metadata uses create-time validation and may change only the creator's never-sent draft with create-own permission, in the existing revision/idempotency transaction. Registration number/year/kind, organization, ownership and creation time stay outside this input. Returned records retain immutable metadata. Omitted metadata preserves the previous revise behavior; omit absent metadata during serialization to preserve hashes of earlier idempotent requests.

## Data and rollout

No schema or migration changes. Existing summaries, process snapshots, frozen recipients, document revisions, signature evidence and registration counters retain their owners and meaning. Deploy matching backend/generated contracts/frontend through the normal coordinated maintenance release. Old binaries remain schema-compatible, but the new UI requires the expanded list response and draft-metadata handling; rollback must restore a matching frontend/backend pair while preserving the existing correspondence-aware File Platform runtime boundary.

## Acceptance

Verify complete counts beyond 30 rows, view/filter/cursor independence, read-scope denial, current/future/all/any/reassigned recipients, signature permissions, returned/terminal states, snapshot names/deadlines and old/new draft revision requests. Verify atomic metadata validation and denied paths. Generate/check API artifacts from Rust. Exercise wizard retention/upload failure, recipient selection, timeline/main action, signature placement and templates in focused frontend and desktop/mobile browser checks. Independent Reviewer and QA assess the integrated frozen candidate under `.rules`.
