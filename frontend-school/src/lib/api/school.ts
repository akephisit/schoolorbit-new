import { apiClient } from './client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type OptionalNonNull<T> = { [Key in keyof T]?: Exclude<T[Key], null> };

export type SchoolSettingsDto = Schemas['SchoolSettingsResponse'];
export type PublicSchoolInfoDto = Schemas['PublicSchoolInfoData'];
export type SchoolSettings = OptionalNonNull<SchoolSettingsDto>;
export type PublicSchoolInfo = OptionalNonNull<PublicSchoolInfoDto>;

export interface UpdateSchoolSettingsRequest {
	logoPath?: string;
	logoFileId?: string;
}

function schoolSettingsFromDto(dto: SchoolSettingsDto): SchoolSettings {
	return {
		...(dto.logoUrl === null ? {} : { logoUrl: dto.logoUrl }),
		...(dto.logoFileId === null ? {} : { logoFileId: dto.logoFileId })
	};
}

function publicSchoolInfoFromDto(dto: PublicSchoolInfoDto): PublicSchoolInfo {
	return {
		...(dto.logoUrl === null ? {} : { logoUrl: dto.logoUrl }),
		...(dto.schoolName === null ? {} : { schoolName: dto.schoolName })
	};
}

export async function getSchoolSettings(): Promise<SchoolSettings> {
	const res = await apiClient.get<SchoolSettingsDto>('/api/school/settings');
	if (!res.success) throw new Error(res.error);
	return res.data ? schoolSettingsFromDto(res.data) : {};
}

export async function updateSchoolSettings(data: UpdateSchoolSettingsRequest): Promise<void> {
	const res = await apiClient.patch<Record<string, never>>('/api/school/settings', data);
	if (!res.success) throw new Error(res.error);
}

export async function deleteSchoolLogo(): Promise<void> {
	const res = await apiClient.delete<Record<string, never>>('/api/school/settings/logo');
	if (!res.success) throw new Error(res.error);
}

export async function getPublicSchoolInfo(): Promise<PublicSchoolInfo> {
	const res = await apiClient.get<PublicSchoolInfoDto>('/api/school/public');
	if (!res.success) return {};
	return res.data ? publicSchoolInfoFromDto(res.data) : {};
}
