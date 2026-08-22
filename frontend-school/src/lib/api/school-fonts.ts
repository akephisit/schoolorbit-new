import { apiClient, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type SchoolFontSummary = Schemas['SchoolFontSummary'];
export type SchoolFontListResponse = Schemas['SchoolFontListResponse'];
export type InspectSchoolFontUploadsRequest = Schemas['InspectSchoolFontUploadsRequest'];
export type AttachSchoolFontBatchRequest = Schemas['AttachSchoolFontBatchRequest'];
export type SchoolFontUploadInspection = Schemas['SchoolFontUploadInspection'];
export type SchoolFontUploadInspectionFile = Schemas['SchoolFontUploadInspectionFile'];
export type SchoolFontUploadStatus = Schemas['SchoolFontUploadStatus'];
export type SchoolFontDeleteConflict = Schemas['SchoolFontDeleteConflict'];
type EmptyData = Schemas['EmptyData'];

export async function listSchoolFonts(): Promise<SchoolFontListResponse> {
	const response = await apiClient.get<SchoolFontListResponse>('/api/school-fonts');
	return requireApiData(response, 'โหลดคลังฟอนต์ไม่สำเร็จ');
}

export async function inspectSchoolFontUploads(
	payload: InspectSchoolFontUploadsRequest
): Promise<SchoolFontUploadInspection> {
	const response = await apiClient.post<SchoolFontUploadInspection>(
		'/api/school-fonts/inspect',
		payload
	);
	return requireApiData(response, 'ตรวจข้อมูลฟอนต์ไม่สำเร็จ');
}

export async function attachSchoolFontBatch(
	payload: AttachSchoolFontBatchRequest
): Promise<SchoolFontListResponse> {
	const response = await apiClient.post<SchoolFontListResponse>('/api/school-fonts/batch', payload);
	return requireApiData(response, 'เพิ่มฟอนต์เข้าคลังไม่สำเร็จ');
}

export async function deleteSchoolFont(fontId: string): Promise<void> {
	const response = await apiClient.delete<EmptyData, SchoolFontDeleteConflict>(
		`/api/school-fonts/${encodeURIComponent(fontId)}`
	);
	requireApiData(response, 'ลบฟอนต์ไม่สำเร็จ');
}
