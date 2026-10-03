import { apiClient, requireApiData, type ApiRequestOptions } from './client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type OptionalNonNull<T> = { [Key in keyof T]?: Exclude<T[Key], null> };

export type SchoolSettingsDto = Schemas['SchoolSettingsResponse'];
export type PublicSchoolInfoDto = Schemas['PublicSchoolInfoData'];
export type PublicSchoolStatistics = Schemas['PublicSchoolStatistics'];
export type PublicSchoolOrganization = Schemas['PublicSchoolOrganization'];
export type SchoolSettings = OptionalNonNull<SchoolSettingsDto>;
export type PublicSchoolInfo = OptionalNonNull<PublicSchoolInfoDto>;

export interface UpdateSchoolSettingsRequest {
	logoFileId?: string | null;
}

export async function getPublicSchoolStatistics(
	options: ApiRequestOptions = {}
): Promise<PublicSchoolStatistics> {
	return requireApiData(
		await apiClient.getPublic<PublicSchoolStatistics>('/api/school/public/statistics', options),
		'โหลดสถิติโรงเรียนไม่สำเร็จ'
	);
}

export async function getPublicSchoolOrganization(
	options: ApiRequestOptions = {}
): Promise<PublicSchoolOrganization> {
	return requireApiData(
		await apiClient.getPublic<PublicSchoolOrganization>('/api/school/public/organization', options),
		'โหลดโครงสร้างบริหารไม่สำเร็จ'
	);
}

function schoolSettingsFromDto(dto: SchoolSettingsDto): SchoolSettings {
	return {
		...(dto.logoFileId === null ? {} : { logoFileId: dto.logoFileId })
	};
}

function publicSchoolInfoFromDto(dto: PublicSchoolInfoDto): PublicSchoolInfo {
	return {
		...(dto.logoFileId === null ? {} : { logoFileId: dto.logoFileId }),
		...(dto.schoolName === null ? {} : { schoolName: dto.schoolName })
	};
}

export async function getSchoolSettings(options: ApiRequestOptions = {}): Promise<SchoolSettings> {
	const res = await apiClient.get<SchoolSettingsDto>('/api/school/settings', options);
	return schoolSettingsFromDto(requireApiData(res, 'โหลดการตั้งค่าโรงเรียนไม่สำเร็จ'));
}

export async function updateSchoolSettings(data: UpdateSchoolSettingsRequest): Promise<void> {
	const res = await apiClient.patch<Record<string, never>>('/api/school/settings', data);
	if (!res.success) throw new Error(res.error);
}

export async function deleteSchoolLogo(): Promise<void> {
	const res = await apiClient.delete<Record<string, never>>('/api/school/settings/logo');
	if (!res.success) throw new Error(res.error);
}

async function loadPublicSchoolInfo(
	options: ApiRequestOptions = {}
): Promise<
	{ info: PublicSchoolInfo; error?: undefined } | { info: PublicSchoolInfo; error: string }
> {
	const res = await apiClient.getPublic<PublicSchoolInfoDto>('/api/school/public', options);
	if (!res.success) return { info: {}, error: res.error };
	return { info: res.data ? publicSchoolInfoFromDto(res.data) : {} };
}

export async function getPublicSchoolInfo(): Promise<PublicSchoolInfo> {
	return (await loadPublicSchoolInfo()).info;
}

export async function getRequiredPublicSchoolInfo(
	options: ApiRequestOptions = {}
): Promise<PublicSchoolInfo> {
	const result = await loadPublicSchoolInfo(options);
	if (result.error) throw new Error(result.error);
	return result.info;
}
