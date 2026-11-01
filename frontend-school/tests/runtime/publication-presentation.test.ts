import assert from 'node:assert/strict';
import test from 'node:test';
import {
	deliveryPublicationLabel,
	publicationDate,
	publicationEffectiveLabel,
	publicationSourceLabel,
	publicationTime,
	timetablePublicationLabel
} from '../../src/lib/academic/publication-presentation.ts';
import { selectDeliveryVersion } from '../../src/lib/academic/learning-delivery-page.ts';
import {
	selectPreferredBoardVersion,
	selectPreferredTemplateVersion
} from '../../src/lib/academic/timetable/version-selection.ts';
import { navigationSummary } from '../e2e/fixtures/delivery-navigation-route-data.ts';
import { makeDeliveryPublication, makeTimetableVersion } from '../e2e/timetable-test-harness.ts';

test('same-day publication labels distinguish accepted order and Bangkok publication time', () => {
	const first = {
		...navigationSummary(),
		status: 'published' as const,
		publicationNo: 5,
		effectiveFrom: '2026-11-01',
		publishedAt: '2026-10-31T18:15:00Z'
	};
	const second = { ...first, publicationNo: 6, publishedAt: '2026-11-01T04:30:00Z' };
	assert.match(deliveryPublicationLabel(first), /รุ่นที่ 5/);
	assert.match(deliveryPublicationLabel(second), /รุ่นที่ 6/);
	assert.ok(deliveryPublicationLabel(second).includes(publicationDate('2026-11-01')));
	assert.ok(deliveryPublicationLabel(first).includes(publicationTime(first.publishedAt)));
	assert.match(publicationTime(first.publishedAt), /1\/11\/69.*01:15/);
	assert.doesNotMatch(
		deliveryPublicationLabel(navigationSummary()),
		/รุ่นที่ null|รุ่นที่ 0|undefined/
	);
});

test('opening planning selection follows publication order even when the newest start is earlier', () => {
	const old = {
		...navigationSummary(),
		id: 'nov5',
		status: 'published' as const,
		publicationNo: 5,
		effectiveFrom: '2026-11-05',
		effectiveUntil: '2026-10-31',
		isSuperseded: true
	};
	const replacement = {
		...old,
		id: 'nov1',
		publicationNo: 6,
		effectiveFrom: '2026-11-01',
		effectiveUntil: null,
		isSuperseded: false
	};
	assert.equal(selectDeliveryVersion([old, replacement])?.id, replacement.id);
	assert.equal(selectDeliveryVersion([old, replacement], old.id)?.id, old.id);
	assert.equal(selectDeliveryVersion([old]), null);
});

test('superseded timetable plans remain explicitly accessible and cannot become defaults', () => {
	const superseded = {
		...makeTimetableVersion('published'),
		id: 'superseded',
		publicationNo: 5,
		effectiveFrom: '2026-11-05',
		effectiveUntil: '2026-10-31',
		displayState: 'superseded' as const
	};
	const current = {
		...superseded,
		id: 'current',
		publicationNo: 6,
		effectiveFrom: '2026-11-01',
		effectiveUntil: null,
		displayState: 'current' as const
	};
	const future = {
		...current,
		id: 'future',
		publicationNo: 7,
		effectiveFrom: '2026-11-10',
		displayState: 'upcoming' as const
	};
	for (const select of [selectPreferredBoardVersion, selectPreferredTemplateVersion]) {
		assert.equal(select([superseded, future, current], null)?.id, current.id);
		assert.equal(select([superseded, future, current], superseded.id)?.id, superseded.id);
		assert.equal(select([superseded], null), null);
	}
	assert.equal(
		publicationEffectiveLabel({ ...superseded, isSuperseded: true }),
		'แผนนี้ถูกแทนที่แล้ว'
	);
	assert.match(timetablePublicationLabel(superseded), /ถูกแทนที่.*รุ่นที่ 5/);
});

test('source labels retain exact publication evidence and disclose a missing historical timestamp', () => {
	const source = makeDeliveryPublication('exact-source', 6, '2026-11-05');
	assert.match(publicationSourceLabel(source), /รุ่นที่ 6/);
	assert.ok(publicationSourceLabel(source).includes(publicationDate('2026-11-05')));
	assert.doesNotMatch(publicationSourceLabel(source), /exact-source/);
	assert.match(
		publicationSourceLabel({ ...source, publishedAt: null }),
		/ไม่พบเวลาเผยแพร่ที่บันทึกไว้/
	);
});
