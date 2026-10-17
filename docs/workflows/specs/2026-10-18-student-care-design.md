# Student Care approved design

This workflow input records the user-approved Student Care architecture. Implementation stays on the feature branch; do not merge `main`. Repository `.rules` remains authoritative. This artifact is temporary and is removed after the implementation PR records its outcome.

## Scope and boundaries

Student Care is a separate domain and navigation area from Behavior points. It covers an assessment library, assessment rounds and respondent assignments, server-saved responses and results, home visits, explicitly assigned support cases, and a scoped dashboard/student history. It does not change Behavior scores, diagnose a condition, automatically classify incomplete assessments as normal, or grant guardians all mental-health responses about their children.

The school chooses the exact instrument versions used by each round. The library supports multiple instruments and variants, including SDQ, EQ, depression screening, and school-configured instruments. There are **no draft/ready/retired instrument statuses**. Corrections create another immutable version; a version already selected by a round or submission remains fixed and historically reproducible.

## Domain and persistence owners

The Student Care backend domain owns library validation, immutable versions, assignment/resource access, draft/final response lifecycle, scoring, results, visits, cases, and dashboard queries. Typed Rust DTOs and domain HTTP declarations own the wire contract; generated OpenAPI and TypeScript DTOs are consumed directly. The permissions contract owns generated capability names. Existing auth, academic/student/guardian/staff providers and file policies retain their responsibilities; Student Care composes their public APIs rather than duplicating their records or authorization.

New migrations are additive and ordered after the current migration timeline. Tenant isolation, resource-scope unions, revisions/concurrent-write refusal, and transactional validation apply to every mutation. Sensitive content is excluded from generic person lookups, ordinary notifications, logs, and error text. No sensitive browser storage or offline assessment cache is introduced.

## Assessment library and rights

An instrument has identifying metadata and immutable versions/variants. Metadata includes the name/code, intended ages, respondent type, language, provenance/source, and reproduction/usage guidance. The version definition declares typed questions, allowed answers/options, numeric scores, named scales and contributing items, interpretation bands, and urgent answer markers. Validation rejects inconsistent references, duplicate identifiers, invalid numeric/range definitions, and incomplete online definitions. Scoring uses only the selected version and submitted values, with explicit incomplete/unscored states.

The library supports a manual-result mode for paper or external instruments where digital content is unavailable or not licensed. That workflow records authorized scale scores and provenance without copying protected questions. SDQ electronic reproduction requires prior publisher authorization; do not bundle SDQ questions, invented norms, clinical text, or unverifiable reproduction claims. Initial metadata/manual-score support is acceptable; authorized configured online instruments must work end to end. Configuration uses an approachable typed editor or validated JSON import/export with actionable field feedback. Ordinary respondent forms do not expose implementation details.

## Rounds, assignments, responses, results

A round fixes its selected immutable version(s), academic/date context and intended participants/respondents. Staff can plan and manage rounds under exact capabilities, then assign staff, students or guardians through explicit respondent identities. Own assignment inboxes expose only assignments for the current authorized actor. Assigned/school staff access remains bounded by the domain resource policy; broad homeroom visibility alone is not case ownership.

Online responses render the version's actual question types and valid options. A respondent can save an incomplete draft to the server, reopen it, and submit only when required fields pass validation. Saving a draft never publishes a final result. Final submission pins the version and answers and is immutable through the ordinary draft operation. Manual-result entry has equivalent validation, rights, finalization and historical provenance. Show pending/incomplete/unscored results honestly; calculated bands and urgent flags are screening/support signals, not diagnoses.

Results are projected according to the caller's rights. Guardians may complete their own assignments and see only results explicitly exposed to their allowed context; the relationship does not grant access to every child response. Detailed answers, staff case notes and referrals require their own permitted resources. Unknown and denied states do not leak existence or content.

## Home visits

Staff can plan visits for authorized students, assign responsible staff, record scheduling/context and complete a mobile-friendly visit form with observations and follow-up needs. Preserve entered form values after failed saves and use server revisions. Optional photos are offered only if an actual private backend file policy supports this purpose; no placeholder upload or public attachment path is acceptable. Visit details remain sensitive and resource scoped.

## Support cases

Cases have explicitly assigned responsible staff and a student context. Support events, follow-up deadlines, acknowledgments, referrals and closure form an auditable chronological history. Case mutation rights are checked separately from read access; changing an assignee or state is a domain operation. Urgent markers can inform follow-up without inventing a medical diagnosis or automatically granting additional access. Closed cases retain their history. User-facing states and actions must accurately reflect backend-supported transitions.

## Frontend structure and state

Dedicated routes include `/staff/student-care`, `/staff/student-care/library`, `/staff/student-care/rounds`, `/staff/student-care/visits`, `/staff/student-care/cases`, and student/detail/workflow children. `/student/student-care` and `/parent/student-care` are own assignment inboxes. A shared assessment-response route may be used where its access metadata and backend resource policy support each respondent role. Long-lived workflows use URLs, not one giant in-memory tab workspace.

Each route loader owns its primary safe GETs, passes SvelteKit's `fetch` through the central typed API client, and starts independent visible regions concurrently using `captureRouteLoad`. Inactive forms/dialogs and capability-only lookups remain lazy. Each region has focused loading, error, retry and mutation reconciliation. Identity/context changes clear sensitive state, cancel superseded work and reject late results. Backend-returned resources patch the affected local region without broad reloads or discarding unrelated drafts.

Use generated route `PageProps`, Svelte 5 runes, `_meta.menu` for real service links and `_meta.access` for children. Use the existing permission/context stores, PageShell/PageHeader, PageState, PageSkeleton, LoadingButton and local UI primitives. Exact capabilities gate controls as well as route access. Forms have associated visible labels, retained drafts, accessible pending/errors, responsive mobile layouts and theme-token contrast in light/dark modes.

## Acceptance and verification

Acceptance requires working library creation/versioning/import validation, round creation/assignments, all three respondent inboxes, server draft reopen/final submission/manual scores, appropriately scoped results, home-visit planning/recording, case events/follow-up/acknowledgment/referral/closure, and a truthful scoped dashboard/student history. No misleading mock data, invented instrument content, library status controls, local sensitive draft persistence or fake file uploads may ship.

Backend coverage establishes validation/scoring, immutable history, concurrent revisions, tenant/resource denied paths and scope unions with disposable fixtures. Frontend checks establish helper transformations, retained forms, permission-gated routes/actions, independent loading/retries, real Chromium interactions and mobile/desktop light/dark layouts against synthetic API fixtures. Generation/check/tests, Svelte/TypeScript checks, focused lint/format, documentation policy and `git diff --check` must pass. Fixture/browser evidence remains distinct from real-tenant clinical/licensing acceptance. Senior owns Git/checkpoints/PR and independent Reviewer/QA acceptance; no main merge is authorized.
