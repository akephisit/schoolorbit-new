import { apiClient, requireApiData } from '$lib/api/client';

export type AssessmentExamMode = 'none' | 'in_timetable' | 'outside_timetable' | 'practical';
export type AssessmentPlanStatus = 'not_configured' | 'draft' | 'submitted' | 'locked';
export type AssessmentAllocationStatus =
	| 'not_started'
	| 'complete'
	| 'under_allocated'
	| 'over_allocated';

export interface AssessmentPlanFilters {
	academicSemesterId?: string;
	classroomId?: string;
	subjectId?: string;
	instructorId?: string;
	status?: AssessmentPlanStatus;
}

export interface AssessmentPlanSummary {
	planId?: string;
	classroomCourseId: string;
	classroomId: string;
	subjectId: string;
	academicSemesterId: string;
	primaryInstructorId?: string;
	status: AssessmentPlanStatus;
	subjectCode?: string;
	subjectNameTh?: string;
	subjectNameEn?: string;
	classroomName?: string;
	classroomCount: number;
	instructorName?: string;
	categoryCount: number;
	itemCount: number;
	totalScore: number;
	outsideTimetableCount: number;
	inTimetableCount: number;
	midtermExamDurationMinutes?: number | null;
	finalExamDurationMinutes?: number | null;
	hasUnallocatedCategories: boolean;
}

export interface AssessmentPlanDetail {
	id?: string;
	classroomCourseId: string;
	subjectId: string;
	academicSemesterId: string;
	status: AssessmentPlanStatus;
	submittedAt?: string;
	lockedAt?: string;
	categories: AssessmentCategory[];
}

export interface AssessmentCategory {
	id?: string;
	code?: string;
	name: string;
	maxScore: number;
	examMode: AssessmentExamMode;
	examDurationMinutes?: number | null;
	displayOrder: number;
	itemTotalScore: number;
	allocationStatus: AssessmentAllocationStatus;
	items: AssessmentItem[];
}

export interface AssessmentItem {
	id: string;
	categoryId: string;
	name: string;
	maxScore: number;
	displayOrder: number;
	isActive: boolean;
}

export interface SaveAssessmentPlanRequest {
	categories: SaveAssessmentCategoryRequest[];
}

export interface SaveAssessmentCategoryRequest {
	id?: string;
	code?: string;
	name: string;
	maxScore: number;
	examMode: AssessmentExamMode;
	examDurationMinutes?: number | null;
	displayOrder: number;
	items: SaveAssessmentItemRequest[];
}

export interface SaveAssessmentItemRequest {
	id?: string;
	name: string;
	maxScore: number;
	displayOrder: number;
	isActive: boolean;
}

export interface AssessmentSettings {
	teacherAccessEnabled: boolean;
}

export interface UpdateAssessmentSettingsRequest {
	teacherAccessEnabled: boolean;
}

function assessmentPlanQuery(filters: AssessmentPlanFilters = {}): string {
	const params = new URLSearchParams();
	if (filters.academicSemesterId) params.set('academic_semester_id', filters.academicSemesterId);
	if (filters.classroomId) params.set('classroom_id', filters.classroomId);
	if (filters.subjectId) params.set('subject_id', filters.subjectId);
	if (filters.instructorId) params.set('instructor_id', filters.instructorId);
	if (filters.status) params.set('status', filters.status);
	const query = params.toString();
	return query ? `?${query}` : '';
}

export async function listAssessmentPlans(
	filters: AssessmentPlanFilters = {}
): Promise<{ data: AssessmentPlanSummary[] }> {
	const response = await apiClient.get<AssessmentPlanSummary[]>(
		`/api/academic/assessments/plans${assessmentPlanQuery(filters)}`
	);
	return {
		data: requireApiData(response, 'ไม่สามารถโหลดภาพรวมโครงสร้างคะแนนได้')
	};
}

export async function getAssessmentPlan(courseId: string): Promise<{ data: AssessmentPlanDetail }> {
	const response = await apiClient.get<AssessmentPlanDetail>(
		`/api/academic/assessments/courses/${courseId}`
	);
	return {
		data: requireApiData(response, 'ไม่สามารถโหลดโครงสร้างคะแนนรายวิชาได้')
	};
}

export async function getAssessmentSettings(): Promise<{ data: AssessmentSettings }> {
	const response = await apiClient.get<AssessmentSettings>('/api/academic/assessments/settings');
	return {
		data: requireApiData(response, 'ไม่สามารถโหลดการตั้งค่าโครงสร้างคะแนนได้')
	};
}

export async function updateAssessmentSettings(
	payload: UpdateAssessmentSettingsRequest
): Promise<{ data: AssessmentSettings }> {
	const response = await apiClient.put<AssessmentSettings>(
		'/api/academic/assessments/settings',
		payload
	);
	return {
		data: requireApiData(response, 'ไม่สามารถบันทึกการตั้งค่าโครงสร้างคะแนนได้')
	};
}

export async function saveAssessmentPlan(
	courseId: string,
	payload: SaveAssessmentPlanRequest
): Promise<{ data: AssessmentPlanDetail }> {
	const response = await apiClient.put<AssessmentPlanDetail>(
		`/api/academic/assessments/courses/${courseId}`,
		payload
	);
	return {
		data: requireApiData(response, 'ไม่สามารถบันทึกโครงสร้างคะแนนได้')
	};
}

export async function submitAssessmentPlan(
	courseId: string
): Promise<{ data: AssessmentPlanDetail }> {
	const response = await apiClient.post<AssessmentPlanDetail>(
		`/api/academic/assessments/courses/${courseId}/submit`
	);
	return {
		data: requireApiData(response, 'ไม่สามารถส่งโครงสร้างคะแนนได้')
	};
}
