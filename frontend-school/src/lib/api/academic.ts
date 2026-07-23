import { apiClient, type ApiRequestOptions, type ApiResponse } from '$lib/api/client';
import type { components } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];
type LoadedApiResponse<T> = ApiResponse<T> & { success: true; data: T };
type EmptyResponseData = Schemas['EmptyData'];

// Types
export type AcademicYear = Schemas['AcademicYear'];
export type Semester = Schemas['Semester'];
export type GradeLevel = Schemas['GradeLevelResponse'];
export type AcademicStructureData = Schemas['AcademicStructure'];
export type ClassroomAdvisor = Schemas['ClassroomAdvisor'];
export type Classroom = Schemas['Classroom'];
export type StudentEnrollment = Schemas['StudentEnrollment'];
export type CreateAcademicYearRequest = Schemas['CreateAcademicYearRequest'];
export type UpdateAcademicYearRequest = Schemas['UpdateAcademicYearRequest'];
export type CreateSemesterRequest = Schemas['CreateSemesterRequest'];
export type UpdateSemesterRequest = Schemas['UpdateSemesterRequest'];
export type CreateGradeLevelRequest = Schemas['CreateGradeLevelRequest'];
export type CreateClassroomRequest = Schemas['CreateClassroomRequest'];
export type UpdateClassroomRequest = Schemas['UpdateClassroomRequest'];
export type EnrollStudentRequest = Schemas['EnrollStudentRequest'];
export type SubjectGroup = Schemas['SubjectGroup'];
export type Subject = Schemas['Subject'];
export type CurriculumInstructorRole = Schemas['CurriculumInstructorRole'];
export type SubjectDefaultInstructor = Schemas['SubjectDefaultInstructor'];
export type CreateSubjectRequest = Schemas['CreateSubjectRequest'];
export type UpdateSubjectRequest = Schemas['UpdateSubjectRequest'];
export type AddSubjectDefaultInstructorRequest = Schemas['AddSubjectDefaultInstructorRequest'];
export type UpdateSubjectDefaultInstructorRoleRequest =
	Schemas['UpdateSubjectDefaultInstructorRoleRequest'];
export type StudyPlan = Schemas['StudyPlan'];
export type CreateStudyPlanRequest = Schemas['CreateStudyPlanRequest'];
export type UpdateStudyPlanRequest = Schemas['UpdateStudyPlanRequest'];
export type StudyPlanVersion = Schemas['StudyPlanVersion'];
export type CreateStudyPlanVersionRequest = Schemas['CreateStudyPlanVersionRequest'];
export type UpdateStudyPlanVersionRequest = Schemas['UpdateStudyPlanVersionRequest'];
export type StudyPlanSubject = Schemas['StudyPlanSubject'];
export type AddSubjectsToVersionRequest = Schemas['AddSubjectsToVersionRequest'];
export type SubjectInPlan = Schemas['SubjectInPlan'];
export type GenerateCoursesFromPlanRequest = Schemas['GenerateCoursesFromPlanRequest'];
export type GenerateCoursesFromPlanResponse = Schemas['GenerateCoursesData'];
export type ActivitySlot = Schemas['ActivitySlot'];
export type ActivitySlotFilter = Schemas['ActivitySlotFilter'];
export type ActivitySlotTimetableContextResponse =
	Schemas['ActivitySlotTimetableContextResponse'];
export type UpdateActivitySlotRequest = Schemas['UpdateActivitySlotRequest'];
export type ActivityRegistrationType = Schemas['ActivityRegistrationType'];
export type AddSlotInstructorRequest = Schemas['AddSlotInstructorRequest'];
export type AddSlotInstructorsBatchRequest = Schemas['AddSlotInstructorsBatchRequest'];
export type ActivityGroup = Schemas['ActivityGroup'];
export type ActivityGroupFilter = Schemas['ActivityGroupFilter'];
export type CreateActivityGroupRequest = Schemas['CreateActivityGroupRequest'];
export type UpdateActivityGroupRequest = Schemas['UpdateActivityGroupRequest'];
export type ActivityGroupMember = Schemas['ActivityGroupMember'];
export type ActivityMemberResult = Schemas['ActivityMemberResult'];
export type AddMembersRequest = Schemas['AddMembersRequest'];
export type UpdateMemberResultRequest = Schemas['UpdateMemberResultRequest'];
export type ActivityInstructor = Schemas['InstructorInfo'];
export type ActivityGroupInstructorRole = Schemas['ActivityGroupInstructorRole'];
export type InstructorRoleRequest = Schemas['InstructorRoleRequest'];
export type SlotInstructor = Schemas['SlotInstructorInfo'];
export type SlotClassroomAssignment = Schemas['SlotClassroomAssignment'];
export type UpsertSlotClassroomAssignmentRequest = Schemas['UpsertSlotClassroomAssignmentRequest'];
export type BatchUpsertSlotClassroomAssignmentsRequest =
	Schemas['BatchUpsertSlotClassroomAssignmentsRequest'];
export type ActivityInsertedCountData = Schemas['ActivityInsertedCountData'];
export type ActivityAddedCountData = Schemas['ActivityAddedCountData'];
export type ActivityDeletedCountData = Schemas['ActivityDeletedCountData'];
export type ActivityProcessedCountData = Schemas['ActivityProcessedCountData'];
export type StudyPlanVersionActivity = Schemas['StudyPlanVersionActivity'];
export type CreatePlanActivityRequest = Schemas['CreatePlanActivityRequest'];
export type UpdatePlanActivityRequest = Schemas['UpdatePlanActivityRequest'];
export type GenerateActivitiesFromPlanRequest = Schemas['GenerateActivitiesFromPlanRequest'];
export type GenerateActivitiesFromPlanResponse = Schemas['GenerateActivitiesFromPlanOutcome'];
export type ActivityCatalog = Schemas['ActivityCatalog'];
export type ActivityCatalogType = Schemas['ActivityCatalogType'];
export type ActivitySchedulingMode = Schemas['ActivitySchedulingMode'];
export type CatalogDefaultInstructor = Schemas['CatalogDefaultInstructor'];
export type CatalogDefaultInstructorInput = Schemas['CatalogDefaultInstructorInput'];
export type CreateCatalogRequest = Schemas['CreateCatalogRequest'];
export type UpdateCatalogRequest = Schemas['UpdateCatalogRequest'];
export type AddCatalogDefaultInstructorRequest = Schemas['AddCatalogDefaultInstructorRequest'];
export type UpdateCatalogDefaultInstructorRoleRequest =
	Schemas['UpdateCatalogDefaultInstructorRoleRequest'];
export type ClassroomCourse = Schemas['ClassroomCourse'];
export type ClassroomCourseSettings = Schemas['ClassroomCourseSettings'];
export type CourseInstructor = Schemas['CourseInstructor'];
export type CourseInstructorRole = Schemas['CourseInstructorRole'];
export type AssignCoursesRequest = Schemas['AssignCoursesRequest'];
export type UpdateCourseRequest = Schemas['UpdateCourseRequest'];
export type AddCourseInstructorRequest = Schemas['AddCourseInstructorRequest'];
export type BatchListCourseInstructorsRequest = Schemas['BatchListCourseInstructorsRequest'];
export type UpdateCourseInstructorRoleRequest = Schemas['UpdateCourseInstructorRoleRequest'];
export type ClassroomActivity = Schemas['ClassroomActivity'];
export type CourseAssignedCountData = Schemas['CourseAssignedCountData'];

export interface ClassroomCourseFilters {
	classroomId?: string;
	instructorId?: string;
	semesterId?: string;
	subjectId?: string;
}

// Lookup Types
export interface LookupItem {
	id: string;
	code: string;
	name: string;
	description?: string;
	level_type?: string;
	year?: number;
	is_current?: boolean; // For academic years
	short_name?: string; // e.g. "อ.1", "ป.3", "ม.6" for grade levels
}

// API Functions

// Helper for authenticated requests
async function fetchApi<T = EmptyResponseData>(
	path: string,
	options: RequestInit = {}
): Promise<LoadedApiResponse<T>> {
	const method = (options.method || 'GET').toUpperCase();
	const body = options.body ? JSON.parse(options.body.toString()) : undefined;

	let response: ApiResponse<T>;
	if (method === 'POST') {
		response = await apiClient.post<T>(path, body);
	} else if (method === 'PUT') {
		response = await apiClient.put<T>(path, body);
	} else if (method === 'DELETE') {
		response = await apiClient.delete<T>(path);
	} else {
		response = await apiClient.get<T>(
			path,
			options.signal ? { signal: options.signal } : undefined
		);
	}

	if (!response.success) throw new Error(response.error || 'Request failed');
	if (response.data === undefined) throw new Error('Response data missing');
	return { success: true, data: response.data, message: response.message };
}

export const getAcademicStructure = async (): Promise<{ data: AcademicStructureData }> => {
	return await fetchApi<AcademicStructureData>('/api/academic/structure');
};

export const createAcademicYear = async (data: CreateAcademicYearRequest) => {
	return await fetchApi<AcademicYear>('/api/academic/years', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateAcademicYear = async (id: string, data: UpdateAcademicYearRequest) => {
	return await fetchApi<AcademicYear>(`/api/academic/years/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const toggleActiveYear = async (id: string) => {
	return await fetchApi(`/api/academic/years/${id}/active`, {
		method: 'PUT'
	});
};

export const createSemester = async (data: CreateSemesterRequest) => {
	return await fetchApi<Semester>('/api/academic/semesters', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateSemester = async (id: string, data: UpdateSemesterRequest) => {
	return await fetchApi<Semester>(`/api/academic/semesters/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteSemester = async (id: string) => {
	return await fetchApi(`/api/academic/semesters/${id}`, {
		method: 'DELETE'
	});
};

export const createGradeLevel = async (data: CreateGradeLevelRequest) => {
	return await fetchApi<GradeLevel>('/api/academic/levels', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const deleteGradeLevel = async (id: string) => {
	return await fetchApi(`/api/academic/levels/${id}`, {
		method: 'DELETE'
	});
};

export const listClassrooms = async (filters?: {
	year_id?: string;
}): Promise<{ data: Classroom[] }> => {
	const params = new URLSearchParams();
	if (filters?.year_id) params.append('year_id', filters.year_id);

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<Classroom[]>(`/api/academic/classrooms${queryString}`);
};

export const createClassroom = async (data: CreateClassroomRequest) => {
	return await fetchApi<Classroom>('/api/academic/classrooms', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateClassroom = async (id: string, data: UpdateClassroomRequest) => {
	return await fetchApi<Classroom>(`/api/academic/classrooms/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const getEnrollments = async (classId: string): Promise<{ data: StudentEnrollment[] }> => {
	return await fetchApi<StudentEnrollment[]>(`/api/academic/enrollments/class/${classId}`);
};

export const enrollStudents = async (data: EnrollStudentRequest) => {
	return await fetchApi('/api/academic/enrollments', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const removeEnrollment = async (id: string) => {
	return await fetchApi(`/api/academic/enrollments/${id}`, {
		method: 'DELETE'
	});
};

export const updateEnrollmentNumber = async (id: string, class_number: number | null) => {
	return await fetchApi(`/api/academic/enrollments/${id}/number`, {
		method: 'PUT',
		body: JSON.stringify({ class_number })
	});
};

export const autoAssignClassNumbers = async (
	classroomId: string,
	sortBy: 'student_code' | 'name' | 'gender_name'
) => {
	return await fetchApi(`/api/academic/enrollments/class/${classroomId}/auto-number`, {
		method: 'POST',
		body: JSON.stringify({ sort_by: sortBy })
	});
};

// ==========================================
// Curriculum API
// ==========================================

export const listSubjectGroups = async (): Promise<{ data: SubjectGroup[] }> => {
	return await fetchApi<SubjectGroup[]>('/api/academic/subjects/groups');
};

export const listSubjects = async (
	filters: {
		group_id?: string;
		subject_type?: string;
		search?: string;
		active_only?: boolean;
		/** Return, for each code, the latest version effective in this year (start_academic_year_id <= target year). */
		active_in_year_id?: string;
		term?: string;
		/** When true (default on backend), return only the latest version per code. Pass false to show all versions. */
		latest_only?: boolean;
	} = {}
): Promise<{ data: Subject[] }> => {
	const params = new URLSearchParams();
	if (filters.group_id) params.append('group_id', filters.group_id);
	if (filters.subject_type) params.append('subject_type', filters.subject_type);
	if (filters.search) params.append('search', filters.search);
	if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));
	if (filters.active_in_year_id) params.append('active_in_year_id', filters.active_in_year_id);
	if (filters.term) params.append('term', filters.term);
	if (filters.latest_only !== undefined) params.append('latest_only', String(filters.latest_only));

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<Subject[]>(`/api/academic/subjects${queryString}`);
};

export const createSubject = async (data: CreateSubjectRequest) => {
	return await fetchApi<Subject>('/api/academic/subjects', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateSubject = async (id: string, data: UpdateSubjectRequest) => {
	return await fetchApi<Subject>(`/api/academic/subjects/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteSubject = async (id: string) => {
	return await fetchApi(`/api/academic/subjects/${id}`, {
		method: 'DELETE'
	});
};

export const lookupGradeLevels = async (
	params: Record<string, string | number | boolean> = {}
): Promise<{ data: LookupItem[] }> => {
	const sp = new URLSearchParams();
	for (const [k, v] of Object.entries(params)) sp.append(k, String(v));
	const queryString = sp.toString();
	return await fetchApi<LookupItem[]>(`/api/lookup/grade-levels?${queryString}`);
};

export const lookupAcademicYears = async (
	active_only: boolean = true
): Promise<{ data: LookupItem[] }> => {
	return await fetchApi<LookupItem[]>(`/api/lookup/academic-years?active_only=${active_only}`);
};

// Year-Level Configuration
export const getYearLevelConfig = async (yearId: string): Promise<{ data: string[] }> => {
	// Returns array of grade_level_ids
	return await fetchApi<string[]>(`/api/academic/years/${yearId}/levels`);
};

export const saveYearLevelConfig = async (yearId: string, gradeLevelIds: string[]) => {
	return await fetchApi(`/api/academic/years/${yearId}/levels`, {
		method: 'PUT',
		body: JSON.stringify({ grade_level_ids: gradeLevelIds })
	});
};

export const listClassroomCourses = async (
	filters: ClassroomCourseFilters = {}
): Promise<{ data: ClassroomCourse[] }> => {
	const url = '/api/academic/planning/courses';
	const params = new URLSearchParams();
	if (filters.classroomId) params.append('classroom_id', filters.classroomId);
	if (filters.instructorId) params.append('instructor_id', filters.instructorId);
	if (filters.semesterId) params.append('academic_semester_id', filters.semesterId);
	if (filters.subjectId) params.append('subject_id', filters.subjectId);

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<ClassroomCourse[]>(url + queryString);
};

export const assignCourses = async (data: AssignCoursesRequest) => {
	return await fetchApi<CourseAssignedCountData>('/api/academic/planning/courses', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const removeCourse = async (id: string) => {
	return await fetchApi(`/api/academic/planning/courses/${id}`, { method: 'DELETE' });
};

export const updateCourse = async (
	id: string,
	data: UpdateCourseRequest
) => {
	return await fetchApi(`/api/academic/planning/courses/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

// ==========================================
// Study Plans (หลักสูตรสถานศึกษา)
// ==========================================

// Study Plans CRUD
export const listStudyPlans = async (
	filters: {
		active_only?: boolean;
	} = {}
): Promise<{ data: StudyPlan[] }> => {
	const params = new URLSearchParams();
	if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<StudyPlan[]>(`/api/academic/study-plans${queryString}`);
};

export const getStudyPlan = async (id: string): Promise<{ data: StudyPlan }> => {
	return await fetchApi<StudyPlan>(`/api/academic/study-plans/${id}`);
};

export const createStudyPlan = async (data: CreateStudyPlanRequest) => {
	return await fetchApi<StudyPlan>('/api/academic/study-plans', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateStudyPlan = async (id: string, data: UpdateStudyPlanRequest) => {
	return await fetchApi<StudyPlan>(`/api/academic/study-plans/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteStudyPlan = async (id: string) => {
	return await fetchApi(`/api/academic/study-plans/${id}`, {
		method: 'DELETE'
	});
};

// Study Plan Versions CRUD
export const listStudyPlanVersions = async (
	filters: {
		study_plan_id?: string;
		active_only?: boolean;
	} = {}
): Promise<{ data: StudyPlanVersion[] }> => {
	const params = new URLSearchParams();
	if (filters.study_plan_id) params.append('study_plan_id', filters.study_plan_id);
	if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<StudyPlanVersion[]>(`/api/academic/study-plan-versions${queryString}`);
};

export const getStudyPlanVersion = async (id: string): Promise<{ data: StudyPlanVersion }> => {
	return await fetchApi<StudyPlanVersion>(`/api/academic/study-plan-versions/${id}`);
};

export const createStudyPlanVersion = async (data: CreateStudyPlanVersionRequest) => {
	return await fetchApi<StudyPlanVersion>('/api/academic/study-plan-versions', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateStudyPlanVersion = async (id: string, data: UpdateStudyPlanVersionRequest) => {
	return await fetchApi<StudyPlanVersion>(`/api/academic/study-plan-versions/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteStudyPlanVersion = async (id: string) => {
	return await fetchApi(`/api/academic/study-plan-versions/${id}`, {
		method: 'DELETE'
	});
};

// Study Plan Subjects Management
export const listStudyPlanSubjects = async (
	filters: {
		study_plan_version_id?: string;
		grade_level_id?: string;
		term?: string;
	} = {}
): Promise<{ data: StudyPlanSubject[] }> => {
	const params = new URLSearchParams();
	if (filters.study_plan_version_id)
		params.append('study_plan_version_id', filters.study_plan_version_id);
	if (filters.grade_level_id) params.append('grade_level_id', filters.grade_level_id);
	if (filters.term) params.append('term', filters.term);

	const queryString = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<StudyPlanSubject[]>(
		`/api/academic/study-plan-versions/${filters.study_plan_version_id}/subjects${queryString}`
	);
};

export const addSubjectsToVersion = async (
	versionId: string,
	subjects: SubjectInPlan[]
) => {
	return await fetchApi<{ count: number }>(
		`/api/academic/study-plan-versions/${versionId}/subjects`,
		{
			method: 'POST',
			body: JSON.stringify({ subjects })
		}
	);
};

export const deleteStudyPlanSubject = async (id: string) => {
	return await fetchApi(`/api/academic/study-plan-subjects/${id}`, {
		method: 'DELETE'
	});
};

// Bulk: Generate Courses (+ activities) from Study Plan
export const generateCoursesFromPlan = async (
	data: GenerateCoursesFromPlanRequest
): Promise<GenerateCoursesFromPlanResponse> => {
	const response = await fetchApi<GenerateCoursesFromPlanResponse>(
		'/api/academic/planning/generate-from-plan',
		{
			method: 'POST',
			body: JSON.stringify(data)
		}
	);
	return response.data;
};

export const ALL_DAYS = [
	{ value: 'MON', label: 'จันทร์', shortLabel: 'จ' },
	{ value: 'TUE', label: 'อังคาร', shortLabel: 'อ' },
	{ value: 'WED', label: 'พุธ', shortLabel: 'พ' },
	{ value: 'THU', label: 'พฤหัสบดี', shortLabel: 'พฤ' },
	{ value: 'FRI', label: 'ศุกร์', shortLabel: 'ศ' },
	{ value: 'SAT', label: 'เสาร์', shortLabel: 'ส' },
	{ value: 'SUN', label: 'อาทิตย์', shortLabel: 'อา' }
];

/** Parse school_days string → filtered day list */
export function getSchoolDays(schoolDaysStr?: string) {
	const values = (schoolDaysStr || 'MON,TUE,WED,THU,FRI').split(',').map((d) => d.trim());
	return ALL_DAYS.filter((d) => values.includes(d.value));
}

export const ACTIVITY_TYPE_LABELS: Record<string, string> = {
	scout: 'ลูกเสือ / เนตรนารี / ยุวกาชาด',
	club: 'ชุมนุม',
	guidance: 'แนะแนว',
	social: 'กิจกรรมเพื่อสังคม',
	other: 'อื่น ๆ'
};

export function getActivityTypeLabel(activityType?: string | null): string {
	if (!activityType) return ACTIVITY_TYPE_LABELS.other;
	return ACTIVITY_TYPE_LABELS[activityType] ?? activityType;
}

export const listActivitySlots = async (
	filter: ActivitySlotFilter = {}
): Promise<{ data: ActivitySlot[] }> => {
	const params = new URLSearchParams();
	if (filter.semester_id) params.set('semester_id', filter.semester_id);
	if (filter.activity_type) params.set('activity_type', filter.activity_type);
	if (filter.teacher_reg_open !== undefined)
		params.set('teacher_reg_open', String(filter.teacher_reg_open));
	if (filter.student_reg_open !== undefined)
		params.set('student_reg_open', String(filter.student_reg_open));
	return await fetchApi<ActivitySlot[]>(`/api/academic/activity-slots?${params}`);
};

export const getActivitySlotTimetableContext = async (
	semesterId: string,
	options: ApiRequestOptions = {}
): Promise<{ data: ActivitySlotTimetableContextResponse }> => {
	const params = new URLSearchParams({ semester_id: semesterId });
	return await fetchApi<ActivitySlotTimetableContextResponse>(
		`/api/academic/activity-slots/timetable-context?${params}`,
		{ signal: options.signal }
	);
};

// Slots must come from plan via generate_courses_from_plan — no standalone creation.
// Only semester-specific fields can be updated (template fields live in activity_catalog).
export const updateActivitySlot = async (id: string, data: UpdateActivitySlotRequest) => {
	return await fetchApi<ActivitySlot>(`/api/academic/activity-slots/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteActivitySlot = async (id: string) => {
	return await fetchApi(`/api/academic/activity-slots/${id}`, { method: 'DELETE' });
};

// Slot Instructors
export const listSlotInstructors = async (slotId: string): Promise<{ data: SlotInstructor[] }> => {
	return await fetchApi<SlotInstructor[]>(`/api/academic/activity-slots/${slotId}/instructors`);
};

export const addSlotInstructor = async (slotId: string, userId: string) => {
	const data: AddSlotInstructorRequest = { user_id: userId };
	return await fetchApi(`/api/academic/activity-slots/${slotId}/instructors`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const addSlotInstructorsBatch = async (slotId: string, userIds: string[]) => {
	const data: AddSlotInstructorsBatchRequest = { user_ids: userIds };
	return await fetchApi<ActivityAddedCountData>(
		`/api/academic/activity-slots/${slotId}/instructors/batch`,
		{
			method: 'POST',
			body: JSON.stringify(data)
		}
	);
};

export const removeSlotInstructor = async (slotId: string, userId: string) => {
	return await fetchApi(`/api/academic/activity-slots/${slotId}/instructors/${userId}`, {
		method: 'DELETE'
	});
};

export const removeAllSlotInstructors = async (slotId: string) => {
	return await fetchApi<ActivityDeletedCountData>(
		`/api/academic/activity-slots/${slotId}/instructors/all`,
		{
			method: 'DELETE'
		}
	);
};

export const deleteAllSlotGroups = async (slotId: string) => {
	return await fetchApi<ActivityDeletedCountData>(`/api/academic/activity-slots/${slotId}/groups`, {
		method: 'DELETE'
	});
};

export const deleteSlotTimetableEntries = async (slotId: string) => {
	return await fetchApi<ActivityDeletedCountData>(
		`/api/academic/activity-slots/${slotId}/timetable-entries`,
		{
			method: 'DELETE'
		}
	);
};

// ==========================================
// Slot Classroom Assignments (ครูต่อห้อง — independent)
// ==========================================

export const listSlotClassroomAssignments = async (
	slotId: string
): Promise<{ data: SlotClassroomAssignment[] }> => {
	return await fetchApi<SlotClassroomAssignment[]>(
		`/api/academic/activity-slots/${slotId}/classroom-assignments`
	);
};

export const batchUpsertSlotClassroomAssignments = async (
	slotId: string,
	assignments: UpsertSlotClassroomAssignmentRequest[]
) => {
	const data: BatchUpsertSlotClassroomAssignmentsRequest = { assignments };
	return await fetchApi<ActivityProcessedCountData>(
		`/api/academic/activity-slots/${slotId}/classroom-assignments`,
		{
			method: 'POST',
			body: JSON.stringify(data)
		}
	);
};

export const deleteSlotClassroomAssignment = async (slotId: string, assignmentId: string) => {
	return await fetchApi(
		`/api/academic/activity-slots/${slotId}/classroom-assignments/${assignmentId}`,
		{ method: 'DELETE' }
	);
};

export const deleteAllSlotClassroomAssignments = async (slotId: string) => {
	return await fetchApi<ActivityDeletedCountData>(
		`/api/academic/activity-slots/${slotId}/classroom-assignments/all`,
		{
			method: 'DELETE'
		}
	);
};

// ==========================================
// Activity Groups (กิจกรรมจริง ภายใต้ slot)
// ==========================================

export const listActivityGroups = async (
	filter: ActivityGroupFilter = {}
): Promise<{ data: ActivityGroup[] }> => {
	const params = new URLSearchParams();
	if (filter.slot_id) params.set('slot_id', filter.slot_id);
	if (filter.semester_id) params.set('semester_id', filter.semester_id);
	if (filter.activity_type) params.set('activity_type', filter.activity_type);
	if (filter.instructor_id) params.set('instructor_id', filter.instructor_id);
	if (filter.registration_open !== undefined)
		params.set('registration_open', String(filter.registration_open));
	if (filter.search) params.set('search', filter.search);
	return await fetchApi<ActivityGroup[]>(`/api/academic/activities?${params}`);
};

export const createActivityGroup = async (data: CreateActivityGroupRequest) => {
	return await fetchApi<ActivityGroup>('/api/academic/activities', {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateActivityGroup = async (id: string, data: UpdateActivityGroupRequest) => {
	return await fetchApi<ActivityGroup>(`/api/academic/activities/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteActivityGroup = async (id: string) => {
	return await fetchApi(`/api/academic/activities/${id}`, { method: 'DELETE' });
};

export const listActivityMembers = async (
	groupId: string
): Promise<{ data: ActivityGroupMember[] }> => {
	return await fetchApi<ActivityGroupMember[]>(`/api/academic/activities/${groupId}/members`);
};

export const addActivityMembers = async (groupId: string, studentIds: string[]) => {
	const data: AddMembersRequest = { student_ids: studentIds };
	return await fetchApi<ActivityInsertedCountData>(`/api/academic/activities/${groupId}/members`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const removeActivityMember = async (groupId: string, studentId: string) => {
	return await fetchApi(`/api/academic/activities/${groupId}/members/${studentId}`, {
		method: 'DELETE'
	});
};

export const updateMemberResult = async (memberId: string, result: string) => {
	if (result !== 'pass' && result !== 'fail') throw new Error('Invalid activity member result');
	const data: UpdateMemberResultRequest = { result };
	return await fetchApi(`/api/academic/activities/members/${memberId}/result`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const listActivityInstructors = async (
	groupId: string
): Promise<{ data: ActivityInstructor[] }> => {
	return await fetchApi<ActivityInstructor[]>(`/api/academic/activities/${groupId}/instructors`);
};

export const addActivityInstructor = async (
	groupId: string,
	instructorId: string,
	role: ActivityGroupInstructorRole
) => {
	const data: InstructorRoleRequest = { instructor_id: instructorId, role };
	return await fetchApi(`/api/academic/activities/${groupId}/instructors`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const removeActivityInstructor = async (groupId: string, instructorId: string) => {
	return await fetchApi(`/api/academic/activities/${groupId}/instructors/${instructorId}`, {
		method: 'DELETE'
	});
};

// Student self-enrollment
export const selfEnrollActivity = async (groupId: string) => {
	return await fetchApi(`/api/academic/activities/${groupId}/enroll`, { method: 'POST' });
};

export const selfUnenrollActivity = async (groupId: string) => {
	return await fetchApi(`/api/academic/activities/${groupId}/enroll`, { method: 'DELETE' });
};

export const getMyActivityEnrollments = async (): Promise<{ data: string[] }> => {
	return await fetchApi<string[]>('/api/academic/activities/my-enrollments');
};

export const listCourseInstructors = async (
	courseId: string
): Promise<{ data: CourseInstructor[] }> => {
	return await fetchApi<CourseInstructor[]>(
		`/api/academic/planning/courses/${courseId}/instructors`
	);
};

export const batchListCourseInstructors = async (
	courseIds: string[]
): Promise<{ data: Record<string, CourseInstructor[]> }> => {
	if (courseIds.length === 0) return { data: {} };
	const data: BatchListCourseInstructorsRequest = { course_ids: courseIds };
	return await fetchApi<Record<string, CourseInstructor[]>>(
		'/api/academic/planning/courses/instructors/batch',
		{
			method: 'POST',
			body: JSON.stringify(data)
		}
	);
};

export const batchListCourseInstructorsFromQuery = async (
	courseIds: string[]
): Promise<{ data: Record<string, CourseInstructor[]> }> => {
	if (courseIds.length === 0) return { data: {} };
	const params = new URLSearchParams({ course_ids: courseIds.join(',') });
	return await fetchApi<Record<string, CourseInstructor[]>>(
		`/api/academic/planning/courses/instructors?${params}`
	);
};

export const addCourseInstructor = async (
	courseId: string,
	instructorId: string,
	role: CourseInstructorRole = 'secondary'
) => {
	const data: AddCourseInstructorRequest = { instructor_id: instructorId, role };
	return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const removeCourseInstructor = async (courseId: string, instructorId: string) => {
	return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors/${instructorId}`, {
		method: 'DELETE'
	});
};

export const updateCourseInstructorRole = async (
	courseId: string,
	instructorId: string,
	role: CourseInstructorRole
) => {
	const data: UpdateCourseInstructorRoleRequest = { role };
	return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors/${instructorId}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

// ==========================================
// Subject Default Instructors (team teaching at catalog level)
// ครูประจำวิชาใน คลังรายวิชา — auto-copy ไป classroom_course_instructors ตอน assign
// ==========================================

export const listSubjectDefaultInstructors = async (
	subjectId: string
): Promise<{ data: SubjectDefaultInstructor[] }> => {
	return await fetchApi<SubjectDefaultInstructor[]>(
		`/api/academic/subjects/${subjectId}/default-instructors`
	);
};

export const batchListSubjectDefaultInstructors = async (
	subjectIds: string[]
): Promise<{ data: Record<string, SubjectDefaultInstructor[]> }> => {
	if (subjectIds.length === 0) return { data: {} };
	const params = new URLSearchParams({ subject_ids: subjectIds.join(',') });
	return await fetchApi<Record<string, SubjectDefaultInstructor[]>>(
		`/api/academic/subjects/default-instructors?${params}`
	);
};

export const addSubjectDefaultInstructor = async (
	subjectId: string,
	instructorId: string,
	role: CurriculumInstructorRole = 'secondary'
) => {
	return await fetchApi(`/api/academic/subjects/${subjectId}/default-instructors`, {
		method: 'POST',
		body: JSON.stringify({ instructor_id: instructorId, role })
	});
};

export const removeSubjectDefaultInstructor = async (subjectId: string, instructorId: string) => {
	return await fetchApi(`/api/academic/subjects/${subjectId}/default-instructors/${instructorId}`, {
		method: 'DELETE'
	});
};

export const updateSubjectDefaultInstructorRole = async (
	subjectId: string,
	instructorId: string,
	role: CurriculumInstructorRole
) => {
	return await fetchApi(`/api/academic/subjects/${subjectId}/default-instructors/${instructorId}`, {
		method: 'PUT',
		body: JSON.stringify({ role })
	});
};

// ==========================================
// Study Plan Version Activities (template)
// ==========================================

export const listPlanActivities = async (
	versionId: string
): Promise<{ data: StudyPlanVersionActivity[] }> => {
	return await fetchApi<StudyPlanVersionActivity[]>(
		`/api/academic/study-plan-versions/${versionId}/activities`
	);
};

export const addPlanActivity = async (versionId: string, data: CreatePlanActivityRequest) => {
	return await fetchApi<StudyPlanVersionActivity>(
		`/api/academic/study-plan-versions/${versionId}/activities`,
		{
			method: 'POST',
			body: JSON.stringify(data)
		}
	);
};

export const updatePlanActivity = async (id: string, data: UpdatePlanActivityRequest) => {
	return await fetchApi<StudyPlanVersionActivity>(`/api/academic/study-plan-activities/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

// ==========================================
// Activity Catalog (คลังกิจกรรม)
// ==========================================

export const listActivityCatalog = async (
	opts: {
		/** When true (default), return only the latest version per name. */
		latest_only?: boolean;
	} = {}
): Promise<{ data: ActivityCatalog[] }> => {
	const params = new URLSearchParams();
	if (opts.latest_only === false) params.set('latest_only', 'false');
	const qs = params.toString() ? `?${params.toString()}` : '';
	return await fetchApi<ActivityCatalog[]>(`/api/academic/activity-catalog${qs}`);
};

export const createActivityCatalog = async (data: CreateCatalogRequest) => {
	return await fetchApi<ActivityCatalog>(`/api/academic/activity-catalog`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const updateActivityCatalog = async (id: string, data: UpdateCatalogRequest) => {
	return await fetchApi<ActivityCatalog>(`/api/academic/activity-catalog/${id}`, {
		method: 'PUT',
		body: JSON.stringify(data)
	});
};

export const deleteActivityCatalog = async (id: string) => {
	return await fetchApi(`/api/academic/activity-catalog/${id}`, { method: 'DELETE' });
};

// ==========================================
// Activity Catalog Default Instructors (team default ต่อ catalog version)
// Pattern เดียวกับ SubjectDefaultInstructor
// ==========================================

export const listCatalogDefaultInstructors = async (
	catalogId: string
): Promise<{ data: CatalogDefaultInstructor[] }> => {
	return await fetchApi<CatalogDefaultInstructor[]>(
		`/api/academic/activity-catalog/${catalogId}/default-instructors`
	);
};

export const addCatalogDefaultInstructor = async (
	catalogId: string,
	instructorId: string,
	role: CurriculumInstructorRole = 'secondary'
) => {
	const data: AddCatalogDefaultInstructorRequest = { instructor_id: instructorId, role };
	return await fetchApi(`/api/academic/activity-catalog/${catalogId}/default-instructors`, {
		method: 'POST',
		body: JSON.stringify(data)
	});
};

export const removeCatalogDefaultInstructor = async (catalogId: string, instructorId: string) => {
	return await fetchApi(
		`/api/academic/activity-catalog/${catalogId}/default-instructors/${instructorId}`,
		{
			method: 'DELETE'
		}
	);
};

export const updateCatalogDefaultInstructorRole = async (
	catalogId: string,
	instructorId: string,
	role: CurriculumInstructorRole
) => {
	const data: UpdateCatalogDefaultInstructorRoleRequest = { role };
	return await fetchApi(
		`/api/academic/activity-catalog/${catalogId}/default-instructors/${instructorId}`,
		{
			method: 'PUT',
			body: JSON.stringify(data)
		}
	);
};

export const deletePlanActivity = async (id: string) => {
	return await fetchApi(`/api/academic/study-plan-activities/${id}`, {
		method: 'DELETE'
	});
};

export const generateActivitiesFromPlan = async (
	data: GenerateActivitiesFromPlanRequest
): Promise<GenerateActivitiesFromPlanResponse> => {
	const response = await fetchApi<GenerateActivitiesFromPlanResponse>(
		`/api/academic/activities/generate-from-plan`,
		{
			method: 'POST',
			body: JSON.stringify(data)
		}
	);
	return response.data;
};

// ==========================================
// Classroom Activities (junction-backed)
// หน้า Course Planning = source of truth ต่อห้อง
// ==========================================

export const listClassroomActivities = async (
	classroomId: string,
	semesterId: string
): Promise<{ data: ClassroomActivity[] }> => {
	const params = new URLSearchParams({ semester_id: semesterId });
	return await fetchApi<ClassroomActivity[]>(
		`/api/academic/planning/classrooms/${classroomId}/activities?${params}`
	);
};

export const removeClassroomFromSlot = async (classroomId: string, slotId: string) => {
	return await fetchApi(`/api/academic/planning/classrooms/${classroomId}/activities/${slotId}`, {
		method: 'DELETE'
	});
};
