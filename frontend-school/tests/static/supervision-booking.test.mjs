import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

test('teaching supervision booking uses a weekly timetable grid with exact observed dates', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);
	const supervisionModels = await readRepoFile('backend-school/src/modules/supervision/models.rs');
	const supervisionService = await readRepoFile(
		'backend-school/src/modules/supervision/services.rs'
	);
	const migration = await readRepoFile('backend-school/migrations/008_supervision_observed_at.sql');

	assert.match(supervisionApi, /observedAt\?:\s*string\s*\|\s*null/);
	assert.match(supervisionPage, /currentBookingCycle/);
	assert.match(supervisionPage, /bookingWeekStartDate/);
	assert.match(supervisionPage, /selectedBookingDate/);
	assert.match(supervisionPage, /goToPreviousBookingWeek/);
	assert.match(supervisionPage, /goToNextBookingWeek/);
	assert.match(supervisionPage, /timetableObservedAt\(/);
	assert.match(supervisionPage, /observationForTimetableCell\(/);
	assert.match(supervisionPage, /statusLabel\(cellObservation\.status\)/);
	assert.match(supervisionPage, /observedAt:[\s\S]*manualMode[\s\S]*timetableObservedAt/);
	assert.doesNotMatch(
		supervisionPage,
		/<Select\.Root\s+type="single"\s+bind:value=\{selectedCycleId\}/
	);

	assert.match(supervisionModels, /pub\s+observed_at:\s*DateTime<Utc>/);
	assert.match(supervisionModels, /pub\s+observed_at:\s*Option<DateTime<Utc>>/);
	assert.match(supervisionService, /day_of_week_matches_observed_at/);
	assert.match(supervisionService, /validate_observed_at_in_cycle/);
	assert.match(migration, /ADD COLUMN observed_at timestamp with time zone/);
	assert.match(migration, /DROP COLUMN manual_observed_at/);
});

test('teaching supervision templates expose a read-only form preview', async () => {
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);

	assert.match(supervisionPage, /previewTemplateDialogOpen/);
	assert.match(supervisionPage, /openTemplatePreviewDialog\(template\)/);
	assert.match(supervisionPage, />\s*ดูตัวอย่าง\s*</);
	assert.match(supervisionPage, /<Dialog\.Title>ตัวอย่างแบบประเมินนิเทศ<\/Dialog\.Title>/);
	assert.match(supervisionPage, /templateRatingColumns\(previewTemplate\)/);
	assert.match(supervisionPage, /\{#each previewTemplate\.sections as section/);
	assert.match(supervisionPage, /\{#each section\.items as item/);
	assert.match(supervisionPage, /aria-label=\{`ช่องคะแนน \$\{score\}/);
	assert.match(supervisionPage, /readonly/i);
});
