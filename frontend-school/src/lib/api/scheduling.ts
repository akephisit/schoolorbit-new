import { apiClient } from './client';
import type { UUID } from '$lib/types';

// ==================== Types ====================

export type SchedulingAlgorithm = 'GREEDY' | 'BACKTRACKING' | 'HYBRID';
export type SchedulingStatus = 'PENDING' | 'RUNNING' | 'COMPLETED' | 'FAILED' | 'CANCELLED';
export type LockedSlotScope = 'CLASSROOM' | 'GRADE_LEVEL' | 'ALL_SCHOOL';

export interface SchedulingConfig {
    force_overwrite?: boolean;
    respect_preferences?: boolean;
    allow_partial?: boolean;
    min_quality_score?: number;
    timeout_seconds?: number;
    allow_multiple_sessions_per_day?: boolean;
}

export interface CreateSchedulingJobRequest {
    academic_semester_id: UUID;
    classroom_ids: UUID[];
    algorithm?: SchedulingAlgorithm;
    config?: SchedulingConfig;
}

export interface FailedCourse {
    course_id: UUID;
    subject_code: string;
    subject_name: string;
    classroom: string;
    reason: string;
}

export interface SchedulingJobResponse {
    id: UUID;
    academic_semester_id: UUID;
    classroom_ids: UUID[];
    algorithm: SchedulingAlgorithm;
    status: SchedulingStatus;
    progress: number;
    quality_score?: number;
    scheduled_courses: number;
    total_courses: number;
    failed_courses: FailedCourse[];
    started_at?: string;
    completed_at?: string;
    duration_seconds?: number;
    error_message?: string;
    created_by?: UUID;
    created_at: string;
}

export interface TimeSlot {
    day: string;
    period_id: UUID;
}

export interface InstructorPreference {
    id: UUID;
    instructor_id: UUID;
    academic_year_id: UUID;
    hard_unavailable_slots: TimeSlot[];
    preferred_slots: TimeSlot[];
    max_periods_per_day?: number;
    min_periods_per_day?: number;
    preferred_days?: string[];
    avoid_days?: string[];
    notes?: string;
}

export interface CreateInstructorPreferenceRequest {
    instructor_id: UUID;
    academic_year_id: UUID;
    hard_unavailable_slots?: TimeSlot[];
    preferred_slots?: TimeSlot[];
    max_periods_per_day?: number;
    min_periods_per_day?: number;
    preferred_days?: string[];
    avoid_days?: string[];
    notes?: string;
}

export interface InstructorRoomAssignment {
    id: UUID;
    instructor_id: UUID;
    room_id: UUID;
    academic_year_id: UUID;
    is_preferred?: boolean;
    is_required?: boolean;
    for_subjects?: string[];
    reason?: string;
}

export interface CreateInstructorRoomAssignmentRequest {
    instructor_id: UUID;
    room_id: UUID;
    academic_year_id: UUID;
    is_preferred?: boolean;
    is_required?: boolean;
    for_subjects?: string[];
    reason?: string;
}

export interface LockedSlot {
    id: UUID;
    academic_semester_id: UUID;
    scope_type: LockedSlotScope;
    scope_ids?: UUID[];
    subject_id: UUID;
    day_of_week: string;
    period_ids: UUID[];
    room_id?: UUID;
    instructor_id?: UUID;
    reason?: string;
    locked_by?: UUID;
}

export interface CreateLockedSlotRequest {
    academic_semester_id: UUID;
    scope_type: LockedSlotScope;
    scope_ids?: UUID[];
    subject_id: UUID;
    day_of_week: string;
    period_ids: UUID[];
    room_id?: UUID;
    instructor_id?: UUID;
    reason?: string;
}

// ==================== Scheduling Constraints ====================

export interface InstructorConstraintView {
    id: UUID;
    first_name: string;
    last_name: string;
    short_name?: string;
    max_periods_per_day?: number;
    hard_unavailable_slots?: any; // JSON
    preferred_slots?: any; // JSON
    assigned_room_id?: UUID;
    assigned_room_name?: string;
}

export interface UpdateInstructorConstraintRequest {
    max_periods_per_day?: number;
    hard_unavailable_slots?: any;
    preferred_slots?: any;
    assigned_room_id?: UUID;
}

export interface SubjectConstraintView {
    id: UUID;
    code: string;
    name: string;
    min_consecutive_periods?: number; // 
    max_consecutive_periods?: number;
    preferred_time_of_day?: string; // MORNING, AFTERNOON
    required_room_type?: string;
    periods_per_week?: number;
}

export interface UpdateSubjectConstraintRequest {
    min_consecutive_periods?: number;
    max_consecutive_periods?: number;
    preferred_time_of_day?: string;
    required_room_type?: string;
    periods_per_week?: number;
}

// Constraints API
export async function listInstructorConstraints() {
    return apiClient.get<InstructorConstraintView[]>('/api/academic/scheduling/instructors');
}

export async function updateInstructorConstraints(id: UUID, req: UpdateInstructorConstraintRequest) {
    return apiClient.put<any>(`/api/academic/scheduling/instructors/${id}`, req);
}

export async function listSubjectConstraints() {
    return apiClient.get<SubjectConstraintView[]>('/api/academic/scheduling/subjects');
}

export async function updateSubjectConstraints(id: UUID, req: UpdateSubjectConstraintRequest) {
    return apiClient.put<any>(`/api/academic/scheduling/subjects/${id}`, req);
}

// ==================== Legacy / Other API Functions (Keep if needed) ====================
// ... (Previous job APIs)
export async function autoScheduleTimetable(request: CreateSchedulingJobRequest) {
    return apiClient.post<{ job_id: UUID; status: string; message: string }>(
        '/api/academic/scheduling/auto-schedule',
        request
    );
}
// ...

export async function getSchedulingJob(jobId: UUID) {
    return apiClient.get<SchedulingJobResponse>(`/api/academic/scheduling/jobs/${jobId}`);
}

export async function listSchedulingJobs(params?: { semester_id?: UUID; limit?: number }) {
    const queryParams = new URLSearchParams();
    if (params?.semester_id) queryParams.append('semester_id', params.semester_id);
    if (params?.limit) queryParams.append('limit', params.limit.toString());
    const query = queryParams.toString();
    return apiClient.get<SchedulingJobResponse[]>(
        `/api/academic/scheduling/jobs${query ? `?${query}` : ''}`
    );
}

// Instructor Preferences (Legacy/Direct)
export async function createInstructorPreference(request: CreateInstructorPreferenceRequest) {
    return apiClient.post<InstructorPreference>('/api/academic/instructor-preferences', request);
}

export async function updateInstructorPreference(
    id: UUID,
    request: Partial<CreateInstructorPreferenceRequest>
) {
    return apiClient.put<InstructorPreference>(`/api/academic/instructor-preferences/${id}`, request);
}

export async function getInstructorPreference(instructorId: UUID, academicYearId: UUID) {
    const query = new URLSearchParams({
        instructor_id: instructorId,
        academic_year_id: academicYearId
    }).toString();
    return apiClient.get<InstructorPreference>(`/api/academic/instructor-preferences?${query}`);
}

// Instructor Room Assignments (Legacy/Direct)
export async function createInstructorRoomAssignment(request: CreateInstructorRoomAssignmentRequest) {
    return apiClient.post<InstructorRoomAssignment>('/api/academic/instructor-rooms', request);
}

export async function listInstructorRoomAssignments(params?: {
    instructor_id?: UUID;
    academic_year_id?: UUID;
}) {
    const queryParams = new URLSearchParams();
    if (params?.instructor_id) queryParams.append('instructor_id', params.instructor_id);
    if (params?.academic_year_id) queryParams.append('academic_year_id', params.academic_year_id);
    const query = queryParams.toString();
    return apiClient.get<InstructorRoomAssignment[]>(
        `/api/academic/instructor-rooms${query ? `?${query}` : ''}`
    );
}

export async function deleteInstructorRoomAssignment(id: UUID) {
    return apiClient.delete(`/api/academic/instructor-rooms/${id}`);
}

// Locked Slots
export async function createLockedSlot(request: CreateLockedSlotRequest) {
    return apiClient.post<LockedSlot>('/api/academic/timetable/locked-slots', request);
}

export async function listLockedSlots(params?: { semester_id?: UUID }) {
    const query = params?.semester_id ? `?semester_id=${params.semester_id}` : '';
    return apiClient.get<LockedSlot[]>(`/api/academic/timetable/locked-slots${query}`);
}

export async function deleteLockedSlot(id: UUID) {
    return apiClient.delete(`/api/academic/timetable/locked-slots/${id}`);
}
