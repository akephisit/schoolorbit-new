import assert from 'node:assert/strict';
import test from 'node:test';
import {
	compareDeliveryOfferings,
	deliveryOfferingLabel,
	deliverySubjectGroupOptions,
	filterDeliveryOfferings
} from '../../src/lib/academic/learning-delivery-presentation.ts';
import {
	navigationIds,
	navigationOfferingIds,
	navigationVersion
} from '../e2e/fixtures/delivery-navigation-route-data.ts';

test('offerings use natural Thai course-code order and activities use names without technical codes', () => {
	const version = navigationVersion();
	const before = version.snapshot.offerings.map((item) => item.id);
	const filters = { kind: 'all' as const, offeringSearch: '', subjectGroupId: 'all' };
	assert.deepEqual(filterDeliveryOfferings(version, filters).map(deliveryOfferingLabel), [
		'ค20101 · คณิตศาสตร์เพิ่มเติม',
		'ค21101 · คณิตศาสตร์ต้น',
		'ค23101 · คณิตศาสตร์ปลาย',
		'ท11101 · ภาษาไทยเก่า',
		'ชุมนุม',
		'แนะแนว'
	]);
	const course21 = version.snapshot.offerings.find(
		(item) => item.id === navigationOfferingIds.basic21
	)!;
	const course23 = version.snapshot.offerings.find(
		(item) => item.id === navigationOfferingIds.basic23
	)!;
	assert.ok(compareDeliveryOfferings(course21, course23) < 0);
	assert.deepEqual(
		version.snapshot.offerings.map((item) => item.id),
		before
	);
	assert.deepEqual(
		filterDeliveryOfferings(version, { ...filters, offeringSearch: 'ACT-TECHNICAL' }),
		[]
	);
	assert.equal(
		filterDeliveryOfferings(version, { ...filters, offeringSearch: 'ชุมนุม' })[0].kind,
		'activity'
	);
});

test('subject-group options and filtering come from metadata for the exact visible pinned course versions', () => {
	const version = navigationVersion();
	version.courseMetadata.push({
		subjectVersionId: 'not-visible',
		subjectGroupId: 'other',
		subjectGroupName: 'ไม่เกี่ยวข้อง'
	});
	assert.deepEqual(deliverySubjectGroupOptions(version), [
		{ id: navigationIds.math, name: 'คณิตศาสตร์' },
		{ id: navigationIds.thai, name: 'ภาษาไทย' }
	]);
	const filters = { kind: 'all' as const, offeringSearch: '', subjectGroupId: navigationIds.math };
	assert.deepEqual(
		filterDeliveryOfferings(version, filters).map((item) => item.id),
		[navigationOfferingIds.additional, navigationOfferingIds.basic21, navigationOfferingIds.basic23]
	);
	assert.deepEqual(filterDeliveryOfferings(version, { ...filters, kind: 'activity' }), []);
	assert.deepEqual(filterDeliveryOfferings(version, { ...filters, subjectGroupId: 'other' }), []);
	assert.equal(
		filterDeliveryOfferings(version, { ...filters, offeringSearch: 'ค21' })[0].id,
		navigationOfferingIds.basic21
	);
});
