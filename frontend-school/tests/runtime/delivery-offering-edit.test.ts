import assert from 'node:assert/strict';
import test from 'node:test';
import {currentDeliveryDate, defaultRosterTrackingDate} from '../../src/lib/academic/learning-delivery-page.ts';

test('current roster defaults follow Bangkok today across UTC midnight and never invent a business start', () => {
	for (const [instant, date] of [
		['2026-10-26T16:59:59Z','2026-10-26'],
		['2026-10-26T17:00:00Z','2026-10-27'],
		['2026-10-27T17:00:00Z','2026-10-28']
	]) {
		const now = new Date(instant);
		assert.equal(currentDeliveryDate(now), date);
		assert.equal(defaultRosterTrackingDate(now), date);
	}
});
