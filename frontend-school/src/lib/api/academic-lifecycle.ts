import { apiClient, requireApiData, type ApiRequestOptions } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

export type TermLifecycleWorkspace = components['schemas']['TermLifecycleWorkspace'];
export type TermTransitionAction = components['schemas']['TermTransitionAction'];
export type TermTransitionRequest =
	operations['transitionAcademicTerm']['requestBody']['content']['application/json'];
export type TermTransitionOutcome = components['schemas']['TermTransitionOutcome'];
export type TermActivationWorkspace = components['schemas']['TermActivationWorkspace'];
export type OpeningPolicy = components['schemas']['OpeningPolicy'];
export type UpdateOpeningPolicyInput =
	operations['updateAcademicOpeningPolicy']['requestBody']['content']['application/json'];
export type TermPreparationModule = components['schemas']['TermPreparationModule'];
export type TermPreparationMappingKind = components['schemas']['TermPreparationMappingKind'];
export type TermPreparationMappings = components['schemas']['TermPreparationMappings'];
export type TermPreparationWorkspace = components['schemas']['TermPreparationWorkspace'];
export type TermPreparationOutcome = components['schemas']['TermPreparationOutcome'];
export type PreviewTermPreparationInput =
	operations['previewAcademicTermPreparation']['requestBody']['content']['application/json'];
export type ApplyTermPreparationInput =
	operations['applyAcademicTermPreparation']['requestBody']['content']['application/json'];

export type YearLifecycleWorkspace = components['schemas']['YearLifecycleWorkspace'];
export type YearTransitionAction = components['schemas']['YearTransitionAction'];
export type YearTransitionRequest =
	operations['transitionAcademicYear']['requestBody']['content']['application/json'];
export type YearTransitionOutcome = components['schemas']['YearTransitionOutcome'];
export type YearReopeningWorkspace = components['schemas']['YearReopeningWorkspace'];
export type YearReopeningRequest =
	operations['reopenAcademicYear']['requestBody']['content']['application/json'];
export type YearReopeningOutcome = components['schemas']['YearReopeningOutcome'];

export async function getYearReopeningWorkspace(
	yearId: operations['getYearReopeningWorkspace']['parameters']['path']['year_id'],
	options: ApiRequestOptions = {}
): Promise<YearReopeningWorkspace> {
	return requireApiData(
		await apiClient.get<YearReopeningWorkspace>(
			`/api/academic/lifecycle/years/${encodeURIComponent(yearId)}/reopening`,
			options
		),
		'ตรวจเงื่อนไขการเปิดปีเก่ากลับไม่สำเร็จ'
	);
}

export async function reopenAcademicYear(
	yearId: operations['reopenAcademicYear']['parameters']['path']['year_id'],
	request: YearReopeningRequest
): Promise<YearReopeningOutcome> {
	return requireApiData(
		await apiClient.post<YearReopeningOutcome>(
			`/api/academic/lifecycle/years/${encodeURIComponent(yearId)}/reopening`,
			request
		),
		'เปิดปีเก่ากลับไม่สำเร็จ'
	);
}

export async function getYearLifecycleWorkspace(
	yearId: operations['getYearLifecycleWorkspace']['parameters']['path']['year_id'],
	options: ApiRequestOptions = {}
): Promise<YearLifecycleWorkspace> {
	return requireApiData(
		await apiClient.get<YearLifecycleWorkspace>(
			`/api/academic/lifecycle/years/${encodeURIComponent(yearId)}`,
			options
		),
		'โหลดความพร้อมปีการศึกษาไม่สำเร็จ'
	);
}

export async function transitionAcademicYear(
	yearId: operations['transitionAcademicYear']['parameters']['path']['year_id'],
	request: YearTransitionRequest
): Promise<YearTransitionOutcome> {
	return requireApiData(
		await apiClient.post<YearTransitionOutcome>(
			`/api/academic/lifecycle/years/${encodeURIComponent(yearId)}/transitions`,
			request
		),
		'เปลี่ยนสถานะปีการศึกษาไม่สำเร็จ'
	);
}

export async function getTermLifecycleWorkspace(
	termId: operations['getTermLifecycleWorkspace']['parameters']['path']['term_id'],
	query: operations['getTermLifecycleWorkspace']['parameters']['query'],
	options: ApiRequestOptions = {}
): Promise<TermLifecycleWorkspace> {
	const search = new URLSearchParams(query);
	return requireApiData(
		await apiClient.get<TermLifecycleWorkspace>(
			`/api/academic/lifecycle/terms/${encodeURIComponent(termId)}?${search}`,
			options
		),
		'โหลดความพร้อมภาคเรียนไม่สำเร็จ'
	);
}

export async function getTermActivationWorkspace(
	termId: operations['getTermActivationWorkspace']['parameters']['path']['term_id'],
	query: operations['getTermActivationWorkspace']['parameters']['query'],
	options: ApiRequestOptions = {}
): Promise<TermActivationWorkspace> {
	const search = new URLSearchParams(query);
	return requireApiData(
		await apiClient.get<TermActivationWorkspace>(
			`/api/academic/lifecycle/terms/${encodeURIComponent(termId)}/activation?${search}`,
			options
		),
		'ตรวจความพร้อมเปิดใช้ภาคเรียนไม่สำเร็จ'
	);
}

export async function getAcademicOpeningPolicy(
	options: ApiRequestOptions = {}
): Promise<OpeningPolicy> {
	return requireApiData(
		await apiClient.get<OpeningPolicy>('/api/academic/lifecycle/opening-policy', options),
		'โหลดเกณฑ์เปิดภาคเรียนไม่สำเร็จ'
	);
}

export async function updateAcademicOpeningPolicy(
	input: UpdateOpeningPolicyInput
): Promise<OpeningPolicy> {
	return requireApiData(
		await apiClient.put<OpeningPolicy>('/api/academic/lifecycle/opening-policy', input),
		'บันทึกเกณฑ์เปิดภาคเรียนไม่สำเร็จ'
	);
}

export async function transitionAcademicTerm(
	termId: operations['transitionAcademicTerm']['parameters']['path']['term_id'],
	request: TermTransitionRequest
): Promise<TermTransitionOutcome> {
	return requireApiData(
		await apiClient.post<TermTransitionOutcome>(
			`/api/academic/lifecycle/terms/${encodeURIComponent(termId)}/transitions`,
			request
		),
		'เปลี่ยนสถานะภาคเรียนไม่สำเร็จ'
	);
}

export async function previewAcademicTermPreparation(
	request: PreviewTermPreparationInput,
	options: ApiRequestOptions = {}
): Promise<TermPreparationWorkspace> {
	return requireApiData(
		await apiClient.post<TermPreparationWorkspace>(
			'/api/academic/lifecycle/term-preparations/preview',
			request,
			options
		),
		'ตรวจตัวอย่างการเตรียมภาคเรียนไม่สำเร็จ'
	);
}

export async function applyAcademicTermPreparation(
	request: ApplyTermPreparationInput
): Promise<TermPreparationOutcome> {
	return requireApiData(
		await apiClient.post<TermPreparationOutcome>(
			'/api/academic/lifecycle/term-preparations/apply',
			request
		),
		'เตรียมภาคเรียนไม่สำเร็จ'
	);
}
