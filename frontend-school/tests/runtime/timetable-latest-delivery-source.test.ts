import assert from 'node:assert/strict';
import test from 'node:test';
import {
	initialTimetablePublicationDate,
	draftSaveOutcome,
	timetableSourceIsCurrent
} from '../../src/lib/academic/timetable/latest-delivery-source.ts';
import { collaborationWorkspace } from './timetable-collaboration-fixture.ts';
import { timetableIds } from '../e2e/timetable-test-harness.ts';

test('publication starts no earlier than today, latest opening or the active-history guard', () => {
	const workspace = collaborationWorkspace();
	workspace.publicationDateRules = { today: '2026-11-01', minimumEffectiveFrom: null };
	workspace.latestDeliveryPublication = {
		...workspace.latestDeliveryPublication,
		effectiveFrom: '2026-11-05'
	};
	assert.equal(initialTimetablePublicationDate(workspace), '2026-11-05');
	workspace.publicationDateRules.minimumEffectiveFrom = '2026-11-08';
	assert.equal(initialTimetablePublicationDate(workspace), '2026-11-08');
	workspace.publicationDateRules = { today: '2026-11-10', minimumEffectiveFrom: '2026-11-08' };
	assert.equal(initialTimetablePublicationDate(workspace), '2026-11-10');
	assert.equal(workspace.latestDeliveryPublication.effectiveFrom, '2026-11-05');
});

test('drafts require the exact latest pin while published history retains its original pin', () => {
	const workspace = collaborationWorkspace();
	assert.equal(timetableSourceIsCurrent(workspace), true);
	workspace.latestDeliveryVersionId = timetableIds.newDeliveryVersion;
	assert.equal(timetableSourceIsCurrent(workspace), false);
	workspace.version.status = 'published';
	assert.equal(timetableSourceIsCurrent(workspace), true);
	assert.equal(workspace.version.deliveryVersionId, timetableIds.deliveryVersion);
});

test('only dispatched writes without definitive rejection remain ambiguous', () => {
	assert.equal(draftSaveOutcome(false, false), 'not_submitted');
	assert.equal(draftSaveOutcome(true, false), 'unknown');
	assert.equal(draftSaveOutcome(false, true), 'source_rejected');
	assert.equal(draftSaveOutcome(true, true), 'source_rejected');
});
