# Correspondence reading, register labels, and reusable text stamps

Status: Senior-approved within the user's requested scope; implement through coordinated owner assignments.
Scope: School correspondence, its My Work presentation, persisted settings, and document composition.
Base: `ec6436038f96f06e2374e45b0851181737174505`; shared branch `feat/correspondence-reader-templates`.

## Requested outcome

1. Configure register-number formatting separately for incoming, outgoing, and internal books.
2. Show the Thai source label `งานสารบรรณ` in My Work while retaining the `correspondence` wire identifier.
3. Provide an explicit open/read action in the register list and record each person's first successful read once.
4. Display the current document immediately; place historical revisions behind secondary controls.
5. Explain the initial empty draft and separate document revisions from workflow revisions without discarding evidence.
6. Put the document before workflow controls on mobile, with reachable actions and usable loading/error states.
7. Default new routing choices to required primary recipient, purpose `acknowledge` (เพื่อทราบ), onward allowed, and organization selector.
8. Provide reusable, editable, positioned text stamps for registration, directions, and signer captions.
9. Show errors beside their fields and focus/scroll to the first invalid field while preserving entered work.

## Existing owners and evidence

- `school-workflow` owns correspondence authorization, registration, immutable revisions, process/read participation, settings, and My Work projections.
- `correspondence.rs::create` requires an existing draft before resource-bound file uploads; revision 1 consequently has no original PDF.
- `CreateForm.svelte::persist` creates that draft, uploads once, then revises to attach file identities; `send` changes workflow state without creating another document revision.
- `correspondence_files.rs` owns relationships and retention; File Platform owns storage. Referencing an existing file in another revision does not upload another PDF.
- `document_export.rs::preview/export` reads frozen originals, verifies identities/checksums, and composes bytes in memory; generated output is not persisted.
- Each preview still loads detail and reads/parses its source; detail includes all revisions/history/signatures. These are code observations, not measured production costs.
- Migration 121 protects accepted document/file/signing evidence; migration 125 owns register-scoped routing profiles. Latest migration at design time is 126.
- Backend DTOs in `process_models.rs` and handlers in `http/process_handlers.rs` own the wire contract; `src/api_contract.rs` registers schemas.
- Existing correspondence components, API wrapper, region/mutation helpers, and `DocumentViewer.svelte` own the frontend flow. My Work owns its displayed source label.

## First-read contract and lifecycle

- Add `POST /api/correspondence/{id}/read` with `{ documentRevision: number }`.
- Return `CorrespondenceReadReceipt { correspondenceId, userId, documentRevision, firstReadAt }`; server supplies identity and time.
- Add nullable `myReadReceipt` to detail. Do not expand list contracts unless an agreed list behavior needs that data.
- Authorize through existing `require_read`, verify that revision belongs to the book, and use a ledger unique on `(correspondence_id, user_id)`.
- Insert once and return the stored receipt on repeat, including concurrent calls and calls for later revisions. No client request ID or expected workflow revision is required.
- Invoke after the first page actually renders successfully. Loading metadata, opening a route, failed rendering, and speculative preview requests do not count.
- GET detail/preview remain read-only. Reading never acknowledges, completes, forwards, signs, or creates assignment authority.
- A narrow process-owned transaction operation updates applicable active actor recipient/reviewer read projections and canonical My Work read state while preserving the earliest timestamp.
- Only rendering the locked current document revision projects active work as read. A valid historical render may freeze the original book receipt without marking newer content read; newly activated slots use their current read time.
- Existing `ProcessAction::Read` delegates to the same idempotent ownership path, retaining its assignment authorization and typed response contract.
- Repeated reads create no record revision, process snapshot/event, or history churn; the receipt remains the first successfully recorded revision.
- Preserve existing read audit rows. Backfill the earliest immutable explicit Read history only where book, actor, revision, and timestamp map unambiguously to valid rows.
- Unmappable or ambiguous historical Read rows fail migration/cutover closed with an aggregate finding; preserve audit and refuse silent skipping or fabricated later first reads.
- Migration 127 is tentative until implementation rechecks the migration sequence; use additive ledger/FK/index changes, not edits to applied migrations.

## Number formatting and settings

- Extend the existing typed per-register settings with `numbering: { format, minimumDigits }` and `textStampTemplates`.
- Default numbering is `{ format: '{number}/{year}', minimumDigits: 1 }`; the settings preview shows `1/2569` for that example.
- Accept bounded plain prefixes/suffixes and only `{number}` and `{year}` placeholders; require exactly one number token, reject unknown tokens/control characters, and bound minimum digits to 1–8.
- Preserve server-reserved numeric counters, Bangkok/Buddhist register year, uniqueness, and concurrent create behavior.
- Provide no manual number entry, counter advancement/reset, live next-number projection, or renumbering of existing books in this change.
- Freeze the formatted display label on each newly registered book; old records without a snapshot keep their original `number/year` label forever.
- Profile updates retain expected-revision checks, existing process-manager authorization, and accepted request replay behavior.
- Missing profiles default to `acknowledge` and onward true. Explicitly saved purpose/onward preferences and accepted process snapshots stay unchanged.
- Primary-required remains true; the frontend initially selects organization units while preserving explicit user choices during editing.

## Text stamps and source identity

- Persist bounded templates as `{ id, name, body, border }` in the same register profile; do not confuse them with routing/process templates or signature assets.
- Support multiline bodies and nine tokens: `{schoolName}`, `{registerNumber}`, `{date}`, `{time}`, `{registeredDate}`, `{registeredTime}`, `{signerName}`, `{signerTitle}`, `{instruction}`.
- Provide three editable starter presets: school/receipt number/registered date/time; directive with editable instruction and actor identity/current date; `ทราบ` acknowledgment caption with identity/current date.
- Use the book's frozen formatted label and actual school identity from the existing branding provider; expose narrow context with `actorDisplayName` and `signerTitle`.
- Resolve title from an explicit current `position_title` or a valid known Thai position label; never show raw position codes or authorization roles as formal titles. Unknown titles remain blank and require editable input when used.
- Never hardcode example people/schools, invent titles, or change previously accepted signing-evidence identity semantics.
- Date/time use Bangkok-local presentation: `{date}`/`{time}` are insertion/action time, while `{registeredDate}`/`{registeredTime}` derive from the book's creation time. Require editable instruction input before expanding `{instruction}`.
- Resolve tokens into editable concrete text before acceptance; later template/profile changes never rewrite accepted evidence or imply workflow completion.
- A saved template body is content, not executable interpolation. Reject unsupported tokens and oversized input; render ordinary text without HTML interpretation.
- Instantiate the existing `kind: 'text'` annotation with a normalized page box; users can edit text, move, and resize before acceptance. Templates need no separate placement schema; the accepted annotation persists its actual page/box.
- Add optional annotation `border` defaulting false; frontend SVG and backend PDF composition draw the same rectangle in canonical page coordinates.
- Reuse existing Thai font, text fitting, crop/rotation geometry, annotation limits, and persisted action/revision boundaries.
- Caption text remains editable annotation content. Actual signing continues to derive authoritative signer identity, consent, revision, and hash through the existing signing owner.
- New optional request fields and false/default annotation fields omit absent/default values during serialization so pre-upgrade receipt payload hashes remain reproducible.

## Reader and validation behavior

- Default selection is `detail.document`; historical content remains available with clear revision/date labels, including an honest initial draft/no-file explanation.
- Auto-render once per book/document identity; preserve cancellation, stale-response protection, private-file authorization, and cleanup on navigation/revision changes.
- On mobile, render document/loading/error/retry before long metadata, history, or action forms; avoid compulsory full-width sidebars before the document.
- Keep explicit acknowledgment/action controls distinct from the recorded read indicator. A read failure must not discard annotations or block manual completion actions.
- Expose clear list open/read affordances without implying that a completed read is an acknowledgment.
- Share typed field validation across create, settings, routing, stamps, and action forms where their owners already share semantics.
- Preserve error summaries, add field associations and `aria-invalid`, focus after the relevant section becomes visible, and scroll the first invalid field into view.
- Retain selected files, pending mutation identities, unsent annotation drafts, signature consent, and server conflict/retry behavior.

## Acceptance and rollout

- Verify read concurrency/replay/denial and read-versus-acknowledgment; preserved counters/old labels; settings defaults/versioning; template/token/border/Thai geometry; old receipt hashes.
- Exercise desktop/mobile current-document opening, historical selection, failed/cancelled rendering, field focus, template editing, and unchanged signing/route workflows.
- Use the focused owners and canonical recipes in [Testing](../../TESTING.md#correspondence-and-reusable-processes) and [API contracts](../../TESTING.md#api-contract).
- Deploy migration/backend/generated frontend together under existing maintenance procedures; no new workers, notification transport, or storage derivative pipeline is introduced.
- After formatted labels, receipts, or bordered annotations are accepted, recover by rolling forward to a compatible runtime; old binaries may lose new JSON fields when rewriting summaries/profiles.
- Preserve all audit/file/process evidence. Record the completed outcome in the implementation PR and remove these temporary workflow artifacts at the existing completion gate.
