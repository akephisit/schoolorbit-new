import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import test from 'node:test';

const projectRoot = new URL('../../', import.meta.url);

async function source(relativePath) {
	return readFile(new URL(relativePath, projectRoot), 'utf8');
}

const issuedComponents = [
	'src/lib/components/certificates/CertificateIssuedTable.svelte',
	'src/lib/components/certificates/CertificateIssueConfirmationDialog.svelte',
	'src/lib/components/certificates/CertificateRevokeDialog.svelte',
	'src/lib/components/certificates/CertificateDownloadButton.svelte',
	'src/lib/components/certificates/CertificateBatchDownloadDialog.svelte'
];

test('issued route uses generated read, download, and school-only revoke permissions', async () => {
	const route = await source('src/routes/(app)/staff/certificates/[campaignId]/issued/+page.ts');
	assert.match(route, /PERMISSIONS\.CERTIFICATE_READ_ORGANIZATION_UNIT/);
	assert.match(route, /PERMISSIONS\.CERTIFICATE_READ_SCHOOL/);
	assert.doesNotMatch(route, /CERTIFICATE_(?:ISSUE|REVOKE)_SCHOOL/);

	const page = await source('src/routes/(app)/staff/certificates/[campaignId]/issued/+page.svelte');
	for (const permission of [
		'PERMISSIONS.CERTIFICATE_DOWNLOAD_ORGANIZATION_UNIT',
		'PERMISSIONS.CERTIFICATE_DOWNLOAD_SCHOOL',
		'PERMISSIONS.CERTIFICATE_REVOKE_SCHOOL'
	]) {
		assert.match(page, new RegExp(permission.replaceAll('.', '\\.')));
	}
	assert.doesNotMatch(page, /certificate\.revoke\.school/);
});

test('issue approval keeps one browser idempotency key until a typed outcome returns', async () => {
	const review = await source(
		'src/lib/components/certificates/CertificateIssueRequestReview.svelte'
	);
	const dialog = await source(
		'src/lib/components/certificates/CertificateIssueConfirmationDialog.svelte'
	);
	const combined = `${review}\n${dialog}`;

	assert.match(combined, /issueCertificates/);
	assert.match(combined, /crypto\.randomUUID\(\)/);
	assert.match(combined, /idempotencyKey/);
	assert.match(combined, /outcome\s*===\s*['"]issued['"]/);
	assert.match(combined, /outcome\s*===\s*['"]returned['"]/);
	assert.match(combined, /ออกเลขแล้ว/);
	assert.match(combined, /ยังไม่มีเลขเกียรติบัตรถูกจอง/);
});

test('generated issue outcome contract exposes runtime camelCase fields', async () => {
	const generated = await source('src/lib/api/generated/school-api.ts');
	for (const field of [
		'issueRunId',
		'requestId',
		'campaignId',
		'activitySequence',
		'firstCertificateSequence',
		'lastCertificateSequence',
		'issueCodes',
		'candidateProblems'
	]) {
		assert.match(generated, new RegExp(`${field}:`));
	}
	for (const staleField of ['issue_run_id', 'first_certificate_sequence', 'candidate_problems']) {
		assert.doesNotMatch(generated, new RegExp(`${staleField}:`));
	}
});

test('issued UI filters public fields and keeps revoke and download capability-gated', async () => {
	const combined = (await Promise.all(issuedComponents.map((file) => source(file)))).join('\n');

	for (const boundary of [
		'listIssuedCertificates',
		'revokeIssuedCertificate',
		'createIssuedCertificateRenderManifest',
		'createIssuedCertificateRenderManifests',
		'loadCertificateRenderer',
		'downloadCertificatePdf',
		'validateCertificateBatchSize'
	]) {
		assert.match(combined, new RegExp(boundary), `missing issued UI boundary: ${boundary}`);
	}
	assert.match(combined, /certificateNumber/);
	assert.match(combined, /firstName/);
	assert.match(combined, /lastName/);
	assert.match(combined, /templateId/);
	assert.match(combined, /status/);
	assert.match(combined, /capabilities\.canDownload/);
	assert.match(combined, /capabilities\.canRevoke/);
	assert.match(combined, /status\s*===\s*['"]issued['"]/);
	assert.doesNotMatch(combined, /studentId|staffUsername|nationalId/i);
});

test('batch validates the 200-item limit before requesting manifests or loading renderer', async () => {
	const batch = await source(
		'src/lib/components/certificates/CertificateBatchDownloadDialog.svelte'
	);
	const validateAt = batch.indexOf('validateCertificateBatchSize(');
	const manifestAt = batch.indexOf('createIssuedCertificateRenderManifests(');
	const rendererAt = batch.indexOf('loadCertificateRenderer(');
	assert.ok(validateAt >= 0);
	assert.ok(manifestAt > validateAt);
	assert.ok(rendererAt > manifestAt);
	assert.match(batch, /selectedCertificateIds/);
	assert.match(batch, /MAX_CERTIFICATE_BATCH_SIZE/);
});

test('replacement links target a stable candidate row anchor', async () => {
	const issuedTable = await source('src/lib/components/certificates/CertificateIssuedTable.svelte');
	const candidateTable = await source(
		'src/lib/components/certificates/CertificateCandidateTable.svelte'
	);
	assert.match(issuedTable, /recipients#candidate-\$\{certificate\.replacementCandidateId\}/);
	assert.match(candidateTable, /id=\{`candidate-\$\{candidate\.id\}`\}/);
});

test('issued workspace clears campaign-local selection before loading a new route id', async () => {
	const table = await source('src/lib/components/certificates/CertificateIssuedTable.svelte');
	assert.match(table, /function resetCampaignView\(\)/);
	assert.match(
		table,
		/requestedCampaignId !== ''\s*&&\s*requestedCampaignId !== campaignId[\s\S]*?resetCampaignView\(\)/
	);
	assert.match(table, /disabled=\{loading \|\| selectedCertificateIds\.length === 0\}/);
	assert.match(
		table,
		/certificate\.status === 'issued'[\s\S]*?certificate\.capabilities\.canDownload/
	);
});
