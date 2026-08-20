import { apiClient, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type CertificateCampaignStatus = Schemas['CertificateCampaignStatus'];
export type CertificateCampaignCapabilities = Schemas['CertificateCampaignCapabilities'];
export type CertificateCampaignSummary = Schemas['CertificateCampaignSummary'];
export type CertificateCampaignDetail = Schemas['CertificateCampaignDetail'];
export type CertificateCampaignListQuery = Schemas['CertificateCampaignListQuery'];
export type CreateCertificateCampaignRequest = Schemas['CreateCertificateCampaignRequest'];
export type UpdateCertificateCampaignRequest = Schemas['UpdateCertificateCampaignRequest'];
export type ChangeCertificateCampaignStatusRequest =
	Schemas['ChangeCertificateCampaignStatusRequest'];
export type NullableUuidUpdate = Schemas['NullableUuidUpdate'];
export type CertificateOwnerOption = Schemas['OrganizationUnitLookupItem'];
export type CertificateTemplateDetail = Schemas['CertificateTemplateDetail'];
export type CertificateTemplateDeleteResult = Schemas['CertificateTemplateDeleteResult'];
export type CertificateTemplateVariableCatalog = Schemas['CertificateTemplateVariableCatalog'];
export type CertificateRenderManifest = Schemas['CertificateRenderManifest'];
export type CreateCertificateTemplateRequest = Schemas['CreateCertificateTemplateRequest'];
export type UpdateCertificateTemplateRequest = Schemas['UpdateCertificateTemplateRequest'];
export type AttachCertificateBackgroundRequest = Schemas['AttachCertificateBackgroundRequest'];
export type AttachCertificateAssetRequest = Schemas['AttachCertificateAssetRequest'];
export type InspectCertificateFontUploadsRequest = Schemas['InspectCertificateFontUploadsRequest'];
export type AttachCertificateFontBatchRequest = Schemas['AttachCertificateFontBatchRequest'];
export type CertificateFontUploadInspection = Schemas['CertificateFontUploadInspection'];
export type CertificateFontUploadInspectionFile = Schemas['CertificateFontUploadInspectionFile'];
export type CertificateFontUploadStatus = Schemas['CertificateFontUploadStatus'];
export type CertificatePreviewManifestRequest = Schemas['CertificatePreviewManifestRequest'];
export type RecipientType = Schemas['RecipientType'];
export type CertificateImportRequest = Schemas['CertificateImportRequest'];
export type CertificateCandidateDetail = Schemas['CertificateCandidateDetail'];
export type CertificateCandidateListQuery = Schemas['CertificateCandidateListQuery'];
export type CertificateCandidateListResponse = Schemas['CertificateCandidateListResponse'];
export type CertificateCandidateImportResult = Schemas['CertificateCandidateImportResult'];
export type CertificateCandidateBulkRequest = Schemas['CertificateCandidateBulkRequest'];
export type CertificateCandidateBulkResult = Schemas['CertificateCandidateBulkResult'];
export type CertificateCandidateAccount = Schemas['CertificateCandidateAccount'];
export type CertificateAccountSearchQuery = Schemas['CertificateAccountSearchQuery'];
export type CreateManualExternalCandidateRequest = Schemas['CreateManualExternalCandidateRequest'];
export type CreateAccountCertificateCandidateRequest =
	Schemas['CreateAccountCertificateCandidateRequest'];
export type UpdateCertificateCandidateRequest = Schemas['UpdateCertificateCandidateRequest'];
export type CertificateIssueRequestStatus = Schemas['CertificateIssueRequestStatus'];
export type CertificateIssueCode = Schemas['CertificateIssueCode'];
export type CertificateIssueRequestListQuery = Schemas['CertificateIssueRequestListQuery'];
export type CertificateIssueRequestCapabilities = Schemas['CertificateIssueRequestCapabilities'];
export type CertificateIssueRequestSummary = Schemas['CertificateIssueRequestSummary'];
export type CertificateIssueRequestItem = Schemas['CertificateIssueRequestItem'];
export type CertificateIssueRequestDetail = Schemas['CertificateIssueRequestDetail'];
export type SubmitCertificateIssueRequest = Schemas['SubmitCertificateIssueRequest'];
export type ReturnCertificateIssueRequest = Schemas['ReturnCertificateIssueRequest'];
export type CertificateResourceLocked = Schemas['CertificateResourceLocked'];
export type IssueCertificateRequest = Schemas['IssueCertificateRequest'];
export type IssueCertificateOutcome = Schemas['IssueCertificateOutcome'];
export type CertificateStatus = Schemas['CertificateStatus'];
export type CertificateCapabilities = Schemas['CertificateCapabilities'];
export type IssuedCertificateListQuery = Schemas['IssuedCertificateListQuery'];
export type IssuedCertificateSummary = Schemas['IssuedCertificateSummary'];
export type IssuedCertificateDetail = Schemas['IssuedCertificateDetail'];
export type RevokeCertificateRequest = Schemas['RevokeCertificateRequest'];
export type RevokeCertificateResult = Schemas['RevokeCertificateResult'];
export type CertificateRenderManifestBatchRequest =
	Schemas['CertificateRenderManifestBatchRequest'];
type EmptyData = Schemas['EmptyData'];

export async function listCertificateCampaigns(
	query: CertificateCampaignListQuery = {}
): Promise<CertificateCampaignSummary[]> {
	const params = new URLSearchParams();
	if (query.academicYearId) params.set('academicYearId', query.academicYearId);
	if (query.status) params.set('status', query.status);
	if (query.search?.trim()) params.set('search', query.search.trim());
	const suffix = params.size > 0 ? `?${params.toString()}` : '';
	const response = await apiClient.get<CertificateCampaignSummary[]>(
		`/api/certificates/campaigns${suffix}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดกิจกรรมเกียรติบัตรได้');
}

export async function getCertificateCampaign(
	campaignId: string
): Promise<CertificateCampaignDetail> {
	const response = await apiClient.get<CertificateCampaignDetail>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดกิจกรรมเกียรติบัตรได้');
}

export async function createCertificateCampaign(
	payload: CreateCertificateCampaignRequest
): Promise<CertificateCampaignDetail> {
	const response = await apiClient.post<CertificateCampaignDetail>(
		'/api/certificates/campaigns',
		payload
	);
	return requireApiData(response, 'ไม่สามารถสร้างกิจกรรมเกียรติบัตรได้');
}

export async function updateCertificateCampaign(
	campaignId: string,
	payload: UpdateCertificateCampaignRequest
): Promise<CertificateCampaignDetail> {
	const response = await apiClient.put<CertificateCampaignDetail, CertificateResourceLocked>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกกิจกรรมเกียรติบัตรได้');
}

export async function changeCertificateCampaignStatus(
	campaignId: string,
	payload: ChangeCertificateCampaignStatusRequest
): Promise<CertificateCampaignDetail> {
	const response = await apiClient.put<CertificateCampaignDetail, CertificateResourceLocked>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/status`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถเปลี่ยนสถานะกิจกรรมเกียรติบัตรได้');
}

export async function deleteCertificateCampaign(campaignId: string): Promise<EmptyData> {
	const response = await apiClient.delete<EmptyData, CertificateResourceLocked>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}`
	);
	return requireApiData(response, 'ไม่สามารถลบกิจกรรมเกียรติบัตรได้');
}

export async function listCertificateOwnerOptions(): Promise<CertificateOwnerOption[]> {
	const response = await apiClient.get<CertificateOwnerOption[]>('/api/certificates/owner-options');
	return requireApiData(response, 'ไม่สามารถโหลดหน่วยงานเจ้าของกิจกรรมได้');
}

export async function listCertificateTemplates(
	campaignId: string
): Promise<CertificateTemplateDetail[]> {
	const response = await apiClient.get<CertificateTemplateDetail[]>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/templates`
	);
	return requireApiData(response, 'ไม่สามารถโหลดแม่แบบเกียรติบัตรได้');
}

export async function createCertificateTemplate(
	campaignId: string,
	payload: CreateCertificateTemplateRequest
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.post<CertificateTemplateDetail>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/templates`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถสร้างแม่แบบเกียรติบัตรได้');
}

export async function getCertificateTemplate(
	templateId: string
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.get<CertificateTemplateDetail>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดแม่แบบเกียรติบัตรได้');
}

export async function updateCertificateTemplate(
	templateId: string,
	payload: UpdateCertificateTemplateRequest
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.put<CertificateTemplateDetail, CertificateResourceLocked>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกแม่แบบเกียรติบัตรได้');
}

export async function deleteCertificateTemplate(
	templateId: string
): Promise<CertificateTemplateDeleteResult> {
	const response = await apiClient.delete<
		CertificateTemplateDeleteResult,
		CertificateResourceLocked
	>(`/api/certificates/templates/${encodeURIComponent(templateId)}`);
	return requireApiData(response, 'ไม่สามารถลบแม่แบบเกียรติบัตรได้');
}

export async function attachCertificateTemplateBackground(
	templateId: string,
	payload: AttachCertificateBackgroundRequest
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.put<CertificateTemplateDetail, CertificateResourceLocked>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/background`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถแนบพื้นหลังเกียรติบัตรได้');
}

export async function attachCertificateTemplateAsset(
	templateId: string,
	payload: AttachCertificateAssetRequest
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.post<CertificateTemplateDetail, CertificateResourceLocked>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/assets`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถแนบทรัพยากรแม่แบบได้');
}

export async function inspectCertificateFontUploads(
	templateId: string,
	payload: InspectCertificateFontUploadsRequest
): Promise<CertificateFontUploadInspection> {
	const response = await apiClient.post<CertificateFontUploadInspection>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/assets/fonts/inspect`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถตรวจสอบไฟล์ฟอนต์ได้');
}

export async function attachCertificateFontBatch(
	templateId: string,
	payload: AttachCertificateFontBatchRequest
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.post<CertificateTemplateDetail, CertificateResourceLocked>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/assets/fonts/batch`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถแนบชุดฟอนต์กับแม่แบบได้');
}

export async function deleteCertificateTemplateAsset(
	templateId: string,
	assetId: string
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.delete<CertificateTemplateDetail, CertificateResourceLocked>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/assets/${encodeURIComponent(assetId)}`
	);
	return requireApiData(response, 'ไม่สามารถลบทรัพยากรแม่แบบได้');
}

export async function getCertificateTemplateVariableCatalog(
	templateId: string
): Promise<CertificateTemplateVariableCatalog> {
	const response = await apiClient.get<CertificateTemplateVariableCatalog>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/variables`
	);
	return requireApiData(response, 'ไม่สามารถโหลดตัวแปรแม่แบบได้');
}

export async function createCertificateTemplatePreviewManifest(
	templateId: string,
	payload: CertificatePreviewManifestRequest,
	options: ApiRequestOptions = {}
): Promise<CertificateRenderManifest> {
	const response = await apiClient.post<CertificateRenderManifest>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/preview-manifest`,
		payload,
		options
	);
	return requireApiData(response, 'ไม่สามารถสร้างข้อมูลพรีวิวเกียรติบัตรได้');
}

export async function listCertificateCandidates(
	campaignId: string,
	query: CertificateCandidateListQuery = {}
): Promise<CertificateCandidateListResponse> {
	const params = new URLSearchParams();
	if (query.status) params.set('status', query.status);
	if (query.templateId) params.set('templateId', query.templateId);
	if (query.search?.trim()) params.set('search', query.search.trim());
	const suffix = params.size > 0 ? `?${params.toString()}` : '';
	const response = await apiClient.get<CertificateCandidateListResponse>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/candidates${suffix}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายชื่อผู้รับเกียรติบัตรได้');
}

export async function importCertificateCandidates(
	campaignId: string,
	payload: CertificateImportRequest
): Promise<CertificateCandidateImportResult> {
	const response = await apiClient.post<CertificateCandidateImportResult>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/candidates/import`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถนำเข้ารายชื่อผู้รับเกียรติบัตรได้');
}

export async function createManualCertificateCandidate(
	campaignId: string,
	payload: CreateManualExternalCandidateRequest
): Promise<CertificateCandidateImportResult> {
	const response = await apiClient.post<CertificateCandidateImportResult>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/candidates/manual`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถเพิ่มบุคคลภายนอกได้');
}

export async function searchCertificateCandidateAccounts(
	campaignId: string,
	query: CertificateAccountSearchQuery
): Promise<CertificateCandidateAccount[]> {
	const params = new URLSearchParams({
		recipientType: query.recipientType,
		search: query.search.trim()
	});
	const response = await apiClient.get<CertificateCandidateAccount[]>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/candidates/account-search?${params.toString()}`
	);
	return requireApiData(response, 'ไม่สามารถค้นหาบัญชีผู้รับได้');
}

export async function createAccountCertificateCandidate(
	campaignId: string,
	payload: CreateAccountCertificateCandidateRequest
): Promise<CertificateCandidateImportResult> {
	const response = await apiClient.post<CertificateCandidateImportResult>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/candidates/account-search`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถเพิ่มผู้รับจากบัญชีได้');
}

export async function bulkUpdateCertificateCandidates(
	campaignId: string,
	payload: CertificateCandidateBulkRequest
): Promise<CertificateCandidateBulkResult> {
	const response = await apiClient.post<CertificateCandidateBulkResult, CertificateResourceLocked>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/candidates/bulk`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถปรับปรุงรายชื่อที่เลือกได้');
}

export async function getCertificateCandidate(
	candidateId: string
): Promise<CertificateCandidateDetail> {
	const response = await apiClient.get<CertificateCandidateDetail>(
		`/api/certificates/candidates/${encodeURIComponent(candidateId)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดข้อมูลผู้รับเกียรติบัตรได้');
}

export async function updateCertificateCandidate(
	candidateId: string,
	payload: UpdateCertificateCandidateRequest
): Promise<CertificateCandidateDetail> {
	const response = await apiClient.put<CertificateCandidateDetail, CertificateResourceLocked>(
		`/api/certificates/candidates/${encodeURIComponent(candidateId)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกข้อมูลผู้รับเกียรติบัตรได้');
}

export async function deleteCertificateCandidate(
	candidateId: string
): Promise<CertificateCandidateDetail> {
	const response = await apiClient.delete<CertificateCandidateDetail, CertificateResourceLocked>(
		`/api/certificates/candidates/${encodeURIComponent(candidateId)}`
	);
	return requireApiData(response, 'ไม่สามารถลบรายชื่อผู้รับเกียรติบัตรได้');
}

export async function listCertificateCampaignIssueRequests(
	campaignId: string
): Promise<CertificateIssueRequestSummary[]> {
	const response = await apiClient.get<CertificateIssueRequestSummary[]>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/issue-requests`
	);
	return requireApiData(response, 'ไม่สามารถโหลดประวัติคำขอออกเกียรติบัตรได้');
}

export async function submitCertificateIssueRequest(
	campaignId: string,
	payload: SubmitCertificateIssueRequest
): Promise<CertificateIssueRequestDetail> {
	const response = await apiClient.post<CertificateIssueRequestDetail, CertificateResourceLocked>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/issue-requests`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถส่งคำขอออกเกียรติบัตรได้');
}

export async function listCertificateIssueRequests(
	query: CertificateIssueRequestListQuery = {}
): Promise<CertificateIssueRequestSummary[]> {
	const params = new URLSearchParams();
	if (query.status) params.set('status', query.status);
	const suffix = params.size > 0 ? `?${params.toString()}` : '';
	const response = await apiClient.get<CertificateIssueRequestSummary[]>(
		`/api/certificates/issue-requests${suffix}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดคิวคำขอออกเกียรติบัตรได้');
}

export async function getCertificateIssueRequest(
	requestId: string
): Promise<CertificateIssueRequestDetail> {
	const response = await apiClient.get<CertificateIssueRequestDetail>(
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดคำขอออกเกียรติบัตรได้');
}

export async function withdrawCertificateIssueRequest(
	requestId: string
): Promise<CertificateIssueRequestDetail> {
	const response = await apiClient.post<CertificateIssueRequestDetail>(
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/withdraw`
	);
	return requireApiData(response, 'ไม่สามารถถอนคำขอออกเกียรติบัตรได้');
}

export async function startCertificateIssueRequestReview(
	requestId: string
): Promise<CertificateIssueRequestDetail> {
	const response = await apiClient.post<CertificateIssueRequestDetail>(
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/review`
	);
	return requireApiData(response, 'ไม่สามารถเริ่มตรวจคำขอออกเกียรติบัตรได้');
}

export async function returnCertificateIssueRequest(
	requestId: string,
	payload: ReturnCertificateIssueRequest
): Promise<CertificateIssueRequestDetail> {
	const response = await apiClient.post<CertificateIssueRequestDetail>(
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/return`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถส่งคำขอกลับให้แก้ไขได้');
}

export async function issueCertificates(
	requestId: string,
	payload: IssueCertificateRequest
): Promise<IssueCertificateOutcome> {
	const response = await apiClient.post<IssueCertificateOutcome>(
		`/api/certificates/issue-requests/${encodeURIComponent(requestId)}/issue`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถออกเลขเกียรติบัตรได้');
}

export async function listIssuedCertificates(
	campaignId: string,
	query: IssuedCertificateListQuery = {}
): Promise<IssuedCertificateSummary[]> {
	const params = new URLSearchParams();
	if (query.status) params.set('status', query.status);
	if (query.templateId) params.set('templateId', query.templateId);
	if (query.search?.trim()) params.set('search', query.search.trim());
	const suffix = params.size > 0 ? `?${params.toString()}` : '';
	const response = await apiClient.get<IssuedCertificateSummary[]>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/issued${suffix}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายการเกียรติบัตรที่ออกแล้วได้');
}

export async function getIssuedCertificate(
	certificateId: string
): Promise<IssuedCertificateDetail> {
	const response = await apiClient.get<IssuedCertificateDetail>(
		`/api/certificates/${encodeURIComponent(certificateId)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายละเอียดเกียรติบัตรได้');
}

export async function revokeIssuedCertificate(
	certificateId: string,
	payload: RevokeCertificateRequest
): Promise<RevokeCertificateResult> {
	const response = await apiClient.post<RevokeCertificateResult>(
		`/api/certificates/${encodeURIComponent(certificateId)}/revoke`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถเพิกถอนเกียรติบัตรได้');
}

export async function createIssuedCertificateRenderManifest(
	certificateId: string
): Promise<CertificateRenderManifest> {
	const response = await apiClient.post<CertificateRenderManifest>(
		`/api/certificates/${encodeURIComponent(certificateId)}/render-manifest`
	);
	return requireApiData(response, 'ไม่สามารถเตรียมไฟล์เกียรติบัตรได้');
}

export async function createIssuedCertificateRenderManifests(
	campaignId: string,
	payload: CertificateRenderManifestBatchRequest
): Promise<CertificateRenderManifest[]> {
	const response = await apiClient.post<CertificateRenderManifest[]>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/render-manifests`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถเตรียมไฟล์เกียรติบัตรที่เลือกได้');
}

export async function listOwnCertificates(
	options: ApiRequestOptions = {}
): Promise<IssuedCertificateSummary[]> {
	const response = await apiClient.get<IssuedCertificateSummary[]>('/api/me/certificates', options);
	return requireApiData(response, 'ไม่สามารถโหลดคลังเกียรติบัตรได้');
}

export async function getOwnCertificate(certificateId: string): Promise<IssuedCertificateDetail> {
	const response = await apiClient.get<IssuedCertificateDetail>(
		`/api/me/certificates/${encodeURIComponent(certificateId)}`
	);
	return requireApiData(response, 'ไม่สามารถโหลดรายละเอียดเกียรติบัตรได้');
}

export async function createOwnCertificateRenderManifest(
	certificateId: string
): Promise<CertificateRenderManifest> {
	const response = await apiClient.post<CertificateRenderManifest>(
		`/api/me/certificates/${encodeURIComponent(certificateId)}/render-manifest`
	);
	return requireApiData(response, 'ไม่สามารถเตรียมไฟล์เกียรติบัตรได้');
}
