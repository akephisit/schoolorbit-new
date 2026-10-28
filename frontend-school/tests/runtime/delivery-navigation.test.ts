import assert from 'node:assert/strict';
import test from 'node:test';
import {
	deliveryOfferingHref,
	deliveryFindingHref,
	deliveryOverviewHref,
	deliveryOverviewReturnHref,
	deliveryOverviewStateHref,
	deliveryReturnHref,
	readDeliveryOverviewState,
	repairedDeliveryOfferingHref
} from '../../src/lib/navigation/learning-delivery.ts';

const origin = 'https://schoolorbit.invalid';
const context = {
	academicYearId: 'year-2',
	academicTermId: 'term-2',
	deliveryVersionId: 'version-2'
};
const overview = new URL(
	`${deliveryOverviewHref(context)}&view=offerings&kind=course&offeringSearch=ค21&subjectGroupId=math&homeroomSearch=1%2F2&readiness=attention&expandedRooms=room-2,room-1`,
	origin
);

test('offering links carry the exact academic context and restore the originating view and both filter sets', () => {
	const returnTo = overview.pathname + overview.search;
	const link = new URL(deliveryOfferingHref('offering', context, returnTo), origin);
	assert.equal(link.pathname, '/staff/academic/delivery/offering');
	assert.equal(link.searchParams.get('academicYearId'), 'year-2');
	assert.equal(link.searchParams.get('academicTermId'), 'term-2');
	assert.equal(link.searchParams.get('deliveryVersionId'), 'version-2');
	assert.equal(deliveryReturnHref(link, context), returnTo);
	assert.deepEqual(readDeliveryOverviewState(overview), {
		view: 'offerings',
		kind: 'course',
		offeringSearch: 'ค21',
		subjectGroupId: 'math',
		homeroomSearch: '1/2',
		readiness: 'attention',
		expandedRooms: ['room-2', 'room-1'],
		missingPage: 0
	});
});

test('presentation updates preserve independent filters and explicit collapsed room state', () => {
	const next = new URL(
		deliveryOverviewStateHref(overview, { view: 'homerooms', expandedRooms: [] }),
		origin
	);
	assert.equal(readDeliveryOverviewState(next).view, 'homerooms');
	assert.equal(readDeliveryOverviewState(next).offeringSearch, 'ค21');
	assert.deepEqual(readDeliveryOverviewState(next).expandedRooms, []);
	assert.equal(
		readDeliveryOverviewState(new URL(deliveryOverviewHref(context), origin)).expandedRooms,
		null
	);
	assert.equal(next.searchParams.get('subjectGroupId'), 'math');
	assert.equal(next.searchParams.get('deliveryVersionId'), 'version-2');
	const reset = new URL(deliveryOverviewStateHref(next, { expandedRooms: null }), origin);
	assert.equal(reset.searchParams.has('expandedRooms'), false);
});

test('overview return serialization uses the actual selected version and drops unrelated query fields', () => {
	const url = new URL(overview);
	url.searchParams.set('deliveryVersionId', 'old-version');
	url.searchParams.set('returnTo', '//outside.invalid');
	const result = new URL(deliveryOverviewReturnHref(url, context), origin);
	assert.equal(result.searchParams.get('deliveryVersionId'), 'version-2');
	assert.equal(result.searchParams.get('returnTo'), null);
	assert.equal(result.searchParams.get('offeringSearch'), 'ค21');
});

test('unsafe, unknown, duplicated or foreign-context returns fall back to the actual academic overview', () => {
	for (const destination of [
		'https://outside.invalid/staff/academic/delivery',
		'//outside.invalid/staff/academic/delivery',
		'/staff/academic/delivery/other?academicYearId=year-2&academicTermId=term-2',
		'/staff/academic/delivery?academicYearId=year-2&academicTermId=term-1',
		`${deliveryOverviewHref(context)}#other`,
		`${deliveryOverviewHref(context)}&returnTo=/other`,
		`${deliveryOverviewHref(context)}&unknown=value`,
		`${deliveryOverviewHref(context)}&view=homerooms&view=offerings`
	]) {
		const detail = new URL(deliveryOfferingHref('offering', context), origin);
		detail.searchParams.set('returnTo', destination);
		assert.equal(deliveryReturnHref(detail, context), deliveryOverviewHref(context), destination);
	}
});

test('legacy or stale context repair preserves only a verified version and a matching return', () => {
	const legacy = new URL('/staff/academic/delivery/offering?deliveryVersionId=version-2', origin);
	const repaired = new URL(repairedDeliveryOfferingHref(legacy, context)!, origin);
	assert.equal(repaired.searchParams.get('academicYearId'), 'year-2');
	assert.equal(repaired.searchParams.get('academicTermId'), 'term-2');
	assert.equal(repaired.searchParams.get('deliveryVersionId'), 'version-2');
	const stale = new URL(
		'/staff/academic/delivery/offering?academicYearId=year-1&academicTermId=term-1&deliveryVersionId=foreign',
		origin
	);
	stale.searchParams.set(
		'returnTo',
		'/staff/academic/delivery?academicYearId=year-1&academicTermId=term-1'
	);
	const corrected = new URL(
		repairedDeliveryOfferingHref(stale, { ...context, deliveryVersionId: null })!,
		origin
	);
	assert.equal(corrected.searchParams.get('deliveryVersionId'), null);
	assert.equal(corrected.searchParams.get('returnTo'), null);
	assert.equal(repairedDeliveryOfferingHref(corrected, context), null);
});

test('readiness offering links add academic and origin context while retaining the finding target version', () => {
	const returnTo = overview.pathname + overview.search;
	const target = new URL(
		deliveryFindingHref(
			'/staff/academic/delivery/offering?deliveryVersionId=target-version&groupId=group',
			'offering',
			context,
			returnTo
		),
		origin
	);
	assert.equal(target.searchParams.get('academicYearId'), context.academicYearId);
	assert.equal(target.searchParams.get('academicTermId'), context.academicTermId);
	assert.equal(target.searchParams.get('deliveryVersionId'), 'target-version');
	assert.equal(target.searchParams.get('groupId'), 'group');
	assert.equal(target.searchParams.get('returnTo'), returnTo);
	for (const route of [
		'/staff/academic/timetable?version=table',
		'/staff/academic/delivery/other?deliveryVersionId=version',
		'https://outside.invalid/staff/academic/delivery/offering'
	])
		assert.equal(deliveryFindingHref(route, 'offering', context, returnTo), route);
});

test('missing requirement pagination persists with return context and resets only for its shared filters', () => {
	const paged = new URL(deliveryOverviewStateHref(overview, { missingPage: 2 }), origin);
	assert.equal(readDeliveryOverviewState(paged).missingPage, 2);
	const detail = new URL(
		deliveryOfferingHref('offering', context, paged.pathname + paged.search),
		origin
	);
	assert.equal(deliveryReturnHref(detail, context), paged.pathname + paged.search);
	for (const patch of [{ kind: 'activity' as const }, { offeringSearch: 'ใหม่' }]) {
		const changed = new URL(deliveryOverviewStateHref(paged, patch), origin);
		assert.equal(readDeliveryOverviewState(changed).missingPage, 0);
		assert.equal(changed.searchParams.has('missingPage'), false);
	}
	const switched = new URL(
		deliveryOverviewStateHref(paged, { view: 'homerooms', subjectGroupId: 'other' }),
		origin
	);
	assert.equal(readDeliveryOverviewState(switched).missingPage, 2);
	for (const value of ['-1', '1.5', 'NaN', 'Infinity', '9007199254740992']) {
		const invalid = new URL(paged);
		invalid.searchParams.set('missingPage', value);
		assert.equal(readDeliveryOverviewState(invalid).missingPage, 0);
	}
	const reset = new URL(deliveryOverviewStateHref(paged, { missingPage: 0 }), origin);
	assert.equal(reset.searchParams.has('missingPage'), false);
});
