import assert from 'node:assert/strict';
import test from 'node:test';
import { readFile } from 'node:fs/promises';
const source = (path) => readFile(new URL(`../../src/${path}`, import.meta.url), 'utf8');
test('request queue detail history and issued records have direct exact route owners', async () => {
	for (const [route, api] of [
		['staff/certificate-requests', 'listCertificateIssueRequests'],
		['staff/certificate-requests/[requestId]', 'getCertificateIssueRequest'],
		['staff/certificates/[campaignId]/requests', 'listCertificateCampaignIssueRequests'],
		['staff/certificates/[campaignId]/issued', 'listIssuedCertificates']
	]) {
		const loader = await source(`routes/(app)/${route}/+page.ts`);
		assert.match(loader, /school:app-identity/);
		assert.match(loader, /captureRouteLoad/);
		assert.ok(loader.includes(api));
		assert.match(loader, /requestFetch: fetch/);
	}
	for (const component of [
		'CertificateIssueQueue',
		'CertificateIssueRequestReview',
		'CertificateCampaignRequests',
		'CertificateIssuedTable'
	]) {
		const child = await source(`lib/components/certificates/${component}.svelte`);
		assert.doesNotMatch(child, /onMount|afterNavigate/);
		assert.match(child, /LatestRequest/);
		assert.match(child, /\$effect\.pre/);
	}
});
test('request history readers do not require submission and no unopened batch renderer is mounted', async () => {
	const route = await source('routes/(app)/staff/certificates/[campaignId]/requests/+page.ts');
	assert.match(route, /CERTIFICATE_READ_ORGANIZATION_UNIT/);
	assert.match(route, /CERTIFICATE_READ_SCHOOL/);
	assert.doesNotMatch(route, /CERTIFICATE_SUBMIT_/);
	const table = await source('lib/components/certificates/CertificateIssuedTable.svelte');
	assert.match(table, /\{#if batchOpen[^}]*\}[\s\S]*<CertificateBatchDownloadDialog/);
});
