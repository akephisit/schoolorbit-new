import { apiClient } from './client';

// ==========================================
// Types
// ==========================================

export interface ReportConfig {
    reportMode: 'zone' | 'institution' | 'both' | null;
    zone?: {
        schools: string[];
    };
    institution?: {
        ownSchool: string;
    };
}

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
    status: 'draft' | 'open' | 'exam_announced' | 'announced' | 'enrolling' | 'closed';
    isVisible: boolean;
    createdAt: string;
    updatedAt: string;
    // Joined
    academicYearName?: string;
    gradeLevelName?: string;
    applicationCount?: number;
    reportConfig?: ReportConfig;
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
    // ข้อมูลส่วนตัวเพิ่มเติม
    religion?: string;
    ethnicity?: string;
    nationality?: string;
    // ที่อยู่ตามทะเบียนบ้าน
    addressLine?: string;    // home address line (legacy + backward compat)
    subDistrict?: string;    // ตำบล/แขวง (home)
    district?: string;       // อำเภอ/เขต (home)
    province?: string;       // จังหวัด (home)
    postalCode?: string;     // รหัสไปรษณีย์ (home)
    homeHouseNo?: string;
    homeMoo?: string;
    homeSoi?: string;
    homeRoad?: string;
    homePhone?: string;
    // ที่อยู่ปัจจุบัน
    currentHouseNo?: string;
    currentMoo?: string;
    currentSoi?: string;
    currentRoad?: string;
    currentSubDistrict?: string;
    currentDistrict?: string;
    currentProvince?: string;
    currentPostalCode?: string;
    currentPhone?: string;
    // โรงเรียนเดิม
    previousSchool?: string;
    previousGrade?: string;
    previousGpa?: number;
    previousStudyYear?: string;
    previousSchoolProvince?: string;
    // บิดา
    fatherName?: string;
    fatherPhone?: string;
    fatherOccupation?: string;
    fatherNationalId?: string;
    fatherIncome?: number;
    // มารดา
    motherName?: string;
    motherPhone?: string;
    motherOccupation?: string;
    motherNationalId?: string;
    motherIncome?: number;
    // ผู้ปกครอง
    guardianName?: string;
    guardianPhone?: string;
    guardianRelation?: string;
    guardianNationalId?: string;
    guardianOccupation?: string;
    guardianIncome?: number;
    guardianIs?: 'father' | 'mother' | 'other';
    // ครอบครัว
    parentStatus?: string[];
    parentStatusOther?: string;
    // Status
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

export interface DocumentRef {
    tempFileId: string;
    docType: string;
}

export interface ApplicationDocument {
    id: string;
    applicationId: string;
    fileId: string;
    docType: string;
    createdAt: string;
    fileUrl?: string;
    originalFilename?: string;
    fileSize?: number;
    mimeType?: string;
}

export interface TempUploadResponse {
    tempFileId: string;
    originalFilename: string;
    fileSize: number;
    docType: string;
    url: string;
}

export const DOC_TYPE_LABELS: Record<string, { label: string; required: boolean }> = {
    photo_1_5inch:    { label: 'รูปถ่าย 1.5 นิ้ว', required: true },
    transcript_por:   { label: 'สำเนาเอกสารแสดงผลการเรียน (ปพ.)', required: true },
    certificate_por7: { label: 'หลักฐานใบรับรอง ปพ.7', required: true },
    id_card_student:  { label: 'สำเนาบัตรประชาชนนักเรียน', required: true },
    id_card_father:   { label: 'สำเนาบัตรประชาชนบิดา', required: false },
    id_card_mother:   { label: 'สำเนาบัตรประชาชนมารดา', required: false },
    id_card_guardian: { label: 'สำเนาบัตรประชาชนผู้ปกครอง (ถ้าอยู่กับผู้ปกครอง)', required: false },
    house_reg_student: { label: 'สำเนาทะเบียนบ้านนักเรียน', required: true },
    house_reg_father:  { label: 'สำเนาทะเบียนบ้านบิดา', required: false },
    house_reg_mother:  { label: 'สำเนาทะเบียนบ้านมารดา', required: false },
    house_reg_guardian: { label: 'สำเนาทะเบียนบ้านผู้ปกครอง (หากอยู่กับผู้ปกครอง)', required: false },
    name_change_doc:  { label: 'เอกสารเปลี่ยนชื่อ-นามสกุล (ถ้ามี)', required: false },
    birth_cert:       { label: 'สำเนาสูติบัตร (กรณีไม่มีบัตรประชาชนและทะเบียนบ้านบิดา/มารดา)', required: false },
};

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

export async function updateRoundVisibility(id: string, isVisible: boolean) {
    const res = await apiClient.patch(`/api/admission/rounds/${id}/visibility`, { isVisible });
    if (!res.success) throw new Error(res.error || 'ไม่สามารถอัปเดตการแสดงผลได้');
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

export async function getApplication(id: string): Promise<{ application: AdmissionApplication; documents: ApplicationDocument[] }> {
    const res = await apiClient.get<AdmissionApplication>(`/api/admission/applications/${id}`) as unknown as { success: boolean; error?: string; data: AdmissionApplication; documents: ApplicationDocument[] };
    if (!res.success) throw new Error(res.error);
    return { application: res.data, documents: res.documents ?? [] };
}

export async function verifyApplication(id: string) {
    const res = await apiClient.put(`/api/admission/applications/${id}/verify`, {});
    if (!res.success) throw new Error(res.error);
}

export async function rejectApplication(id: string, rejectionReason: string) {
    const res = await apiClient.put(`/api/admission/applications/${id}/reject`, { rejectionReason });
    if (!res.success) throw new Error(res.error);
}

export async function deleteApplication(id: string) {
    const res = await apiClient.delete(`/api/admission/applications/${id}`);
    if (!res.success) throw new Error(res.error);
}

export async function updateApplicationByStaff(id: string, data: Partial<AdmissionApplication>) {
    const res = await apiClient.put(`/api/admission/applications/${id}`, data);
    if (!res.success) throw new Error(res.error);
}

export async function unverifyApplication(id: string) {
    const res = await apiClient.put(`/api/admission/applications/${id}/unverify`, {});
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

export async function portalGetExamSeat(nationalId: string, dateOfBirth: string): Promise<{
    seatNumber: number;
    examId?: string;
    roomName: string;
    buildingName?: string;
    examDate?: string;
} | null> {
    const res = await apiClient.post('/api/admission/portal/exam-seat', {
        nationalId,
        dateOfBirth
    });
    if (!res.success) throw new Error(res.error || 'ไม่สามารถโหลดข้อมูลที่นั่งสอบได้');
    return (res.data as { seatNumber: number; examId?: string; roomName: string; buildingName?: string; examDate?: string } | null) ?? null;
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
    data: Partial<SubmitApplicationData>
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

export type SubmitApplicationData = {
    admissionTrackId: string;
    nationalId: string;
    title?: string;
    firstName: string;
    lastName: string;
    gender?: string;
    dateOfBirth?: string;
    phone?: string;
    email?: string;
    religion?: string;
    ethnicity?: string;
    nationality?: string;
    // Home address
    addressLine?: string;
    subDistrict?: string;
    district?: string;
    province?: string;
    postalCode?: string;
    homeHouseNo?: string;
    homeMoo?: string;
    homeSoi?: string;
    homeRoad?: string;
    homePhone?: string;
    // Current address
    currentHouseNo?: string;
    currentMoo?: string;
    currentSoi?: string;
    currentRoad?: string;
    currentSubDistrict?: string;
    currentDistrict?: string;
    currentProvince?: string;
    currentPostalCode?: string;
    currentPhone?: string;
    // Previous school
    previousSchool?: string;
    previousGrade?: string;
    previousGpa?: number;
    previousStudyYear?: string;
    previousSchoolProvince?: string;
    // Father
    fatherName?: string;
    fatherPhone?: string;
    fatherOccupation?: string;
    fatherNationalId?: string;
    fatherIncome?: number;
    // Mother
    motherName?: string;
    motherPhone?: string;
    motherOccupation?: string;
    motherNationalId?: string;
    motherIncome?: number;
    // Guardian
    guardianName?: string;
    guardianPhone?: string;
    guardianRelation?: string;
    guardianNationalId?: string;
    guardianOccupation?: string;
    guardianIncome?: number;
    guardianIs?: string;
    // Family status
    parentStatus?: string[];
    parentStatusOther?: string;
    // Documents
    documents?: DocumentRef[];
};

export async function submitApplication(roundId: string, data: SubmitApplicationData) {
    const res = await apiClient.post<{ applicationNumber: string }>(`/api/admission/apply/${roundId}`, data);
    if (!res.success) throw new Error(res.error || 'ไม่สามารถยื่นใบสมัครได้');
    return res.data!;
}

// ==========================================
// Portal Document Upload API
// ==========================================

export async function portalUploadTempFile(
    file: File,
    docType: string,
    authNationalId?: string,
    authDateOfBirth?: string,
): Promise<{ fileId: string; fileUrl: string; fileSize: number; docType: string }> {
    const { PUBLIC_BACKEND_URL } = await import('$env/static/public');
    const formData = new FormData();
    formData.append('file', file);
    formData.append('doc_type', docType);
    if (authNationalId) formData.append('national_id', authNationalId);
    if (authDateOfBirth) formData.append('date_of_birth', authDateOfBirth);

    const res = await fetch(`${PUBLIC_BACKEND_URL}/api/admission/portal/upload`, {
        method: 'POST',
        credentials: 'include',
        body: formData,
    });
    if (!res.ok) {
        const err = await res.json().catch(() => ({}));
        throw new Error((err as any)?.error || 'ไม่สามารถอัปโหลดไฟล์ได้');
    }
    const json = await res.json();
    return json.data;
}

// ==========================================
// Staff Document Management API
// ==========================================

export interface StaffDocumentUploadResponse {
    id: string;
    fileId: string;
    docType: string;
    fileUrl: string;
    fileSize: number;
}

export async function staffUploadDocument(
    appId: string,
    docType: string,
    blob: Blob,
): Promise<StaffDocumentUploadResponse> {
    const formData = new FormData();
    formData.append('doc_type', docType);
    formData.append('file', blob, `${docType}.jpg`);
    const res = await apiClient.postMultipart<StaffDocumentUploadResponse>(
        `/api/admission/applications/${appId}/documents`,
        formData
    );
    if (!res.success) throw new Error(res.error || 'ไม่สามารถอัปโหลดเอกสารได้');
    return res.data!;
}

export async function staffDeleteDocument(appId: string, docType: string): Promise<void> {
    const res = await apiClient.delete(`/api/admission/applications/${appId}/documents/${docType}`);
    if (!res.success) throw new Error((res as any).error || 'ไม่สามารถลบเอกสารได้');
}

export async function portalDeleteDocument(
    nationalId: string,
    dateOfBirth: string,
    docType: string,
): Promise<void> {
    const params = new URLSearchParams({ national_id: nationalId, date_of_birth: dateOfBirth });
    const res = await apiClient.delete(`/api/admission/portal/documents/${docType}?${params}`);
    if (!res.success) throw new Error((res as any).error || 'ไม่สามารถลบเอกสารได้');
}

// ==========================================
// Helpers
// ==========================================

export const roundStatusLabel: Record<string, string> = {
    draft: 'ฉบับร่าง',
    open: 'เปิดรับสมัคร',
    exam_announced: 'ประกาศที่นั่งสอบ',
    announced: 'ประกาศผลคัดเลือก',
    enrolling: 'รายงานตัว/มอบตัว',
    closed: 'ปิดรอบ'
};

export const roundStatusColor: Record<string, string> = {
    draft: 'bg-gray-100 text-gray-700',
    open: 'bg-green-100 text-green-700',
    exam_announced: 'bg-blue-100 text-blue-700',
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

// ==========================================
// Exam Room Types
// ==========================================

export interface ExamRoom {
    id: string;
    roomId?: string;
    roomName: string;
    buildingName?: string;
    capacity: number;
    displayOrder: number;
    assignedCount: number;
}

export interface ExamSeatAssignment {
    seatNumber: number;
    examId?: string;
    applicationId: string;
    applicationNumber?: string;
    fullName: string;
    nationalId: string;
    trackName?: string;
}

export interface ExamRoomGroup {
    examRoomId: string;
    roomName: string;
    buildingName?: string;
    capacity: number;
    seats: ExamSeatAssignment[];
}

export interface ExamConfig {
    examIdType?: 'application_number' | 'sequential' | 'custom_prefix';
    examIdPrefix?: string;
    sortOrder?: 'by_application' | 'by_track' | 'random';
}

export interface ExamSeatDetail {
    seatNumber: number;
    examId?: string;
    roomName: string;
    buildingName?: string;
    examDate?: string;
}

// ==========================================
// Exam Room API Functions
// ==========================================

export async function listExamRooms(roundId: string) {
    const res = await apiClient.get(`/api/admission/rounds/${roundId}/exam-rooms`);
    return res.data as { rooms: ExamRoom[]; totalCapacity: number; totalAssigned: number };
}

export async function addExamRoom(roundId: string, data: {
    roomId?: string;
    customName?: string;
    capacityOverride?: number;
    displayOrder?: number;
}) {
    const res = await apiClient.post(`/api/admission/rounds/${roundId}/exam-rooms`, data);
    return res.data;
}

export async function updateExamRoom(roundId: string, roomId: string, data: {
    capacityOverride?: number;
    displayOrder?: number;
    customName?: string;
}) {
    const res = await apiClient.put(`/api/admission/rounds/${roundId}/exam-rooms/${roomId}`, data);
    return res.data;
}

export async function removeExamRoom(roundId: string, roomId: string) {
    const res = await apiClient.delete(`/api/admission/rounds/${roundId}/exam-rooms/${roomId}`);
    return res.data;
}

export async function copyExamRoomsFromRound(roundId: string, fromRoundId: string) {
    const res = await apiClient.post(`/api/admission/rounds/${roundId}/exam-rooms/copy-from/${fromRoundId}`);
    return res.data as { message: string };
}

export async function getExamConfig(roundId: string) {
    const res = await apiClient.get(`/api/admission/rounds/${roundId}/exam-config`);
    return res.data as ExamConfig;
}

export async function updateExamConfig(roundId: string, config: ExamConfig) {
    const res = await apiClient.put(`/api/admission/rounds/${roundId}/exam-config`, config);
    return res.data;
}

export async function assignExamSeats(roundId: string, options?: {
    examIdType?: string;
    examIdPrefix?: string;
    sortOrder?: string;
    mode?: 'full' | 'append';
}) {
    const res = await apiClient.post(`/api/admission/rounds/${roundId}/assign-exam-seats`, options ?? {});
    return res.data as { message: string; assignedCount: number; rooms: { roomName: string; count: number }[] };
}

export async function getExamSeats(roundId: string) {
    const res = await apiClient.get(`/api/admission/rounds/${roundId}/exam-seats`);
    return res.data as ExamRoomGroup[];
}

export async function getApplicationExamSeat(applicationId: string) {
    const res = await apiClient.get(`/api/admission/applications/${applicationId}/exam-seat`);
    return res.data as ExamSeatDetail | null;
}

// ==========================================
// Student ID Pre-Assignment
// ==========================================

export interface StudentIdEntry {
    applicationId: string;
    applicationNumber?: string;
    fullName: string;
    assignedStudentId?: string;
    roomName?: string;
    rankInRoom?: number;
    previousSchool?: string;
}

export async function listStudentIds(roundId: string): Promise<{ data: StudentIdEntry[] }> {
    const res = await apiClient.get(`/api/admission/rounds/${roundId}/student-ids`);
    if (!res.success) throw new Error(res.error);
    return { data: res.data as StudentIdEntry[] };
}

export async function batchUpdateStudentIds(
    roundId: string,
    updates: { applicationId: string; studentId: string | null }[]
): Promise<{ updated: number }> {
    const res = await apiClient.patch(`/api/admission/rounds/${roundId}/student-ids`, updates);
    if (!res.success) throw new Error(res.error);
    return res.data as { updated: number };
}
