# Automatic LAN readiness and one-click receipt

Date: 2026-11-02. Approved scope: the user's revised flow and Senior working agreement.
Base: `c59d7db8a9dbd48887be4b1d10ed830828580443`, branch `fix/lan-file-transfer-auto-ready`.

## Outcome and existing owners

Entering the staff file-transfer page automatically makes the authenticated device ready. The sender selects files and recipients and presses Send. An incoming request shows the sender, file count and total size. Its primary Receive action authorizes that exact immutable batch once and, in folder mode, opens the directory picker once. The existing manifest preview may remain a secondary action; it never authorizes binary file bytes by itself.

`FileTransferSession` remains the owner of discovery identity, admission, jobs, captured source files, consent and completed downloads. The page composes its typed operations and owns the unsent input/recipient draft. `FileTransferTransport` owns the bounded peer protocol; `ReceivedBatchStore` owns sealed metadata, destination writes and cleanup. Existing auth, visibility-idle and API owners remain authoritative. Keep protocol 3, generated DTOs, endpoints, backend StreamGuard cleanup, six-peer admission, disk credit and ZIP behavior unchanged. Add no relay, STUN/TURN, service, database or dependency.

## Readiness and connection recovery

The current pause callback calls `disconnect`, which stops the visibility subscription, so it cannot resume. The same disconnect clears the source batch, and the page clears File objects whenever discovery identity changes. Separate connection loss from feature lifetime and authenticated draft ownership.

Mount arms automatic readiness after the layout's existing auth result settles; it must not duplicate the initial current-user request. Use the existing valid default label, `อุปกรณ์ของฉัน`, with the existing staff display name/device suffix. No hardware hostname or additional profile/session lookup is needed. Remove the initial Open transfer action.

Keep `createVisibilityIdle` subscribed for the mounted lifetime. After sixty seconds hidden, pause the connection, retire interrupted jobs and close the known device; do not stop the listener. On visible resume, or an online/focus wake while recovery is pending, reconcile through `authAPI.refreshCurrentUser({silent:true,invalidate:true})` and safe HTTP device reads. The notification owner's generation-fenced recovery is the representative pattern. Prevent duplicate recovery tasks and timers, and use bounded backoff with jitter for safe GET/auth recovery. Hidden/offline conditions defer work; disposal cancels future work.

Backend stream loss removes the device and pairs immediately and supplies no replay. Recover as a fresh owned device, with a new connection generation. Cancel interrupted and queued jobs truthfully, invalidate their consent/picker operations and retire their correlations. Never automatically resend a batch or resume old file consent. Preserve unsent File objects, the captured source draft and still-valid exact recipient selections within the same authenticated identity. Device-ID rotation clears QR resources, not the file draft. Completed files keep their existing retained-owner lifetime.

Registration is a non-idempotent POST without a nonce. Initial connection and each recovery episode permit one fresh registration operation after safe reconciliation/known-device cleanup. Do not automatically retry an ambiguous registration network failure or unknown server failure. Preserve the draft and expose a truthful explicit retry action. A known fresh device whose stream cannot become ready also ends that episode instead of creating devices in a loop. Safe GET/auth retries and definitive pre-write refusals must not be confused with ambiguous creation. This automatically handles ordinary known-device SSE loss while refusing unsafe automatic POST replay.

Use the central auth store's session epoch to distinguish identity change from temporary unavailability. Block network mutations and future exports while auth cannot be verified; retained same-identity files remain private pending reconciliation. Revocation, account/session replacement, route exit and disposal invalidate callbacks and clear owned temporary data and URLs. Completed files already saved to the chosen user directory remain on the device. The feature must not introduce another auth store or auth retry queue.

## One Receive intent

Expose a typed primary receive operation for pending requests and reviewed manifests. Keep save-mode choice visible before Receive. Capture pair ID, request ID, immutable batch summary, device/connection generation, auth epoch and a unique consent-operation token. Invoke the folder picker directly in the original click stack before any await. Picker cancellation or denial leaves the request unaccepted, with explicit browser-download fallback available.

After the selected destination result is still current, open the metadata store and accept the backend pair through the existing metadata-consent path. Hold the fenced destination result as private session intent. The bounded peer manifest must validate and seal against the approved count/total before destination configuration. `ReceivedBatchStore.configureDestination` retains its sealed-manifest precondition. Await destination readiness before sending `receive_ready`; then files save automatically using the existing provider/prefix pump. Page presence and optional preview never start binary receipt.

The manifest callback advances the intent to a new operation stage and invokes a private session continuation. It must not wait for the accept HTTP response: a held or lost response can follow authoritative SSE/native manifest progress. Guard success, catch and finally by their exact operation stage so an old response cannot clear newer pending state or errors. Preview-only completion clears its own pending operation but sends no file consent. Cancellation, fresh-device recovery, auth change or disposal discards the intent; a late picker or manifest result cannot create files or authorize another pair.

## Verification and delivery

Preserve prior denial, correlation, close, quota, archive, filename, partial-file and completed-file ownership coverage. Adapt initial readiness expectations and add one-click, picker-ordering, no-pre-consent-binary, hidden/resume, SSE-drop, draft-preservation, ambiguous-register refusal and stale-callback cases. Tests must reflect backend stream-drop removal rather than invent replay. Keep native browser execution separate from builds and Docker lifecycle. Use focused helper/auth/visibility checks, application type check, scoped lint/format, production build, native fixtures and documentation policy. Freeze the integrated candidate for independent Reviewer and QA before PR readiness and integration. Actual OS picker and Gallery behavior remain manual browser acceptance limits.
