# Supervision Rubric Template Builder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a real multi-section teaching supervision rubric builder with a paper-form preset, 1-5 rating controls, and score/quality summaries.

**Architecture:** Keep the existing backend schema and API because templates already support nested sections/items. Add a focused frontend utility module for rubric defaults and score math, then wire the existing supervision page to use nested template forms instead of the current one-rating-item dialog.

**Tech Stack:** SvelteKit 5 CSR, TypeScript, local shadcn-svelte components, existing `frontend-school/src/lib/api/supervision.ts` typed API, Node static tests.

---

### Task 1: Add Rubric Helper Module

**Files:**
- Create: `frontend-school/src/lib/utils/supervision-rubric.ts`
- Test: `frontend-school/tests/static/supervision-rubric.test.mjs`

- [ ] **Step 1: Write the failing helper test**

Add a Node test that imports `createPaperSupervisionRubricSections`, `calculateRubricDraftSummary`, and `qualityLevelFromPercentage`.

```js
import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	createPaperSupervisionRubricSections,
	calculateRubricDraftSummary,
	qualityLevelFromPercentage
} from '../../src/lib/utils/supervision-rubric.ts';

describe('supervision rubric helpers', () => {
	it('builds the paper-form preset with four rubric sections and one comment section', () => {
		const sections = createPaperSupervisionRubricSections();
		assert.equal(sections.length, 5);
		assert.equal(sections[0].title, '1. ลักษณะการปฏิบัติงาน');
		assert.equal(sections[2].items.length, 11);
		assert.equal(sections.at(-1)?.items[0].itemType, 'text');
	});

	it('calculates score percentage and quality level from rating drafts', () => {
		const sections = createPaperSupervisionRubricSections();
		const drafts = Object.fromEntries(
			sections.flatMap((section) =>
				section.items
					.filter((item) => item.itemType === 'rating')
					.map((item) => [item.localId, { ratingScore: '5', textResponse: '' }])
			)
		);
		const summary = calculateRubricDraftSummary(sections, drafts, 5);
		assert.equal(summary.ratingItemCount, 20);
		assert.equal(summary.answeredRatingCount, 20);
		assert.equal(summary.totalScore, 100);
		assert.equal(summary.percentage, 100);
		assert.equal(summary.qualityLabel, 'ดีมาก');
	});

	it('maps paper-form quality thresholds', () => {
		assert.equal(qualityLevelFromPercentage(90), 'ดีมาก');
		assert.equal(qualityLevelFromPercentage(80), 'ดี');
		assert.equal(qualityLevelFromPercentage(70), 'พอใช้');
		assert.equal(qualityLevelFromPercentage(60), 'ควรปรับปรุง');
		assert.equal(qualityLevelFromPercentage(59.99), 'ไม่ผ่าน');
	});
});
```

- [ ] **Step 2: Run the failing test**

Run: `cd frontend-school && node --test tests/static/supervision-rubric.test.mjs`

Expected: FAIL because `src/lib/utils/supervision-rubric.ts` does not exist.

- [ ] **Step 3: Implement the helper module**

Create `supervision-rubric.ts` with:

```ts
export type RubricItemType = 'rating' | 'text';

export interface RubricFormItem {
	localId: string;
	label: string;
	description: string;
	itemType: RubricItemType;
	required: boolean;
	sortOrder: number;
}

export interface RubricFormSection {
	localId: string;
	title: string;
	description: string;
	sortOrder: number;
	items: RubricFormItem[];
}

export interface RubricResponseDraft {
	ratingScore: string;
	textResponse: string;
}
```

Implement:
- `createPaperSupervisionRubricSections()`
- `createBlankRubricSection(sortOrder: number)`
- `createBlankRubricItem(itemType: RubricItemType, sortOrder: number)`
- `qualityLevelFromPercentage(percentage: number | null | undefined)`
- `calculateRubricDraftSummary(sections, drafts, ratingMax)`
- `sectionRubricProgress(section, drafts)`

- [ ] **Step 4: Verify helper test passes**

Run: `cd frontend-school && node --test tests/static/supervision-rubric.test.mjs`

Expected: PASS.

### Task 2: Add Static Contract Guards

**Files:**
- Modify: `frontend-school/tests/static/api-response-contract.test.mjs`

- [ ] **Step 1: Add failing static assertions**

Inside `teaching supervision frontend contract uses typed API and permission metadata`, assert:

```js
assert.match(supervisionPage, /createPaperSupervisionRubricSections/);
assert.match(supervisionPage, /templateForm\.sections/);
assert.match(supervisionPage, /addTemplateSection/);
assert.match(supervisionPage, /addTemplateItem/);
assert.match(supervisionPage, /moveTemplateItem/);
assert.match(supervisionPage, /calculateRubricDraftSummary/);
assert.match(supervisionPage, /sectionRubricProgress/);
assert.doesNotMatch(supervisionPage, /ratingLabel/);
assert.doesNotMatch(supervisionPage, /textLabel/);
```

- [ ] **Step 2: Run the failing static tests**

Run: `cd frontend-school && npm run test:static`

Expected: FAIL because the supervision page still uses `ratingLabel` and `textLabel`.

### Task 3: Replace Basic Template Dialog With Rubric Builder

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte`

- [ ] **Step 1: Update imports and template form state**

Import helper functions from `$lib/utils/supervision-rubric`. Replace `templateForm.ratingLabel` and `templateForm.textLabel` with `templateForm.sections`.

- [ ] **Step 2: Add template builder actions**

Implement page-local functions:
- `resetTemplateForm()`
- `loadPaperTemplatePreset()`
- `addTemplateSection()`
- `removeTemplateSection(sectionLocalId)`
- `moveTemplateSection(sectionLocalId, direction)`
- `addTemplateItem(sectionLocalId, itemType)`
- `removeTemplateItem(sectionLocalId, itemLocalId)`
- `moveTemplateItem(sectionLocalId, itemLocalId, direction)`

- [ ] **Step 3: Update `createTemplate()` payload**

Build `CreateSupervisionTemplateRequest.sections` from `templateForm.sections`, preserving `sortOrder`, item labels, descriptions, item type, and required flags.

- [ ] **Step 4: Replace dialog markup**

Use local shadcn-svelte components. The dialog must include:
- title/description/status/rating min/max
- "โหลดแบบฟอร์มนิเทศมาตรฐาน" button
- list of sections and nested items
- add/remove/up/down controls
- rating/text item type labels

- [ ] **Step 5: Run static tests**

Run: `cd frontend-school && npm run test:static`

Expected: PASS.

### Task 4: Improve Evaluation Rating UX and Draft Summary

**Files:**
- Modify: `frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte`

- [ ] **Step 1: Use score helper in evaluation panel**

Add derived summary:
- `selectedEvaluationDraftSummary`
- per-section `sectionRubricProgress(section, responseDrafts)`

- [ ] **Step 2: Replace number input with 1-5 selector buttons**

For rating items, render buttons from `ratingMin` to `ratingMax`. Clicking a score updates `responseDrafts[item.id].ratingScore`.

- [ ] **Step 3: Validate required responses before submit**

Before submitting, reject missing required rating/text responses with a toast. Saving drafts can remain lenient.

- [ ] **Step 4: Show total score and quality level**

Show total score, percentage, and quality label above the save/submit buttons.

- [ ] **Step 5: Verify frontend checks**

Run:

```bash
cd frontend-school
npm run test:static
npm run check
npm run lint
```

Expected: all pass.

### Task 5: Final Repository Verification

**Files:**
- Verify changed files only.

- [ ] **Step 1: Run diff check**

Run: `git diff --check`

Expected: no output and exit 0.

- [ ] **Step 2: Review status**

Run: `git status -sb`

Expected: only planned supervision rubric files and the already committed spec/plan state.
