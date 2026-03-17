import { apiClient } from './client';

// ==========================================
// Types
// ==========================================

export interface AdmissionRound {
    id: string;
    academicYearId: string;
    gradeLevelId: string;
    name: string;
    description?: string;
    applyStartDate: string;
    applyEndDate: string;
    examDate?: string;
    resultAnnounceDate?: string;
    enrollmentStartDate?: string;
    enrollmentEndDate?: string;
    status: 'draft' | 'open' | 'exam' | 'scoring' | 'announced' | 'enrolling' | 'closed';
    createdAt: string;
    updatedAt: string;
    // Joined
    academicYearName?: string;
    gradeLevelName?: string;
    applicationCount?: number;
}

export interface AdmissionTrack {
    id: string;
    admissionRoundId: string;
    studyPlanId: string;
    name: string;
    capacityOverride?: number;
    scoringSubjectIds: string[];
    tiebreakMethod: 'applied_at' | 'gpa';
    displayOrder: number;
    createdAt: string;
    // Joined/computed
    studyPlanName?: string;
    computedCapacity?: number;
    roomCount?: number;
    applicationCount?: number;
}

export interface AdmissionExamSubject {
    id: string;
    admissionRoundId: string;
    name: string;
    code?: string;
    maxScore: number;
    displayOrder: number;
    createdAt: string;
}

export interface AdmissionApplication {
    id: string;
    admissionRoundId: string;
    admissionTrackId: string;
    applicationNumber?: string;
    nationalId: string;
    title?: string;
    firstName: string;
    lastName: string;
    gender?: string;
    dateOfBirth?: string;
    phone?: string;
    email?: string;
    addressLine?: string;
    subDistrict?: string;
    district?: string;
    province?: string;
    postalCode?: string;
    previousSchool?: string;
    previousGrade?: string;
    previousGpa?: number;
    fatherName?: string;
    fatherPhone?: string;
    fatherOccupation?: string;
    fatherNationalId?: string;
    motherName?: string;
    motherPhone?: string;
    motherOccupation?: string;
    motherNationalId?: string;
    guardianName?: string;
    guardianPhone?: string;
    guardianRelation?: string;
    guardianNationalId?: string;
    status: string;
    verifiedBy?: string;
    verifiedAt?: string;
    rejectionReason?: string;
    enrolledBy?: string;
    enrolledAt?: string;
    createdUserId?: string;
    createdAt: string;
    updatedAt: string;
    // Joined
    trackName?: string;
    roundName?: string;
}

export interface ApplicationListItem {
    id: string;
    applicationNumber?: string;
    nationalId: string;
    fullName: string;
    trackName?: string;
    status: string;
    phone?: string;
    previousSchool?: string;
    previousGpa?: number;
    createdAt: string;
}

export interface ExamScore {
    id: string;
    applicationId: string;
    examSubjectId: string;
    score?: number;
    subjectName?: string;
    subjectCode?: string;
    maxScore?: number;
}

export interface RoomAssignment {
    rankInTrack?: number;
    rankInRoom?: number;
    totalScore?: number;
    roomName?: string;
    studentConfirmed: boolean;
}

export interface RankingEntry {
    applicationId: string;
    applicationNumber?: string;
    nationalId: string;
    fullName: string;
    selectionScore: number;
    totalScore: number;
    selectionRank: number;
    finalRank?: number;
    assignedRoom?: string;
    assignedRoomId?: string;
    isOverflow: boolean;
}

export interface TrackRankingResult {
    trackId: string;
    trackName: string;
    rooms: { roomId: string; roomName: string; capacity: number }[];
    applications: RankingEntry[];
}

export interface EnrollmentForm {
    id: string;
    applicationId: string;
    formData: Record<string, unknown>;
    preSubmittedAt?: string;
    completedAt?: string;
}

// ==========================================
// Rounds API
// ==========================================

export async function listRounds() {
    const res = await apiClient.get<AdmissionRound[]>('/api/admission/rounds');
    if (!res.success) throw new Error(res.error || 'ไม่สามารถโหลดรอบรับสมัครได้');
    return res.data ?? [];
}

export async function getRound(id: string) {
    const res = await apiClient.get<AdmissionRound>(`/api/admission/rounds/${id}`);
    if (!res.success) throw new Error(res.error || 'ไม่พบรอบรับสมัคร');
    return res.data!;
}

export async function createRound(data: {
    academicYearId: string;
    gradeLevelId: string;
    name: string;
    description?: string;
    applyStartDate: string;
    applyEndDate: string;
    examDate?: string;
    resultAnnounceDate?: string;
    enrollmentStartDate?: string;
    enrollmentEndDate?: string;
}) {
    const res = await apiClient.post<AdmissionRound>('/api/admission/rounds', data);
    if (!res.success) throw new Error(res.error || 'ไม่สามารถสร้างรอบได้');
    return res.data!;
}

export async function updateRound(id: string, data: Partial<AdmissionRound>) {
    const res = await apiClient.put<AdmissionRound>(`/api/admission/rounds/${id}`, data);
    if (!res.success) throw new Error(res.error || 'ไม่สามารถอัปเดตรอบได้');
    return res.data!;
}

export async function updateRoundStatus(
    id: string,
    status: AdmissionRound['status']
) {
    const res = await apiClient.put(`/api/admission/rounds/${id}/status`, { status });
    if (!res.success) throw new Error(res.error || 'ไม่สามารถอัปเดตสถานะได้');
    return res;
}

export async function deleteRound(id: string) {
    const res = await apiClient.delete(`/api/admission/rounds/${id}`);
    if (!res.success) throw new Error(res.error || 'ไม่สามารถลบรอบได้');
}

// ==========================================
// Tracks API
// ==========================================

export async function listTracks(roundId: string) {
    const res = await apiClient.get<AdmissionTrack[]>(`/api/admission/rounds/${roundId}/tracks`);
    if (!res.success) throw new Error(res.error);
    return res.data ?? [];
}

export async function createTrack(roundId: string, data: {
    studyPlanId: string;
    name: string;
    capacityOverride?: number;
    scoringSubjectIds?: string[];
    tiebreakMethod?: string;
    displayOrder?: number;
}) {
    const res = await apiClient.post<AdmissionTrack>(`/api/admission/rounds/${roundId}/tracks`, data);
    if (!res.success) throw new Error(res.error);
    return res.data!;
}

export async function updateTrack(id: string, data: Partial<AdmissionTrack>) {
    const res = await apiClient.put<AdmissionTrack>(`/api/admission/tracks/${id}`, data);
    if (!res.success) throw new Error(res.error);
    return res.data!;
}

export async function deleteTrack(id: string) {
    const res = await apiClient.delete(`/api/admission/tracks/${id}`);
    if (!res.success) throw new Error(res.error);
}

export async function getTrackCapacity(id: string) {
    const res = await apiClient.get<{ rooms: { roomId: string; roomName: string; roomCode: string }[]; roomCount: number }>(`/api/admission/tracks/${id}/capacity`);
    if (!res.success) throw new Error(res.error);
    return res.data!;
}

// ==========================================
// Exam Subjects API
// ==========================================

export async function listSubjects(roundId: string) {
    const res = await apiClient.get<AdmissionExamSubject[]>(`/api/admission/rounds/${roundId}/subjects`);
    if (!res.success) throw new Error(res.error);
    return res.data ?? [];
}

export async function createSubject(roundId: string, data: {
    name: string;
    code?: string;
    maxScore?: number;
    displayOrder?: number;
}) {
    const res = await apiClient.post<AdmissionExamSubject>(`/api/admission/rounds/${roundId}/subjects`, data);
    if (!res.success) throw new Error(res.error);
    return res.data!;
}

export async function updateSubject(id: string, data: Partial<AdmissionExamSubject>) {
    const res = await apiClient.put<AdmissionExamSubject>(`/api/admission/subjects/${id}`, data);
    if (!res.success) throw new Error(res.error);
    return res.data!;
}

export async function deleteSubject(id: string) {
    const res = await apiClient.delete(`/api/admission/subjects/${id}`);
    if (!res.success) throw new Error(res.error);
}

// ==========================================
// Applications API
// ==========================================

export async function listApplications(
    roundId: string,
    filter?: { status?: string; trackId?: string; search?: string }
) {
    let url = `/api/admission/rounds/${roundId}/applications?`;
    if (filter?.status) url += `status=${filter.status}&`;
    if (filter?.trackId) url += `track_id=${filter.trackId}&`;
    if (filter?.search) url += `search=${encodeURIComponent(filter.search)}&`;
    const res = await apiClient.get<ApplicationListItem[]>(url);
    if (!res.success) throw new Error(res.error);
    return res.data ?? [];
}

export async function getApplication(id: string) {
    const res = await apiClient.get<AdmissionApplication>(`/api/admission/applications/${id}`);
    if (!res.success) throw new Error(res.error);
    return res.data!;
}

export async function verifyApplication(id: string) {
    const res = await apiClient.put(`/api/admission/applications/${id}/verify`, {});
    if (!res.success) throw new Error(res.error);
}

export async function rejectApplication(id: string, rejectionReason: string) {
    const res = await apiClient.put(`/api/admission/applications/${id}/reject`, { rejectionReason });
    if (!res.success) throw new Error(res.error);
}

// ==========================================
// Scores API
// ==========================================

export async function getAllScores(roundId: string) {
    const res = await apiClient.get(`/api/admission/rounds/${roundId}/scores`);
    if (!res.success) throw new Error(res.error);
    return res.data;
}

export async function getApplicationScores(id: string) {
    const res = await apiClient.get<ExamScore[]>(`/api/admission/applications/${id}/scores`);
    if (!res.success) throw new Error(res.error);
    return res.data ?? [];
}

export async function updateScores(
    id: string,
    scores: { examSubjectId: string; score?: number }[]
) {
    const res = await apiClient.put(`/api/admission/applications/${id}/scores`, { scores });
    if (!res.success) throw new Error(res.error);
}

export async function bulkUpdateScores(
    roundId: string,
    entries: { applicationId: string; scores: { examSubjectId: string; score?: number }[] }[]
) {
    const res = await apiClient.put(`/api/admission/rounds/${roundId}/scores/bulk`, { entries });
    if (!res.success) throw new Error(res.error);
}

// ==========================================
// Selections API
// ==========================================

export async function getRanking(roundId: string) {
    const res = await apiClient.get<unknown[]>(`/api/admission/rounds/${roundId}/ranking`);
    if (!res.success) throw new Error(res.error);
    return res.data ?? [];
}

export async function getTrackRanking(trackId: string, selectionSubjectIds?: string[]) {
    let url = `/api/admission/tracks/${trackId}/ranking`;
    if (selectionSubjectIds && selectionSubjectIds.length > 0) {
        url += `?selection_subject_ids=${selectionSubjectIds.join(',')}`;
    }
    const res = await apiClient.get<TrackRankingResult>(url);
    if (!res.success) throw new Error(res.error);
    return res.data!;
}

export async function assignRooms(roundId: string, trackId: string, selectionSubjectIds?: string[]) {
    const res = await apiClient.post(`/api/admission/rounds/${roundId}/assign-rooms`, {
        trackId,
        selectionSubjectIds: selectionSubjectIds && selectionSubjectIds.length > 0 ? selectionSubjectIds : undefined,
    });
    if (!res.success) throw new Error(res.error);
    return res;
}

export async function changeApplicationTrack(applicationId: string, trackId: string) {
    const res = await apiClient.patch(`/api/admission/applications/${applicationId}/track`, { trackId });
    if (!res.success) throw new Error(res.error);
}

// ==========================================
// Enrollment API
// ==========================================

export async function listEnrollmentPending(roundId: string) {
    const res = await apiClient.get(`/api/admission/rounds/${roundId}/enrollment`);
    if (!res.success) throw new Error(res.error);
    return res.data;
}

export async function completeEnrollment(
    id: string,
    studentCode?: string,
    formData?: Record<string, unknown>
) {
    const res = await apiClient.post(`/api/admission/applications/${id}/enroll`, {
        studentCode,
        formData
    });
    if (!res.success) throw new Error(res.error);
    return res.data;
}

// ==========================================
// Portal API (Stateless — ส่ง credentials ทุก request)
// ==========================================

export async function portalCheck(nationalId: string, dateOfBirth: string) {
    const res = await apiClient.post('/api/admission/portal/check', {
        nationalId,
        dateOfBirth
    });
    if (!res.success) throw new Error(res.error || 'ข้อมูลไม่ถูกต้อง');
    return res.data;
}

export async function portalGetStatus(nationalId: string, dateOfBirth: string): Promise<any> {
    const res = await apiClient.post('/api/admission/portal/status', {
        nationalId,
        dateOfBirth
    });
    if (!res.success) throw new Error(res.error);
    return res.data;
}

export async function portalConfirm(nationalId: string, dateOfBirth: string) {
    const res = await apiClient.post('/api/admission/portal/confirm', {
        nationalId,
        dateOfBirth
    });
    if (!res.success) throw new Error(res.error);
    return res;
}

export async function portalGetForm(nationalId: string, dateOfBirth: string) {
    const res = await apiClient.post('/api/admission/portal/form', {
        nationalId,
        dateOfBirth
    });
    if (!res.success) throw new Error(res.error);
    return res.data;
}

export async function portalSubmitForm(
    nationalId: string,
    dateOfBirth: string,
    formData: Record<string, unknown>
) {
    const res = await apiClient.put('/api/admission/portal/form', {
        nationalId,
        dateOfBirth,
        formData
    });
    if (!res.success) throw new Error(res.error);
    return res;
}

export async function updateApplication(
    authNationalId: string,
    authDateOfBirth: string,
    data: any
) {
    const res = await apiClient.put('/api/admission/portal/application', {
        authNationalId,
        authDateOfBirth,
        ...data
    });
    if (!res.success) throw new Error(res.error);
    return res;
}

// ==========================================
// Public Submit API
// ==========================================

export async function getPublicRounds() {
    const res = await apiClient.get<AdmissionRound[]>('/api/admission/apply/rounds');
    if (!res.success) throw new Error(res.error || 'ไม่สามารถโหลดรอบการรับสมัครได้');
    return res.data ?? [];
}


export async function getPublicRoundInfo(roundId: string) {
    const res = await apiClient.get<{ round: AdmissionRound; tracks: AdmissionTrack[] }>(`/api/admission/apply/round/${roundId}`);
    if (!res.success) throw new Error(res.error || 'ไม่สามารถดึงข้อมูลรอบการรับสมัครได้');
    return res.data!;
}

export async function submitApplication(roundId: string, data: {
    admissionTrackId: string;
    nationalId: string;
    title?: string;
    firstName: string;
    lastName: string;
    gender?: string;
    dateOfBirth?: string;
    phone?: string;
    email?: string;
    addressLine?: string;
    subDistrict?: string;
    district?: string;
    province?: string;
    postalCode?: string;
    previousSchool?: string;
    previousGrade?: string;
    previousGpa?: number;
    fatherName?: string;
    fatherPhone?: string;
    fatherOccupation?: string;
    fatherNationalId?: string;
    motherName?: string;
    motherPhone?: string;
    motherOccupation?: string;
    motherNationalId?: string;
    guardianName?: string;
    guardianPhone?: string;
    guardianRelation?: string;
    guardianNationalId?: string;
}) {
    const res = await apiClient.post<{ applicationNumber: string }>(`/api/admission/apply/${roundId}`, data);
    if (!res.success) throw new Error(res.error || 'ไม่สามารถยื่นใบสมัครได้');
    return res.data!;
}

// ==========================================
// Helpers
// ==========================================

export const roundStatusLabel: Record<string, string> = {
    draft: 'ฉบับร่าง',
    open: 'เปิดรับสมัคร',
    exam: 'ช่วงสอบ',
    scoring: 'กรอกคะแนน',
    announced: 'ประกาศผลแล้ว',
    enrolling: 'ช่วงมอบตัว',
    closed: 'ปิดแล้ว'
};

export const roundStatusColor: Record<string, string> = {
    draft: 'bg-gray-100 text-gray-700',
    open: 'bg-green-100 text-green-700',
    exam: 'bg-blue-100 text-blue-700',
    scoring: 'bg-orange-100 text-orange-700',
    announced: 'bg-purple-100 text-purple-700',
    enrolling: 'bg-yellow-100 text-yellow-700',
    closed: 'bg-red-100 text-red-700'
};

export const applicationStatusLabel: Record<string, string> = {
    submitted: 'รอตรวจสอบ',
    verified: 'ผ่านการตรวจสอบ',
    scored: 'กรอกคะแนนแล้ว',
    rejected: 'ไม่ผ่าน',
    accepted: 'ได้รับการคัดเลือก',
    enrolled: 'มอบตัวแล้ว',
    withdrawn: 'ถอนตัว'
};

export const applicationStatusColor: Record<string, string> = {
    submitted: 'bg-yellow-100 text-yellow-700',
    verified: 'bg-blue-100 text-blue-700',
    scored: 'bg-cyan-100 text-cyan-700',
    rejected: 'bg-red-100 text-red-700',
    accepted: 'bg-green-100 text-green-700',
    enrolled: 'bg-purple-100 text-purple-700',
    withdrawn: 'bg-gray-100 text-gray-700'
};
