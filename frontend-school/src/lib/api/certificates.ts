import { apiClient, requireApiData } from '$lib/api/client';
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
export type CertificatePreviewManifestRequest = Schemas['CertificatePreviewManifestRequest'];
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
	const response = await apiClient.put<CertificateCampaignDetail>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกกิจกรรมเกียรติบัตรได้');
}

export async function changeCertificateCampaignStatus(
	campaignId: string,
	payload: ChangeCertificateCampaignStatusRequest
): Promise<CertificateCampaignDetail> {
	const response = await apiClient.put<CertificateCampaignDetail>(
		`/api/certificates/campaigns/${encodeURIComponent(campaignId)}/status`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถเปลี่ยนสถานะกิจกรรมเกียรติบัตรได้');
}

export async function deleteCertificateCampaign(campaignId: string): Promise<EmptyData> {
	const response = await apiClient.delete<EmptyData>(
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
	const response = await apiClient.put<CertificateTemplateDetail>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถบันทึกแม่แบบเกียรติบัตรได้');
}

export async function deleteCertificateTemplate(
	templateId: string
): Promise<CertificateTemplateDeleteResult> {
	const response = await apiClient.delete<CertificateTemplateDeleteResult>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}`
	);
	return requireApiData(response, 'ไม่สามารถลบแม่แบบเกียรติบัตรได้');
}

export async function attachCertificateTemplateBackground(
	templateId: string,
	payload: AttachCertificateBackgroundRequest
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.put<CertificateTemplateDetail>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/background`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถแนบพื้นหลังเกียรติบัตรได้');
}

export async function attachCertificateTemplateAsset(
	templateId: string,
	payload: AttachCertificateAssetRequest
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.post<CertificateTemplateDetail>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/assets`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถแนบทรัพยากรแม่แบบได้');
}

export async function deleteCertificateTemplateAsset(
	templateId: string,
	assetId: string
): Promise<CertificateTemplateDetail> {
	const response = await apiClient.delete<CertificateTemplateDetail>(
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
	payload: CertificatePreviewManifestRequest
): Promise<CertificateRenderManifest> {
	const response = await apiClient.post<CertificateRenderManifest>(
		`/api/certificates/templates/${encodeURIComponent(templateId)}/preview-manifest`,
		payload
	);
	return requireApiData(response, 'ไม่สามารถสร้างข้อมูลพรีวิวเกียรติบัตรได้');
}
