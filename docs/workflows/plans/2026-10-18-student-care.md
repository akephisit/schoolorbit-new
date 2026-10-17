# Student Care implementation plan

Execute the [approved design](../specs/2026-10-18-student-care-design.md) under repository `.rules`. Do not merge `main`. This temporary workflow input records implementation tasks and acceptance; runtime contracts remain in code/generated artifacts.

## Working agreement

One shared feature checkout is coordinated by the Senior. Backend owns `backend-school`, contracts, migration/permission/OpenAPI generation and generated frontend outputs. Frontend owns non-generated `frontend-school` source/tests and these approved workflow artifacts. Senior owns Git/index/checkpoints/PR. Writers pause and acknowledge before checkpoints. Independent Reviewer and QA inspect a frozen complete candidate. No writer stages or commits shared work.

## Phase 1: domain and contract boundary

- Backend: inspect providers and migration history; implement the Student Care domain, typed models/validators and resource policy. Define immutable instrument versions without instrument statuses, typed scoring definitions/manual-result mode, rounds/assignments/draft-final lifecycle, visits and cases/events. Keep source/licensing metadata and incomplete/unscored semantics explicit.
- Backend: add ordered migration, permission contract and resource enforcement; register typed HTTP paths/schemas and generate the exact API/permission artifacts. Preserve tenant isolation, guardian response privacy and explicit case responsibility.
- Frontend: inspect existing route-loader/context/API/UI references; record the approved design and plan before contract-dependent work. Prepare route/component responsibilities. Consume generated DTOs only after the agreed contract is available; report missing operations rather than inventing wire types.
- Checkpoint: Senior reviews the concrete API boundary and saves a draft PR checkpoint. Record pending checks honestly and resume writers after acknowledgment.

## Phase 2: complete route workflows

- Frontend: add typed central-client adapter, access/context helpers, reusable focused regions and presentation/draft helpers. Implement dedicated staff overview, library, rounds, visits, cases and student detail routes with correct `_meta` registration.
- Library: create instruments and immutable versions; configure respondent/age/language/source and online/manual mode; expose typed questions/options/scales/bands/urgent markers through a usable editor or strictly validated JSON import/export. Do not add library status controls or unlicensed questionnaire content.
- Rounds: create/manage rounds using chosen immutable versions; assign actual staff/student/guardian respondents; display scoped progress and results with incomplete/unscored distinction.
- Respondents: implement student, parent and staff own-assignment inboxes and accessible response forms; server draft save/reopen, validation and final submission; authorized manual-score entry and scoped result viewing.
- Visits: plan and record authorized home visits with staff responsibility and follow-up in a mobile form. Add photos only if actual private file-purpose support exists.
- Cases: explicit staff assignments, detail/history, events, deadlines/follow-up, acknowledgment, referral and closure using supported domain transitions.
- Keep independent regions concurrent, optional/action reads lazy, mutations narrowly reconciled and drafts retained on failures. Cancel/dispose reads and reject late mutations/reads after identity or route-context change. Never store sensitive drafts in localStorage/sessionStorage/offline caches.
- Checkpoint: Senior reviews usable end-to-end UI and saves the next feature checkpoint; no main merge.

## Phase 3: verification and documentation

- Backend: run focused pure validation/scoring tests, affected package/consumer checks in runtime and API-docs modes, disposable database transactional/resource-policy tests, generated API/permission checks/tests and architecture checks. Verify immutable version snapshots, incomplete results, urgent markers, draft-final refusal, stale revisions, privacy/denied paths and scope unions.
- Frontend: add meaningful Node helper tests for definition/draft transformations and validation; add synthetic Playwright fixtures for full library/round/response/manual/visit/case workflows and denied controls. Test server draft reopen, final submission, failed-save retention, independent region retries, identity disposal and mobile/desktop light/dark interaction where applicable.
- Frontend commands: `npm run check`; focused Prettier/ESLint on owned files; helper tests via `node --test`; `E2E_BASE_URL=http://127.0.0.1:4173 npx playwright test tests/e2e/student-care.spec.ts --project=chromium --workers=1` against a ready isolated local frontend; production build. Backend owns contract/permission generation runs so tooling never mutates shared outputs concurrently.
- Run documentation policy for these artifacts; update canonical TESTING/OPERATIONS/frontend README only for concrete durable behavior/recipes. Run `git diff --check` and review changed paths. Report exact executed commands/results, unresolved issues and unrun acceptance separately.

## Phase 4: independent acceptance and delivery

- Senior reconciles both writers, freezes the candidate and assigns independent Reviewer and QA. Reviewer checks the complete design/implementation/contract/privacy/rules boundary; QA runs the selected checks and verifies actual browser behavior.
- Owning writers repair actionable findings, repeat affected verification and return a new candidate for independent acceptance.
- Senior fetches and verifies the latest-main relationship, following pause/clean integration rules if needed. Save/push the final reviewed feature checkpoint and update the same PR. **Do not merge main.**
- The PR records actual scope, licensing/manual-mode limits, local/browser/database evidence and remaining real-tenant acceptance. Remove these temporary workflow artifacts after the implementation PR records the completed outcome, under Senior coordination.

## Completion checklist

- [ ] Immutable library versions, typed validated online definitions and licensed/manual workflows; no statuses or invented medical content.
- [ ] Round/version selection and authorized respondent assignments/progress.
- [ ] Staff/student/parent own inboxes, server draft reopen, final submissions and manual scores.
- [ ] Scoped, non-diagnostic results; incomplete never presented as normal; guardian boundaries enforced.
- [ ] Mobile visit planning/records and explicitly assigned support-case event/follow-up/referral/closure workflows.
- [ ] Scoped overview/student history with truthful loading/empty/error states and no mock records.
- [ ] Generated contracts/permissions consumed directly; exact route/action access; identity disposal.
- [ ] Focused backend, frontend, browser, generation, documentation and diff checks; independent Reviewer/QA acceptance.
- [ ] Feature PR updated and left unmerged as requested.
