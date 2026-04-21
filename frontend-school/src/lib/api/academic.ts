import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

// Types
export interface AcademicYear {
    id: string;
    year: number;
    name: string;
    start_date: string;
    end_date: string;
    is_active: boolean;
    school_days: string;
    created_at: string;
}

export interface Semester {
    id: string;
    academic_year_id: string;
    term: string;
    name: string;
    start_date: string;
    end_date: string;
    is_active: boolean;
}

export interface GradeLevel {
    id: string;
    level_type: 'kindergarten' | 'primary' | 'secondary';  // Type of education level
    year: number;           // Year within the level (1, 2, 3...)
    code: string;           // Computed: K1, P1, M1
    name: string;           // Computed: อนุบาลศึกษาปีที่ 1, ประถมศึกษาปีที่ 1, etc.
    short_name: string;     // Computed: อ.1, ป.1, ม.1
    is_active: boolean;
}

export interface AcademicStructureData {
    years: AcademicYear[];
    semesters: Semester[];
    levels: GradeLevel[];
}

export interface Classroom {
    id: string;
    code: string;
    name: string;
    academic_year_id: string;
    grade_level_id: string;
    room_number: string;
    advisor_id?: string;
    co_advisor_id?: string;
    study_plan_version_id?: string; // Required - ห้องเรียนทุกห้องต้องใช้หลักสูตร
    capacity?: number;
    is_active: boolean;
    grade_level_name?: string;
    academic_year_label?: string;
    advisor_name?: string;
    student_count?: number;
    year?: number; // Optional year for grade levels
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
async function fetchApi(path: string, options: RequestInit = {}): Promise<any> {
    const response = await fetch(`${BACKEND_URL}${path}`, {
        ...options,
        credentials: 'include',
        headers: {
            'Content-Type': 'application/json',
            ...options.headers
        }
    });

    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Unknown error' }));
        throw new Error(error.error || `Request failed with status ${response.status}`);
    }

    return await response.json();
}

export const getAcademicStructure = async (): Promise<{ data: AcademicStructureData }> => {
    return await fetchApi('/api/academic/structure');
};

export const createAcademicYear = async (data: {
    year: number;
    name: string;
    start_date: string;
    end_date: string;
    is_active: boolean;
}) => {
    return await fetchApi('/api/academic/years', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateAcademicYear = async (id: string, data: Partial<AcademicYear>) => {
    return await fetchApi(`/api/academic/years/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

export const toggleActiveYear = async (id: string) => {
    return await fetchApi(`/api/academic/years/${id}/active`, {
        method: 'PUT'
    });
};

export const createSemester = async (data: {
    academic_year_id: string;
    term: string;
    name: string;
    start_date: string;
    end_date: string;
    is_active?: boolean;
}) => {
    return await fetchApi('/api/academic/semesters', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateSemester = async (id: string, data: {
    term?: string;
    name?: string;
    start_date?: string;
    end_date?: string;
    is_active?: boolean;
}) => {
    return await fetchApi(`/api/academic/semesters/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

export const deleteSemester = async (id: string) => {
    return await fetchApi(`/api/academic/semesters/${id}`, {
        method: 'DELETE'
    });
};

export const createGradeLevel = async (data: {
    level_type: 'kindergarten' | 'primary' | 'secondary';
    year: number;
    next_grade_level_id?: string;
}) => {
    return await fetchApi('/api/academic/levels', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const deleteGradeLevel = async (id: string) => {
    return await fetchApi(`/api/academic/levels/${id}`, {
        method: 'DELETE'
    });
};

export const listClassrooms = async (filters?: { year_id?: string }): Promise<{ data: Classroom[] }> => {
    const params = new URLSearchParams();
    if (filters?.year_id) params.append('year_id', filters.year_id);

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/classrooms${queryString}`);
};

export const createClassroom = async (data: {
    academic_year_id: string;
    grade_level_id: string;
    room_number: string;
    advisor_id?: string;
    co_advisor_id?: string;
    capacity?: number;
    study_plan_version_id?: string;
}) => {
    return await fetchApi('/api/academic/classrooms', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateClassroom = async (id: string, data: {
    room_number?: string;
    advisor_id?: string;
    co_advisor_id?: string;
    study_plan_version_id?: string;
    capacity?: number;
    is_active?: boolean;
}) => {
    return await fetchApi(`/api/academic/classrooms/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

export interface StudentEnrollment {
    id: string;
    student_id: string;
    class_room_id: string;
    enrollment_date: string;
    status: string;
    class_number?: number | null;
    student_name?: string;
    class_name?: string;
    student_code?: string;
}

export const getEnrollments = async (classId: string): Promise<{ data: StudentEnrollment[] }> => {
    return await fetchApi(`/api/academic/enrollments/class/${classId}`);
};

export const enrollStudents = async (data: {
    student_ids: string[];
    class_room_id: string;
    enrollment_date?: string;
    numbering_method?: 'append' | 'student_code' | 'name' | 'gender_name';
}) => {
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

export const autoAssignClassNumbers = async (classroomId: string, sortBy: 'student_code' | 'name' | 'gender_name') => {
    return await fetchApi(`/api/academic/enrollments/class/${classroomId}/auto-number`, {
        method: 'POST',
        body: JSON.stringify({ sort_by: sortBy })
    });
};



// ==========================================
// Curriculum API
// ==========================================

export interface SubjectGroup {
    id: string;
    code: string;
    name_th: string;
    name_en: string;
    display_order: number;
    is_active: boolean;
}

export interface Subject {
    id: string;
    code: string;
    start_academic_year_id: string; // effective-from year for this subject version
    name_th: string;
    name_en?: string;
    credit: number;
    hours_per_semester?: number;
    type: 'BASIC' | 'ADDITIONAL' | 'ACTIVITY';
    group_id?: string;
    description?: string;
    is_active: boolean;
    group_name_th?: string;
    grade_level_ids?: string[];
    term?: string;
    default_instructor_id?: string;
    default_instructor_name?: string;
    /** Pass on create/update to replace default instructor team atomically. */
    default_instructors?: { instructor_id: string; role: 'primary' | 'secondary' }[];
}

export const listSubjectGroups = async (): Promise<{ data: SubjectGroup[] }> => {
    return await fetchApi('/api/academic/subjects/groups');
};

export const listSubjects = async (filters: {
    group_id?: string;
    subject_type?: string;
    search?: string;
    active_only?: boolean;
    /** Return, for each code, the latest version effective in this year (start_academic_year_id <= target year). */
    active_in_year_id?: string;
    term?: string;
    /** When true (default on backend), return only the latest version per code. Pass false to show all versions. */
    latest_only?: boolean;
} = {}): Promise<{ data: Subject[] }> => {
    const params = new URLSearchParams();
    if (filters.group_id) params.append('group_id', filters.group_id);
    if (filters.subject_type) params.append('subject_type', filters.subject_type);
    if (filters.search) params.append('search', filters.search);
    if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));
    if (filters.active_in_year_id) params.append('active_in_year_id', filters.active_in_year_id);
    if (filters.term) params.append('term', filters.term);
    if (filters.latest_only !== undefined) params.append('latest_only', String(filters.latest_only));

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/subjects${queryString}`);
};

export const createSubject = async (data: Partial<Subject>) => {
    return await fetchApi('/api/academic/subjects', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateSubject = async (id: string, data: Partial<Subject>) => {
    return await fetchApi(`/api/academic/subjects/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

export const deleteSubject = async (id: string) => {
    return await fetchApi(`/api/academic/subjects/${id}`, {
        method: 'DELETE'
    });
};

export const lookupGradeLevels = async (params: any = {}): Promise<{ data: LookupItem[] }> => {
    const queryString = new URLSearchParams(params).toString();
    return await fetchApi(`/api/lookup/grade-levels?${queryString}`);
};

export const lookupAcademicYears = async (active_only: boolean = true): Promise<{ data: LookupItem[] }> => {
    return await fetchApi(`/api/lookup/academic-years?active_only=${active_only}`);
};


// Year-Level Configuration
export const getYearLevelConfig = async (yearId: string): Promise<{ data: string[] }> => {
    // Returns array of grade_level_ids
    return await fetchApi(`/api/academic/years/${yearId}/levels`);
};

export const saveYearLevelConfig = async (yearId: string, gradeLevelIds: string[]) => {
    return await fetchApi(`/api/academic/years/${yearId}/levels`, {
        method: 'PUT',
        body: JSON.stringify({ grade_level_ids: gradeLevelIds })
    });
};

export interface ClassroomCourse {
    id: string;
    classroom_id: string;
    subject_id: string;
    academic_semester_id: string;
    primary_instructor_id?: string;
    settings: any;
    subject_code: string;
    subject_name_th: string;
    subject_name_en?: string;
    subject_credit?: number;
    subject_hours?: number;
    subject_type?: string;
    instructor_name?: string;
    classroom_name?: string;
}

// Supports both old signature (string, string?) and new signature (object) for backward compatibility if needed, 
// but here we will change to object based to support instructorId
export const listClassroomCourses = async (
    param1: string | { classroomId?: string; instructorId?: string; semesterId?: string; subjectId?: string },
    param2?: string
): Promise<{ data: ClassroomCourse[] }> => {
    let url = '/api/academic/planning/courses';
    const params = new URLSearchParams();

    if (typeof param1 === 'string') {
        // Old usage: listClassroomCourses(classroomId, semesterId)
        params.append('classroom_id', param1);
        if (param2) params.append('academic_semester_id', param2);
    } else {
        // New usage: object
        if (param1.classroomId) params.append('classroom_id', param1.classroomId);
        if (param1.instructorId) params.append('instructor_id', param1.instructorId);
        if (param1.semesterId) params.append('academic_semester_id', param1.semesterId);
        if (param1.subjectId) params.append('subject_id', param1.subjectId);
    }

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(url + queryString);
};

export const assignCourses = async (data: {
    classroom_id: string;
    academic_semester_id: string;
    subject_ids: string[];
}) => {
    return await fetchApi('/api/academic/planning/courses', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};


export const removeCourse = async (id: string) => {
    return await fetchApi(`/api/academic/planning/courses/${id}`, { method: 'DELETE' });
};

export const updateCourse = async (id: string, data: {
    primary_instructor_id?: string | null;
    settings?: any;
}) => {
    return await fetchApi(`/api/academic/planning/courses/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

// ==========================================
// Study Plans (หลักสูตรสถานศึกษา)
// ==========================================

export interface StudyPlan {
    id: string;
    code: string;
    name_th: string;
    name_en?: string;
    description?: string;
    grade_level_ids?: string[];
    is_active: boolean;
    created_at: string;
    updated_at: string;
}

export interface StudyPlanVersion {
    id: string;
    study_plan_id: string;
    version_name: string;
    start_academic_year_id: string;
    end_academic_year_id?: string;
    description?: string;
    is_active: boolean;
    created_at: string;
    updated_at: string;
    // Joined fields
    study_plan_name_th?: string;
    start_year_name?: string;
}

export interface StudyPlanSubject {
    id: string;
    study_plan_version_id: string;
    grade_level_id: string;
    term: string; // '1', '2', '3'
    subject_id: string;
    subject_code: string;
    display_order: number;
    // Joined fields
    subject_name_th?: string;
    subject_name_en?: string;
    subject_credit?: number;
    subject_type?: string;
    subject_hours?: number;
    grade_level_name?: string;
}

// Study Plans CRUD
export const listStudyPlans = async (filters: {
    active_only?: boolean;
} = {}): Promise<{ data: StudyPlan[] }> => {
    const params = new URLSearchParams();
    if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/study-plans${queryString}`);
};

export const getStudyPlan = async (id: string): Promise<{ data: StudyPlan }> => {
    return await fetchApi(`/api/academic/study-plans/${id}`);
};

export const createStudyPlan = async (data: {
    code: string;
    name_th: string;
    name_en?: string;
    description?: string;
    grade_level_ids?: string[];
}) => {
    return await fetchApi('/api/academic/study-plans', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateStudyPlan = async (id: string, data: Partial<StudyPlan>) => {
    return await fetchApi(`/api/academic/study-plans/${id}`, {
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
export const listStudyPlanVersions = async (filters: {
    study_plan_id?: string;
    active_only?: boolean;
} = {}): Promise<{ data: StudyPlanVersion[] }> => {
    const params = new URLSearchParams();
    if (filters.study_plan_id) params.append('study_plan_id', filters.study_plan_id);
    if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/study-plan-versions${queryString}`);
};

export const getStudyPlanVersion = async (id: string): Promise<{ data: StudyPlanVersion }> => {
    return await fetchApi(`/api/academic/study-plan-versions/${id}`);
};

export const createStudyPlanVersion = async (data: {
    study_plan_id: string;
    version_name: string;
    start_academic_year_id: string;
    end_academic_year_id?: string;
    description?: string;
}) => {
    return await fetchApi('/api/academic/study-plan-versions', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateStudyPlanVersion = async (id: string, data: Partial<StudyPlanVersion>) => {
    return await fetchApi(`/api/academic/study-plan-versions/${id}`, {
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
export const listStudyPlanSubjects = async (filters: {
    study_plan_version_id?: string;
    grade_level_id?: string;
    term?: string;
} = {}): Promise<{ data: StudyPlanSubject[] }> => {
    const params = new URLSearchParams();
    if (filters.study_plan_version_id) params.append('study_plan_version_id', filters.study_plan_version_id);
    if (filters.grade_level_id) params.append('grade_level_id', filters.grade_level_id);
    if (filters.term) params.append('term', filters.term);

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/study-plan-versions/${filters.study_plan_version_id}/subjects${queryString}`);
};

export const addSubjectsToVersion = async (versionId: string, subjects: {
    subject_id: string;
    grade_level_id: string;
    term: string;
    display_order?: number;
}[]) => {
    return await fetchApi(`/api/academic/study-plan-versions/${versionId}/subjects`, {
        method: 'POST',
        body: JSON.stringify({ subjects })
    });
};

export const deleteStudyPlanSubject = async (id: string) => {
    return await fetchApi(`/api/academic/study-plan-subjects/${id}`, {
        method: 'DELETE'
    });
};

// Bulk: Generate Courses (+ activities) from Study Plan
export const generateCoursesFromPlan = async (data: {
    classroom_id: string;
    academic_semester_id: string;
    skip_existing?: boolean;
}): Promise<{
    data: { added_count: number; skipped_count: number; message: string };
    courses_created?: number;
    courses_skipped?: number;
    activities_created?: number;
    activities_skipped?: number;
}> => {
    return await fetchApi('/api/academic/planning/generate-from-plan', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

// ==========================================
// Activity Slots (ช่องกิจกรรม — Admin)
// ==========================================

export interface ActivitySlot {
    id: string;
    activity_catalog_id: string; // FK to specific catalog version (template snapshot)
    semester_id: string;
    registration_type: 'self' | 'assigned';
    teacher_reg_open: boolean;
    student_reg_open: boolean;
    student_reg_start?: string;
    student_reg_end?: string;
    is_active: boolean;
    created_at: string;
    // Live-linked from activity_catalog (read-only — edit at คลังกิจกรรม).
    // Backend always JOINs, so these are present even though DB model is via FK.
    name: string;
    description?: string;
    activity_type: 'scout' | 'club' | 'guidance' | 'social' | 'other';
    periods_per_week: number;
    scheduling_mode: 'synchronized' | 'independent';
    allowed_grade_level_ids?: string[];
    // Other joins
    semester_name?: string;
    group_count?: number;
    total_members?: number;
    /** UUIDs of classrooms participating via activity_slot_classrooms junction */
    classroom_ids?: string[];
}

export const ALL_DAYS = [
    { value: 'MON', label: 'จันทร์', shortLabel: 'จ' },
    { value: 'TUE', label: 'อังคาร', shortLabel: 'อ' },
    { value: 'WED', label: 'พุธ', shortLabel: 'พ' },
    { value: 'THU', label: 'พฤหัสบดี', shortLabel: 'พฤ' },
    { value: 'FRI', label: 'ศุกร์', shortLabel: 'ศ' },
    { value: 'SAT', label: 'เสาร์', shortLabel: 'ส' },
    { value: 'SUN', label: 'อาทิตย์', shortLabel: 'อา' },
];

/** Parse school_days string → filtered day list */
export function getSchoolDays(schoolDaysStr?: string) {
    const values = (schoolDaysStr || 'MON,TUE,WED,THU,FRI').split(',').map(d => d.trim());
    return ALL_DAYS.filter(d => values.includes(d.value));
}

export const ACTIVITY_TYPE_LABELS: Record<string, string> = {
    scout: 'ลูกเสือ / เนตรนารี / ยุวกาชาด',
    club: 'ชุมนุม',
    guidance: 'แนะแนว',
    social: 'กิจกรรมเพื่อสังคม',
    other: 'อื่น ๆ'
};

export const listActivitySlots = async (filter: {
    semester_id?: string;
    activity_type?: string;
    teacher_reg_open?: boolean;
    student_reg_open?: boolean;
} = {}): Promise<{ data: ActivitySlot[] }> => {
    const params = new URLSearchParams();
    if (filter.semester_id) params.set('semester_id', filter.semester_id);
    if (filter.activity_type) params.set('activity_type', filter.activity_type);
    if (filter.teacher_reg_open !== undefined) params.set('teacher_reg_open', String(filter.teacher_reg_open));
    if (filter.student_reg_open !== undefined) params.set('student_reg_open', String(filter.student_reg_open));
    return await fetchApi(`/api/academic/activity-slots?${params}`);
};

// Slots must come from plan via generate_courses_from_plan — no standalone creation.
// Only semester-specific fields can be updated (template fields live in activity_catalog).
export const updateActivitySlot = async (id: string, data: {
    registration_type?: string;
    teacher_reg_open?: boolean;
    student_reg_open?: boolean;
    student_reg_start?: string;
    student_reg_end?: string;
    is_active?: boolean;
}) => {
    return await fetchApi(`/api/academic/activity-slots/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

export const deleteActivitySlot = async (id: string) => {
    return await fetchApi(`/api/academic/activity-slots/${id}`, { method: 'DELETE' });
};

// Slot Instructors
export interface SlotInstructor {
    id: string;
    user_id: string;
    instructor_name?: string;
}

export const listSlotInstructors = async (slotId: string): Promise<{ data: SlotInstructor[] }> => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/instructors`);
};

export const addSlotInstructor = async (slotId: string, userId: string) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/instructors`, {
        method: 'POST',
        body: JSON.stringify({ user_id: userId })
    });
};

export const removeSlotInstructor = async (slotId: string, userId: string) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/instructors/${userId}`, { method: 'DELETE' });
};

export const removeAllSlotInstructors = async (slotId: string) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/instructors/all`, { method: 'DELETE' });
};

export const deleteAllSlotGroups = async (slotId: string) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/groups`, { method: 'DELETE' });
};

export const deleteSlotTimetableEntries = async (slotId: string) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/timetable-entries`, { method: 'DELETE' });
};

// ==========================================
// Slot Classroom Assignments (ครูต่อห้อง — independent)
// ==========================================

export interface SlotClassroomAssignment {
    id: string;
    slot_id: string;
    classroom_id: string;
    instructor_id: string;
    classroom_name?: string;
    instructor_name?: string;
}

export const listSlotClassroomAssignments = async (slotId: string): Promise<{ data: SlotClassroomAssignment[] }> => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/classroom-assignments`);
};

export const batchUpsertSlotClassroomAssignments = async (slotId: string, assignments: { classroom_id: string; instructor_id: string }[]) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/classroom-assignments`, {
        method: 'POST',
        body: JSON.stringify({ assignments })
    });
};

export const deleteSlotClassroomAssignment = async (slotId: string, assignmentId: string) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/classroom-assignments/${assignmentId}`, { method: 'DELETE' });
};

export const deleteAllSlotClassroomAssignments = async (slotId: string) => {
    return await fetchApi(`/api/academic/activity-slots/${slotId}/classroom-assignments/all`, { method: 'DELETE' });
};

// ==========================================
// Activity Groups (กิจกรรมจริง ภายใต้ slot)
// ==========================================

export interface ActivityGroup {
    id: string;
    slot_id?: string;
    name: string;
    description?: string;
    instructor_id?: string;
    max_capacity?: number;
    registration_open: boolean;
    allowed_grade_level_ids?: string[];
    is_active: boolean;
    created_at: string;
    // joined
    instructor_name?: string;
    member_count?: number;
    slot_name?: string;
    activity_type?: string;
    semester_name?: string;
}

export interface ActivityGroupMember {
    id: string;
    activity_group_id: string;
    student_id: string;
    result?: 'pass' | 'fail';
    enrolled_at: string;
    // joined
    student_name?: string;
    student_code?: string;
    classroom_name?: string;
    grade_level_name?: string;
}

export const listActivityGroups = async (filter: {
    slot_id?: string;
    semester_id?: string;
    activity_type?: string;
    instructor_id?: string;
    registration_open?: boolean;
    search?: string;
} = {}): Promise<{ data: ActivityGroup[] }> => {
    const params = new URLSearchParams();
    if (filter.slot_id) params.set('slot_id', filter.slot_id);
    if (filter.semester_id) params.set('semester_id', filter.semester_id);
    if (filter.activity_type) params.set('activity_type', filter.activity_type);
    if (filter.instructor_id) params.set('instructor_id', filter.instructor_id);
    if (filter.registration_open !== undefined) params.set('registration_open', String(filter.registration_open));
    if (filter.search) params.set('search', filter.search);
    return await fetchApi(`/api/academic/activities?${params}`);
};

export const createActivityGroup = async (data: {
    slot_id: string;
    name: string;
    description?: string;
    instructor_id?: string;
    max_capacity?: number;
    allowed_grade_level_ids?: string[];
}) => {
    return await fetchApi('/api/academic/activities', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateActivityGroup = async (id: string, data: Partial<ActivityGroup>) => {
    return await fetchApi(`/api/academic/activities/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

export const deleteActivityGroup = async (id: string) => {
    return await fetchApi(`/api/academic/activities/${id}`, { method: 'DELETE' });
};

export const listActivityMembers = async (groupId: string): Promise<{ data: ActivityGroupMember[] }> => {
    return await fetchApi(`/api/academic/activities/${groupId}/members`);
};

export const addActivityMembers = async (groupId: string, studentIds: string[]) => {
    return await fetchApi(`/api/academic/activities/${groupId}/members`, {
        method: 'POST',
        body: JSON.stringify({ student_ids: studentIds })
    });
};

export const removeActivityMember = async (groupId: string, studentId: string) => {
    return await fetchApi(`/api/academic/activities/${groupId}/members/${studentId}`, { method: 'DELETE' });
};

export const updateMemberResult = async (memberId: string, result: 'pass' | 'fail') => {
    return await fetchApi(`/api/academic/activities/members/${memberId}/result`, {
        method: 'PUT',
        body: JSON.stringify({ result })
    });
};

export interface ActivityInstructor {
    id: string;
    instructor_id: string;
    role: 'primary' | 'assistant';
    instructor_name?: string;
}

export const listActivityInstructors = async (groupId: string): Promise<{ data: ActivityInstructor[] }> => {
    return await fetchApi(`/api/academic/activities/${groupId}/instructors`);
};

export const addActivityInstructor = async (groupId: string, instructorId: string, role: 'primary' | 'assistant') => {
    return await fetchApi(`/api/academic/activities/${groupId}/instructors`, {
        method: 'POST',
        body: JSON.stringify({ instructor_id: instructorId, role })
    });
};

export const removeActivityInstructor = async (groupId: string, instructorId: string) => {
    return await fetchApi(`/api/academic/activities/${groupId}/instructors/${instructorId}`, { method: 'DELETE' });
};

// Student self-enrollment
export const selfEnrollActivity = async (groupId: string) => {
    return await fetchApi(`/api/academic/activities/${groupId}/enroll`, { method: 'POST' });
};

export const selfUnenrollActivity = async (groupId: string) => {
    return await fetchApi(`/api/academic/activities/${groupId}/enroll`, { method: 'DELETE' });
};

export const getMyActivityEnrollments = async (): Promise<{ data: string[] }> => {
    return await fetchApi('/api/academic/activities/my-enrollments');
};

export interface CourseInstructor {
    id: string;
    classroom_course_id: string;
    instructor_id: string;
    role: 'primary' | 'secondary';
    instructor_name?: string;
}

export const listCourseInstructors = async (courseId: string): Promise<{ data: CourseInstructor[] }> => {
    return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors`);
};

export const batchListCourseInstructors = async (courseIds: string[]): Promise<{ data: Record<string, CourseInstructor[]> }> => {
    if (courseIds.length === 0) return { data: {} };
    const params = new URLSearchParams({ course_ids: courseIds.join(',') });
    return await fetchApi(`/api/academic/planning/courses/instructors?${params}`);
};

export const addCourseInstructor = async (
    courseId: string,
    instructorId: string,
    role: 'primary' | 'secondary' = 'secondary'
) => {
    return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors`, {
        method: 'POST',
        body: JSON.stringify({ instructor_id: instructorId, role })
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
    role: 'primary' | 'secondary'
) => {
    return await fetchApi(`/api/academic/planning/courses/${courseId}/instructors/${instructorId}`, {
        method: 'PUT',
        body: JSON.stringify({ role })
    });
};

// ==========================================
// Subject Default Instructors (team teaching at catalog level)
// ครูประจำวิชาใน คลังรายวิชา — auto-copy ไป classroom_course_instructors ตอน assign
// ==========================================

export interface SubjectDefaultInstructor {
    id: string;
    subject_id: string;
    instructor_id: string;
    role: 'primary' | 'secondary';
    instructor_name?: string;
}

export const listSubjectDefaultInstructors = async (
    subjectId: string
): Promise<{ data: SubjectDefaultInstructor[] }> => {
    return await fetchApi(`/api/academic/subjects/${subjectId}/default-instructors`);
};

export const batchListSubjectDefaultInstructors = async (
    subjectIds: string[]
): Promise<{ data: Record<string, SubjectDefaultInstructor[]> }> => {
    if (subjectIds.length === 0) return { data: {} };
    const params = new URLSearchParams({ subject_ids: subjectIds.join(',') });
    return await fetchApi(`/api/academic/subjects/default-instructors?${params}`);
};

export const addSubjectDefaultInstructor = async (
    subjectId: string,
    instructorId: string,
    role: 'primary' | 'secondary' = 'secondary'
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
    role: 'primary' | 'secondary'
) => {
    return await fetchApi(`/api/academic/subjects/${subjectId}/default-instructors/${instructorId}`, {
        method: 'PUT',
        body: JSON.stringify({ role })
    });
};

// ==========================================
// Study Plan Version Activities (template)
// ==========================================

export interface StudyPlanVersionActivity {
    id: string;
    study_plan_version_id: string;
    activity_catalog_id: string;
    /** Pinned term in this plan (snapshot จาก catalog ตอน add). null = ทุกเทอม. */
    term?: string | null;
    display_order: number;
    created_at: string;
    updated_at: string;

    // Joined from catalog (grade scope comes from catalog — no plan override)
    catalog_name?: string;
    catalog_activity_type?: string;
    catalog_description?: string;
    catalog_periods_per_week?: number;
    catalog_scheduling_mode?: string;
    catalog_term?: string; // original catalog.term (for reference/badge only)
    catalog_grade_level_ids?: string[];
}

export const listPlanActivities = async (versionId: string): Promise<{ data: StudyPlanVersionActivity[] }> => {
    return await fetchApi(`/api/academic/study-plan-versions/${versionId}/activities`);
};

export const addPlanActivity = async (versionId: string, data: {
    activity_catalog_id: string;
    /** Override term. Omit to snapshot from catalog.term at insert time. */
    term?: string | null;
    display_order?: number;
}) => {
    return await fetchApi(`/api/academic/study-plan-versions/${versionId}/activities`, {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updatePlanActivity = async (id: string, data: Partial<{
    /** null = ทุกเทอม (always overwrites — caller pass existing value to preserve). */
    term: string | null;
    display_order: number;
}>) => {
    return await fetchApi(`/api/academic/study-plan-activities/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

// ==========================================
// Activity Catalog (คลังกิจกรรม)
// ==========================================

export interface ActivityCatalog {
    id: string;
    name: string;
    start_academic_year_id: string; // version key — name เดียวมีหลาย version แยกตามปี
    activity_type: 'scout' | 'club' | 'guidance' | 'social' | 'other';
    description?: string;
    periods_per_week: number;
    scheduling_mode: 'synchronized' | 'independent';
    is_active: boolean;
    term?: string;
    grade_level_ids?: string[];
    created_at: string;
    updated_at: string;
}

export const listActivityCatalog = async (opts: {
    /** When true (default), return only the latest version per name. */
    latest_only?: boolean;
} = {}): Promise<{ data: ActivityCatalog[] }> => {
    const params = new URLSearchParams();
    if (opts.latest_only === false) params.set('latest_only', 'false');
    const qs = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/activity-catalog${qs}`);
};

export const createActivityCatalog = async (data: {
    name: string;
    start_academic_year_id: string;
    activity_type: string;
    description?: string;
    periods_per_week?: number;
    scheduling_mode?: string;
    term?: string;
    grade_level_ids?: string[];
    /** Default team to insert atomically with catalog creation. */
    default_instructors?: { instructor_id: string; role: 'primary' | 'secondary' }[];
}) => {
    return await fetchApi(`/api/academic/activity-catalog`, {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updateActivityCatalog = async (id: string, data: Partial<ActivityCatalog>) => {
    return await fetchApi(`/api/academic/activity-catalog/${id}`, {
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

export interface CatalogDefaultInstructor {
    id: string;
    catalog_id: string;
    instructor_id: string;
    role: 'primary' | 'secondary';
    instructor_name?: string;
}

export const listCatalogDefaultInstructors = async (
    catalogId: string
): Promise<{ data: CatalogDefaultInstructor[] }> => {
    return await fetchApi(`/api/academic/activity-catalog/${catalogId}/default-instructors`);
};

export const addCatalogDefaultInstructor = async (
    catalogId: string,
    instructorId: string,
    role: 'primary' | 'secondary' = 'secondary'
) => {
    return await fetchApi(`/api/academic/activity-catalog/${catalogId}/default-instructors`, {
        method: 'POST',
        body: JSON.stringify({ instructor_id: instructorId, role })
    });
};

export const removeCatalogDefaultInstructor = async (catalogId: string, instructorId: string) => {
    return await fetchApi(`/api/academic/activity-catalog/${catalogId}/default-instructors/${instructorId}`, {
        method: 'DELETE'
    });
};

export const updateCatalogDefaultInstructorRole = async (
    catalogId: string,
    instructorId: string,
    role: 'primary' | 'secondary'
) => {
    return await fetchApi(`/api/academic/activity-catalog/${catalogId}/default-instructors/${instructorId}`, {
        method: 'PUT',
        body: JSON.stringify({ role })
    });
};

export const deletePlanActivity = async (id: string) => {
    return await fetchApi(`/api/academic/study-plan-activities/${id}`, {
        method: 'DELETE'
    });
};

export const generateActivitiesFromPlan = async (data: {
    study_plan_version_id: string;
    semester_id: string;
}): Promise<{ success: boolean; created: number; skipped: number; total_templates: number }> => {
    return await fetchApi(`/api/academic/activities/generate-from-plan`, {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

// ==========================================
// Classroom Activities (junction-backed)
// หน้า Course Planning = source of truth ต่อห้อง
// ==========================================

export interface ClassroomActivity {
    slot_id: string;
    activity_catalog_id: string;
    name: string;
    activity_type: 'scout' | 'club' | 'guidance' | 'social' | 'other';
    periods_per_week: number;
    scheduling_mode: 'synchronized' | 'independent';
    is_active: boolean;
}

export const listClassroomActivities = async (
    classroomId: string,
    semesterId: string
): Promise<{ data: ClassroomActivity[] }> => {
    const params = new URLSearchParams({ semester_id: semesterId });
    return await fetchApi(`/api/academic/planning/classrooms/${classroomId}/activities?${params}`);
};

export const removeClassroomFromSlot = async (classroomId: string, slotId: string) => {
    return await fetchApi(
        `/api/academic/planning/classrooms/${classroomId}/activities/${slotId}`,
        { method: 'DELETE' }
    );
};

