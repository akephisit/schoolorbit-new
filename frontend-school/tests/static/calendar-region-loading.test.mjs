import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
test('calendar route owns concurrent events, categories and tags with event fetch', async () => {
	const loader = await source('routes/(app)/staff/calendar/+page.ts');
	for (const read of ['listCalendarEvents', 'listCalendarCategories', 'listCalendarTags'])
		assert.match(loader, new RegExp(read));
	assert.match(loader, /school:app-identity/);
	assert.match(loader, /requestFetch: fetch/);
	assert.match(loader, /captureRouteLoad/);
});
test('calendar page separates catalog retries from event context and optional target reads', async () => {
	const page = await source('routes/(app)/staff/calendar/+page.svelte');
	assert.doesNotMatch(page, /onMount|calendarLoadSequence/);
	assert.match(page, /eventsRequest/);
	assert.match(page, /categoriesRequest/);
	assert.match(page, /tagsRequest/);
	assert.match(page, /\$effect\.pre/);
	assert.match(page, /pushState/);
	assert.match(page, /eventDialogOpen && canManageCalendar/);
});
