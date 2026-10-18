import {
	ApiClientError,
	BACKEND_URL,
	apiClient,
	requireApiData,
	type ApiRequestOptions
} from './client';
import type { components } from '#lib/api/generated/school-api.js';

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

export type PublicSchoolIconSize = Schemas['PublicSchoolIconSize'];

export async function getPublicSchoolFavicon(
	options: ApiRequestOptions = {},
	size: PublicSchoolIconSize = '256'
): Promise<Blob> {
	const requestFetch = options.requestFetch ?? globalThis.fetch;
	const blob = requireApiData(
		await apiClient.getExternalBlob(
			`${BACKEND_URL}/api/school/public/favicon?${new URLSearchParams({ size })}`,
			{
				...options,
				requestFetch: (input, init) => requestFetch(input, { ...init, redirect: 'manual' })
			}
		),
		'ไม่สามารถอ่านไอคอนโรงเรียนได้'
	);
	if (blob.type !== 'image/png' || blob.size > 2 * 1024 * 1024)
		throw new ApiClientError('รูปแบบไอคอนโรงเรียนไม่ถูกต้อง', 502);
	return blob;
}

/** Only the configured backend may select this tenant's public social card. */
export async function getPublicSchoolShareImage(options: ApiRequestOptions = {}): Promise<Blob> {
	const requestFetch = options.requestFetch ?? globalThis.fetch;
	const limit = 2 * 1024 * 1024;
	return requireApiData(
		await apiClient.getExternalBlob(`${BACKEND_URL}/api/school/public/share-image`, {
			...options,
			requestFetch: async (input, init) => {
				const response = await requestFetch(input, { ...init, redirect: 'manual' });
				if (!response.ok) {
					await response.body?.cancel();
					throw new ApiClientError('ไม่สามารถอ่านภาพแชร์โรงเรียนได้', response.status);
				}
				if (
					response.headers.get('content-type')?.split(';', 1)[0].trim().toLowerCase() !==
						'image/png' ||
					Number(response.headers.get('content-length')) > limit ||
					!response.body
				) {
					await response.body?.cancel();
					throw new ApiClientError('รูปแบบภาพแชร์โรงเรียนไม่ถูกต้อง', 502);
				}
				const reader = response.body.getReader();
				const chunks: Uint8Array<ArrayBuffer>[] = [];
				let size = 0;
				try {
					while (true) {
						const { done, value } = await reader.read();
						if (done) break;
						size += value.byteLength;
						if (size > limit) throw new ApiClientError('ภาพแชร์โรงเรียนมีขนาดใหญ่เกินไป', 502);
						chunks.push(new Uint8Array(value));
					}
				} finally {
					await reader.cancel();
					reader.releaseLock();
				}
				const png = new Blob(chunks, { type: 'image/png' });
				const header = new Uint8Array(await png.slice(0, 24).arrayBuffer());
				const view = new DataView(header.buffer);
				if (
					header.length < 24 ||
					![137, 80, 78, 71, 13, 10, 26, 10].every((byte, index) => header[index] === byte) ||
					view.getUint32(12) !== 0x49484452 ||
					view.getUint32(16) !== 1200 ||
					view.getUint32(20) !== 630
				) {
					throw new ApiClientError('รูปแบบภาพแชร์โรงเรียนไม่ถูกต้อง', 502);
				}
				return new Response(png, { headers: { 'content-type': 'image/png' } });
			}
		}),
		'ไม่สามารถอ่านภาพแชร์โรงเรียนได้'
	);
}
