import {
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type ResultQuery = NonNullable<operations['listAcademicGradingPolicies']['parameters']['query']>;
type ResultGroupPath = NonNullable<operations['getCourseResultPreparation']['parameters']['path']>;
type ResultSubjectPath = NonNullable<operations['lockCourseSubjectResults']['parameters']['path']>;
type ResultPolicyPath = NonNullable<
	operations['activateAcademicGradingPolicy']['parameters']['path']
>;
type EffectiveResultQuery = NonNullable<
	operations['searchEffectiveAcademicResults']['parameters']['query']
>;

export type AcademicResultContext = ResultQuery;
export type AcademicGradingPolicyInput = Schemas['GradingPolicyInput'];
export type AcademicGradingPolicyVersion = Schemas['GradingPolicyVersion'];
export type AcademicPolicyActivationInput = Schemas['PolicyActivationInput'];
export type CourseResultSelectionInput = Schemas['SelectionInput'];
export type CourseResultPreparationWorkspace = Schemas['CoursePreparationWorkspace'];
export type ActivityResultBatchInput = Schemas['ActivityBatchInput'];
export type ActivityResultPreparationWorkspace = Schemas['ActivityPreparationWorkspace'];
export type AcademicResultConfirmationInput = Schemas['ResultConfirmationInput'];
export type AcademicResultReadiness = Schemas['ResultReadiness'];
export type CourseResultLockOutcome = Schemas['CourseResultLockOutcome'];
export type ActivityResultLockOutcome = Schemas['ActivityResultLockOutcome'];
export type BulkActivityResultLockOutcome = Schemas['BulkActivityResultLockOutcome'];
export type EffectiveResultKind = Schemas['EffectiveResultKind'];
export type EffectiveResultSearchItem = Schemas['EffectiveResultSearchItem'];
export type EffectiveResult = Schemas['EffectiveResult'];
export type AcademicResultCorrectionInput = Schemas['ResultCorrectionInput'];
export type AcademicResultSearch = EffectiveResultQuery;

function resultData<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	return request.then((response) => requireApiData(response, fallback));
}

function query(context: AcademicResultContext): ResultQuery {
	const value = {
		academicYearId: context.academicYearId.trim(),
		academicTermId: context.academicTermId.trim()
	} satisfies ResultQuery;
	if (!value.academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	if (!value.academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return value;
}

function groupPath(groupId: string): ResultGroupPath {
	return { group_id: groupId } satisfies ResultGroupPath;
}

export function listAcademicGradingPolicies(
	context: AcademicResultContext,
	options: ApiRequestOptions = {}
): Promise<AcademicGradingPolicyVersion[]> {
	return resultData(
		apiClient.get<AcademicGradingPolicyVersion[]>('/api/academic/results/policies', {
			...options,
			query: query(context)
		}),
		'ไม่สามารถโหลดเกณฑ์ตัดผลการเรียนได้'
	);
}

export function createAcademicGradingPolicy(
	context: AcademicResultContext,
	body: AcademicGradingPolicyInput
): Promise<AcademicGradingPolicyVersion> {
	const input = body satisfies AcademicGradingPolicyInput;
	return resultData(
		apiClient.post<AcademicGradingPolicyVersion>('/api/academic/results/policies', input, {
			query: query(context)
		}),
		'ไม่สามารถสร้างเกณฑ์ตัดผลการเรียนได้'
	);
}

export function activateAcademicGradingPolicy(
	policyId: string,
	context: AcademicResultContext,
	body: AcademicPolicyActivationInput
): Promise<AcademicGradingPolicyVersion> {
	const path = { policy_id: policyId } satisfies ResultPolicyPath;
	const input = body satisfies AcademicPolicyActivationInput;
	return resultData(
		apiClient.post<AcademicGradingPolicyVersion>(
			`/api/academic/results/policies/${encodeURIComponent(path.policy_id)}/activate`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถเปิดใช้เกณฑ์ตัดผลการเรียนได้'
	);
}

export function getCourseResultPreparation(
	groupId: string,
	context: AcademicResultContext,
	options: ApiRequestOptions = {}
): Promise<CourseResultPreparationWorkspace> {
	const path = groupPath(groupId);
	return resultData(
		apiClient.get<CourseResultPreparationWorkspace>(
			`/api/academic/results/groups/${encodeURIComponent(path.group_id)}/course`,
			{ ...options, query: query(context) }
		),
		'ไม่สามารถโหลดผลการเรียนรายวิชาได้'
	);
}

export function saveCourseResultSelection(
	groupId: string,
	context: AcademicResultContext,
	body: CourseResultSelectionInput
): Promise<CourseResultPreparationWorkspace> {
	const path = groupPath(groupId);
	const input = body satisfies CourseResultSelectionInput;
	return resultData(
		apiClient.put<CourseResultPreparationWorkspace>(
			`/api/academic/results/groups/${encodeURIComponent(path.group_id)}/course/selection`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถบันทึกผลการเรียนที่เลือกได้'
	);
}

export function confirmCourseGroupResults(
	groupId: string,
	context: AcademicResultContext,
	body: AcademicResultConfirmationInput
): Promise<CourseResultPreparationWorkspace> {
	const path = groupPath(groupId);
	const input = body satisfies AcademicResultConfirmationInput;
	return resultData(
		apiClient.post<CourseResultPreparationWorkspace>(
			`/api/academic/results/groups/${encodeURIComponent(path.group_id)}/course/confirm`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถยืนยันผลการเรียนของกลุ่มได้'
	);
}

export function getActivityResultPreparation(
	groupId: string,
	context: AcademicResultContext,
	options: ApiRequestOptions = {}
): Promise<ActivityResultPreparationWorkspace> {
	const path = groupPath(groupId);
	return resultData(
		apiClient.get<ActivityResultPreparationWorkspace>(
			`/api/academic/results/groups/${encodeURIComponent(path.group_id)}/activity`,
			{ ...options, query: query(context) }
		),
		'ไม่สามารถโหลดผลกิจกรรมได้'
	);
}

export function saveActivityResultOutcomes(
	groupId: string,
	context: AcademicResultContext,
	body: ActivityResultBatchInput
): Promise<ActivityResultPreparationWorkspace> {
	const path = groupPath(groupId);
	const input = body satisfies ActivityResultBatchInput;
	return resultData(
		apiClient.put<ActivityResultPreparationWorkspace>(
			`/api/academic/results/groups/${encodeURIComponent(path.group_id)}/activity/outcomes`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถบันทึกผลกิจกรรมได้'
	);
}

export function confirmActivityGroupResults(
	groupId: string,
	context: AcademicResultContext,
	body: AcademicResultConfirmationInput
): Promise<ActivityResultPreparationWorkspace> {
	const path = groupPath(groupId);
	const input = body satisfies AcademicResultConfirmationInput;
	return resultData(
		apiClient.post<ActivityResultPreparationWorkspace>(
			`/api/academic/results/groups/${encodeURIComponent(path.group_id)}/activity/confirm`,
			input,
			{ query: query(context) }
		),
		'ไม่สามารถยืนยันผลกิจกรรมของกลุ่มได้'
	);
}

export function getAcademicResultReadiness(
	context: AcademicResultContext,
	options: ApiRequestOptions = {}
): Promise<AcademicResultReadiness> {
	return resultData(
		apiClient.get<AcademicResultReadiness>('/api/academic/results/readiness', {
			...options,
			query: query(context)
		}),
		'ไม่สามารถตรวจความพร้อมผลการเรียนได้'
	);
}

export function lockCourseSubjectResults(
	subjectId: string,
	context: AcademicResultContext
): Promise<CourseResultLockOutcome> {
	const path = { subject_id: subjectId } satisfies ResultSubjectPath;
	return resultData(
		apiClient.post<CourseResultLockOutcome>(
			`/api/academic/results/subjects/${encodeURIComponent(path.subject_id)}/lock`,
			undefined,
			{ query: query(context) }
		),
		'ไม่สามารถล็อกผลการเรียนรายวิชาได้'
	);
}

export function lockActivityGroupResults(
	groupId: string,
	context: AcademicResultContext
): Promise<ActivityResultLockOutcome> {
	const path = groupPath(groupId);
	return resultData(
		apiClient.post<ActivityResultLockOutcome>(
			`/api/academic/results/groups/${encodeURIComponent(path.group_id)}/activity/lock`,
			undefined,
			{ query: query(context) }
		),
		'ไม่สามารถล็อกผลกิจกรรมได้'
	);
}

export function lockAllReadyActivityResults(
	context: AcademicResultContext
): Promise<BulkActivityResultLockOutcome> {
	return resultData(
		apiClient.post<BulkActivityResultLockOutcome>(
			'/api/academic/results/activities/lock-ready',
			undefined,
			{ query: query(context) }
		),
		'ไม่สามารถล็อกผลกิจกรรมที่พร้อมได้'
	);
}

export function searchEffectiveAcademicResults(
	filters: AcademicResultSearch,
	options: ApiRequestOptions = {}
): Promise<EffectiveResultSearchItem[]> {
	const query = {
		...filters,
		academicYearId: filters.academicYearId.trim(),
		academicTermId: filters.academicTermId.trim()
	} satisfies EffectiveResultQuery;
	if (!query.academicYearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	if (!query.academicTermId) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return resultData(
		apiClient.get<EffectiveResultSearchItem[]>('/api/academic/results/effective', {
			...options,
			query
		}),
		'ไม่สามารถค้นหาผลการเรียนได้'
	);
}

export function correctEffectiveAcademicResult(
	context: AcademicResultContext,
	body: AcademicResultCorrectionInput
): Promise<EffectiveResult> {
	const input = body satisfies AcademicResultCorrectionInput;
	return resultData(
		apiClient.post<EffectiveResult>('/api/academic/results/corrections', input, {
			query: query(context)
		}),
		'ไม่สามารถแก้ไขผลการเรียนได้'
	);
}
