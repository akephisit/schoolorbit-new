import { PUBLIC_BACKEND_URL } from '$env/static/public';

const BACKEND_URL = PUBLIC_BACKEND_URL || 'https://school-api.schoolorbit.app';

async function fetchApi(path: string, options: RequestInit = {}): Promise<any> {
    const response = await fetch(`${BACKEND_URL}${path}`, {
        ...options,
        credentials: 'include',
        headers: { 'Content-Type': 'application/json', ...options.headers }
    });
    if (!response.ok) {
        const error = await response.json().catch(() => ({ error: 'Unknown error' }));
        throw new Error(error.error || `Request failed with status ${response.status}`);
    }
    return response.json();
}

// ==========================================
// Types
// ==========================================

export type AdmissionStatus = 'draft' | 'open' | 'closed' | 'announced' | 'done';
export type ApplicationStatus =
    | 'pending' | 'reviewing' | 'interview_scheduled'
    | 'accepted' | 'rejected' | 'waitlisted' | 'confirmed' | 'cancelled';

export interface RequiredDocument {
    key: string;
    label: string;
    required: boolean;
}

export interface AdmissionPeriod {
    id: string;
    academic_year_id: string;
    academic_year_name?: string;
    name: string;
    description?: string;
    open_date: string;
    close_date: string;
    announcement_date?: string;
    confirmation_deadline?: string;
    status: AdmissionStatus;
    capacity_per_class?: number;
    total_capacity?: number;
    waitlist_capacity?: number;
    required_documents: RequiredDocument[];
    application_fee?: number;
    // Stats
    application_count?: number;
    pending_count?: number;
    accepted_count?: number;
    confirmed_count?: number;
    created_at: string;
    updated_at: string;
}

export interface AdmissionApplication {
    id: string;
    admission_period_id: string;
    application_number: string;
    applicant_first_name: string;
    applicant_last_name: string;
    applicant_title?: string;
    applicant_national_id?: string;
    applicant_date_of_birth?: string;
    applicant_gender?: string;
    applicant_nationality?: string;
    applicant_religion?: string;
    applicant_blood_type?: string;
    applicant_phone?: string;
    applicant_email?: string;
    applicant_address?: string;
    applicant_photo_url?: string;
    previous_school?: string;
    previous_grade?: string;
    previous_gpa?: number;
    applying_grade_level_id?: string;
    applying_classroom_preference?: string;
    guardian_name?: string;
    guardian_relationship?: string;
    guardian_phone?: string;
    guardian_email?: string;
    guardian_occupation?: string;
    guardian_national_id?: string;
    status: ApplicationStatus;
    staff_notes?: string;
    rejection_reason?: string;
    interview_score?: number;
    exam_score?: number;
    total_score?: number;
    submitted_at?: string;
    reviewed_at?: string;
    // Joined
    grade_level_name?: string;
    period_name?: string;
    reviewer_name?: string;
    created_at: string;
    updated_at: string;
}

export interface AdmissionInterview {
    id: string;
    application_id: string;
    interview_type: string;
    scheduled_at?: string;
    location?: string;
    interviewer_id?: string;
    score?: number;
    max_score?: number;
    notes?: string;
    status: string;
    interviewer_name?: string;
    applicant_name?: string;
    created_at: string;
}

export interface AdmissionSelection {
    id: string;
    application_id: string;
    admission_period_id: string;
    selection_type: 'main' | 'waitlist';
    rank?: number;
    assigned_grade_level_id?: string;
    is_confirmed: boolean;
    confirmed_at?: string;
    confirmation_deadline?: string;
    student_user_id?: string;
    notes?: string;
    // Joined
    applicant_name?: string;
    application_number?: string;
    grade_level_name?: string;
    total_score?: number;
    applying_grade_level_name?: string;
    created_at: string;
    updated_at: string;
}

export interface AdmissionStats {
    period_id: string;
    total: number;
    pending: number;
    reviewing: number;
    accepted: number;
    rejected: number;
    waitlisted: number;
    confirmed: number;
    cancelled: number;
}

// ==========================================
// Period APIs
// ==========================================

export const listAdmissionPeriods = async (filters: {
    academic_year_id?: string;
    status?: string;
} = {}): Promise<{ data: AdmissionPeriod[] }> => {
    const params = new URLSearchParams();
    if (filters.academic_year_id) params.append('academic_year_id', filters.academic_year_id);
    if (filters.status) params.append('status', filters.status);
    const q = params.toString() ? `?${params}` : '';
    return fetchApi(`/api/admission/periods${q}`);
};

export const getAdmissionPeriod = async (id: string): Promise<{ data: AdmissionPeriod }> =>
    fetchApi(`/api/admission/periods/${id}`);

export const createAdmissionPeriod = async (data: {
    academic_year_id: string;
    name: string;
    description?: string;
    open_date: string;
    close_date: string;
    announcement_date?: string;
    confirmation_deadline?: string;
    target_grade_level_ids?: string[];
    capacity_per_class?: number;
    total_capacity?: number;
    waitlist_capacity?: number;
    required_documents?: RequiredDocument[];
    application_fee?: number;
}) => fetchApi('/api/admission/periods', { method: 'POST', body: JSON.stringify(data) });

export const updateAdmissionPeriod = async (id: string, data: Partial<AdmissionPeriod> & { status?: string }) =>
    fetchApi(`/api/admission/periods/${id}`, { method: 'PUT', body: JSON.stringify(data) });

export const deleteAdmissionPeriod = async (id: string) =>
    fetchApi(`/api/admission/periods/${id}`, { method: 'DELETE' });

export const getAdmissionPeriodStats = async (id: string): Promise<{ data: AdmissionStats }> =>
    fetchApi(`/api/admission/periods/${id}/stats`);

// ==========================================
// Application APIs
// ==========================================

export const listApplications = async (filters: {
    admission_period_id?: string;
    status?: string;
    search?: string;
    page?: number;
    page_size?: number;
} = {}): Promise<{ data: AdmissionApplication[]; total: number; page: number; page_size: number; total_pages: number }> => {
    const params = new URLSearchParams();
    if (filters.admission_period_id) params.append('admission_period_id', filters.admission_period_id);
    if (filters.status) params.append('status', filters.status);
    if (filters.search) params.append('search', filters.search);
    if (filters.page) params.append('page', String(filters.page));
    if (filters.page_size) params.append('page_size', String(filters.page_size));
    const q = params.toString() ? `?${params}` : '';
    return fetchApi(`/api/admission/applications${q}`);
};

export const getApplication = async (id: string): Promise<{
    data: AdmissionApplication;
    documents: any[];
    interviews: AdmissionInterview[];
}> => fetchApi(`/api/admission/applications/${id}`);

export const createApplication = async (data: Partial<AdmissionApplication>) =>
    fetchApi('/api/admission/applications', { method: 'POST', body: JSON.stringify(data) });

export const updateApplicationStatus = async (id: string, data: {
    status: ApplicationStatus;
    staff_notes?: string;
    rejection_reason?: string;
    interview_score?: number;
    exam_score?: number;
    total_score?: number;
}) => fetchApi(`/api/admission/applications/${id}`, { method: 'PUT', body: JSON.stringify(data) });

export const getApplicationLogs = async (id: string): Promise<{ data: any[] }> =>
    fetchApi(`/api/admission/applications/${id}/logs`);

// ==========================================
// Interview APIs
// ==========================================

export const createInterview = async (data: {
    application_id: string;
    interview_type?: string;
    scheduled_at?: string;
    location?: string;
    interviewer_id?: string;
    max_score?: number;
}) => fetchApi('/api/admission/interviews', { method: 'POST', body: JSON.stringify(data) });

export const updateInterview = async (id: string, data: Partial<AdmissionInterview>) =>
    fetchApi(`/api/admission/interviews/${id}`, { method: 'PUT', body: JSON.stringify(data) });

// ==========================================
// Selection APIs
// ==========================================

export const listSelections = async (periodId: string): Promise<{ data: AdmissionSelection[] }> =>
    fetchApi(`/api/admission/periods/${periodId}/selections`);

export const createSelections = async (periodId: string, data: {
    application_ids: string[];
    selection_type?: 'main' | 'waitlist';
    confirmation_deadline?: string;
}) => fetchApi(`/api/admission/periods/${periodId}/selections`, { method: 'POST', body: JSON.stringify(data) });

export const confirmSelection = async (selectionId: string) =>
    fetchApi(`/api/admission/selections/${selectionId}/confirm`, { method: 'POST' });

// ==========================================
// Generate Students
// ==========================================

export const generateStudents = async (periodId: string, data: {
    selection_ids?: string[];
    classroom_id?: string;
    password_prefix?: string;
}) => fetchApi(`/api/admission/periods/${periodId}/generate-students`, { method: 'POST', body: JSON.stringify(data) });

// ==========================================
// Helpers
// ==========================================

export const APPLICATION_STATUS_LABELS: Record<ApplicationStatus, string> = {
    pending: 'รอพิจารณา',
    reviewing: 'กำลังพิจารณา',
    interview_scheduled: 'นัดสัมภาษณ์',
    accepted: 'ผ่านการคัดเลือก',
    rejected: 'ไม่ผ่าน',
    waitlisted: 'รายชื่อสำรอง',
    confirmed: 'ยืนยันสิทธิ์แล้ว',
    cancelled: 'ยกเลิก',
};

export const APPLICATION_STATUS_COLORS: Record<ApplicationStatus, string> = {
    pending: 'bg-yellow-100 text-yellow-800 border-yellow-200',
    reviewing: 'bg-blue-100 text-blue-800 border-blue-200',
    interview_scheduled: 'bg-purple-100 text-purple-800 border-purple-200',
    accepted: 'bg-green-100 text-green-800 border-green-200',
    rejected: 'bg-red-100 text-red-800 border-red-200',
    waitlisted: 'bg-orange-100 text-orange-800 border-orange-200',
    confirmed: 'bg-emerald-100 text-emerald-800 border-emerald-200',
    cancelled: 'bg-gray-100 text-gray-600 border-gray-200',
};

export const PERIOD_STATUS_LABELS: Record<AdmissionStatus, string> = {
    draft: 'ร่าง',
    open: 'เปิดรับสมัคร',
    closed: 'ปิดรับสมัคร',
    announced: 'ประกาศผลแล้ว',
    done: 'เสร็จสิ้น',
};

export const PERIOD_STATUS_COLORS: Record<AdmissionStatus, string> = {
    draft: 'bg-gray-100 text-gray-700',
    open: 'bg-green-100 text-green-800',
    closed: 'bg-yellow-100 text-yellow-800',
    announced: 'bg-blue-100 text-blue-800',
    done: 'bg-purple-100 text-purple-800',
};
