import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
test('campaign list selected identity and creation options are direct route-owned reads', async () => {
	for (const [route, names] of [
		['staff/certificates', ['listCertificateCampaigns']],
		['staff/certificates/new', ['lookupAcademicYears', 'listCertificateOwnerOptions']],
		['staff/certificates/[campaignId]/overview', ['getCertificateCampaign']]
	]) {
		const loader = await source(`routes/(app)/${route}/+page.ts`);
		for (const name of names) assert.ok(loader.includes(name));
		assert.match(loader, /captureRouteLoad/);
		assert.match(loader, /school:app-identity/);
		assert.doesNotMatch(await source(`routes/(app)/${route}/+page.svelte`), /onMount/);
	}
});
test('overview optional editor starts independent references after opening', async () => {
	const page = await source('routes/(app)/staff/certificates/[campaignId]/overview/+page.svelte');
	assert.match(page, /LatestRequest/);
	assert.match(page, /editOpen/);
	assert.doesNotMatch(page, /editReferencesLoaded|loadingEditReferences/);
});
