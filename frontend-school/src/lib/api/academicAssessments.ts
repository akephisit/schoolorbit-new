import { ApiClientError, apiClient, requireApiData, type ApiResponse } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

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

export type AssessmentPlanFilters = NonNullable<
	operations['listAssessmentPlans']['parameters']['query']
>;
type AssessmentPlanPath = NonNullable<operations['getAssessmentPlan']['parameters']['path']>;
type AssessmentControlPath = NonNullable<
	operations['updateAssessmentPhaseControl']['parameters']['path']
>;
type AssessmentControlQuery = NonNullable<
	operations['listAssessmentPhaseControls']['parameters']['query']
>;

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

function assessmentPlanQuery(filters: AssessmentPlanFilters): AssessmentPlanFilters {
	const academicTermId = filters.academicTermId.trim();
	if (!academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return { ...filters, academicTermId } satisfies AssessmentPlanFilters;
}

export const listAssessmentPlans = (filters: AssessmentPlanFilters) =>
	assessmentData(
		apiClient.get<AssessmentPlanSummary[]>('/api/academic/assessments/plans', {
			query: assessmentPlanQuery(filters)
		}),
		'ไม่สามารถโหลดภาพรวมโครงสร้างคะแนนได้'
	);

export const getAssessmentPlan = (offeringId: string) => {
	const path = { offering_id: offeringId } satisfies AssessmentPlanPath;
	return assessmentData(
		apiClient.get<AssessmentPlanDetail>(
			`/api/academic/assessments/offerings/${encodeURIComponent(path.offering_id)}`
		),
		'ไม่สามารถโหลดโครงสร้างคะแนนของรายการเปิดสอนได้'
	);
};

export const listAssessmentPhaseControls = (academicTermId: string) => {
	const query = { academicTermId: academicTermId.trim() } satisfies AssessmentControlQuery;
	if (!query.academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return assessmentData(
		apiClient.get<AssessmentPhaseControl[]>('/api/academic/assessments/phase-controls', { query }),
		'ไม่สามารถโหลดช่วงเวลาเปิดแก้โครงสร้างคะแนนได้'
	);
};

export const updateAssessmentPhaseControl = (
	controlId: string,
	payload: UpdateAssessmentPhaseControlRequest
) => {
	const path = { control_id: controlId } satisfies AssessmentControlPath;
	return assessmentData(
		apiClient.put<AssessmentPhaseControl>(
			`/api/academic/assessments/phase-controls/${encodeURIComponent(path.control_id)}`,
			payload
		),
		'ไม่สามารถบันทึกช่วงเวลาเปิดแก้โครงสร้างคะแนนได้'
	);
};

export const saveAssessmentPlan = (offeringId: string, payload: SaveAssessmentPlanRequest) => {
	const path = { offering_id: offeringId } satisfies AssessmentPlanPath;
	return assessmentData(
		apiClient.put<AssessmentPlanDetail>(
			`/api/academic/assessments/offerings/${encodeURIComponent(path.offering_id)}`,
			payload
		),
		'ไม่สามารถบันทึกโครงสร้างคะแนนได้'
	);
};
