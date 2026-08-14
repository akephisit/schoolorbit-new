import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

test('certificate permission contract exposes the complete approved capability set', async () => {
	const contract = JSON.parse(
		await readFile(path.join(repoRoot, 'contracts/permissions.json'), 'utf8')
	);
	const actualCertificateCodes = contract.permissions
		.filter((permission) => permission.module === 'certificate')
		.map((permission) => `${permission.module}.${permission.action}.${permission.scope}`);
	const expected = [
		'certificate.read.own',
		'certificate.read.organization_unit',
		'certificate.read.school',
		'certificate.create.organization_unit',
		'certificate.create.school',
		'certificate.update.organization_unit',
		'certificate.update.school',
		'certificate.delete.organization_unit',
		'certificate.delete.school',
		'certificate.submit.organization_unit',
		'certificate.submit.school',
		'certificate.issue.school',
		'certificate.revoke.school',
		'certificate.download.organization_unit',
		'certificate.download.school'
	];

	assert.deepEqual(actualCertificateCodes.toSorted(), expected.toSorted());
});

test('certificate campaign API is generated and its wrapper consumes named DTOs', async () => {
	const openapi = JSON.parse(
		await readFile(path.join(repoRoot, 'contracts/openapi/school-api.json'), 'utf8')
	);
	const expectedOperations = [
		['/api/certificates/campaigns', 'get', 'listCertificateCampaigns'],
		['/api/certificates/campaigns', 'post', 'createCertificateCampaign'],
		['/api/certificates/campaigns/{campaign_id}', 'get', 'getCertificateCampaign'],
		['/api/certificates/campaigns/{campaign_id}', 'put', 'updateCertificateCampaign'],
		['/api/certificates/campaigns/{campaign_id}', 'delete', 'deleteCertificateCampaign'],
		['/api/certificates/campaigns/{campaign_id}/status', 'put', 'changeCertificateCampaignStatus'],
		['/api/certificates/owner-options', 'get', 'listCertificateOwnerOptions']
	];
	for (const [route, method, operationId] of expectedOperations) {
		assert.equal(openapi.paths?.[route]?.[method]?.operationId, operationId);
	}
	for (const schema of [
		'CertificateCampaignSummary',
		'CertificateCampaignDetail',
		'CertificateCampaignCapabilities',
		'CreateCertificateCampaignRequest',
		'UpdateCertificateCampaignRequest',
		'ChangeCertificateCampaignStatusRequest',
		'NullableUuidUpdate'
	]) {
		assert.ok(openapi.components?.schemas?.[schema], `missing generated schema ${schema}`);
	}

	const wrapper = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/api/certificates.ts'),
		'utf8'
	);
	assert.match(wrapper, /type Schemas = components\['schemas'\]/);
	assert.match(wrapper, /Schemas\['CertificateCampaignSummary'\]/);
	assert.match(wrapper, /Schemas\['CertificateCampaignDetail'\]/);
	assert.match(wrapper, /Schemas\['CreateCertificateCampaignRequest'\]/);
	assert.match(wrapper, /requireApiData/);
	assert.doesNotMatch(wrapper, /\b(?:interface|Record<string, unknown>|ApiResponse<unknown>)\b/);
	assert.doesNotMatch(wrapper, /certificate\.(?:read|create|update|delete|submit|download)\./);
});
