import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];
export type LeaveKind = Schemas['LeaveKind'];
export type LeaveUnit = Schemas['LeaveUnit'];
export type LeavePolicyUnit = Schemas['LeavePolicyUnit'];
export type LeaveUsageItem = Schemas['LeaveUsageItem'];
export type LeaveStatus = Schemas['LeaveStatus'];
export type LeaveType = Schemas['LeaveType'];
export type LeavePolicyVersion = Schemas['LeavePolicyVersion'];
export type LeaveRequestSummary = Schemas['LeaveRequestSummary'];
export type LeaveRequestDetail = Schemas['LeaveRequestDetail'];
export type LeaveRequestPage = Schemas['LeaveRequestPage'];
export type LeavePeriodInput = Schemas['LeavePeriodInput'];
export type LeavePersonalOptions = Schemas['LeavePersonalOptions'];
export type LeaveRoutingOptions = Schemas['LeaveRoutingOptions'];
export type LeaveUsageReport = Schemas['LeaveUsageReport'];
export type SaveLeaveTypeRequest = Schemas['SaveLeaveTypeRequest'];
export type SaveLeavePolicyRequest = Schemas['SaveLeavePolicyRequest'];
export type CreateLeaveRequest = Schemas['CreateLeaveRequest'];
export type UpdateLeaveRequest = Schemas['UpdateLeaveRequest'];
export type LeaveMutationRequest = Schemas['LeaveMutationRequest'];
export type LeaveActionRequest = Schemas['LeaveActionRequest'];
export type CancelLeaveRequest = Schemas['CancelLeaveRequest'];
export type ProcessTemplateStep = Schemas['ProcessTemplateStep'];
export type ProcessAction = Schemas['ProcessAction'];
export type SignatureInput = Schemas['SignatureInput'];
export type LeaveListQuery = NonNullable<
	operations['listStaffLeaveRequests']['parameters']['query']
>;
export type LeaveUsageQuery = NonNullable<operations['getStaffLeaveUsage']['parameters']['query']>;

const base = '/api/staff-leave';

export async function listLeaveTypes(
	options: Omit<ApiRequestOptions, 'query'> & {
		query?: NonNullable<operations['listStaffLeaveTypes']['parameters']['query']>;
	} = {}
): Promise<LeaveType[]> {
	return requireApiData(
		await apiClient.get<LeaveType[]>(`${base}/types`, options),
		'โหลดประเภทคำขอไม่สำเร็จ'
	);
}

export async function saveLeaveType(input: SaveLeaveTypeRequest, id?: string): Promise<LeaveType> {
	const response = id
		? await apiClient.put<LeaveType>(`${base}/types/${id}`, input)
		: await apiClient.post<LeaveType>(`${base}/types`, input);
	return requireApiData(response, 'บันทึกประเภทคำขอไม่สำเร็จ');
}

export async function deleteLeaveType(id: string, input: LeaveMutationRequest): Promise<void> {
	requireApiData(
		await apiClient.deleteWithBody<Schemas['EmptyData']>(`${base}/types/${id}`, input),
		'ลบประเภทคำขอไม่สำเร็จ'
	);
}

export async function listLeavePolicies(
	id: string,
	options: ApiRequestOptions = {}
): Promise<LeavePolicyVersion[]> {
	return requireApiData(
		await apiClient.get<LeavePolicyVersion[]>(`${base}/types/${id}/policies`, options),
		'โหลดนโยบายไม่สำเร็จ'
	);
}

export async function saveLeavePolicy(
	id: string,
	input: SaveLeavePolicyRequest
): Promise<LeavePolicyVersion> {
	return requireApiData(
		await apiClient.post<LeavePolicyVersion>(`${base}/types/${id}/policies`, input),
		'บันทึกนโยบายไม่สำเร็จ'
	);
}

export async function getEffectiveLeavePolicy(
	id: string,
	startDate: string,
	options: ApiRequestOptions = {}
): Promise<LeavePolicyVersion> {
	return requireApiData(
		await apiClient.get<LeavePolicyVersion>(`${base}/types/${id}/effective-policy`, {
			...options,
			query: { startDate }
		}),
		'ยังไม่มีนโยบายที่ใช้กับประเภทบุคลากรและวันที่นี้'
	);
}

export async function getLeaveOptions(
	options: ApiRequestOptions = {}
): Promise<LeavePersonalOptions> {
	return requireApiData(
		await apiClient.get<LeavePersonalOptions>(`${base}/options`, options),
		'โหลดตัวเลือกคำขอไม่สำเร็จ'
	);
}

export async function getLeaveRoutingOptions(
	options: ApiRequestOptions = {}
): Promise<LeaveRoutingOptions> {
	return requireApiData(
		await apiClient.get<LeaveRoutingOptions>(`${base}/routing-options`, options),
		'โหลดผู้ดำเนินการไม่สำเร็จ'
	);
}

export async function listLeaveRequests(
	options: Omit<ApiRequestOptions, 'query'> & { query?: LeaveListQuery } = {}
): Promise<LeaveRequestPage> {
	return requireApiData(
		await apiClient.get<LeaveRequestPage>(`${base}/requests`, options),
		'โหลดคำขอไม่สำเร็จ'
	);
}

export async function getLeaveRequest(
	id: string,
	options: ApiRequestOptions = {}
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.get<LeaveRequestDetail>(`${base}/requests/${id}`, options),
		'โหลดรายละเอียดคำขอไม่สำเร็จ'
	);
}

export async function createLeaveRequest(input: CreateLeaveRequest): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.post<LeaveRequestDetail>(`${base}/requests`, input),
		'บันทึกฉบับร่างไม่สำเร็จ'
	);
}

export async function updateLeaveRequest(
	id: string,
	input: UpdateLeaveRequest
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.put<LeaveRequestDetail>(`${base}/requests/${id}`, input),
		'บันทึกคำขอไม่สำเร็จ'
	);
}

export async function submitLeaveRequest(
	id: string,
	input: SubmitLeaveRequest
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.post<LeaveRequestDetail>(`${base}/requests/${id}/submit`, input),
		'ส่งคำขอไม่สำเร็จ'
	);
}

export async function actOnLeaveRequest(
	id: string,
	input: LeaveActionRequest
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.post<LeaveRequestDetail>(`${base}/requests/${id}/actions`, input),
		'ดำเนินการไม่สำเร็จ'
	);
}

export async function cancelLeaveRequest(
	id: string,
	input: CancelLeaveRequest
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.post<LeaveRequestDetail>(`${base}/requests/${id}/cancel`, input),
		'ยกเลิกคำขอไม่สำเร็จ'
	);
}

export async function getLeaveUsage(
	options: Omit<ApiRequestOptions, 'query'> & { query?: LeaveUsageQuery } = {}
): Promise<LeaveUsageReport> {
	return requireApiData(
		await apiClient.get<LeaveUsageReport>(`${base}/usage`, options),
		'โหลดสรุปการใช้สิทธิ์ไม่สำเร็จ'
	);
}

export type LeaveCoverageOptions = Schemas['LeaveCoverageOptions'];
export type PreviewLeaveCoverageRequest = Schemas['PreviewLeaveCoverageRequest'];
export type LeaveCoveragePlan = Schemas['LeaveCoveragePlan'];
export type LeaveCoverageLesson = Schemas['LeaveCoverageLesson'];
export type LeaveCoverageAssignment = Schemas['LeaveCoverageAssignment'];
export type LeaveCoverageSelection = Schemas['LeaveCoverageSelection'];
export type LeaveCoverageCandidate = Schemas['LeaveCoverageCandidate'];
export type LeaveCoverageCandidates = Schemas['LeaveCoverageCandidates'];
export type LeaveCoverageCandidatesRequest = Schemas['LeaveCoverageCandidatesRequest'];
export type LeaveCoverageReturnLessonsRequest = Schemas['LeaveCoverageReturnLessonsRequest'];
export type LeaveCoverageReturnLessons = Schemas['LeaveCoverageReturnLessons'];
export type SaveLeaveCoverageRequest = Schemas['SaveLeaveCoverageRequest'];
export type ConfirmLeaveCoverageRequest = Schemas['ConfirmLeaveCoverageRequest'];
export type LeaveCoverageTask = Schemas['LeaveCoverageTask'];
export type LeaveCoverageTaskPage = Schemas['LeaveCoverageTaskPage'];
export type SubmitLeaveRequest = Schemas['SubmitLeaveRequest'];
export type DatedTeachingLesson = Schemas['DatedTeachingLesson'];
export type LeaveCoverageTaskQuery = NonNullable<
	operations['listAssignedStaffLeaveCoverage']['parameters']['query']
>;
export async function getLeaveCoverageOptions(
	options: ApiRequestOptions = {}
): Promise<LeaveCoverageOptions> {
	return requireApiData(
		await apiClient.get<LeaveCoverageOptions>(`${base}/coverage/options`, options),
		'โหลดบริบทปีและภาคเรียนไม่สำเร็จ'
	);
}
export async function previewLeaveCoverage(
	input: PreviewLeaveCoverageRequest,
	options: ApiRequestOptions = {}
): Promise<LeaveCoveragePlan> {
	return requireApiData(
		await apiClient.post<LeaveCoveragePlan>(`${base}/coverage/preview`, input, options),
		'ตรวจคาบสอนที่ได้รับผลกระทบไม่สำเร็จ'
	);
}
export async function getLeaveCoverageCandidates(
	id: string,
	input: LeaveCoverageCandidatesRequest,
	options: ApiRequestOptions = {}
): Promise<LeaveCoverageCandidates> {
	return requireApiData(
		await apiClient.post<LeaveCoverageCandidates>(
			`${base}/requests/${id}/coverage/candidates`,
			input,
			options
		),
		'ค้นหาครูที่ว่างไม่สำเร็จ'
	);
}
export async function getLeaveCoverageReturnLessons(
	id: string,
	input: LeaveCoverageReturnLessonsRequest,
	options: ApiRequestOptions = {}
): Promise<LeaveCoverageReturnLessons> {
	return requireApiData(
		await apiClient.post<LeaveCoverageReturnLessons>(
			`${base}/requests/${id}/coverage/return-lessons`,
			input,
			options
		),
		'ค้นหาคาบสอนคืนไม่สำเร็จ'
	);
}
export async function saveLeaveCoverage(
	id: string,
	input: SaveLeaveCoverageRequest
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.put<LeaveRequestDetail>(`${base}/requests/${id}/coverage`, input),
		'บันทึกแผนคาบสอนไม่สำเร็จ'
	);
}
export async function refreshLeaveCoverage(
	id: string,
	input: LeaveMutationRequest
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.post<LeaveRequestDetail>(`${base}/requests/${id}/coverage/refresh`, input),
		'ปรับคาบตามตารางล่าสุดไม่สำเร็จ'
	);
}
export async function replanLeaveCoverage(
	id: string,
	input: LeaveMutationRequest
): Promise<LeaveRequestDetail> {
	return requireApiData(
		await apiClient.post<LeaveRequestDetail>(`${base}/requests/${id}/coverage/replan`, input),
		'นำคำขอกลับมาจัดแผนไม่สำเร็จ'
	);
}
export async function listAssignedLeaveCoverage(
	options: Omit<ApiRequestOptions, 'query'> & { query?: LeaveCoverageTaskQuery } = {}
): Promise<LeaveCoverageTaskPage> {
	return requireApiData(
		await apiClient.get<LeaveCoverageTaskPage>(`${base}/coverage/assigned`, options),
		'โหลดคาบสอนที่มอบหมายไม่สำเร็จ'
	);
}
export async function confirmLeaveCoverage(
	id: string,
	input: ConfirmLeaveCoverageRequest
): Promise<LeaveCoverageTask> {
	return requireApiData(
		await apiClient.post<LeaveCoverageTask>(`${base}/requests/${id}/coverage/confirm`, input),
		'ตอบรับคาบสอนไม่สำเร็จ'
	);
}
