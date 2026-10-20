# Budget projects workspace

Status: the user approved this feature design in chat and authorized implementation on 2026-10-20. This artifact records that scope; it does not request another approval. Execute with the [implementation plan](../plans/2026-10-20-budget-projects.md) and current [development rules](../../../.rules).

## Product scope

Build a School budget workspace for planning, approval, spending and reconciliation. The hierarchy is project → activities → plan lines. The project form has four visible groups: name, fiscal year, responsible organization unit, and start/end dates. Projects and activities use names and context, with internal UUIDs and relationships retained; they have no user-entered project or activity codes.

Fiscal years are managed budget settings, separate from academic year/term context. Reuse existing organization units and staff identities. Settings provide CRUD for fiscal years, funding sources and expense categories, with validation and preservation of referenced history. A funding source answers where money comes from; an expense category answers what it purchases. Keep those dimensions separate throughout entry, balances and reports.

An activity has a name and optional default funding source and expense category. Defaults fill new lines only. Changing existing lines requires an explicit bulk action identifying the affected lines; editing defaults must never silently recategorize them.

A plan line contains name, quantity, unit, unit price, funding source and expense category. A supply-catalog reference is optional. Persist a name/unit/price snapshot so later catalog edits cannot rewrite the saved line or an accepted plan. Explicit draft edits create a new saved revision; accepted snapshots remain immutable. Manual lines remain fully supported.

## Approval, spending and balances

Plans move from draft through submission and review to return or approval. Return records the reason and permits a new draft revision. Approval accepts an immutable snapshot of the reviewed revision. Subsequent changes use audited amendments; never overwrite the approved baseline. Audit actors, timestamps, reasons and revision relationships.

Spending requests belong to an activity and can request multiple lines. Each request can have multiple payments. Approval reserves the approved amount after a transactional balance recheck under locks; payment converts the relevant reservation into actual spending. Closing a request releases its remaining reservation. Repeated commands cannot duplicate approval, reservation, payment or release. Refuse stale revisions and concurrent overspending, including races between requests, payments and budget adjustments.

Support budget additions, reductions, transfers, loans and repayments with approver, reason and immutable history. Track loan principal, repayments and outstanding amount explicitly. Apply same-fiscal-year and same-funding-source constraints unless an explicit policy permits an exception; absence of a policy is not permission. Reductions, outgoing movements and loans must respect existing commitments. Report the approved baseline, approved adjustments, current budget, reserved, actual, available and loan outstanding consistently, with movements counted once.

Store monetary values as integer satang and serialize them as strings on the JSON wire. Quantity precision, multiplication and rounding are a backend contract decision to resolve and document before schema/API implementation; do not infer them from browser floating-point arithmetic. Validate signs, bounds and arithmetic overflow. Idempotency, revision checks and balance locks belong to authoritative services and transactions.

## Ownership and integration

Backend must evaluate a cohesive `school-budget` owner against the crate-admission criteria in `.rules`, including public API, dependency direction, focused tests and measured compile scope. The name is a candidate, not evidence that a crate boundary is already justified. Inspect the actual implementation before fixing persistence, handler and composition paths.

Use generated permissions and API contracts, existing tenant/session/resource policies, and application-owned composition adapters. Budget owns budget decisions, snapshots, ledgers, audit and notification meaning. Reuse the owning public operations for process approval, files, work and notifications; extend narrow provider ports when necessary. Do not add parallel process engines, file storage, work queues or notification transport. Existing supply catalog ownership is `school-administration`; process ownership is `school-workflow`. Evidence attachments remain private and authorized against their budget resource, including download and deletion paths.

## Workspace and reports

Provide a dashboard; projects and project detail with activities/lines; request history and review; finance/payment work; adjustments; settings; and reports with exports. Use the existing School page, table, picker, feedback and regional-loading patterns. Fiscal-year selection must work independently of the academic header. Show actionable validation, permission denial, empty, loading, failed-read/retry and stale-edit states. Restrict review, approval, payments, adjustments, settings and exports by their own backend resource policies as well as UI permissions.

Reports aggregate the saved line amounts across funding source and expense category without double counting. An acceptance example is a mathematics project in fiscal year 2570 with three activities: grant-funded materials 3,588 baht, equipment 3,040 baht, and learner-development-funded materials 3,300 baht. The project totals 9,928 baht; the pivot retains both the separate funding dimensions and materials/equipment classification. Example amounts are fixtures, not seeded production configuration.

Calendar integration, strategy alignment, PDCA/KPIs, evaluations, rationale/objectives and committees are outside this feature. Do not expand the minimal project form with those fields.

## Acceptance and release boundaries

Verify draft/review/return/approval and immutable amendments, catalog snapshot stability, explicit-only bulk default application, request/payment/release accounting, every budget movement and outstanding-loan reconciliation. Exercise retries, stale revisions, concurrent approvals/payments/adjustments, transaction rollback, authorization denial, tenant/resource isolation and private evidence. Reports and exports must reconcile to the same authoritative ledger.

Use new sequential migrations only. Backend must inspect migration history and decide schema/binary compatibility and recovery before cutover; preserve existing identities, required codes and unrelated data. Record the concrete compatibility boundary and any old-binary prohibition with implementation evidence before release. Delivery follows existing local verification, independent review/QA, draft PR, latest-main Pipeline, merge and selected deployment gates. This design does not claim any implementation or check has passed.
