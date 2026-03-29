import { apiClient } from './client';

export interface SchoolSettings {
    logoUrl?: string;
    logoFileId?: string;
}

export interface UpdateSchoolSettingsRequest {
    logoPath?: string;
    logoFileId?: string;
}

export interface PublicSchoolInfo {
    logoUrl?: string;
    schoolName?: string;
}

export async function getSchoolSettings(): Promise<SchoolSettings> {
    const res = await apiClient.get('/api/school/settings');
    if (!res.success) throw new Error(res.error);
    return (res.data as SchoolSettings) ?? {};
}

export async function updateSchoolSettings(data: UpdateSchoolSettingsRequest): Promise<void> {
    const res = await apiClient.patch('/api/school/settings', data);
    if (!res.success) throw new Error(res.error);
}

export async function deleteSchoolLogo(): Promise<void> {
    const res = await apiClient.delete('/api/school/settings/logo');
    if (!res.success) throw new Error(res.error);
}

export async function getPublicSchoolInfo(): Promise<PublicSchoolInfo> {
    const res = await apiClient.get('/api/school/public');
    if (!res.success) return {};
    return (res.data as PublicSchoolInfo) ?? {};
}
