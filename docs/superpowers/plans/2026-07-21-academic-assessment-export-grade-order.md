# Academic Assessment Export Grade Ordering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make both academic assessment XLSX downloads order rows by grade level first, so secondary grades appear as `ม.1`, `ม.2`, through `ม.6`.

**Architecture:** Keep the existing frontend-only workbook generation and its single `sortedAssessmentExportPlans` helper. Change comparator priority without mutating the source array, and strengthen the existing static regression test so it proves grade keys occur before subject-group keys.

**Tech Stack:** SvelteKit 5, TypeScript, Node test runner, SheetJS `xlsx`

## Global Constraints

- Preserve the existing ordering within each grade: subject-group display order, subject-group name, classroom room number, subject code, then subject title.
- Apply the ordering to both the overview and exam-format downloads through `sortedAssessmentExportPlans`.
- Do not change backend code, API contracts, database schema, migrations, permissions, or deployment configuration.
- Keep sorting immutable by retaining the copied array (`[...sourcePlans]`).

---

### Task 1: Order assessment export rows by grade before subject group

**Files:**
- Modify: `frontend-school/tests/static/academic-assessment-structure.test.mjs:382-414`
- Modify: `frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte:205-218`

**Interfaces:**
- Consumes: `AssessmentPlanSummary.gradeLevelSort`, `gradeYear`, `subjectGroupDisplayOrder`, `subjectGroupName`, `classroomRoomNumber`, `subjectCode`, and `courseTitle(plan)`
- Produces: `sortedAssessmentExportPlans(sourcePlans: AssessmentPlanSummary[]): AssessmentPlanSummary[]`, used by both branches of `exportAssessmentReport`

- [x] **Step 1: Write the failing regression assertion**

Rename the existing test:

```js
test('academic assessment export sorts by grade level before subject group', async () => {
```

Then insert these assertions immediately before that test's closing `});`:

```js
	const exportSortHelper = page.slice(
		page.indexOf('function sortedAssessmentExportPlans'),
		page.indexOf('function assessmentPlanKey')
	);
	const gradeLevelSortIndex = exportSortHelper.indexOf(
		'compareNullableNumber(left.gradeLevelSort, right.gradeLevelSort)'
	);
	const gradeYearIndex = exportSortHelper.indexOf(
		'compareNullableNumber(left.gradeYear, right.gradeYear)'
	);
	const subjectGroupOrderIndex = exportSortHelper.indexOf(
		'compareNullableNumber(left.subjectGroupDisplayOrder, right.subjectGroupDisplayOrder)'
	);
	const subjectGroupNameIndex = exportSortHelper.indexOf(
		'compareExportText(left.subjectGroupName, right.subjectGroupName)'
	);

	assert.ok(gradeLevelSortIndex >= 0);
	assert.ok(gradeYearIndex > gradeLevelSortIndex);
	assert.ok(subjectGroupOrderIndex > gradeYearIndex);
	assert.ok(subjectGroupNameIndex > subjectGroupOrderIndex);
```

- [x] **Step 2: Run the focused test and verify RED**

Run:

```bash
cd frontend-school
node --test --test-name-pattern="academic assessment export sorts" tests/static/academic-assessment-structure.test.mjs
```

Expected: FAIL at `assert.ok(subjectGroupOrderIndex > gradeYearIndex)` because the current helper compares subject group before grade.

- [x] **Step 3: Implement the minimal comparator change**

Change only the comparator priority in `sortedAssessmentExportPlans`:

```ts
function sortedAssessmentExportPlans(sourcePlans: AssessmentPlanSummary[]) {
	return [...sourcePlans].sort((left, right) => {
		return (
			compareNullableNumber(left.gradeLevelSort, right.gradeLevelSort) ||
			compareNullableNumber(left.gradeYear, right.gradeYear) ||
			compareNullableNumber(left.subjectGroupDisplayOrder, right.subjectGroupDisplayOrder) ||
			compareExportText(left.subjectGroupName, right.subjectGroupName) ||
			compareExportText(left.classroomRoomNumber, right.classroomRoomNumber) ||
			compareExportText(left.subjectCode, right.subjectCode) ||
			compareExportText(courseTitle(left), courseTitle(right))
		);
	});
}
```

- [x] **Step 4: Run the focused test and verify GREEN**

Run:

```bash
cd frontend-school
node --test --test-name-pattern="academic assessment export sorts" tests/static/academic-assessment-structure.test.mjs
```

Expected: PASS.

- [x] **Step 5: Validate the modified Svelte component**

Run:

```bash
cd frontend-school
npx @sveltejs/mcp svelte-autofixer 'src/routes/(app)/staff/academic/assessments/+page.svelte' --svelte-version 5
npm run check
```

Expected: the autofixer reports no issues or suggestions, and `svelte-check` exits with 0 errors.

- [x] **Step 6: Run frontend regression and repository hygiene checks**

Run:

```bash
cd frontend-school
npm run test:static
cd ..
git diff --check
git status --short
```

Expected: all static tests pass, `git diff --check` is silent, and status lists only the intended plan, test, and Svelte changes.

- [x] **Step 7: Commit the implementation**

```bash
git add docs/superpowers/plans/2026-07-21-academic-assessment-export-grade-order.md \
	frontend-school/tests/static/academic-assessment-structure.test.mjs \
	'frontend-school/src/routes/(app)/staff/academic/assessments/+page.svelte'
git commit -m "fix: sort assessment exports by grade level"
```
