import {
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];

export type ExamRoundKind = NonNullable<Schemas['CreateExamRoundRequest']['examKind']>;
export type ExamRound = Schemas['ExamRound'];
export type ExamDay = Schemas['ExamDay'];
export type BlockedWindow = Schemas['BlockedWindow'];
export type ExamDayDetail = Schemas['ExamDayDetail'];
export type ExamDayRoomAssignmentView = Schemas['ExamDayRoomAssignmentView'];
export type ExamInvigilatorView = Schemas['ExamInvigilatorView'];
export type ExamScheduleItem = Schemas['ExamScheduleItemView'];
export type ExamSession = Schemas['ExamSessionView'];
export type ExamScheduleWorkspace = Schemas['ExamScheduleWorkspace'];
export type ExamScheduleReadiness = Schemas['ExamScheduleReadiness'];
export type ExamScheduleReadinessCode = Schemas['ExamScheduleReadinessCode'];
export type ExamScheduleReadinessFinding = Schemas['ExamScheduleReadinessFinding'];
export type DayRoomAssignmentView = Schemas['DayRoomAssignmentView'];
export type InvigilatorView = Schemas['InvigilatorView'];
export type ExamInvigilatorAssignmentSummary = Schemas['ExamInvigilatorAssignmentSummary'];
export type ExamInvigilatorDayWorkload = Schemas['ExamInvigilatorDayWorkload'];
export type ExamInvigilatorStaffWorkload = Schemas['ExamInvigilatorStaffWorkload'];
export type ExamInvigilatorWorkspace = Schemas['ExamInvigilatorWorkspace'];
export type ExamInvigilatorStaffOption = Schemas['ExamInvigilatorStaffOption'];
export type SeatAssignmentView = Schemas['SeatAssignmentView'];
export type PersonalExamScheduleRound = Schemas['PersonalExamScheduleRound'];
export type PersonalExamSessionView = Schemas['PersonalExamSessionView'];
export type StaffExamScheduleRound = Schemas['StaffExamScheduleRound'];
export type StaffExamDay = Schemas['StaffExamDay'];
export type StaffExamSession = Schemas['StaffExamSession'];
export type StaffExamRoomAssignment = Schemas['StaffExamRoomAssignment'];
export type StaffExamInvigilator = Schemas['StaffExamInvigilator'];

export type CreateExamRoundInput = Schemas['CreateExamRoundRequest'];
export type UpdateExamRoundInput = Schemas['UpdateExamRoundRequest'];
export type BlockedWindowInput = Schemas['BlockedWindowInput'];
export type UpsertExamDayInput = Schemas['UpsertExamDayRequest'];
export type UpsertDayRoomAssignmentInput = Schemas['UpsertDayRoomAssignmentRequest'];
export type GenerateSeatsInput = Schemas['GenerateSeatsRequest'];
export type UpdateExamInvigilatorsInput = Schemas['UpdateExamInvigilatorsRequest'];
export type PlaceExamSessionInput = Schemas['PlaceExamSessionRequest'];
export type ExamInvigilatorStaffOptionsFilter = NonNullable<
	operations['listExamInvigilatorStaffOptions']['parameters']['query']
>;

function apiData<T>(response: ApiResponse<T>, fallbackError: string): T {
	return requireApiData(response, fallbackError);
}

function requiredTerm(academicTermId: string): string {
	const selected = academicTermId.trim();
	if (!selected) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return selected;
}

function examScheduleQuery(academicTermId: string): string {
	const params = new URLSearchParams();
	params.set('academicTermId', academicTermId);
	return `?${params.toString()}`;
}

function examInvigilatorStaffOptionsQuery(filters: ExamInvigilatorStaffOptionsFilter = {}): string {
	const params = new URLSearchParams();
	if (filters.search) params.set('search', filters.search);
	if (filters.limit !== undefined) params.set('limit', String(filters.limit));
	const query = params.toString();
	return query ? `?${query}` : '';
}

export async function listExamRounds(
	academicTermId: string,
	options: ApiRequestOptions = {}
): Promise<ExamRound[]> {
	return apiData(
		await apiClient.get<ExamRound[]>(
			`/api/academic/exam-schedules${examScheduleQuery(requiredTerm(academicTermId))}`,
			options
		),
		'ไม่สามารถโหลดรอบตารางสอบได้'
	);
}

export async function createExamRound(input: CreateExamRoundInput): Promise<ExamRound> {
	return apiData(
		await apiClient.post<ExamRound>('/api/academic/exam-schedules', input),
		'ไม่สามารถสร้างรอบตารางสอบได้'
	);
}

export async function updateExamRound(
	roundId: string,
	input: UpdateExamRoundInput
): Promise<ExamRound> {
	return apiData(
		await apiClient.patch<ExamRound>(
			`/api/academic/exam-schedules/${encodeURIComponent(roundId)}`,
			input
		),
		'ไม่สามารถบันทึกรอบตารางสอบได้'
	);
}

export async function deleteExamRound(roundId: string): Promise<Schemas['EmptyData']> {
	return apiData(
		await apiClient.delete<Schemas['EmptyData']>(
			`/api/academic/exam-schedules/${encodeURIComponent(roundId)}`
		),
		'ไม่สามารถลบรอบตารางสอบได้'
	);
}

export async function getExamScheduleWorkspace(
	roundId: string,
	options: ApiRequestOptions = {}
): Promise<ExamScheduleWorkspace> {
	return apiData(
		await apiClient.get<ExamScheduleWorkspace>(
			`/api/academic/exam-schedules/${encodeURIComponent(roundId)}`,
			options
		),
		'ไม่สามารถโหลดพื้นที่จัดตารางสอบได้'
	);
}

export async function getExamInvigilatorWorkspace(
	roundId: string
): Promise<ExamInvigilatorWorkspace> {
	return apiData(
		await apiClient.get<ExamInvigilatorWorkspace>(
			`/api/academic/exam-schedules/${encodeURIComponent(roundId)}/invigilators`
		),
		'ไม่สามารถโหลดข้อมูลกรรมการคุมสอบได้'
	);
}

export async function listExamInvigilatorStaffOptions(
	roundId: string,
	filters: ExamInvigilatorStaffOptionsFilter = {}
): Promise<ExamInvigilatorStaffOption[]> {
	return apiData(
		await apiClient.get<ExamInvigilatorStaffOption[]>(
			`/api/academic/exam-schedules/${encodeURIComponent(roundId)}/invigilator-staff-options${examInvigilatorStaffOptionsQuery(filters)}`
		),
		'ไม่สามารถโหลดรายชื่อครูสำหรับจัดกรรมการได้'
	);
}

export async function upsertExamDay(
	roundId: string,
	input: UpsertExamDayInput
): Promise<ExamDayDetail> {
	return apiData(
		await apiClient.post<ExamDayDetail>(
			`/api/academic/exam-schedules/${encodeURIComponent(roundId)}/days`,
			input
		),
		'ไม่สามารถบันทึกวันสอบได้'
	);
}

export async function updateExamDay(
	examDayId: string,
	input: UpsertExamDayInput
): Promise<ExamDayDetail> {
	return apiData(
		await apiClient.patch<ExamDayDetail>(
			`/api/academic/exam-schedules/days/${encodeURIComponent(examDayId)}`,
			input
		),
		'ไม่สามารถแก้ไขวันสอบได้'
	);
}

export async function deleteExamDay(examDayId: string): Promise<Schemas['EmptyData']> {
	return apiData(
		await apiClient.delete<Schemas['EmptyData']>(
			`/api/academic/exam-schedules/days/${encodeURIComponent(examDayId)}`
		),
		'ไม่สามารถลบวันสอบได้'
	);
}

export async function listDayRoomAssignments(examDayId: string): Promise<DayRoomAssignmentView[]> {
	return apiData(
		await apiClient.get<DayRoomAssignmentView[]>(
			`/api/academic/exam-schedules/days/${encodeURIComponent(examDayId)}/room-assignments`
		),
		'ไม่สามารถโหลดห้องสอบประจำวันได้'
	);
}

export async function upsertDayRoomAssignment(
	examDayId: string,
	input: UpsertDayRoomAssignmentInput
): Promise<DayRoomAssignmentView> {
	return apiData(
		await apiClient.post<DayRoomAssignmentView>(
			`/api/academic/exam-schedules/days/${encodeURIComponent(examDayId)}/room-assignments`,
			input
		),
		'ไม่สามารถบันทึกห้องสอบประจำวันได้'
	);
}

export async function updateExamAssignmentInvigilators(
	assignmentId: string,
	input: UpdateExamInvigilatorsInput
): Promise<DayRoomAssignmentView> {
	return apiData(
		await apiClient.put<DayRoomAssignmentView>(
			`/api/academic/exam-schedules/room-assignments/${encodeURIComponent(assignmentId)}/invigilators`,
			input
		),
		'ไม่สามารถบันทึกกรรมการคุมสอบได้'
	);
}

export async function assignExamAssignmentInvigilator(
	assignmentId: string,
	staffId: string
): Promise<ExamInvigilatorWorkspace> {
	return apiData(
		await apiClient.put<ExamInvigilatorWorkspace>(
			`/api/academic/exam-schedules/room-assignments/${encodeURIComponent(assignmentId)}/invigilators/${encodeURIComponent(staffId)}`
		),
		'ไม่สามารถบันทึกกรรมการคุมสอบได้'
	);
}

export async function removeExamAssignmentInvigilator(
	assignmentId: string,
	staffId: string
): Promise<ExamInvigilatorWorkspace> {
	return apiData(
		await apiClient.delete<ExamInvigilatorWorkspace>(
			`/api/academic/exam-schedules/room-assignments/${encodeURIComponent(assignmentId)}/invigilators/${encodeURIComponent(staffId)}`
		),
		'ไม่สามารถลบกรรมการคุมสอบได้'
	);
}

export async function generateSeatsForAssignment(
	assignmentId: string,
	input: GenerateSeatsInput
): Promise<SeatAssignmentView[]> {
	return apiData(
		await apiClient.post<SeatAssignmentView[]>(
			`/api/academic/exam-schedules/room-assignments/${encodeURIComponent(assignmentId)}/seats`,
			input
		),
		'ไม่สามารถสร้างเลขที่นั่งสอบได้'
	);
}

export async function placeExamSession(input: PlaceExamSessionInput): Promise<ExamSession> {
	return apiData(
		await apiClient.post<ExamSession>('/api/academic/exam-schedules/sessions', input),
		'ไม่สามารถจัดวางคาบสอบได้'
	);
}

export async function deleteExamSession(sessionId: string): Promise<Schemas['EmptyData']> {
	return apiData(
		await apiClient.delete<Schemas['EmptyData']>(
			`/api/academic/exam-schedules/sessions/${encodeURIComponent(sessionId)}`
		),
		'ไม่สามารถลบคาบสอบได้'
	);
}

export async function listMyExamSchedules(
	academicTermId: string,
	options: ApiRequestOptions = {}
): Promise<PersonalExamScheduleRound[]> {
	return apiData(
		await apiClient.get<PersonalExamScheduleRound[]>(
			`/api/me/exam-schedules${examScheduleQuery(requiredTerm(academicTermId))}`,
			options
		),
		'ไม่สามารถโหลดตารางสอบได้'
	);
}

export async function listStaffExamSchedules(
	academicTermId: string,
	options: ApiRequestOptions = {}
): Promise<StaffExamScheduleRound[]> {
	return apiData(
		await apiClient.get<StaffExamScheduleRound[]>(
			`/api/staff/exam-schedules${examScheduleQuery(requiredTerm(academicTermId))}`,
			options
		),
		'ไม่สามารถโหลดตารางสอบสำหรับครูได้'
	);
}

export async function listChildExamSchedules(
	studentId: string,
	academicTermId: string,
	options: ApiRequestOptions = {}
): Promise<PersonalExamScheduleRound[]> {
	return apiData(
		await apiClient.get<PersonalExamScheduleRound[]>(
			`/api/parent/students/${encodeURIComponent(studentId)}/exam-schedules${examScheduleQuery(requiredTerm(academicTermId))}`,
			options
		),
		'ไม่สามารถโหลดตารางสอบของนักเรียนได้'
	);
}
