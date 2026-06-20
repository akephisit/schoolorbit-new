import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
	calculateRubricDraftSummary,
	createPaperSupervisionRubricSections,
	qualityLevelFromPercentage,
	sectionRubricProgress
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

	it('calculates section score from selected ratings instead of answered count', () => {
		const [section] = createPaperSupervisionRubricSections();
		const drafts = {
			[section.items[0].localId]: { ratingScore: '5', textResponse: '' },
			[section.items[1].localId]: { ratingScore: '4', textResponse: '' },
			[section.items[2].localId]: { ratingScore: '2', textResponse: '' }
		};

		const progress = sectionRubricProgress(section, drafts, 5);

		assert.equal(progress.ratingCount, 3);
		assert.equal(progress.answeredRatingCount, 3);
		assert.equal(progress.totalScore, 11);
		assert.equal(progress.maxScore, 15);
		assert.equal(progress.percentage, 73.33);
		assert.equal(progress.qualityLabel, 'พอใช้');
	});

	it('maps paper-form quality thresholds', () => {
		assert.equal(qualityLevelFromPercentage(90), 'ดีมาก');
		assert.equal(qualityLevelFromPercentage(80), 'ดี');
		assert.equal(qualityLevelFromPercentage(70), 'พอใช้');
		assert.equal(qualityLevelFromPercentage(60), 'ควรปรับปรุง');
		assert.equal(qualityLevelFromPercentage(59.99), 'ไม่ผ่าน');
	});
});
