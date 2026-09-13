# Promotion Policy and Recommendations Implementation Plan

> Execute inline with `superpowers:executing-plans`; run test/build jobs serially. Keep the current feature branch and project root.

**Goal:** Persist explicitly reviewed school promotion policies and produce reviewable recommendations from current annual snapshots, without enrolling or moving any student.

**Architecture:** Lifecycle owns immutable reviewed policy versions and recommendation findings. Core validates grade progression and program references; Results supplies annual snapshot facts. Execution, target enrollment, and year activation consume these reviewed inputs through separate Core commands in the run/execution plan.

**Tech Stack:** Rust/Axum/SQLx/PostgreSQL; exact decimal arithmetic; generated permissions/OpenAPI; Svelte 5 and local shadcn.

**Spec:** `../specs/2026-09-10-academic-year-promotion-design.md`.

## Constraints and interfaces

- Recommendations are not final decisions and never mutate scores, placements, student-year status, or year status.
- Policy configuration has no automatically approved default. Creating a reviewed version requires both school policy-management and promotion-review capabilities.
- Each rule matches one exact source grade/program. No fallback to another program, another curriculum version, or a grade-name guess.
- A rule's success outcome is `promote` or `graduate`. An unmet criterion returns `review_required`, not an invented repeat/hold/graduate decision. Staff later explicitly choose from all six approved decision outcomes.
- Thresholds describe the selected annual result, not GPAX or a government graduation certificate. Required cumulative/transcript checks remain explicitly unavailable until eligible historical inputs have been reviewed.
- A reviewed annual hold always requires individual review; numeric zero remains numeric zero and cannot become a missing result.
- Core's configured grade progression is validated and snapshotted at policy approval. Policy history does not reference mutable/deletable progression-row IDs as permanent FKs. Execution revalidates destination identities and target-year applicability.
- New migrations follow 071. Never edit 068–071 once applied.

```rust
// lifecycle/models/promotion_policy.rs
enum PromotionSuccessOutcome { Promote, Graduate }
struct PromotionRuleInput {
    from_grade_level_id: Uuid,
    from_study_program_id: Uuid,
    target_grade_level_id: Option<Uuid>,
    target_study_program_id: Option<Uuid>,
    success_outcome: PromotionSuccessOutcome,
    minimum_earned_credits: String,
    require_no_exceptional_outcomes: bool,
    require_activities_passed: bool,
    minimum_learner_level: i16,
}
struct PromotionPolicyInput { name: String, rules: Vec<PromotionRuleInput> }
struct PromotionPolicyVersion {
    id: Uuid, name: String, rules: Vec<PromotionRuleInput>,
    progression_row_version: i64, reviewed_by: Uuid, reviewed_at: DateTime<Utc>,
}
enum PromotionRecommendationFinding {
    PolicyRuleMissing, AnnualResultMissing, AnnualResultStale, ReviewedAnnualHold,
    InsufficientEarnedCredits, ExceptionalOutcomes, ActivitiesNotPassed,
    LearnerEvaluationNotPassed, LearnerEvaluationMissing,
}
struct PromotionRecommendation {
    suggested_outcome: Option<PromotionSuccessOutcome>,
    target_grade_level_id: Option<Uuid>, target_study_program_id: Option<Uuid>,
    findings: Vec<PromotionRecommendationFinding>,
}
```

## Task 1: Exact policy validation and deterministic recommendation

**Files:** Create `backend-school/src/modules/academic/lifecycle/models/promotion_policy.rs`, `lifecycle/services/promotion_recommendation.rs`; export from existing `models.rs` and `services.rs`.

```rust
pub(crate) fn validate_policy(input: &PromotionPolicyInput) -> Result<(), AppError>;
pub(crate) fn recommend(
    rule: Option<&PromotionRuleInput>,
    annual: Option<&AnnualResultRevision>,
) -> Result<PromotionRecommendation, AppError>;
```

- [x] Write pure tests with literal expectations: exact 0.50 credit boundary, valid numeric-zero annual result, missing/stale annual result, reviewed hold, exceptional results, failed activities, each learner-evaluation domain, and a missing rule. Removing any readiness/hold check must turn a protected case red.
- [x] Reject empty/whitespace names, names over 200 characters, empty or over-500 rule lists, duplicate source grade/program pairs, nil IDs, noncanonical/negative/overprecision credits, and learner levels outside 0–3.
- [x] Promote requires both target IDs and a different grade; graduate requires neither. Missing criteria never default to passing.
- [x] Run `CARGO_BUILD_JOBS=1 scripts/test_backend_school.sh promotion_recommendation -- --test-threads=1` RED, implement, then GREEN.
- [x] Inspect annual term snapshots, not mutable score cells. Require both evaluation domains per included source term, complete summaries, and a quality level meeting the explicit threshold. An absent/duplicate domain produces a review finding.
- [x] Compare earned credits using the shared canonical decimal parser/BigDecimal. Never compare rounded GPA values or floating-point numbers.

Example behavior test:

```rust
let result = recommend(Some(&rule), None).unwrap();
assert_eq!(result.suggested_outcome, None);
assert_eq!(result.findings, vec![PromotionRecommendationFinding::AnnualResultMissing]);
```

## Task 2: Reviewed immutable policy storage and scoped Core validation

**Files:** Create `backend-school/migrations/072_promotion_policies.sql`, `lifecycle/services/promotion_policy.rs`, `core/services/promotion_context.rs`; modify permission contract/generators and their generated outputs.

```rust
// Core read provider, caller owns authorization and transaction:
pub(crate) async fn validate_promotion_rules(
    tx: &mut Transaction<'_, Postgres>, rules: &[PromotionRuleInput],
) -> Result<i64, AppError>; // validated progression-set row version
// Lifecycle service:
pub async fn list_promotion_policies(pool: &PgPool, actor: &ActorContext)
    -> Result<Vec<PromotionPolicyVersion>, AppError>;
pub async fn create_promotion_policy(pool: &PgPool, actor: &ActorContext,
    input: PromotionPolicyInput) -> Result<PromotionPolicyVersion, AppError>;
```

- [ ] Add generated `academic_promotion.read.school`, `manage.school`, `approve.school`, `execute.school`, and `correct.school` capabilities, reusing the contract's existing action vocabulary. Grant defaults only to the verified system ADMIN role. Manage does not imply review or execute; a reader has no write path.
- [x] Persist immutable policy rows with a JSON array of typed rules, progression revision, reviewer FK and timestamp. Use `Json<Vec<PromotionRuleInput>>`; reject update/delete with the existing immutable official-record trigger. Do not seed a policy.
- [x] In a transaction, lock the progression-set row for share before reading its rules. Validate all distinct source/target grade IDs, program IDs and required source-to-target mappings in bulk. A curriculum-specific progression applies only to its source program's curriculum; duplicate applicable mappings are rejected as ambiguous.
- [x] Test valid promotion and graduation mappings, foreign/missing program/grade references, unrelated curriculum mappings, same-level repeat not accepted as an automatic promote rule, denied teacher/reader/manage-only creation, and policy immutability.
- [x] Store the validated mapping as policy rules rather than a permanent FK to a replaceable progression row. Later edits create a new reviewed policy version without rewriting historical versions.
- [x] Append the policy approval audit in the same transaction and prove audit failure leaves no policy row. No student or result table is written.
- [ ] Run the focused policy/provider tests and permission generator/check/test serially.

## Task 3: Typed endpoints and explicit school-office policy form

**Files:** `lifecycle/handlers.rs`, `academic/lifecycle.rs`, `api_contract.rs`, generated OpenAPI/TS, new `frontend-school/src/lib/api/academic-promotion.ts`, `staff/academic/promotion/policies/+page.ts` and `+page.svelte`, `tests/e2e/academic-promotion-policies.spec.ts`.

```text
GET  /api/academic/lifecycle/promotion-policies
POST /api/academic/lifecycle/promotion-policies
GET  /api/academic/lifecycle/promotion-policies/options
```

- [x] Write OpenAPI tests requiring typed envelopes, camel-case fields, denied unknown inputs, actual 400/401/403/422 responses, and no implicit year/term query parameters.
- [x] Register endpoints/DTOs and regenerate artifacts. Frontend wrappers consume only generated wire types.
- [ ] Provide a read-first policy list/history. Load a minimal Core-owned grade/program/progression reference projection lazily on details/create, under promotion read permission; no curriculum-management grant or student/PII payload is required. The create form appears only for both manage+approve grants. No implicit year/term filters: policy references span curriculum versions, with target-year applicability revalidated during execution.
- [ ] The form shows source grade/program, success action, target grade/program when promoting, minimum earned credits for this year, exceptional-outcome/activity criteria, and learner threshold. Use local shadcn Select/Checkbox/Input/Dialog and existing Thai grade labels; no free-text IDs.
- [x] Show existing Core progression configuration with an explicit action to configure missing mappings using its existing authorized API. Do not silently invent a progression when a policy is saved. The `PromotionProgressionsDialog.svelte` uses the existing optimistic replace command; its same-grade repeat validation is covered by a real Core test.
- [ ] Confirm `ยืนยันเกณฑ์รุ่นใหม่` before creation, explaining that this records reviewed school criteria but moves no students. Patch only the returned version into the list.
- [x] Browser tests: reader has no create affordance, manager without review cannot create, exact grade/program selections and criteria are submitted, graduate removes target fields, invalid form cannot submit, and mobile confirmation can close before saving.
- [ ] Run Svelte tooling and the applicable `.rules` verification matrix. Continue with persistent promotion runs and Core execution; policy creation alone does not complete Release 4.
