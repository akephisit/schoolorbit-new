import { apiClient, BACKEND_URL, requireApiData } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type FileMetadata = Schemas['FileMetadata'];
export type FileDeleteResult = Schemas['FileDeleteResult'];
export type FileDownloadGrantResponse = Schemas['FileDownloadGrantResponse'];
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

export async function downloadGrantedFile(
	grant: FileDownloadGrantResponse,
	signal?: AbortSignal
): Promise<Blob> {
	const response = await fetch(grant.url, {
		method: 'GET',
		mode: 'cors',
		credentials: 'omit',
		referrerPolicy: 'no-referrer',
		signal
	});
	if (!response.ok) {
		throw new Error(`ดาวน์โหลดไฟล์ไม่สำเร็จ (${response.status})`);
	}
	return response.blob();
}

export async function downloadFile(
	fileId: string,
	resourceId?: string,
	signal?: AbortSignal
): Promise<Blob> {
	const response = await apiClient.post<FileDownloadGrantResponse>(
		`/api/files/${fileId}/download${resourceQuery(resourceId)}`
	);
	const grant = requireApiData(response, 'ดาวน์โหลดไฟล์ไม่สำเร็จ');
	signal?.throwIfAborted();
	return downloadGrantedFile(grant, signal);
}

export function publicFileUrl(fileId: string): string {
	return `${BACKEND_URL}/api/public/files/${fileId}/content`;
}
