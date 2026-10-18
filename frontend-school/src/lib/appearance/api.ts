import { apiClient, requireApiData, type ApiRequestOptions } from '../api/client.js';
import {
	validateAppearanceResponse,
	type SchoolAppearanceResponse,
	type UpdateSchoolAppearanceRequest
} from './model.js';

function validated(value: SchoolAppearanceResponse): SchoolAppearanceResponse {
	const result = validateAppearanceResponse(value);
	if (!result) throw new Error('รูปแบบการแสดงผลจากเซิร์ฟเวอร์ไม่ถูกต้อง');
	return result;
}
export async function getPublicSchoolAppearance(options: ApiRequestOptions = {}) {
	return validated(
		requireApiData(
			await apiClient.getPublic<SchoolAppearanceResponse>('/api/school/public/appearance', options),
			'โหลดรูปแบบโรงเรียนไม่สำเร็จ'
		)
	);
}
export async function getSchoolAppearance(options: ApiRequestOptions = {}) {
	return validated(
		requireApiData(
			await apiClient.get<SchoolAppearanceResponse>('/api/school/settings/appearance', options),
			'โหลดรูปแบบโรงเรียนไม่สำเร็จ'
		)
	);
}
export async function updateSchoolAppearance(
	data: UpdateSchoolAppearanceRequest,
	options: ApiRequestOptions = {}
) {
	return validated(
		requireApiData(
			await apiClient.patch<SchoolAppearanceResponse>(
				'/api/school/settings/appearance',
				data,
				options
			),
			'บันทึกรูปแบบโรงเรียนไม่สำเร็จ'
		)
	);
}
