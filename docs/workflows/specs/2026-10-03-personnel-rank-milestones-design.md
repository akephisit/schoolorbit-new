# Personnel rank milestones design

## Approved scope and purpose

Continue phase 3 of the approved personnel simplification design, authorized by the user's instruction to finish the remaining personnel work. Display dated rank tenure and next-rank calendar milestones on the scoped personnel profile and overview. This is a planning aid, never an eligibility decision or an automatic application submission.

## Verified sources and boundaries

Read the full relevant criteria, not search snippets. Official consolidated letter [1932/2567](https://otepc.go.th/th/content_page/item/5169-2024-11-22-11-47-11.html), PDF pages 7 and 13, requires four consecutive years for ordinary teacher-rank transitions; three years is conditional on the separate reduction criteria. PA results, workload, conduct and certification remain additional requirements. [1222/2568](https://otepc.go.th/en/content_page/item/5622-1222-2568-9-12-2564-18-2567.html), read letter and teacher attachment pages 3–5, changes academic works and assessment procedures rather than the ordinary tenure threshold; its teacher changes take effect 16 May 2026. [587/2569](https://otepc.go.th/en/content_page/item/6043-587-2569.html), dated 24 September 2026, explicitly disallows using a master's degree used for initial teacher-assistant appointment as a higher qualification for reduction. Never infer reduction from the education dropdown. Special-area doubled time, equivalent posts and award/innovation routes require human review and are not this ordinary calendar calculator.

## Canonical owner and rule version

Keep a typed, immutable versioned rule catalogue in `school-staff`, shared by profile and overview calculations. Rules are system-owned backend policy, not editable reference-data pages or frontend constants. Each result exposes rule version, effective-from and reviewed-on dates, source links and computation date. The initial system policy edition is available from its review date, 3 October 2026; earlier computation dates require an explicitly reviewed historical edition. This is the system edition date, not a replacement for the legal effective dates in the source letters. Select rules by the computation date, not the person's original appointment date. Estimates of future dates use the current reviewed rule version and are not promises that future regulations will stay unchanged. Future policy changes introduce another version; do not rewrite a historical rule version.

The capability shares staff history, access scopes and lifecycle. A module in the existing `school-staff` crate keeps a narrow pure calculator and focused tests without new dependency cycles; a further crate would add no measured invalidation benefit. No database/schema/permission/writing API change is needed: the canonical history already stores inputs, results are recomputed, and no formal review or approval decision is persisted.

## Calendar calculation and states

Initial track: explicit `civil_servant` personnel type and catalogue position code `teacher`. Teacher assistant, other/custom positions, other personnel types and not-applicable rank are unsupported; absent type/position/rank is incomplete. Highest rank has no next target. Supported progression is none → proficient → senior_proficient → expert → senior_expert.

Read current entries only. Require known effective dates for civil-servant type and teacher position and, when a rank is held, for that rank. Start from the latest of these concurrent fact dates. No use of hiring date, order date, created/updated timestamps, free text or education degree as a substitute. Missing dates, dates after computation date, or inconsistent current facts produce explicit incomplete/review reasons, no guessed date.

Add four calendar years for the ordinary milestone, and show a separate three-year conditional comparison only with the prominent requirement to verify reduction evidence. Preserve month/day; clamp 29 February to 28 February for non-leap anniversary years. Pure calculator uses calendar dates, Bangkok determines today. Ordinary milestone buckets: future (>90 days), due within 90 days (1–90 inclusive), minimum time reached pending qualification review (today or past), incomplete, unsupported, no next rank. Never emit `eligible`, approved or ready-to-submit as an automated result.

Continuous service, PA, workload, conduct, qualification recognition, special routes and formal certifications must be checked by the responsible staff. Dates show recorded tenure only; the UI states this limitation beside the milestone, not hidden in documentation.

## API, read efficiency and privacy

Expose typed milestone in the existing career history response after existing profile authorization. No public-profile leakage or new permission bypass. Pagination must return the same current result from canonical projections, not derive from the paginated history subset. Re-reading after correction refreshes that one region.

Add an independent overview milestone endpoint with the same `StaffListAccess` filter and status selection as existing charts. It computes all scoped counts from a minimal set-based query, emits at most 50 named rows ordered by ordinary date then name and UUID, and reports totals and fixed 50-row pagination. Never perform per-person profile queries. Missing facts remain counted, even without staff_info. No credentials, national IDs, audit notes or order text in summary responses. Independent loading/error/retry and stale response/identity cancellation preserve existing charts.

## Presentation and verification

Profile card: next rank, ordinary date, conditional reduced comparison, recorded start date, computation date and rule/source disclosure; clear incomplete/unsupported/highest-rank states. Dashboard: distinct summary cards, chronological table with accessible profile buttons, explicit no-matching-results and bounded-list note; responsive mobile cards rather than squeezed columns. Verify light/dark desktop/mobile, loading, error/retry, null data, denied scope, identity/status race and mutation refresh.

Run pure Rust calendar/state/version tests; database scope and typed endpoint tests through native Podman; Rust fmt/static architecture/workspace all targets; API generation/check/tests; frontend lint/check/static/build and mocked personnel/career/milestone Playwright. Integrate only exact tested tree, deploy both tenants and verify scoped endpoints and live UI. Draft bridge retirement is a separate completed bounded cleanup, preserving current v4 ownership and privacy.
