import { ApiClientError, apiClient, requireApiData, type ApiResponse } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type AssessmentPlanSummary = Schemas['AssessmentPlanSummary'];
export type AssessmentPlanDetail = Schemas['AssessmentPlanDetail'];
export type AssessmentCategory = Schemas['AssessmentCategory'];
export type AssessmentItem = Schemas['AssessmentItem'];
export type SaveAssessmentPlanRequest = Schemas['SaveAssessmentPlanRequest'];
export type SaveAssessmentCategoryRequest = Schemas['SaveAssessmentCategoryRequest'];
export type SaveAssessmentItemRequest = Schemas['SaveAssessmentItemRequest'];
export type AssessmentSettings = Schemas['AssessmentSettingsResponse'];
export type UpdateAssessmentSettingsRequest = Schemas['UpdateAssessmentSettingsRequest'];
export type AssessmentPlanStatus = AssessmentPlanSummary['status'];
export type AssessmentExamMode = AssessmentCategory['examMode'];
export type AssessmentAllocationStatus = AssessmentCategory['allocationStatus'];

export interface AssessmentPlanFilters {
	academicTermId: string;
	subjectId?: string;
	instructorId?: string;
	status?: AssessmentPlanStatus;
}

async function assessmentData<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	const response = await request;
	if (response.status === 409) {
		throw new ApiClientError(
			`${response.error || fallback} กรุณาเก็บข้อมูลที่แก้ไว้ แล้วโหลดข้อมูลล่าสุดก่อนบันทึกอีกครั้ง`,
			409
		);
	}
	return requireApiData(response, fallback);
}

function assessmentPlanQuery(filters: AssessmentPlanFilters): string {
	const academicTermId = filters.academicTermId.trim();
	if (!academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');

	const params = new URLSearchParams({ academicTermId });
	if (filters.subjectId) params.set('subjectId', filters.subjectId);
	if (filters.instructorId) params.set('instructorId', filters.instructorId);
	if (filters.status) params.set('status', filters.status);
	return `?${params.toString()}`;
}

export const listAssessmentPlans = (filters: AssessmentPlanFilters) =>
	assessmentData(
		apiClient.get<AssessmentPlanSummary[]>(
			`/api/academic/assessments/plans${assessmentPlanQuery(filters)}`
		),
		'ไม่สามารถโหลดภาพรวมโครงสร้างคะแนนได้'
	);

export const getAssessmentPlan = (offeringId: string) =>
	assessmentData(
		apiClient.get<AssessmentPlanDetail>(
			`/api/academic/assessments/offerings/${encodeURIComponent(offeringId)}`
		),
		'ไม่สามารถโหลดโครงสร้างคะแนนของชุดการเรียนได้'
	);

export const getAssessmentSettings = () =>
	assessmentData(
		apiClient.get<AssessmentSettings>('/api/academic/assessments/settings'),
		'ไม่สามารถโหลดการตั้งค่าโครงสร้างคะแนนได้'
	);

export const updateAssessmentSettings = (payload: UpdateAssessmentSettingsRequest) =>
	assessmentData(
		apiClient.put<AssessmentSettings>('/api/academic/assessments/settings', payload),
		'ไม่สามารถบันทึกการตั้งค่าโครงสร้างคะแนนได้'
	);

export const saveAssessmentPlan = (offeringId: string, payload: SaveAssessmentPlanRequest) =>
	assessmentData(
		apiClient.put<AssessmentPlanDetail>(
			`/api/academic/assessments/offerings/${encodeURIComponent(offeringId)}`,
			payload
		),
		'ไม่สามารถบันทึกโครงสร้างคะแนนได้'
	);

export const submitAssessmentPlan = (offeringId: string) =>
	assessmentData(
		apiClient.post<AssessmentPlanDetail>(
			`/api/academic/assessments/offerings/${encodeURIComponent(offeringId)}/submit`
		),
		'ไม่สามารถส่งโครงสร้างคะแนนได้'
	);
