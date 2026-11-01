import assert from 'node:assert/strict';
import test from 'node:test';
import {
	currentDeliveryDate,
	defaultRosterTrackingDate,
	deliveryVersionEffectiveLabel,
	weeklyPeriodAdjustmentRequest
} from '../../src/lib/academic/learning-delivery-page.ts';

const offering = { id: 'offering', kind: 'course' as const };
const adjustment = {
	id: 'adjustment',
	actionKind: 'adjust_weekly_period_target' as const,
	learningOfferingId: offering.id,
	weeklyPeriodTarget: 3,
	rowVersion: 5,
	createdAt: '2026-10-27T00:00:00Z',
	updatedAt: '2026-10-27T00:00:00Z',
	createdBy: 'actor'
};

test('tracking defaults use Bangkok today for published resources and retain explicit draft preparation dates', () => {
	const now = new Date('2026-10-26T18:00:00Z');
	assert.equal(currentDeliveryDate(now), '2026-10-27');
	for (const status of ['draft', 'published'] as const) {
		assert.equal(defaultRosterTrackingDate(status, undefined, now), '2026-10-27');
		assert.equal(defaultRosterTrackingDate(status, null, now), '2026-10-27');
		assert.equal(defaultRosterTrackingDate(status, '2026-05-01', now), '2026-10-27');
	}
	assert.equal(defaultRosterTrackingDate('published', '2026-11-01', now), '2026-10-27');
	assert.equal(defaultRosterTrackingDate('draft', '2026-11-01', now), '2026-11-01');
	assert.equal(currentDeliveryDate(new Date('2026-10-27T17:00:00Z')), '2026-10-28');
});

test('undated drafts explain publication dates without showing null', () => {
	assert.equal(deliveryVersionEffectiveLabel(null), 'เลือกวันเริ่มใช้ตอนเผยแพร่');
	assert.equal(deliveryVersionEffectiveLabel('2026-10-27'), 'เริ่มใช้ 2026-10-27');
});

test('period adjustment uses the exact offering item revision and current change-set revision', () => {
	const changeSet = {
		status: 'draft' as const,
		rowVersion: 12,
		items: [
			{ ...adjustment, id: 'other', learningOfferingId: 'other-offering', rowVersion: 20 },
			adjustment
		]
	};
	assert.deepEqual(weeklyPeriodAdjustmentRequest(changeSet, offering, 0), {
		action: 'adjust_weekly_period_target',
		changeSetRowVersion: 12,
		itemRowVersion: 5,
		learningOfferingId: offering.id,
		weeklyPeriodTarget: 0
	});
	assert.equal(changeSet.items[1].weeklyPeriodTarget, 3);
	assert.equal(changeSet.items[0].rowVersion, 20);
	assert.equal(
		weeklyPeriodAdjustmentRequest({ ...changeSet, items: [] }, offering, 4)?.itemRowVersion,
		null
	);
	for (const kind of ['course', 'activity'] as const) {
		const target = { ...offering, kind };
		assert.deepEqual(weeklyPeriodAdjustmentRequest(changeSet, target, 0), {
			action: 'adjust_weekly_period_target',
			changeSetRowVersion: 12,
			itemRowVersion: 5,
			learningOfferingId: offering.id,
			weeklyPeriodTarget: 0
		});
		assert.equal(weeklyPeriodAdjustmentRequest(changeSet, target, 1)?.weeklyPeriodTarget, 1);
		assert.equal(
			weeklyPeriodAdjustmentRequest({ ...changeSet, status: 'published' }, target, 0),
			null
		);
		for (const invalid of [-1, 1.5, Number.NaN, Number.POSITIVE_INFINITY]) {
			assert.equal(weeklyPeriodAdjustmentRequest(changeSet, target, invalid), null);
		}
	}
});
