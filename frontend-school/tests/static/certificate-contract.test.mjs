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
		['/api/certificates/candidates/{candidate_id}', 'delete', 'deleteCertificateCandidate'],
		[
			'/api/certificates/campaigns/{campaign_id}/issue-requests',
			'get',
			'listCertificateCampaignIssueRequests'
		],
		[
			'/api/certificates/campaigns/{campaign_id}/issue-requests',
			'post',
			'submitCertificateIssueRequest'
		],
		['/api/certificates/issue-requests', 'get', 'listCertificateIssueRequests'],
		['/api/certificates/issue-requests/{request_id}', 'get', 'getCertificateIssueRequest'],
		[
			'/api/certificates/issue-requests/{request_id}/withdraw',
			'post',
			'withdrawCertificateIssueRequest'
		],
		[
			'/api/certificates/issue-requests/{request_id}/review',
			'post',
			'startCertificateIssueRequestReview'
		],
		[
			'/api/certificates/issue-requests/{request_id}/return',
			'post',
			'returnCertificateIssueRequest'
		],
		['/api/certificates/issue-requests/{request_id}/issue', 'post', 'issueCertificates'],
		['/api/certificates/campaigns/{campaign_id}/issued', 'get', 'listIssuedCertificates'],
		['/api/certificates/{certificate_id}', 'get', 'getIssuedCertificate'],
		['/api/certificates/{certificate_id}/revoke', 'post', 'revokeIssuedCertificate'],
		[
			'/api/certificates/{certificate_id}/render-manifest',
			'post',
			'createIssuedCertificateRenderManifest'
		],
		[
			'/api/certificates/campaigns/{campaign_id}/render-manifests',
			'post',
			'createIssuedCertificateRenderManifests'
		]
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
		'UpdateCertificateCandidateRequest',
		'CertificateIssueRequestStatus',
		'CertificateIssueCode',
		'SubmitCertificateIssueRequest',
		'ReturnCertificateIssueRequest',
		'CertificateIssueRequestListQuery',
		'CertificateIssueRequestCapabilities',
		'CertificateIssueRequestSummary',
		'CertificateIssueRequestItem',
		'CertificateIssueRequestDetail',
		'CertificateStatus',
		'CertificateCapabilities',
		'IssueCertificateRequest',
		'IssueCertificateOutcome',
		'CertificateIssueCandidateProblem',
		'IssuedCertificateListQuery',
		'IssuedCertificateSummary',
		'IssuedCertificateDetail',
		'RevokeCertificateRequest',
		'RevokeCertificateResult',
		'CertificateReplacementCandidate',
		'CertificateRenderManifestBatchRequest',
		'CertificateResourceLocked'
	]) {
		assert.ok(openapi.components?.schemas?.[schema], `missing generated schema ${schema}`);
	}
	assert.ok(
		openapi.components.schemas.CertificateCampaignCapabilities.required.includes(
			'canManageTemplates'
		),
		'campaign capability must expose exact-scope template workflow access'
	);
	assert.ok(
		openapi.components.schemas.CertificateCampaignCapabilities.required.includes(
			'canPrepareCandidates'
		),
		'campaign capability must expose exact-scope candidate preparation access'
	);
	const mutationConflictRef =
		'#/components/schemas/ApiErrorResponseWithOptionalData_CertificateResourceLocked';
	for (const [route, method] of [
		['/api/certificates/campaigns/{campaign_id}', 'put'],
		['/api/certificates/campaigns/{campaign_id}', 'delete'],
		['/api/certificates/campaigns/{campaign_id}/status', 'put'],
		['/api/certificates/templates/{template_id}', 'put'],
		['/api/certificates/templates/{template_id}', 'delete'],
		['/api/certificates/templates/{template_id}/background', 'put'],
		['/api/certificates/templates/{template_id}/assets', 'post'],
		['/api/certificates/templates/{template_id}/assets/{asset_id}', 'delete'],
		['/api/certificates/campaigns/{campaign_id}/candidates/bulk', 'post'],
		['/api/certificates/candidates/{candidate_id}', 'put'],
		['/api/certificates/candidates/{candidate_id}', 'delete']
	]) {
		assert.equal(
			openapi.paths?.[route]?.[method]?.responses?.['409']?.content?.['application/json']?.schema
				?.$ref,
			mutationConflictRef,
			`${method} ${route} must expose optional typed lock data`
		);
	}

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
	for (const operation of [
		'listCertificateCampaignIssueRequests',
		'submitCertificateIssueRequest',
		'listCertificateIssueRequests',
		'getCertificateIssueRequest',
		'withdrawCertificateIssueRequest',
		'startCertificateIssueRequestReview',
		'returnCertificateIssueRequest',
		'issueCertificates',
		'listIssuedCertificates',
		'getIssuedCertificate',
		'revokeIssuedCertificate',
		'createIssuedCertificateRenderManifest',
		'createIssuedCertificateRenderManifests'
	]) {
		assert.match(wrapper, new RegExp(`export async function ${operation}\\b`));
	}
	assert.match(wrapper, /requireApiData/);
	assert.match(wrapper, /apiClient\.put<CertificateCampaignDetail,\s*CertificateResourceLocked>/);
	assert.match(
		wrapper,
		/apiClient\.post<CertificateIssueRequestDetail,\s*CertificateResourceLocked>/
	);
	assert.doesNotMatch(wrapper, /\b(?:interface|Record<string, unknown>|ApiResponse<unknown>)\b/);
	assert.doesNotMatch(wrapper, /certificate\.(?:read|create|update|delete|submit|download)\./);

	const recipientWorkspace = await readFile(
		path.join(
			repoRoot,
			'frontend-school/src/lib/components/certificates/CertificateRecipientWorkspace.svelte'
		),
		'utf8'
	);
	assert.match(recipientWorkspace, /campaign\?\.capabilities\.canPrepareCandidates/);
	assert.doesNotMatch(recipientWorkspace, /hasUpdatePermission/);
	const recipientRoute = await readFile(
		path.join(
			repoRoot,
			'frontend-school/src/routes/(app)/staff/certificates/[campaignId]/recipients/+page.svelte'
		),
		'utf8'
	);
	assert.doesNotMatch(recipientRoute, /hasUpdatePermission/);

	const fileWrapper = await readFile(
		path.join(repoRoot, 'frontend-school/src/lib/api/files.ts'),
		'utf8'
	);
	assert.match(fileWrapper, /CertificateTemplateFilePurpose/);
	assert.match(fileWrapper, /uploadCertificateTemplateFile/);
	assert.match(fileWrapper, /return uploadFile\(file, purpose, templateId\)/);
});
