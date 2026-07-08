import { apiClient, requireApiData, type ApiResponse } from '$lib/api/client';

type EmptyResponseData = Record<string, never>;

export type ExamRoundStatus = 'draft' | 'published';
export type ExamRoundKind = 'midterm' | 'final';

export interface ExamRound {
	id: string;
	academicSemesterId: string;
	name: string;
	description?: string | null;
	examKind: ExamRoundKind;
	status: ExamRoundStatus;
	publishedAt?: string | null;
	createdAt: string;
	updatedAt: string;
}

export interface ExamDay {
	id: string;
	examRoundId: string;
	examDate: string;
	label?: string | null;
	startTime: string;
	endTime: string;
}

export interface BlockedWindow {
	id?: string | null;
	label: string;
	startTime: string;
	endTime: string;
}

export interface ExamDayDetail extends ExamDay {
	gradeLevelIds: string[];
	blockedWindows: BlockedWindow[];
	roomAssignments: ExamDayRoomAssignmentView[];
}

export interface ExamDayRoomAssignmentView {
	id: string;
	examDayId: string;
	classroomId: string;
	roomId: string;
	capacityOverride?: number | null;
	classroomName?: string | null;
	roomName?: string | null;
	roomCapacity?: number | null;
	invigilators: ExamInvigilatorView[];
}

export interface ExamInvigilatorView {
	id: string;
	examDayId: string;
	dayRoomAssignmentId: string;
	staffId: string;
	staffName?: string | null;
	roleLabel?: string | null;
}

export interface ExamScheduleItem {
	id: string;
	examRoundId: string;
	academicSemesterId: string;
	assessmentCategoryId: string;
	assessmentPlanId: string;
	classroomCourseId: string;
	classroomId: string;
	subjectId: string;
	gradeLevelId: string;
	durationMinutes: number;
	importedAt: string;
	assessmentCategoryName?: string | null;
	subjectCode?: string | null;
	subjectNameTh?: string | null;
	subjectNameEn?: string | null;
	subjectGroupId?: string | null;
	subjectGroupName?: string | null;
	subjectGroupDisplayOrder?: number | null;
	subjectType?: string | null;
	classroomName?: string | null;
	gradeLevelName?: string | null;
	gradeLevelType?: string | null;
	gradeLevelYear?: number | null;
}

export interface ExamSession {
	id: string;
	examScheduleItemId: string;
	examRoundId: string;
	examDayId: string;
	startsAt: string;
	endsAt: string;
	academicSemesterId: string;
	assessmentCategoryId: string;
	assessmentPlanId: string;
	classroomCourseId: string;
	classroomId: string;
	subjectId: string;
	gradeLevelId: string;
	durationMinutes: number;
	importedAt: string;
	examDate?: string | null;
	assessmentCategoryName?: string | null;
	subjectCode?: string | null;
	subjectNameTh?: string | null;
	subjectNameEn?: string | null;
	subjectGroupId?: string | null;
	subjectGroupName?: string | null;
	subjectGroupDisplayOrder?: number | null;
	subjectType?: string | null;
	classroomName?: string | null;
	gradeLevelName?: string | null;
	gradeLevelType?: string | null;
	gradeLevelYear?: number | null;
	roomId?: string | null;
	roomName?: string | null;
	buildingName?: string | null;
	invigilators: ExamInvigilatorView[];
}

export interface ExamScheduleWorkspace {
	round: ExamRound;
	days: ExamDayDetail[];
	unscheduledItems: ExamScheduleItem[];
	scheduledSessions: ExamSession[];
	readiness: ExamScheduleReadiness;
}

export interface ExamScheduleReadiness {
	canPublish: boolean;
	blockers: string[];
}

export interface ImportExamItemsResult {
	insertedCount: number;
	skippedExistingCount: number;
	skippedMissingDurationCount: number;
}

export interface ClearMismatchedExamItemsResult {
	deletedCount: number;
}

export interface DayRoomAssignmentView {
	id: string;
	examDayId: string;
	classroomId: string;
	classroomName: string;
	roomId: string;
	roomName: string;
	buildingName?: string | null;
	roomCapacity?: number | null;
	capacityOverride?: number | null;
	invigilators: InvigilatorView[];
	seatsGenerated: boolean;
}

export interface InvigilatorView {
	staffId: string;
	displayName: string;
}

export interface ExamInvigilatorAssignmentSummary {
	assignmentId: string;
	examDayId: string;
	classroomId: string;
	classroomName: string;
	roomId: string;
	roomName: string;
	sessionMinutes: number;
	invigilators: InvigilatorView[];
}

export interface ExamInvigilatorDayWorkload {
	examDayId: string;
	minutes: number;
	assignmentCount: number;
}

export interface ExamInvigilatorStaffWorkload {
	staffId: string;
	staffName: string;
	totalMinutes: number;
	assignedDayCount: number;
	assignmentCount: number;
	days: ExamInvigilatorDayWorkload[];
}

export interface ExamInvigilatorWorkspace {
	roundId: string;
	assignments: ExamInvigilatorAssignmentSummary[];
	staffWorkloads: ExamInvigilatorStaffWorkload[];
}

export interface ExamInvigilatorStaffOption {
	staffId: string;
	displayName: string;
}

export interface SeatAssignmentView {
	id: string;
	dayRoomAssignmentId: string;
	studentId: string;
	studentName: string;
	seatNumber: string;
}

export interface PersonalExamScheduleRound {
	roundId: string;
	roundName: string;
	academicSemesterId: string;
	publishedAt?: string | null;
	sessions: PersonalExamSessionView[];
}

export interface PersonalExamSessionView {
	examDate: string;
	startsAt: string;
	endsAt: string;
	subjectName: string;
	assessmentCategoryName: string;
	classroomName: string;
	roomName: string;
	buildingName?: string | null;
	seatNumber?: string | null;
}

export interface ExamScheduleFilters {
	academicSemesterId?: string;
}

export interface CreateExamRoundInput {
	academicSemesterId: string;
	name: string;
	description?: string | null;
	examKind: ExamRoundKind;
}

export interface UpdateExamRoundInput {
	name?: string;
	description?: string | null;
	examKind?: ExamRoundKind;
}

export interface BlockedWindowInput {
	label: string;
	startTime: string;
	endTime: string;
}

export interface UpsertExamDayInput {
	examDate: string;
	label?: string | null;
	startTime: string;
	endTime: string;
	gradeLevelIds: string[];
	blockedWindows: BlockedWindowInput[];
}

export interface ImportExamItemsInput {
	gradeLevelIds?: string[];
}

export interface UpsertDayRoomAssignmentInput {
	classroomId: string;
	roomId: string;
	capacityOverride?: number | null;
}

export interface GenerateSeatsInput {
	regenerate: boolean;
}

export interface UpdateExamInvigilatorsInput {
	invigilatorStaffIds: string[];
}

export interface PlaceExamSessionInput {
	examScheduleItemId: string;
	examDayId: string;
	startsAt: string;
}

export interface ExamInvigilatorStaffOptionsFilter {
	search?: string;
	limit?: number;
}

function examScheduleQuery(filters: ExamScheduleFilters = {}): string {
	const params = new URLSearchParams();
	if (filters.academicSemesterId) params.set('academic_semester_id', filters.academicSemesterId);
	const query = params.toString();
	return query ? `?${query}` : '';
}

function examInvigilatorStaffOptionsQuery(filters: ExamInvigilatorStaffOptionsFilter = {}): string {
	const params = new URLSearchParams();
	if (filters.search) params.set('search', filters.search);
	if (filters.limit) params.set('limit', String(filters.limit));
	const query = params.toString();
	return query ? `?${query}` : '';
}

function apiData<T>(response: ApiResponse<T>, fallbackError: string): T {
	return requireApiData(response, fallbackError);
}

export async function listExamRounds(filters: ExamScheduleFilters = {}): Promise<ExamRound[]> {
	const response = await apiClient.get<ExamRound[]>(
		`/api/academic/exam-schedules${examScheduleQuery(filters)}`
	);
	return apiData(response, 'ไม่สามารถโหลดรอบตารางสอบได้');
}

export async function createExamRound(input: CreateExamRoundInput): Promise<ExamRound> {
	const response = await apiClient.post<ExamRound>('/api/academic/exam-schedules', input);
	return apiData(response, 'ไม่สามารถสร้างรอบตารางสอบได้');
}

export async function updateExamRound(
	roundId: string,
	input: UpdateExamRoundInput
): Promise<ExamRound> {
	const response = await apiClient.patch<ExamRound>(
		`/api/academic/exam-schedules/${roundId}`,
		input
	);
	return apiData(response, 'ไม่สามารถบันทึกรอบตารางสอบได้');
}

export async function getExamScheduleWorkspace(roundId: string): Promise<ExamScheduleWorkspace> {
	const response = await apiClient.get<ExamScheduleWorkspace>(
		`/api/academic/exam-schedules/${roundId}`
	);
	return apiData(response, 'ไม่สามารถโหลดพื้นที่จัดตารางสอบได้');
}

export async function getExamInvigilatorWorkspace(
	roundId: string
): Promise<ExamInvigilatorWorkspace> {
	const response = await apiClient.get<ExamInvigilatorWorkspace>(
		`/api/academic/exam-schedules/${roundId}/invigilators`
	);
	return apiData(response, 'ไม่สามารถโหลดข้อมูลกรรมการคุมสอบได้');
}

export async function listExamInvigilatorStaffOptions(
	roundId: string,
	filters: ExamInvigilatorStaffOptionsFilter = {}
): Promise<ExamInvigilatorStaffOption[]> {
	const response = await apiClient.get<ExamInvigilatorStaffOption[]>(
		`/api/academic/exam-schedules/${roundId}/invigilator-staff-options${examInvigilatorStaffOptionsQuery(filters)}`
	);
	return apiData(response, 'ไม่สามารถโหลดรายชื่อครูสำหรับจัดกรรมการได้');
}

export async function importExamItems(
	roundId: string,
	input: ImportExamItemsInput
): Promise<ImportExamItemsResult> {
	const response = await apiClient.post<ImportExamItemsResult>(
		`/api/academic/exam-schedules/${roundId}/import-items`,
		input
	);
	return apiData(response, 'ไม่สามารถนำเข้ารายการสอบได้');
}

export async function clearMismatchedExamItems(
	roundId: string
): Promise<ClearMismatchedExamItemsResult> {
	const response = await apiClient.post<ClearMismatchedExamItemsResult>(
		`/api/academic/exam-schedules/${roundId}/clear-mismatched-items`
	);
	return apiData(response, 'ไม่สามารถล้างรายการสอบที่ไม่ตรงรอบสอบได้');
}

export async function upsertExamDay(
	roundId: string,
	input: UpsertExamDayInput
): Promise<ExamDayDetail> {
	const response = await apiClient.post<ExamDayDetail>(
		`/api/academic/exam-schedules/${roundId}/days`,
		input
	);
	return apiData(response, 'ไม่สามารถบันทึกวันสอบได้');
}

export async function deleteExamDay(examDayId: string): Promise<EmptyResponseData> {
	const response = await apiClient.delete<EmptyResponseData>(
		`/api/academic/exam-schedules/days/${examDayId}`
	);
	return apiData(response, 'ไม่สามารถลบวันสอบได้');
}

export async function listDayRoomAssignments(examDayId: string): Promise<DayRoomAssignmentView[]> {
	const response = await apiClient.get<DayRoomAssignmentView[]>(
		`/api/academic/exam-schedules/days/${examDayId}/room-assignments`
	);
	return apiData(response, 'ไม่สามารถโหลดห้องสอบประจำวันได้');
}

export async function upsertDayRoomAssignment(
	examDayId: string,
	input: UpsertDayRoomAssignmentInput
): Promise<DayRoomAssignmentView> {
	const response = await apiClient.post<DayRoomAssignmentView>(
		`/api/academic/exam-schedules/days/${examDayId}/room-assignments`,
		input
	);
	return apiData(response, 'ไม่สามารถบันทึกห้องสอบประจำวันได้');
}

export async function updateExamAssignmentInvigilators(
	assignmentId: string,
	input: UpdateExamInvigilatorsInput
): Promise<DayRoomAssignmentView> {
	const response = await apiClient.put<DayRoomAssignmentView>(
		`/api/academic/exam-schedules/room-assignments/${assignmentId}/invigilators`,
		input
	);
	return apiData(response, 'ไม่สามารถบันทึกกรรมการคุมสอบได้');
}

export async function assignExamAssignmentInvigilator(
	assignmentId: string,
	staffId: string
): Promise<ExamInvigilatorWorkspace> {
	const response = await apiClient.put<ExamInvigilatorWorkspace>(
		`/api/academic/exam-schedules/room-assignments/${assignmentId}/invigilators/${staffId}`
	);
	return apiData(response, 'ไม่สามารถบันทึกกรรมการคุมสอบได้');
}

export async function removeExamAssignmentInvigilator(
	assignmentId: string,
	staffId: string
): Promise<ExamInvigilatorWorkspace> {
	const response = await apiClient.delete<ExamInvigilatorWorkspace>(
		`/api/academic/exam-schedules/room-assignments/${assignmentId}/invigilators/${staffId}`
	);
	return apiData(response, 'ไม่สามารถลบกรรมการคุมสอบได้');
}

export async function generateSeatsForAssignment(
	assignmentId: string,
	input: GenerateSeatsInput
): Promise<SeatAssignmentView[]> {
	const response = await apiClient.post<SeatAssignmentView[]>(
		`/api/academic/exam-schedules/room-assignments/${assignmentId}/seats`,
		input
	);
	return apiData(response, 'ไม่สามารถสร้างเลขที่นั่งสอบได้');
}

export async function placeExamSession(input: PlaceExamSessionInput): Promise<ExamSession> {
	const response = await apiClient.post<ExamSession>(
		'/api/academic/exam-schedules/sessions',
		input
	);
	return apiData(response, 'ไม่สามารถจัดวางคาบสอบได้');
}

export async function deleteExamSession(sessionId: string): Promise<EmptyResponseData> {
	const response = await apiClient.delete<EmptyResponseData>(
		`/api/academic/exam-schedules/sessions/${sessionId}`
	);
	return apiData(response, 'ไม่สามารถลบคาบสอบได้');
}

export async function publishExamRound(roundId: string): Promise<ExamRound> {
	const response = await apiClient.post<ExamRound>(
		`/api/academic/exam-schedules/${roundId}/publish`
	);
	return apiData(response, 'ไม่สามารถเผยแพร่ตารางสอบได้');
}

export async function listMyExamSchedules(
	filters: ExamScheduleFilters = {}
): Promise<PersonalExamScheduleRound[]> {
	const response = await apiClient.get<PersonalExamScheduleRound[]>(
		`/api/me/exam-schedules${examScheduleQuery(filters)}`
	);
	return apiData(response, 'ไม่สามารถโหลดตารางสอบได้');
}

export async function listStaffExamSchedules(
	filters: ExamScheduleFilters = {}
): Promise<PersonalExamScheduleRound[]> {
	const response = await apiClient.get<PersonalExamScheduleRound[]>(
		`/api/staff/exam-schedules${examScheduleQuery(filters)}`
	);
	return apiData(response, 'ไม่สามารถโหลดตารางสอบสำหรับครูได้');
}

export async function listChildExamSchedules(
	studentId: string,
	filters: ExamScheduleFilters = {}
): Promise<PersonalExamScheduleRound[]> {
	const response = await apiClient.get<PersonalExamScheduleRound[]>(
		`/api/parent/students/${studentId}/exam-schedules${examScheduleQuery(filters)}`
	);
	return apiData(response, 'ไม่สามารถโหลดตารางสอบของนักเรียนได้');
}
