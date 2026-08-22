import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const projectRoot = new URL('../../', import.meta.url);

async function source(relativePath) {
	return readFile(new URL(relativePath, projectRoot), 'utf8');
}

const componentFiles = [
	'src/lib/components/certificates/CertificateSubmitRequestDialog.svelte',
	'src/lib/components/certificates/CertificateCampaignRequests.svelte',
	'src/lib/components/certificates/CertificateIssueQueue.svelte',
	'src/lib/components/certificates/CertificateIssueRequestReview.svelte',
	'src/lib/components/certificates/CertificateRecipientWorkspace.svelte',
	'src/lib/components/certificates/CertificateCandidateTable.svelte'
];

const routeFiles = [
	'src/routes/(app)/staff/certificates/[campaignId]/requests/+page.ts',
	'src/routes/(app)/staff/certificates/[campaignId]/requests/+page.svelte',
	'src/routes/(app)/staff/certificate-requests/+page.ts',
	'src/routes/(app)/staff/certificate-requests/+page.svelte',
	'src/routes/(app)/staff/certificate-requests/[requestId]/+page.ts',
	'src/routes/(app)/staff/certificate-requests/[requestId]/+page.svelte'
];

test('certificate request routes keep preparation and school issue scopes separate', async () => {
	const campaignRoute = await source(
		'src/routes/(app)/staff/certificates/[campaignId]/requests/+page.ts'
	);
	for (const permission of [
		'PERMISSIONS.CERTIFICATE_SUBMIT_ORGANIZATION_UNIT',
		'PERMISSIONS.CERTIFICATE_SUBMIT_SCHOOL'
	]) {
		assert.match(campaignRoute, new RegExp(permission.replaceAll('.', '\\.')));
	}
	assert.doesNotMatch(campaignRoute, /CERTIFICATE_ISSUE_SCHOOL/);

	for (const route of [
		'src/routes/(app)/staff/certificate-requests/+page.ts',
		'src/routes/(app)/staff/certificate-requests/[requestId]/+page.ts'
	]) {
		const routeSource = await source(route);
		assert.match(routeSource, /PERMISSIONS\.CERTIFICATE_ISSUE_SCHOOL/);
		assert.doesNotMatch(routeSource, /CERTIFICATE_(?:SUBMIT|UPDATE)_/);
	}
});

test('request UI implements submit, history, queue, review, return, and withdrawal states', async () => {
	const combined = (
		await Promise.all([...componentFiles, ...routeFiles].map((file) => source(file)))
	).join('\n');
	for (const symbol of [
		'submitCertificateIssueRequest',
		'listCertificateCampaignIssueRequests',
		'withdrawCertificateIssueRequest',
		'listCertificateIssueRequests',
		'getCertificateIssueRequest',
		'startCertificateIssueRequestReview',
		'returnCertificateIssueRequest',
		'createCertificateTemplatePreviewManifest'
	]) {
		assert.match(combined, new RegExp(symbol), `missing request workflow boundary: ${symbol}`);
	}
	for (const status of ['pending', 'reviewing', 'returned', 'withdrawn']) {
		assert.match(combined, new RegExp(status), `missing request status: ${status}`);
	}
	for (const label of [
		'ส่งคำขอออกเกียรติบัตร',
		'ประวัติคำขอออกเกียรติบัตร',
		'คิวตรวจคำขอออกเกียรติบัตร',
		'เริ่มตรวจคำขอ',
		'ส่งกลับให้แก้ไข',
		'ถอนคำขอ'
	]) {
		assert.match(combined, new RegExp(label), `missing request workflow label: ${label}`);
	}
	assert.match(combined, /candidate\.validationStatus\s*===\s*['"]ready['"]/);
	assert.match(combined, /canSubmit/);
});

test('school review is read-only and queue rows do not expose recipient names', async () => {
	const review = await source(
		'src/lib/components/certificates/CertificateIssueRequestReview.svelte'
	);
	for (const forbiddenMutation of [
		'updateCertificateCandidate',
		'bulkUpdateCertificateCandidates',
		'updateCertificateTemplate',
		'updateCertificateCampaign',
		'deleteCertificateCandidate'
	]) {
		assert.doesNotMatch(review, new RegExp(forbiddenMutation));
	}
	assert.doesNotMatch(review, /issueCertificateRequest/);
	assert.match(review, /canIssue/);
	assert.match(review, /previewKind:\s*['"]candidate['"]/);
	assert.match(review, /CertificatePreviewDialog/);
	assert.doesNotMatch(review, /window\.innerWidth/);
	assert.doesNotMatch(review, /max-w-none/);

	const queue = await source('src/lib/components/certificates/CertificateIssueQueue.svelte');
	assert.doesNotMatch(queue, /request\.items/);
	for (const field of [
		'ownerOrganizationUnitName',
		'submittedByName',
		'itemCount',
		'templateCount',
		'submittedAt'
	]) {
		assert.match(queue, new RegExp(field));
	}
});

test('review detail waits for exact issue permission before loading recipient rows', async () => {
	const route = await source(
		'src/routes/(app)/staff/certificate-requests/[requestId]/+page.svelte'
	);
	assert.match(route, /\$can\.has\(PERMISSIONS\.CERTIFICATE_ISSUE_SCHOOL\)/);
	assert.match(route, /canIssue/);
	const review = await source(
		'src/lib/components/certificates/CertificateIssueRequestReview.svelte'
	);
	assert.match(review, /if\s*\(!canIssue\)/);
	assert.match(review, /getCertificateIssueRequest/);
});
