import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components } from '#lib/api/generated/school-api.js';

type Schema = components['schemas'];
export type YearRolloverOptions = Schema['YearRolloverOptions'];
export type YearRolloverPreviewInput = Schema['YearRolloverPreviewInput'];
export type YearRolloverPreview = Schema['YearRolloverPreview'];
export type YearRolloverStudent = Schema['YearRolloverStudent'];
export type RolloverDecision = Schema['PromotionDecisionInput'];
export type ConfirmYearRolloverInput = Schema['ConfirmYearRolloverInput'];
export type YearRolloverOutcome = Schema['YearRolloverOutcome'];
export type AcademicRetentionWorkspace = Schema['AcademicRetentionWorkspace'];
export type AcademicRetentionPolicy = Schema['AcademicRetentionPolicy'];
export type UpdateAcademicRetentionPolicy = Schema['UpdateAcademicRetentionPolicy'];

export async function getYearRolloverOptions(options: ApiRequestOptions = {}) {
	return requireApiData(
		await apiClient.get<YearRolloverOptions>(
			'/api/academic/lifecycle/year-rollovers/options',
			options
		),
		'โหลดปีปลายทางไม่สำเร็จ'
	);
}
export async function previewYearRollover(
	input: YearRolloverPreviewInput,
	options: ApiRequestOptions = {}
) {
	return requireApiData(
		await apiClient.post<YearRolloverPreview>(
			'/api/academic/lifecycle/year-rollovers/preview',
			input,
			options
		),
		'โหลดตัวอย่างรายชื่อนักเรียนปีใหม่ไม่สำเร็จ'
	);
}
export async function confirmYearRollover(input: ConfirmYearRolloverInput) {
	return requireApiData(
		await apiClient.post<YearRolloverOutcome>(
			'/api/academic/lifecycle/year-rollovers/confirm',
			input
		),
		'เตรียมรายชื่อนักเรียนปีใหม่ไม่สำเร็จ'
	);
}
export async function getAcademicRetentionWorkspace(
	yearId: string,
	options: ApiRequestOptions = {}
) {
	return requireApiData(
		await apiClient.get<AcademicRetentionWorkspace>(
			`/api/academic/lifecycle/years/${encodeURIComponent(yearId)}/retention`,
			options
		),
		'โหลดระยะเวลาเก็บข้อมูลไม่สำเร็จ'
	);
}
export async function updateAcademicRetentionPolicy(input: UpdateAcademicRetentionPolicy) {
	return requireApiData(
		await apiClient.put<AcademicRetentionPolicy>('/api/academic/lifecycle/retention-policy', input),
		'บันทึกระยะเวลาเก็บข้อมูลไม่สำเร็จ'
	);
}
