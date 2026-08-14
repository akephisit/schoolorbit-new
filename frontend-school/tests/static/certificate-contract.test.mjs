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
		['/api/certificates/owner-options', 'get', 'listCertificateOwnerOptions'],
		['/api/certificates/campaigns/{campaign_id}/templates', 'get', 'listCertificateTemplates'],
		['/api/certificates/campaigns/{campaign_id}/templates', 'post', 'createCertificateTemplate'],
		['/api/certificates/templates/{template_id}', 'get', 'getCertificateTemplate'],
		['/api/certificates/templates/{template_id}', 'put', 'updateCertificateTemplate'],
		['/api/certificates/templates/{template_id}', 'delete', 'deleteCertificateTemplate'],
		[
			'/api/certificates/templates/{template_id}/background',
			'put',
			'attachCertificateTemplateBackground'
		],
		['/api/certificates/templates/{template_id}/assets', 'post', 'attachCertificateTemplateAsset'],
		[
			'/api/certificates/templates/{template_id}/assets/{asset_id}',
			'delete',
			'deleteCertificateTemplateAsset'
		],
		[
			'/api/certificates/templates/{template_id}/variables',
			'get',
			'getCertificateTemplateVariableCatalog'
		],
		[
			'/api/certificates/templates/{template_id}/preview-manifest',
			'post',
			'createCertificateTemplatePreviewManifest'
		],
		['/api/certificates/campaigns/{campaign_id}/candidates', 'get', 'listCertificateCandidates'],
		[
			'/api/certificates/campaigns/{campaign_id}/candidates/import',
			'post',
			'importCertificateCandidates'
		],
		[
			'/api/certificates/campaigns/{campaign_id}/candidates/manual',
			'post',
			'createManualCertificateCandidate'
		],
		[
			'/api/certificates/campaigns/{campaign_id}/candidates/account-search',
			'get',
			'searchCertificateCandidateAccounts'
		],
		[
			'/api/certificates/campaigns/{campaign_id}/candidates/account-search',
			'post',
			'createAccountCertificateCandidate'
		],
		[
			'/api/certificates/campaigns/{campaign_id}/candidates/bulk',
			'post',
			'bulkUpdateCertificateCandidates'
		],
		['/api/certificates/candidates/{candidate_id}', 'get', 'getCertificateCandidate'],
		['/api/certificates/candidates/{candidate_id}', 'put', 'updateCertificateCandidate'],
		['/api/certificates/candidates/{candidate_id}', 'delete', 'deleteCertificateCandidate']
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
		'NullableUuidUpdate',
		'CertificateTemplateDetail',
		'CertificateTemplateCapabilities',
		'CreateCertificateTemplateRequest',
		'UpdateCertificateTemplateRequest',
		'AttachCertificateBackgroundRequest',
		'AttachCertificateAssetRequest',
		'CertificateTemplateDeleteResult',
		'CertificateTemplateVariableCatalog',
		'CertificatePreviewManifestRequest',
		'CertificateRenderManifest',
		'CertificateCandidateDetail',
		'CertificateCandidateCapabilities',
		'CertificateCandidateListQuery',
		'CertificateCandidateListResponse',
		'CertificateCandidateSummary',
		'CertificateImportRequest',
		'CertificateCandidateImportResult',
		'CertificateCandidateBulkRequest',
		'CertificateCandidateBulkResult',
		'CertificateCandidateAccount',
		'CertificateAccountSearchQuery',
		'CreateManualExternalCandidateRequest',
		'CreateAccountCertificateCandidateRequest',
		'UpdateCertificateCandidateRequest'
	]) {
		assert.ok(openapi.components?.schemas?.[schema], `missing generated schema ${schema}`);
	}
	assert.ok(
		openapi.components.schemas.CertificateCampaignCapabilities.required.includes(
			'canManageTemplates'
		),
		'campaign capability must expose exact-scope template workflow access'
	);

	const wrapper = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/api/certificates.ts'),
		'utf8'
	);
	assert.match(wrapper, /type Schemas = components\['schemas'\]/);
	assert.match(wrapper, /Schemas\['CertificateCampaignSummary'\]/);
	assert.match(wrapper, /Schemas\['CertificateCampaignDetail'\]/);
	assert.match(wrapper, /Schemas\['CreateCertificateCampaignRequest'\]/);
	assert.match(wrapper, /Schemas\['CertificateTemplateDetail'\]/);
	assert.match(wrapper, /Schemas\['CertificateRenderManifest'\]/);
	assert.match(wrapper, /createCertificateTemplatePreviewManifest/);
	assert.match(wrapper, /Schemas\['CertificateCandidateDetail'\]/);
	assert.match(wrapper, /importCertificateCandidates/);
	assert.match(wrapper, /bulkUpdateCertificateCandidates/);
	assert.match(wrapper, /searchCertificateCandidateAccounts/);
	assert.match(wrapper, /requireApiData/);
	assert.doesNotMatch(wrapper, /\b(?:interface|Record<string, unknown>|ApiResponse<unknown>)\b/);
	assert.doesNotMatch(wrapper, /certificate\.(?:read|create|update|delete|submit|download)\./);

	const fileWrapper = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/api/files.ts'),
		'utf8'
	);
	assert.match(fileWrapper, /CertificateTemplateFilePurpose/);
	assert.match(fileWrapper, /uploadCertificateTemplateFile/);
	assert.match(fileWrapper, /return uploadFile\(file, purpose, templateId\)/);
});
