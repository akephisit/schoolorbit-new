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
