import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];
export type CorrespondenceDetail = Schemas['CorrespondenceDetail'];
export type CorrespondenceSummary = Schemas['CorrespondenceSummary'];
export type CorrespondenceList = Schemas['CorrespondenceList'];
export type CorrespondenceOptions = Schemas['CorrespondenceOptions'];
export type CorrespondenceReadReceipt = Schemas['CorrespondenceReadReceipt'];
export type CorrespondenceReadRequest = Schemas['CorrespondenceReadRequest'];
export type CorrespondenceNumberingSettings = Schemas['CorrespondenceNumberingSettings'];
export type CorrespondenceTextStampTemplate = Schemas['CorrespondenceTextStampTemplate'];
export type CorrespondenceStampContext = Schemas['CorrespondenceStampContext'];
export type CreateCorrespondenceRequest = Schemas['CreateCorrespondenceRequest'];
export type ReviseCorrespondenceRequest = Schemas['ReviseCorrespondenceRequest'];
export type SendCorrespondenceRequest = Schemas['SendCorrespondenceRequest'];
export type DirectedSendCorrespondenceRequest = Schemas['DirectedSendCorrespondenceRequest'];
export type DirectedRoutingRequest = Schemas['DirectedRoutingRequest'];
export type DirectedAssignment = Schemas['DirectedAssignment'];
export type DirectedRouteRecipient = Schemas['DirectedRouteRecipient'];
export type CorrespondenceRoutingSettings = Schemas['CorrespondenceRoutingSettings'];
export type SaveCorrespondenceRoutingSettingsRequest =
	Schemas['SaveCorrespondenceRoutingSettingsRequest'];
export type ProcessTemplate = Schemas['ProcessTemplate'];
export type ProcessTemplateStep = Schemas['ProcessTemplateStep'];
export type ProcessRecipient = Schemas['ProcessRecipient'];
export type ProcessStepRecipients = Schemas['ProcessStepRecipients'];
export type ProcessAction = Schemas['ProcessAction'];
export type ProcessActionRequest = Schemas['ProcessActionRequest'];
export type SaveProcessTemplateRequest = Schemas['SaveProcessTemplateRequest'];
export type DocumentAnnotation = Schemas['DocumentAnnotation'];
export type InkStroke = Schemas['InkStroke'];
export type SignatureInput = Schemas['SignatureInput'];
export type SignatureAsset = Schemas['SignatureAsset'];

export type CorrespondenceView = Schemas['CorrespondenceListView'];
export interface CorrespondenceFilters {
	view?: CorrespondenceView;
	register?: CorrespondenceSummary['register'];
	status?: CorrespondenceSummary['status'];
	search?: string;
	cursor?: string;
}

export async function listCorrespondence(
	filters: CorrespondenceFilters = {},
	options: ApiRequestOptions = {}
): Promise<CorrespondenceList> {
	const query = new URLSearchParams({ limit: '30' });
	for (const [key, value] of Object.entries(filters)) if (value) query.set(key, value);
	return requireApiData(
		await apiClient.get<CorrespondenceList>(`/api/correspondence?${query}`, options),
		'โหลดทะเบียนหนังสือไม่สำเร็จ'
	);
}
export async function getCorrespondence(
	id: string,
	options: ApiRequestOptions = {}
): Promise<CorrespondenceDetail> {
	return requireApiData(
		await apiClient.get<CorrespondenceDetail>(`/api/correspondence/${id}`, options),
		'โหลดหนังสือไม่สำเร็จ'
	);
}
export async function recordCorrespondenceRead(
	id: string,
	payload: CorrespondenceReadRequest,
	options: ApiRequestOptions = {}
): Promise<CorrespondenceReadReceipt> {
	return requireApiData(
		await apiClient.post<CorrespondenceReadReceipt>(
			`/api/correspondence/${id}/read`,
			payload,
			options
		),
		'บันทึกการเปิดอ่านไม่สำเร็จ'
	);
}
export async function getCorrespondenceOptions(
	search = '',
	options: ApiRequestOptions = {},
	register?: CorrespondenceSummary['register']
): Promise<CorrespondenceOptions> {
	return requireApiData(
		await apiClient.get<CorrespondenceOptions>(
			`/api/correspondence/options?${new URLSearchParams({ search, ...(register ? { register } : {}) })}`,
			options
		),
		'โหลดตัวเลือกไม่สำเร็จ'
	);
}
export async function createCorrespondence(
	payload: CreateCorrespondenceRequest
): Promise<CorrespondenceDetail> {
	return requireApiData(
		await apiClient.post<CorrespondenceDetail>('/api/correspondence', payload),
		'สร้างหนังสือไม่สำเร็จ'
	);
}
export async function reviseCorrespondence(
	id: string,
	payload: ReviseCorrespondenceRequest
): Promise<CorrespondenceDetail> {
	return requireApiData(
		await apiClient.post<CorrespondenceDetail>(`/api/correspondence/${id}/revisions`, payload),
		'บันทึกฉบับแก้ไขไม่สำเร็จ'
	);
}
export async function sendCorrespondence(
	id: string,
	payload: SendCorrespondenceRequest
): Promise<CorrespondenceDetail> {
	return requireApiData(
		await apiClient.post<CorrespondenceDetail>(`/api/correspondence/${id}/send`, payload),
		'ส่งหนังสือไม่สำเร็จ'
	);
}
export async function sendDirectedCorrespondence(
	id: string,
	payload: DirectedSendCorrespondenceRequest
): Promise<CorrespondenceDetail> {
	return requireApiData(
		await apiClient.post<CorrespondenceDetail>(`/api/correspondence/${id}/directed-send`, payload),
		'ส่งหนังสือให้ผู้พิจารณาสั่งการไม่สำเร็จ'
	);
}
export async function getCorrespondenceRoutingSettings(
	options: ApiRequestOptions = {}
): Promise<CorrespondenceRoutingSettings[]> {
	return requireApiData(
		await apiClient.get<CorrespondenceRoutingSettings[]>(
			'/api/correspondence/routing-settings',
			options
		),
		'โหลดการตั้งค่าการสั่งการไม่สำเร็จ'
	);
}
export async function saveCorrespondenceRoutingSettings(
	payload: SaveCorrespondenceRoutingSettingsRequest
): Promise<CorrespondenceRoutingSettings> {
	return requireApiData(
		await apiClient.put<CorrespondenceRoutingSettings>(
			'/api/correspondence/routing-settings',
			payload
		),
		'บันทึกการตั้งค่าการสั่งการไม่สำเร็จ'
	);
}
export async function actOnCorrespondence(
	id: string,
	payload: ProcessActionRequest
): Promise<CorrespondenceDetail> {
	return requireApiData(
		await apiClient.post<CorrespondenceDetail>(`/api/correspondence/${id}/actions`, payload),
		'ดำเนินการไม่สำเร็จ'
	);
}
export async function listProcessTemplates(
	options: ApiRequestOptions = {}
): Promise<ProcessTemplate[]> {
	return requireApiData(
		await apiClient.get<ProcessTemplate[]>('/api/process/templates', options),
		'โหลดแม่แบบไม่สำเร็จ'
	);
}
export async function saveProcessTemplate(
	payload: SaveProcessTemplateRequest,
	id?: string
): Promise<ProcessTemplate> {
	return requireApiData(
		id
			? await apiClient.put<ProcessTemplate>(`/api/process/templates/${id}`, payload)
			: await apiClient.post<ProcessTemplate>('/api/process/templates', payload),
		'บันทึกแม่แบบไม่สำเร็จ'
	);
}
export async function listSignatureAssets(
	options: ApiRequestOptions = {}
): Promise<SignatureAsset[]> {
	return requireApiData(
		await apiClient.get<SignatureAsset[]>('/api/correspondence/signatures', options),
		'โหลดลายมือชื่อไม่สำเร็จ'
	);
}
export async function createSignatureAsset(label: string, fileId: string): Promise<SignatureAsset> {
	return requireApiData(
		await apiClient.post<SignatureAsset>('/api/correspondence/signatures', {
			requestId: crypto.randomUUID(),
			label,
			fileId
		}),
		'บันทึกลายมือชื่อไม่สำเร็จ'
	);
}
export async function exportCorrespondence(id: string): Promise<Blob> {
	return requireApiData(
		await apiClient.getBlob(`/api/correspondence/${id}/export`),
		'ดาวน์โหลดเอกสารไม่สำเร็จ'
	);
}
export async function previewCorrespondence(
	id: string,
	documentRevision: number,
	options: ApiRequestOptions = {}
): Promise<Blob> {
	return requireApiData(
		await apiClient.getBlob(
			`/api/correspondence/${id}/preview?${new URLSearchParams({ documentRevision: String(documentRevision) })}`,
			options
		),
		'เปิดเอกสารไม่สำเร็จ'
	);
}
