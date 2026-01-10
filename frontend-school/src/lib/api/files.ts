import { getAuthToken } from './auth';

const API_URL = import.meta.env.VITE_API_URL || 'http://localhost:8081';

export interface FileUploadResponse {
    success: boolean;
    file: {
        id: string;
        filename: string;
        original_filename: string;
        file_size: number;
        mime_type: string;
        file_type: string;
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
    formData.append('file', file);
    formData.append('file_type', fileType);
    formData.append('is_temporary', isTemporary ? 'true' : 'false');

    const token = getAuthToken();
    if (!token) {
        throw new Error('Not authenticated');
    }

    const response = await fetch(`${API_URL}/api/files/upload`, {
        method: 'POST',
        headers: {
            Authorization: `Bearer ${token}`
        },
        body: formData
    });

    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Upload failed' }));
        throw new Error(error.error || `Upload failed: ${response.statusText}`);
    }

    return response.json();
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
    const token = getAuthToken();
    if (!token) {
        throw new Error('Not authenticated');
    }

    const response = await fetch(`${API_URL}/api/files`, {
        headers: {
            Authorization: `Bearer ${token}`
        }
    });

    if (!response.ok) {
        throw new Error(`Failed to fetch files: ${response.statusText}`);
    }

    return response.json();
}

/**
 * Delete a file
 */
export async function deleteFile(fileId: string): Promise<DeleteFileResponse> {
    const token = getAuthToken();
    if (!token) {
        throw new Error('Not authenticated');
    }

    const response = await fetch(`${API_URL}/api/files/${fileId}`, {
        method: 'DELETE',
        headers: {
            Authorization: `Bearer ${token}`
        }
    });

    if (!response.ok) {
        throw new Error(`Failed to delete file: ${response.statusText}`);
    }

    return response.json();
}
