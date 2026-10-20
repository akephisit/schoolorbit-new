import assert from 'node:assert/strict';
import test from 'node:test';
import {
	createCatalogVersionDraft,
	subjectVersionSource
} from '../../src/lib/academic-core/catalog-version-draft.ts';
import type { SubjectVersion } from '../../src/lib/api/academic-core.ts';

const published: SubjectVersion = {
	id: 'v1',
	subjectId: 'subject-1',
	versionNo: 1,
	nameTh: 'คณิตศาสตร์',
	nameEn: 'Math',
	credit: '1.50',
	description: 'คำอธิบายเดิม',
	effectiveFrom: '2026-05-01',
	effectiveUntil: '2027-03-31',
	gradeLevelIds: ['grade-1'],
	hoursPerSemester: 60,
	periodsPerWeek: 3,
	subjectType: 'BASIC',
	termCode: '1',
	status: 'published',
	rowVersion: 1,
	migrated: false,
	publishedAt: '2026-05-01T00:00:00Z',
	createdAt: '2026-05-01T00:00:00Z',
	updatedAt: '2026-05-01T00:00:00Z'
};

test('a grade-only revision preserves source dates, decimal text and hidden values without editing published data', () => {
	const original = structuredClone(published);
	const draft = createCatalogVersionDraft(published);
	draft.gradeLevelIds.push('grade-2');
	assert.deepEqual(draft, {
		name: published.nameTh,
		secondaryName: published.nameEn,
		exactValue: '1.50',
		totalValue: '60',
		standardPeriodsPerWeek: '3',
		effectiveFrom: '2026-05-01',
		effectiveUntil: '2027-03-31',
		gradeLevelIds: ['grade-1', 'grade-2'],
		classification: 'BASIC',
		description: published.description,
		termCode: '1'
	});
	assert.deepEqual(published, original);
});

test('displayed published source wins over newer drafts and scheduled revisions; fallback only considers published versions', () => {
	const future = { ...published, id: 'v2', versionNo: 2, effectiveFrom: '2027-05-01' };
	const draft = { ...published, id: 'v3', versionNo: 3, status: 'draft' as const };
	assert.equal(subjectVersionSource(published, [draft, future, published]), published);
	assert.equal(subjectVersionSource(null, [draft, published, future]), future);
	assert.equal(createCatalogVersionDraft(future).effectiveFrom, '2027-05-01');
	assert.equal(subjectVersionSource(draft, [draft]), null);
});

test('new subject and post-save defaults are independent empty drafts', () => {
	const draft = createCatalogVersionDraft();
	assert.equal(draft.name, '');
	assert.equal(draft.exactValue, '1.00');
	assert.match(draft.effectiveFrom, /^\d{4}-\d{2}-\d{2}$/);
	assert.equal(draft.gradeLevelIds.length, 0);
	draft.gradeLevelIds.push('grade-1');
	assert.deepEqual(createCatalogVersionDraft().gradeLevelIds, []);
});
