import { ApiClientError, apiClient, requireApiData, type ApiResponse } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type AssessmentPlanSummary = Schemas['AssessmentPlanSummary'];
export type AssessmentPlanDetail = Schemas['AssessmentPlanDetail'];
export type AssessmentPhase = Schemas['AssessmentPhase'];
export type AssessmentPhaseCode = Schemas['AssessmentPhaseCode'];
export type AssessmentExamArrangement = Schemas['AssessmentExamArrangement'];
export type AssessmentPhaseControl = Schemas['AssessmentPhaseControl'];
export type AssessmentCoordinatorOption = Schemas['AssessmentCoordinatorOption'];
export type AssessmentReadiness = Schemas['AssessmentReadiness'];
export type AssessmentReadinessFinding = Schemas['AssessmentReadinessFinding'];
export type SaveAssessmentPlanRequest = Schemas['SaveAssessmentPlanRequest'];
export type SaveAssessmentPhaseRequest = Schemas['SaveAssessmentPhaseRequest'];
export type UpdateAssessmentPhaseControlRequest = Schemas['UpdateAssessmentPhaseControlRequest'];

export interface AssessmentPlanFilters {
	academicTermId: string;
	subjectId?: string;
	instructorId?: string;
	ready?: boolean;
	examArrangement?: AssessmentExamArrangement;
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
	if (filters.ready !== undefined) params.set('ready', String(filters.ready));
	if (filters.examArrangement) params.set('examArrangement', filters.examArrangement);
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
		'ไม่สามารถโหลดโครงสร้างคะแนนของรายการเปิดสอนได้'
	);

export const listAssessmentPhaseControls = (academicTermId: string) =>
	assessmentData(
		apiClient.get<AssessmentPhaseControl[]>(
			`/api/academic/assessments/phase-controls?academicTermId=${encodeURIComponent(academicTermId)}`
		),
		'ไม่สามารถโหลดช่วงเวลาเปิดกรอกคะแนนได้'
	);

export const updateAssessmentPhaseControl = (
	controlId: string,
	payload: UpdateAssessmentPhaseControlRequest
) =>
	assessmentData(
		apiClient.put<AssessmentPhaseControl>(
			`/api/academic/assessments/phase-controls/${encodeURIComponent(controlId)}`,
			payload
		),
		'ไม่สามารถบันทึกช่วงเวลาเปิดกรอกคะแนนได้'
	);

export const saveAssessmentPlan = (offeringId: string, payload: SaveAssessmentPlanRequest) =>
	assessmentData(
		apiClient.put<AssessmentPlanDetail>(
			`/api/academic/assessments/offerings/${encodeURIComponent(offeringId)}`,
			payload
		),
		'ไม่สามารถบันทึกโครงสร้างคะแนนได้'
	);
