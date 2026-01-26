import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

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

// ============================================
// Types
// ============================================

export interface AcademicPeriod {
    id: string;
    academic_year_id: string;
    name: string;
    start_time: string;  // "HH:MM:SS"
    end_time: string;
    order_index: number;
    applicable_days?: string;
    is_active: boolean;
    created_at?: string;
    updated_at?: string;
}

export interface TimetableEntry {
    id: string;
    classroom_course_id?: string;
    day_of_week: 'MON' | 'TUE' | 'WED' | 'THU' | 'FRI' | 'SAT' | 'SUN';
    period_id: string;
    room_id?: string;
    note?: string;
    is_active: boolean;

    // New fields
    entry_type: 'COURSE' | 'BREAK' | 'ACTIVITY' | 'HOMEROOM';
    title?: string;
    classroom_id: string;
    academic_semester_id: string;

    // Joined fields
    subject_code?: string;
    subject_name_th?: string;
    instructor_name?: string;
    classroom_name?: string;
    room_code?: string;
    subject_name_en?: string;
    period_name?: string;
    start_time?: string;
    end_time?: string;
}

export interface CreatePeriodRequest {
    academic_year_id: string;
    name: string;
    start_time: string;  // "HH:MM"
    end_time: string;

    order_index: number;
    applicable_days?: string;
}

export interface CreateTimetableEntryRequest {
    classroom_course_id: string;
    day_of_week: string;
    period_id: string;
    room_id?: string;
    note?: string;
}

export interface CreateBatchTimetableEntriesRequest {
    classroom_ids: string[];
    day_of_week: string;
    period_id: string;
    academic_semester_id: string;
    entry_type: 'ACTIVITY' | 'BREAK' | 'HOMEROOM';
    title: string;
    room_id?: string;
    note?: string;
    subject_id?: string;
    force?: boolean;
}

export interface ConflictInfo {
    conflict_type: string;
    message: string;
    existing_entry?: TimetableEntry;
}

export interface TimetableValidationResponse {
    is_valid: boolean;
    conflicts: ConflictInfo[];
}

// ============================================
// Period API
// ============================================

export const listPeriods = async (filters: {
    academic_year_id?: string;

    active_only?: boolean;
} = {}): Promise<{ data: AcademicPeriod[] }> => {
    const params = new URLSearchParams();
    if (filters.academic_year_id) params.append('academic_year_id', filters.academic_year_id);

    if (filters.active_only !== undefined) params.append('active_only', String(filters.active_only));

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/periods${queryString}`);
};

export const createPeriod = async (data: CreatePeriodRequest) => {
    return await fetchApi('/api/academic/periods', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export const updatePeriod = async (id: string, data: Partial<CreatePeriodRequest>) => {
    return await fetchApi(`/api/academic/periods/${id}`, {
        method: 'PUT',
        body: JSON.stringify(data)
    });
};

export const deletePeriod = async (id: string) => {
    return await fetchApi(`/api/academic/periods/${id}`, { method: 'DELETE' });
};

// ============================================
// Timetable API
// ============================================

export const listTimetableEntries = async (filters: {
    classroom_id?: string;
    instructor_id?: string;
    room_id?: string;
    academic_semester_id?: string;
    day_of_week?: string;
    entry_type?: string;
} = {}): Promise<{ data: TimetableEntry[] }> => {
    const params = new URLSearchParams();
    if (filters.classroom_id) params.append('classroom_id', filters.classroom_id);
    if (filters.instructor_id) params.append('instructor_id', filters.instructor_id);
    if (filters.room_id) params.append('room_id', filters.room_id);
    if (filters.academic_semester_id) params.append('academic_semester_id', filters.academic_semester_id);
    if (filters.day_of_week) params.append('day_of_week', filters.day_of_week);
    if (filters.entry_type) params.append('entry_type', filters.entry_type);

    const queryString = params.toString() ? `?${params.toString()}` : '';
    return await fetchApi(`/api/academic/timetable${queryString}`);
};

export const createBatchTimetableEntries = async (data: CreateBatchTimetableEntriesRequest) => {
    return await fetchApi('/api/academic/timetable/batch', {
        method: 'POST',
        body: JSON.stringify(data)
    });
};

export interface UpdateTimetableEntryRequest {
    day_of_week?: string;
    period_id?: string;
    room_id?: string;
    note?: string;
}

export const createTimetableEntry = async (data: CreateTimetableEntryRequest) => {
    const response = await fetch(`${BACKEND_URL}/api/academic/timetable`, {
        method: 'POST',
        credentials: 'include',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(data)
    });

    const result = await response.json();

    // Handle 409 Conflict specially
    if (response.status === 409) {
        return {
            success: false,
            conflicts: result.conflicts || [],
            message: result.message || 'พบข้อขัดแย้งในตาราง'
        };
    }

    if (!response.ok) {
        throw new Error(result.error || `Request failed with status ${response.status}`);
    }

    return result;
};

export const updateTimetableEntry = async (id: string, data: UpdateTimetableEntryRequest) => {
    const response = await fetch(`${BACKEND_URL}/api/academic/timetable/${id}`, {
        method: 'PUT',
        credentials: 'include',
        headers: {
            'Content-Type': 'application/json',
        },
        body: JSON.stringify(data)
    });

    const result = await response.json();

    // Handle 409 Conflict specially
    if (response.status === 409) {
        return {
            success: false,
            conflicts: result.conflicts || [],
            message: result.message || 'พบข้อขัดแย้งในตาราง'
        };
    }

    if (!response.ok) {
        throw new Error(result.error || `Request failed with status ${response.status}`);
    }

    return result;
};

export const deleteTimetableEntry = async (id: string) => {
    return await fetchApi(`/api/academic/timetable/${id}`, { method: 'DELETE' });
};
