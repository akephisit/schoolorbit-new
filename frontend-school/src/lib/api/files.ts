import { apiClient, BACKEND_URL, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type FileMetadata = Schemas['FileMetadata'];
export type FileDeleteResult = Schemas['FileDeleteResult'];
export type FilePurpose = Schemas['FilePurpose'];

function resourceQuery(resourceId?: string): string {
	if (!resourceId) return '';
	return `?${new URLSearchParams({ resource_id: resourceId }).toString()}`;
}

export async function uploadFile(
	file: File,
	purpose: FilePurpose,
	resourceId?: string
): Promise<FileMetadata> {
	const formData = new FormData();
	formData.append('purpose', purpose);
	if (resourceId) formData.append('resource_id', resourceId);
	formData.append('file', file);

	const response = await apiClient.postMultipart<FileMetadata>('/api/files', formData);
	return requireApiData(response, 'อัปโหลดไฟล์ไม่สำเร็จ');
}

export function uploadProfileImage(file: File, userId?: string): Promise<FileMetadata> {
	return uploadFile(file, 'profile_image', userId);
}

export function getFileMetadata(fileId: string, resourceId?: string): Promise<FileMetadata> {
	return apiClient
		.get<FileMetadata>(`/api/files/${fileId}${resourceQuery(resourceId)}`)
		.then((response) => requireApiData(response, 'ไม่สามารถโหลดข้อมูลไฟล์ได้'));
}

export function deleteFile(fileId: string, resourceId?: string): Promise<FileDeleteResult> {
	return apiClient
		.delete<FileDeleteResult>(`/api/files/${fileId}${resourceQuery(resourceId)}`)
		.then((response) => requireApiData(response, 'ลบไฟล์ไม่สำเร็จ'));
}

export function downloadFile(fileId: string, resourceId?: string): Promise<Blob> {
	return apiClient
		.postBlob(`/api/files/${fileId}/download${resourceQuery(resourceId)}`)
		.then((response) => requireApiData(response, 'ดาวน์โหลดไฟล์ไม่สำเร็จ'));
}

export function publicFileUrl(fileId: string): string {
	return `${BACKEND_URL}/api/public/files/${fileId}/content`;
}
