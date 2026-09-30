import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
for (const [route, read, permission] of [
	['features', 'listFeatures', 'FEATURES_READ_ALL'],
	['school-settings', 'getSchoolSettings', 'SETTINGS_READ_ALL'],
	['school-fonts', 'listSchoolFonts', 'FONT_MANAGE_SCHOOL']
]) {
	test(`${route} owns its primary read in the route rather than a mounted child`, async () => {
		const loader = await source(`routes/(app)/staff/${route}/+page.ts`);
		assert.match(loader, new RegExp(`${read}\\(\\{ requestFetch: fetch \\}\\)`));
		assert.match(loader, new RegExp(`PERMISSIONS\\.${permission}`));
		assert.match(loader, /school:app-identity/);
		const consumer = await source(
			route === 'school-fonts'
				? 'lib/components/school-fonts/SchoolFontLibrary.svelte'
				: `routes/(app)/staff/${route}/+page.svelte`
		);
		assert.doesNotMatch(consumer, /onMount/);
		assert.match(consumer, /LatestRequest/);
		assert.match(consumer, /\$effect\.pre/);
	});
}
