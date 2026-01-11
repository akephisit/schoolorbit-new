import { PUBLIC_BACKEND_URL } from '$env/static/public';

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
    // Append metadata query parameters first (Best practice for streaming servers)
    formData.append('file_type', fileType);
    formData.append('is_temporary', isTemporary ? 'true' : 'false');
    // Append file last
    formData.append('file', file);

    const response = await fetch(`${PUBLIC_BACKEND_URL}/api/files/upload`, {
        method: 'POST',
        credentials: 'include', // Use cookie-based auth
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
    const response = await fetch(`${PUBLIC_BACKEND_URL}/api/files`, {
        credentials: 'include'
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
    const response = await fetch(`${PUBLIC_BACKEND_URL}/api/files/${fileId}`, {
        method: 'DELETE',
        credentials: 'include'
    });

    if (!response.ok) {
        throw new Error(`Failed to delete file: ${response.statusText}`);
    }

    return response.json();
}
