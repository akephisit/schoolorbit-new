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
}) => {
    return await fetchApi('/api/academic/classrooms', {
        method: 'POST',
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
    academic_year_id: string; // UUID FK to academic_years
    name_th: string;
    name_en?: string;
    credit: number;
    hours_per_semester?: number;
    type: 'BASIC' | 'ADDITIONAL' | 'ACTIVITY';
    group_id?: string;
    level_scope?: string;
    description?: string;
    is_active: boolean;
    group_name_th?: string;
    start_academic_year_id?: string;
    grade_level_ids?: string[];
    term?: string;
    default_instructor_id?: string;
    default_instructor_name?: string;
}

export const listSubjectGroups = async (): Promise<{ data: SubjectGroup[] }> => {
    return await fetchApi('/api/academic/subjects/groups');
};

export const listSubjects = async (filters: {
    group_id?: string;
    level_scope?: string;
    subject_type?: string;
    search?: string;
    active_only?: boolean;
    academic_year_id?: string;
    term?: string;
} = {}): Promise<{ data: Subject[] }> => {
    const params = new URLSearchParams();
    if (filters.group_id) params.append('group_id', filters.group_id);
    if (filters.level_scope) params.append('level_scope', filters.level_scope);
    if (filters.subject_type) params.append('subject_type', filters.subject_type);
    if (filters.search) params.append('search', filters.search);
    if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));
    if (filters.academic_year_id) params.append('academic_year_id', filters.academic_year_id);
    if (filters.term) params.append('term', filters.term);

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

export const bulkCopySubjects = async (sourceYearId: string, targetYearId: string) => {
    return await fetchApi('/api/academic/subjects/bulk-copy', {
        method: 'POST',
        body: JSON.stringify({
            source_academic_year_id: sourceYearId,
            target_academic_year_id: targetYearId
        })
    });
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
