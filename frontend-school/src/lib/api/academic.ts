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
} = {}): Promise<{ data: Subject[] }> => {
    const params = new URLSearchParams();
    if (filters.group_id) params.append('group_id', filters.group_id);
    if (filters.level_scope) params.append('level_scope', filters.level_scope);
    if (filters.subject_type) params.append('subject_type', filters.subject_type);
    if (filters.search) params.append('search', filters.search);
    if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));

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

export const lookupGradeLevels = async (): Promise<{ data: LookupItem[] }> => {
    return await fetchApi('/api/lookup/grade-levels');
};

export const lookupAcademicYears = async (): Promise<{ data: LookupItem[] }> => {
    return await fetchApi('/api/lookup/academic-years');
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
