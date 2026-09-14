# Canonical Migration Policy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `.rules` require clean long-term canonical designs while preserving and migrating valid legacy data without permanent runtime compatibility.

**Architecture:** Add one central design policy under feature work, then put database-specific migration guarantees, deployment compatibility limits, and affected verification requirements in their existing authoritative sections. Extend the existing documentation-policy test with stable headline sentinels and section-scoped safety assertions so the new repository-wide boundary cannot disappear silently.

**Tech Stack:** Markdown repository rules, Node.js built-in test runner, existing `frontend-school` documentation-policy test.

**Spec:** `docs/superpowers/specs/2026-09-14-canonical-migration-policy-design.md`

## Global Constraints

- Design new work around one orderly canonical model suitable for long-term maintenance.
- Treat persisted legacy data as migration input rather than a reason to preserve legacy runtime interfaces.
- Preserve the meaning, relationships, and required history of valid existing data through deterministic migration and reconciliation.
- Remove legacy endpoints, fields, tables, aliases, fallbacks, and dual paths after a verified cutover.
- Fail closed when data cannot be mapped safely, without leaking sensitive values.
- Permit only bounded, explicitly owned compatibility needed for a safe rollout or rollback.
- Applied migrations remain immutable; use new sequential forward migrations.
- Authorized retention, deletion, legal-hold, and PDPA rules remain authoritative.
- Do not change `docs/TESTING.md` or `docs/OPERATIONS.md`; this policy introduces no new command or operational procedure.

---

### Task 1: Enforce the canonical replacement and data migration policy

**Files:**
- Modify: `frontend-school/tests/static/documentation-policy.test.mjs:133-162`
- Modify: `.rules:57-73`
- Modify: `.rules:161-176`
- Modify: `.rules:248-268`
- Modify: `.rules:270-346`

**Interfaces:**
- Consumes: the approved policy in `docs/superpowers/specs/2026-09-14-canonical-migration-policy-design.md` and the existing `.rules` section ownership.
- Produces: the `### Canonical replacements and data preservation` rule subsection plus database, deployment, and verification clauses referenced by exact documentation-policy sentinels.

- [ ] **Step 1: Add a failing guard for the durable policy**

Add these exact entries to the `required` array in `project rules own durable development and verification workflows`:

```js
'### Canonical replacements and data preservation',
'Persisted legacy data is migration input, not a runtime compatibility requirement.',
'expand → migrate/backfill → reconcile → switch consumers → contract/cleanup',
'Legacy runtime compatibility is not a default requirement.'
```

Add a `requiredSection(source, startHeading, endHeading)` helper that fails when either boundary is absent and returns only the text owned by that section. In the same test, use it to assert that:

- the canonical policy prohibits silent loss, requires compatibility ownership/removal, and preserves PDPA authority;
- the database section limits forward migrations to database-changing phases, blocks both cutover and cleanup on failed reconciliation, and prohibits sensitive output;
- the deployment section bounds the compatibility window and records the rollback schema boundary; and
- the verification section tests refusal of both cutover and cleanup plus post-cleanup legacy guards.

- [ ] **Step 2: Run the focused test and verify the new guard fails**

Run:

```bash
cd frontend-school
npm run check:docs
```

Expected: FAIL with `.rules must contain: ### Canonical replacements and data preservation` because the policy has not been added yet.

- [ ] **Step 3: Add the canonical design policy to `.rules`**

After the feature-toggle paragraph in section 2, add:

```markdown
### Canonical replacements and data preservation

- Design replacement schemas, APIs, DTOs, and business models around one orderly canonical structure suitable for long-term maintenance. Do not distort the new design merely to keep a legacy runtime interface alive.
- Persisted legacy data is migration input, not a runtime compatibility requirement. Preserve its valid meaning, stable identities, relationships, and required history through explicit transformation into the canonical model; do not silently discard values, guess ambiguous mappings, or create an indefinite duplicate archive.
- After cutover, the runtime has one canonical owner and path. Do not add new product behavior to a legacy path except when required to make migration safe, and remove legacy endpoints, fields, aliases, fallbacks, dual reads/writes, and duplicate schema owners after verified cleanup.
- Compatibility may exist temporarily only when a safe rollout or rollback requires it. The design must name its owner, exact scope, removal condition, and cleanup step before implementation.
- Data preservation does not override authorized retention, deletion, legal-hold, or PDPA requirements. Do not retain data longer than policy allows merely because it originated in a legacy model.
```

In section 6, before the database-backed test rules, add:

```markdown
- For destructive or semantic replacements, use `expand → migrate/backfill → reconcile → switch consumers → contract/cleanup`, scaled to the affected data and runtime risk. Use new sequential forward migrations for every database-changing phase; implement reconciliation and consumer switching as explicit executable gates in the migration and deployment flow.
- Make transformations deterministic and safely retryable under the migration runner and recovery design. Before destructive cleanup, verify current aggregate counts, stable identities, relationships, appropriate checksums, and business invariants.
- If any source value cannot be mapped unambiguously or reconciliation is absent, stale, incomplete, or failed, it blocks both consumer cutover and destructive cleanup. Report only bounded finding codes and aggregate counts; never discard or guess data or expose raw rows, PII, credentials, secrets, or database URLs. Correct the data through an explicit reviewed remediation or recovery plan, then reconcile again.
```

In section 10, after the existing data-cutover deployment rule, add:

```markdown
- Legacy runtime compatibility is not a default requirement. Any temporary bridge must be limited to the approved rollout or rollback window and removed at its predefined cleanup gate; it must not become a permanent alias, fallback, dual path, or parallel business model.
- Before consumer cutover, decide whether the prior application version can safely use the expanded schema. Record this decision, the compatible schema boundary, and the point after which old binaries are prohibited in the approved cutover design. If backward binary compatibility would retain a harmful legacy contract, use a tested roll-forward recovery instead of assuming an old binary can be restored.
```

In section 11, before the static-test ownership list, add:

```markdown
For canonical replacement or cutover changes, use sanitized legacy-shaped fixtures and test successful mapping, preservation of identities/relationships/invariants, safe retry, every known unmappable state, stale or missing reconciliation, and refusal of both consumer cutover and destructive cleanup before evidence is complete. After cleanup, add guards preventing removed legacy contracts and runtime fallbacks from returning. External rehearsals use disposable or protected copies and emit only bounded aggregate evidence.
```

- [ ] **Step 4: Run the focused test and verify the policy passes**

Run:

```bash
cd frontend-school
npm run check:docs
```

Expected: PASS, 7 tests, 0 failures.

- [ ] **Step 5: Review the exact change and repository hygiene**

Run from the repository root:

```bash
git diff --check
git diff -- .rules frontend-school/tests/static/documentation-policy.test.mjs
git status --short
```

Expected: no whitespace errors; only `.rules` and `frontend-school/tests/static/documentation-policy.test.mjs` are modified beyond the already committed design and plan artifacts.

- [ ] **Step 6: Commit the policy and guard**

```bash
git add .rules frontend-school/tests/static/documentation-policy.test.mjs
git commit -m "docs: define canonical data migration policy"
```
