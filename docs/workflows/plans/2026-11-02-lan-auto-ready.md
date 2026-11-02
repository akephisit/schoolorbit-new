# LAN automatic readiness implementation plan

Approved design: [automatic readiness and one-click receipt](../specs/2026-11-02-lan-auto-ready-design.md).
Base: `c59d7db8a9dbd48887be4b1d10ed830828580443`, shared branch `fix/lan-file-transfer-auto-ready`.

## Working agreement

Frontend is the single product/test writer. The Senior owns Git, shared generators and delivery; independent Reviewer and QA assess the frozen result. Backend protocol 3 and existing cleanup behavior remain unchanged. Auth and visibility helpers are consumed through their public APIs, not modified to hide a feature lifecycle defect. Preserve the approved storage, filename, queue, correlation and export boundaries.

## Implementation

1. Separate feature/auth lifetime from connection generation in `file-transfer-session.ts`. Arm readiness once on mount, retain the central visibility listener through a sixty-second pause, and own bounded safe recovery through existing auth reconciliation, known-device cleanup and fresh-device registration. Add a small feature helper only where deterministic recovery decisions need independent tests. One fresh registration per episode; ambiguous creation stops automatic attempts and exposes explicit retry.
2. Preserve source and unsent page drafts on same-auth connection loss. Cancel interrupted/queued jobs and retire their old correlations without replay. Replace the page's device-ID draft reset with auth/lifetime invalidation; retain exact peer choices where still present. Clear old QR identity immediately. Keep revocation/dispose fences and temporary-file cleanup, and block actions/exports during unavailable auth.
3. Add the primary one-click receive operation and private typed intent/stage continuation. Start the picker in the user click, hold its result without creating destination files, validate/seal the peer manifest, configure the destination, then send binary consent. Preserve optional preview and existing public operations where they remain legitimate. Fence all HTTP/picker/manifest continuations against pair, auth, device, connection and operation-stage changes.
4. Update the page and tool description for automatic ready/reconnecting/paused/error states, primary Receive and optional preview. Remove the initial Open button. Keep save-mode choices, automatic-save truthfulness, individual/ZIP recovery and actionable errors. Use existing shared controls and the existing generic default label; do not add identity/profile fetches.
5. Adapt native fixtures/helpers for automatic presence and add deterministic SSE/drop, hidden/visible, online, auth and registration-failure controls. Preserve authoritative readiness synchronization before fixture snapshots. Update the relevant canonical Testing recipe and only necessary behavior documentation.

## Checks and regression requirements

- Run focused file-transfer helper/storage/pump tests and applicable `realtime-idle`, auth refresh/session and recovery helper cases. New lifecycle decisions need deterministic timer/auth tests with bounded attempts and no ambiguous POST retry.
- Run owning Svelte/TypeScript checks and changed-file ESLint/Prettier checks, then build before native browser execution. No generators or Rust checks are required while their contracts remain unchanged.
- Adapt the existing browser `open` helper to assert automatic readiness. Existing optional-preview cases retain their manifest/file separation assertions; new primary Receive cases assert one authorization click, picker user gesture, sealed metadata and destination readiness before any binary bytes.
- Verify hidden visits below sixty seconds do not pause; sixty seconds pauses once and visible resume performs auth/HTTP reconciliation and creates one fresh device. Verify SSE loss while the OS picker is open preserves the same unsent File references and recipient draft, but interrupted jobs/old receive intent never resume automatically.
- Verify auth unavailable/401/role change, offline/online recovery, failed fresh registration, uncertain registration response, route disposal and held stale picker/HTTP/manifest callbacks. Known recovery may register once; uncertain creation never enters an automatic POST loop.
- Retain all applicable previous exact-content, large/many-file, six-peer/seventh-queue, ZIP64, byte-credit, quota, filename, cancellation, history, closed-response and permanent-folder-file regressions. These remain correctness checks, not performance benchmarks.
- Run documentation policy and `git diff --check`; report exact tested inputs and remaining manual OS picker/Gallery limits. Freeze all sources for independent Rules/Security review and QA, repair findings through the writer, then follow normal draft PR/integration and selected deployment gates. Remove these workflow inputs after the implementation PR records the outcome.
