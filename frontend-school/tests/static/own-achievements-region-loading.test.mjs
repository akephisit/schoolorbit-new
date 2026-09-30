import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
test('own achievement and certificate routes own their visible exact scope reads', async () => {
	for (const [route, api, permission] of [
		['staff/achievements/self-recorded', 'getAchievements', 'ACHIEVEMENT_READ_OWN'],
		['staff/achievements/issued', 'listOwnCertificates', 'CERTIFICATE_READ_OWN'],
		['student/certificates', 'listOwnCertificates', 'CERTIFICATE_READ_OWN']
	]) {
		const loader = await source(`routes/(app)/${route}/+page.ts`);
		assert.ok(loader.includes(api));
		assert.ok(loader.includes(permission));
		assert.match(loader, /school:app-identity/);
		assert.match(loader, /captureRouteLoad/);
	}
	for (const path of [
		'lib/components/achievement/SelfRecordedAchievements.svelte',
		'lib/components/certificates/MyCertificateList.svelte'
	])
		assert.doesNotMatch(await source(path), /onMount/);
});
test('achievement assignee choices use opened bounded minimal lookup', async () => {
	const dialog = await source('lib/components/achievement/AchievementDialog.svelte');
	assert.match(dialog, /lookupStaff/);
	assert.doesNotMatch(dialog, /listStaff|page_size: 1000/);
	assert.match(dialog, /LatestRequest/);
});

test('own achievement navigation keeps personal lists tap-only', async () => {
	const layout = await source('routes/(app)/staff/achievements/+layout.svelte');
	assert.equal((layout.match(/data-sveltekit-preload-data="tap"/g) ?? []).length, 2);
	const policy = await source('lib/navigation/menu-preload.ts');
	assert.match(policy, /path\.startsWith\('\/staff\/achievements'\)/);
	assert.ok(policy.includes("'/student/certificates'"));
});
