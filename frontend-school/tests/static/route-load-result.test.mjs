import assert from 'node:assert/strict';
import test from 'node:test';
import { readLearningDeliveryRouteContext } from '../../src/lib/academic/learning-delivery-page.ts';
import { captureRouteLoad } from '../../src/lib/navigation/route-load.ts';

test('captureRouteLoad preserves typed success data', async () => {
	assert.deepEqual(await captureRouteLoad(Promise.resolve({ id: 'one' }), 'โหลดไม่สำเร็จ'), {
		ok: true,
		data: { id: 'one' },
		error: null
	});
});

test('captureRouteLoad returns the concrete error message or fallback', async () => {
	assert.deepEqual(
		await captureRouteLoad(Promise.reject(new Error('เครือข่ายช้า')), 'โหลดไม่สำเร็จ'),
		{
			ok: false,
			data: null,
			error: 'เครือข่ายช้า'
		}
	);
	assert.equal(
		(await captureRouteLoad(Promise.reject('failed'), 'โหลดไม่สำเร็จ')).error,
		'โหลดไม่สำเร็จ'
	);
});

test('delivery route context requires year and term and preserves optional selections', () => {
	assert.equal(
		readLearningDeliveryRouteContext(new URL('https://school.test/staff/academic/delivery')),
		null
	);
	assert.deepEqual(
		readLearningDeliveryRouteContext(
			new URL(
				'https://school.test/staff/academic/delivery?academicYearId=year-1&academicTermId=term-1'
			)
		),
		{
			academicYearId: 'year-1',
			academicTermId: 'term-1',
			timetableVersionId: undefined,
			changeSetId: undefined
		}
	);
	assert.deepEqual(
		readLearningDeliveryRouteContext(
			new URL(
				'https://school.test/staff/academic/delivery?academicYearId=year-1&academicTermId=term-1&timetableVersionId=version-1&changeSetId=change-set-1'
			)
		),
		{
			academicYearId: 'year-1',
			academicTermId: 'term-1',
			timetableVersionId: 'version-1',
			changeSetId: 'change-set-1'
		}
	);
});
