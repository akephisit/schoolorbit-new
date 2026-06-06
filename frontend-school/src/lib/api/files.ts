import { apiClient, requireApiData } from '$lib/api/client';

export interface FileUploadResponse {
	success: boolean;
	file: {
		id: string;
		filename: string;
		original_filename: string;
		file_size: number;
		mime_type: string;
		file_type: string;
		storage_path: string;
		url: string;
		thumbnail_url: string | null;
		width: number | null;
		height: number | null;
		created_at: string;
	};
}

export interface FileListResponse {
	success: boolean;
	files: FileUploadResponse['file'][];
	total: number;
}

export interface DeleteFileResponse {
	success: boolean;
	message: string;
}

/**
 * Upload a file to the server
 */
export async function uploadFile(
	file: File,
	fileType: string = 'other',
	isTemporary: boolean = false
): Promise<FileUploadResponse> {
	const formData = new FormData();
	// Append metadata query parameters first (Best practice for streaming servers)
	formData.append('file_type', fileType);
	formData.append('is_temporary', isTemporary ? 'true' : 'false');
	// Append file last
	formData.append('file', file);

	const response = await apiClient.postMultipart<{ file: FileUploadResponse['file'] }>(
		'/api/files/upload',
		formData
	);
	const data = requireApiData(response, 'Upload failed');
	return { success: true, file: data.file };
}

/**
 * Upload profile image
 */
export async function uploadProfileImage(file: File): Promise<FileUploadResponse> {
	return uploadFile(file, 'profile_image', false);
}

/**
 * Upload document
 */
export async function uploadDocument(file: File): Promise<FileUploadResponse> {
	return uploadFile(file, 'document', false);
}

/**
 * List user's files
 */
export async function listUserFiles(): Promise<FileListResponse> {
	const response = await apiClient.get<{ files: FileUploadResponse['file'][]; total: number }>(
		'/api/files'
	);
	const data = requireApiData(response, 'Failed to fetch files');
	return { success: true, files: data.files, total: data.total };
}

/**
 * Delete a file
 */
export async function deleteFile(fileId: string): Promise<DeleteFileResponse> {
	const response = await apiClient.delete<Record<string, never>>(`/api/files/${fileId}`);
	if (!response.success) throw new Error(response.error || 'Failed to delete file');
	return { success: true, message: response.message || 'ลบไฟล์สำเร็จ' };
}
