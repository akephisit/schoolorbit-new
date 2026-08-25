import { ApiClientError, apiClient, requireApiData, type ApiResponse } from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type AcademicYear = Schemas['AcademicYear'];
export type AcademicTerm = Schemas['AcademicTerm'];
export type AcademicTermType = Schemas['AcademicTermType'];
export type BellSchedule = Schemas['BellSchedule'];
export type BellSchedulePeriod = Schemas['BellSchedulePeriod'];
export type SubjectGroup = Schemas['SubjectGroup'];
export type CatalogSubject = Schemas['CatalogSubject'];
export type SubjectVersion = Schemas['SubjectVersion'];
export type CatalogActivity = Schemas['CatalogActivity'];
export type ActivityVersion = Schemas['ActivityVersion'];
export type Curriculum = Schemas['Curriculum'];
export type CurriculumVersion = Schemas['CurriculumVersion'];
export type StudyProgram = Schemas['StudyProgram'];
export type ProgramRequirement = Schemas['ProgramRequirement'];
export type Homeroom = Schemas['Homeroom'];
export type HomeroomAdvisor = Schemas['HomeroomAdvisor'];
export type StudentAcademicYear = Schemas['StudentAcademicYear'];
export type HomeroomPlacement = Schemas['HomeroomPlacement'];
export type HomeroomPlacementTransfer = Schemas['HomeroomPlacementTransfer'];
export type GradeLevelOption = Schemas['GradeLevelLookupItem'];
export type StudentOption = Schemas['StudentLookupItem'];
export type StaffOption = Schemas['StaffLookupItem'];

export type CreateAcademicYearRequest = Schemas['CreateAcademicYearRequest'];
export type UpdateAcademicYearRequest = Schemas['UpdateAcademicYearRequest'];
export type CreateAcademicTermRequest = Schemas['CreateAcademicTermRequest'];
export type UpdateAcademicTermRequest = Schemas['UpdateAcademicTermRequest'];
export type CreateBellScheduleRequest = Schemas['CreateBellScheduleRequest'];
export type UpdateBellScheduleRequest = Schemas['UpdateBellScheduleRequest'];
export type ReplaceBellSchedulePeriodsRequest = Schemas['ReplaceBellSchedulePeriodsRequest'];
export type CreateSubjectGroupRequest = Schemas['CreateSubjectGroupRequest'];
export type UpdateSubjectGroupRequest = Schemas['UpdateSubjectGroupRequest'];
export type CreateCatalogSubjectRequest = Schemas['CreateCatalogSubjectRequest'];
export type UpdateCatalogSubjectRequest = Schemas['UpdateCatalogSubjectRequest'];
export type CreateSubjectVersionRequest = Schemas['CreateSubjectVersionRequest'];
export type UpdateSubjectVersionRequest = Schemas['UpdateSubjectVersionRequest'];
export type CreateCatalogActivityRequest = Schemas['CreateCatalogActivityRequest'];
export type UpdateCatalogActivityRequest = Schemas['UpdateCatalogActivityRequest'];
export type CreateActivityVersionRequest = Schemas['CreateActivityVersionRequest'];
export type UpdateActivityVersionRequest = Schemas['UpdateActivityVersionRequest'];
export type PublishVersionRequest = Schemas['PublishVersionRequest'];
export type CreateCurriculumRequest = Schemas['CreateCurriculumRequest'];
export type UpdateCurriculumRequest = Schemas['UpdateCurriculumRequest'];
export type CreateCurriculumVersionRequest = Schemas['CreateCurriculumVersionRequest'];
export type UpdateCurriculumVersionRequest = Schemas['UpdateCurriculumVersionRequest'];
export type CreateStudyProgramRequest = Schemas['CreateStudyProgramRequest'];
export type UpdateStudyProgramRequest = Schemas['UpdateStudyProgramRequest'];
export type ReplaceProgramRequirementsRequest = Schemas['ReplaceProgramRequirementsRequest'];
export type CreateHomeroomRequest = Schemas['CreateHomeroomRequest'];
export type UpdateHomeroomRequest = Schemas['UpdateHomeroomRequest'];
export type ReplaceHomeroomAdvisorsRequest = Schemas['ReplaceHomeroomAdvisorsRequest'];
export type CreateStudentAcademicYearRequest = Schemas['CreateStudentAcademicYearRequest'];
export type UpdateStudentAcademicYearRequest = Schemas['UpdateStudentAcademicYearRequest'];
export type CreateHomeroomPlacementRequest = Schemas['CreateHomeroomPlacementRequest'];
export type TransferHomeroomPlacementRequest = Schemas['TransferHomeroomPlacementRequest'];

async function academicData<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	const response = await request;
	if (response.status === 409) {
		const serverMessage = response.error || fallback;
		throw new ApiClientError(
			`${serverMessage} กรุณาโหลดข้อมูลล่าสุดแล้วตรวจสอบแบบร่างอีกครั้ง`,
			409
		);
	}
	return requireApiData(response, fallback);
}

function requiredContext(value: string, label: string): string {
	const selected = value.trim();
	if (!selected) throw new Error(`กรุณาเลือก${label}ก่อน`);
	return encodeURIComponent(selected);
}

function requiredContextValue(value: string, label: string): string {
	const selected = value.trim();
	if (!selected) throw new Error(`กรุณาเลือก${label}ก่อน`);
	return selected;
}

export const listAcademicYears = () =>
	academicData(apiClient.get<AcademicYear[]>('/api/academic/years'), 'ไม่สามารถโหลดปีการศึกษาได้');
export const createAcademicYear = (body: CreateAcademicYearRequest) =>
	academicData(
		apiClient.post<AcademicYear>('/api/academic/years', body),
		'สร้างปีการศึกษาไม่สำเร็จ'
	);
export const updateAcademicYear = (id: string, body: UpdateAcademicYearRequest) =>
	academicData(
		apiClient.patch<AcademicYear>(`/api/academic/years/${id}`, body),
		'แก้ไขปีการศึกษาไม่สำเร็จ'
	);

export const listAcademicTerms = (academicYearId: string) =>
	academicData(
		apiClient.get<AcademicTerm[]>(
			`/api/academic/terms?academicYearId=${requiredContext(academicYearId, 'ปีการศึกษา')}`
		),
		'ไม่สามารถโหลดภาคเรียนได้'
	);
export const createAcademicTerm = (body: CreateAcademicTermRequest) =>
	academicData(apiClient.post<AcademicTerm>('/api/academic/terms', body), 'สร้างภาคเรียนไม่สำเร็จ');
export const updateAcademicTerm = (id: string, body: UpdateAcademicTermRequest) =>
	academicData(
		apiClient.patch<AcademicTerm>(`/api/academic/terms/${id}`, body),
		'แก้ไขภาคเรียนไม่สำเร็จ'
	);
export const deleteAcademicTerm = (id: string) =>
	academicData(
		apiClient.delete<Schemas['EmptyData']>(`/api/academic/terms/${id}`),
		'ลบภาคเรียนไม่สำเร็จ'
	);

export const listBellSchedules = (academicYearId: string) =>
	academicData(
		apiClient.get<BellSchedule[]>(
			`/api/academic/bell-schedules?academicYearId=${requiredContext(academicYearId, 'ปีการศึกษา')}`
		),
		'ไม่สามารถโหลดตารางเวลาได้'
	);
export const createBellSchedule = (body: CreateBellScheduleRequest) =>
	academicData(
		apiClient.post<BellSchedule>('/api/academic/bell-schedules', body),
		'สร้างตารางเวลาไม่สำเร็จ'
	);
export const updateBellSchedule = (id: string, body: UpdateBellScheduleRequest) =>
	academicData(
		apiClient.patch<BellSchedule>(`/api/academic/bell-schedules/${id}`, body),
		'แก้ไขตารางเวลาไม่สำเร็จ'
	);
export const listBellSchedulePeriods = (id: string) =>
	academicData(
		apiClient.get<BellSchedulePeriod[]>(`/api/academic/bell-schedules/${id}/periods`),
		'ไม่สามารถโหลดคาบเรียนได้'
	);
export const replaceBellSchedulePeriods = (id: string, body: ReplaceBellSchedulePeriodsRequest) =>
	academicData(
		apiClient.put<BellSchedulePeriod[]>(`/api/academic/bell-schedules/${id}/periods`, body),
		'บันทึกคาบเรียนไม่สำเร็จ'
	);

export const listSubjectGroups = () =>
	academicData(
		apiClient.get<SubjectGroup[]>('/api/academic/catalog/subject-groups'),
		'ไม่สามารถโหลดกลุ่มสาระได้'
	);
export const createSubjectGroup = (body: CreateSubjectGroupRequest) =>
	academicData(
		apiClient.post<SubjectGroup>('/api/academic/catalog/subject-groups', body),
		'สร้างกลุ่มสาระไม่สำเร็จ'
	);
export const updateSubjectGroup = (id: string, body: UpdateSubjectGroupRequest) =>
	academicData(
		apiClient.patch<SubjectGroup>(`/api/academic/catalog/subject-groups/${id}`, body),
		'แก้ไขกลุ่มสาระไม่สำเร็จ'
	);
export const deleteSubjectGroup = (id: string) =>
	academicData(
		apiClient.delete<Schemas['EmptyData']>(`/api/academic/catalog/subject-groups/${id}`),
		'ลบกลุ่มสาระไม่สำเร็จ'
	);

export const listCatalogSubjects = () =>
	academicData(
		apiClient.get<CatalogSubject[]>('/api/academic/catalog/subjects'),
		'ไม่สามารถโหลดรายวิชาได้'
	);
export const createCatalogSubject = (body: CreateCatalogSubjectRequest) =>
	academicData(
		apiClient.post<CatalogSubject>('/api/academic/catalog/subjects', body),
		'สร้างรหัสรายวิชาไม่สำเร็จ'
	);
export const updateCatalogSubject = (id: string, body: UpdateCatalogSubjectRequest) =>
	academicData(
		apiClient.patch<CatalogSubject>(`/api/academic/catalog/subjects/${id}`, body),
		'แก้ไขรหัสรายวิชาไม่สำเร็จ'
	);
export const listSubjectVersions = (subjectId: string) =>
	academicData(
		apiClient.get<SubjectVersion[]>(`/api/academic/catalog/subjects/${subjectId}/versions`),
		'ไม่สามารถโหลดประวัติรายวิชาได้'
	);
export const createSubjectVersion = (subjectId: string, body: CreateSubjectVersionRequest) =>
	academicData(
		apiClient.post<SubjectVersion>(`/api/academic/catalog/subjects/${subjectId}/versions`, body),
		'สร้างรุ่นรายวิชาไม่สำเร็จ'
	);
export const updateSubjectVersion = (id: string, body: UpdateSubjectVersionRequest) =>
	academicData(
		apiClient.patch<SubjectVersion>(`/api/academic/catalog/subject-versions/${id}`, body),
		'แก้ไขรุ่นรายวิชาไม่สำเร็จ'
	);
export const publishSubjectVersion = (id: string, body: PublishVersionRequest) =>
	academicData(
		apiClient.post<SubjectVersion>(`/api/academic/catalog/subject-versions/${id}/publish`, body),
		'เผยแพร่รุ่นรายวิชาไม่สำเร็จ'
	);

export const listCatalogActivities = () =>
	academicData(
		apiClient.get<CatalogActivity[]>('/api/academic/catalog/activities'),
		'ไม่สามารถโหลดกิจกรรมได้'
	);
export const createCatalogActivity = (body: CreateCatalogActivityRequest) =>
	academicData(
		apiClient.post<CatalogActivity>('/api/academic/catalog/activities', body),
		'สร้างรหัสกิจกรรมไม่สำเร็จ'
	);
export const updateCatalogActivity = (id: string, body: UpdateCatalogActivityRequest) =>
	academicData(
		apiClient.patch<CatalogActivity>(`/api/academic/catalog/activities/${id}`, body),
		'แก้ไขรหัสกิจกรรมไม่สำเร็จ'
	);
export const listActivityVersions = (activityId: string) =>
	academicData(
		apiClient.get<ActivityVersion[]>(`/api/academic/catalog/activities/${activityId}/versions`),
		'ไม่สามารถโหลดประวัติกิจกรรมได้'
	);
export const createActivityVersion = (activityId: string, body: CreateActivityVersionRequest) =>
	academicData(
		apiClient.post<ActivityVersion>(
			`/api/academic/catalog/activities/${activityId}/versions`,
			body
		),
		'สร้างรุ่นกิจกรรมไม่สำเร็จ'
	);
export const updateActivityVersion = (id: string, body: UpdateActivityVersionRequest) =>
	academicData(
		apiClient.patch<ActivityVersion>(`/api/academic/catalog/activity-versions/${id}`, body),
		'แก้ไขรุ่นกิจกรรมไม่สำเร็จ'
	);
export const publishActivityVersion = (id: string, body: PublishVersionRequest) =>
	academicData(
		apiClient.post<ActivityVersion>(`/api/academic/catalog/activity-versions/${id}/publish`, body),
		'เผยแพร่รุ่นกิจกรรมไม่สำเร็จ'
	);

export const listCurricula = () =>
	academicData(apiClient.get<Curriculum[]>('/api/academic/curricula'), 'ไม่สามารถโหลดหลักสูตรได้');
export const createCurriculum = (body: CreateCurriculumRequest) =>
	academicData(
		apiClient.post<Curriculum>('/api/academic/curricula', body),
		'สร้างหลักสูตรไม่สำเร็จ'
	);
export const updateCurriculum = (id: string, body: UpdateCurriculumRequest) =>
	academicData(
		apiClient.patch<Curriculum>(`/api/academic/curricula/${id}`, body),
		'แก้ไขหลักสูตรไม่สำเร็จ'
	);
export const listCurriculumVersions = (curriculumId: string) =>
	academicData(
		apiClient.get<CurriculumVersion[]>(`/api/academic/curricula/${curriculumId}/versions`),
		'ไม่สามารถโหลดรุ่นหลักสูตรได้'
	);
export const createCurriculumVersion = (
	curriculumId: string,
	body: CreateCurriculumVersionRequest
) =>
	academicData(
		apiClient.post<CurriculumVersion>(`/api/academic/curricula/${curriculumId}/versions`, body),
		'สร้างรุ่นหลักสูตรไม่สำเร็จ'
	);
export const updateCurriculumVersion = (id: string, body: UpdateCurriculumVersionRequest) =>
	academicData(
		apiClient.patch<CurriculumVersion>(`/api/academic/curriculum-versions/${id}`, body),
		'แก้ไขรุ่นหลักสูตรไม่สำเร็จ'
	);
export const publishCurriculumVersion = (id: string, body: PublishVersionRequest) =>
	academicData(
		apiClient.post<CurriculumVersion>(`/api/academic/curriculum-versions/${id}/publish`, body),
		'เผยแพร่รุ่นหลักสูตรไม่สำเร็จ'
	);
export const listStudyPrograms = (curriculumVersionId: string) =>
	academicData(
		apiClient.get<StudyProgram[]>(
			`/api/academic/curriculum-versions/${curriculumVersionId}/programs`
		),
		'ไม่สามารถโหลดแผนการเรียนได้'
	);
export const createStudyProgram = (curriculumVersionId: string, body: CreateStudyProgramRequest) =>
	academicData(
		apiClient.post<StudyProgram>(
			`/api/academic/curriculum-versions/${curriculumVersionId}/programs`,
			body
		),
		'สร้างแผนการเรียนไม่สำเร็จ'
	);
export const getStudyProgram = (id: string) =>
	academicData(
		apiClient.get<StudyProgram>(`/api/academic/study-programs/${id}`),
		'ไม่สามารถโหลดแผนการเรียนได้'
	);
export const updateStudyProgram = (id: string, body: UpdateStudyProgramRequest) =>
	academicData(
		apiClient.patch<StudyProgram>(`/api/academic/study-programs/${id}`, body),
		'แก้ไขแผนการเรียนไม่สำเร็จ'
	);
export const listProgramRequirements = (studyProgramId: string) =>
	academicData(
		apiClient.get<ProgramRequirement[]>(
			`/api/academic/study-programs/${studyProgramId}/requirements`
		),
		'ไม่สามารถโหลดข้อกำหนดหลักสูตรได้'
	);
export const replaceProgramRequirements = (
	studyProgramId: string,
	body: ReplaceProgramRequirementsRequest
) =>
	academicData(
		apiClient.put<ProgramRequirement[]>(
			`/api/academic/study-programs/${studyProgramId}/requirements`,
			body
		),
		'บันทึกข้อกำหนดหลักสูตรไม่สำเร็จ'
	);

export const listHomerooms = (academicYearId: string) =>
	academicData(
		apiClient.get<Homeroom[]>(
			`/api/academic/homerooms?academicYearId=${requiredContext(academicYearId, 'ปีการศึกษา')}`
		),
		'ไม่สามารถโหลดห้องประจำชั้นได้'
	);
export const createHomeroom = (body: CreateHomeroomRequest) =>
	academicData(
		apiClient.post<Homeroom>('/api/academic/homerooms', body),
		'สร้างห้องประจำชั้นไม่สำเร็จ'
	);
export const getHomeroom = (id: string) =>
	academicData(
		apiClient.get<Homeroom>(`/api/academic/homerooms/${id}`),
		'ไม่สามารถโหลดห้องประจำชั้นได้'
	);
export const updateHomeroom = (id: string, body: UpdateHomeroomRequest) =>
	academicData(
		apiClient.patch<Homeroom>(`/api/academic/homerooms/${id}`, body),
		'แก้ไขห้องประจำชั้นไม่สำเร็จ'
	);
export const listHomeroomAdvisors = (id: string) =>
	academicData(
		apiClient.get<HomeroomAdvisor[]>(`/api/academic/homerooms/${id}/advisors`),
		'ไม่สามารถโหลดครูที่ปรึกษาได้'
	);
export const replaceHomeroomAdvisors = (id: string, body: ReplaceHomeroomAdvisorsRequest) =>
	academicData(
		apiClient.put<HomeroomAdvisor[]>(`/api/academic/homerooms/${id}/advisors`, body),
		'บันทึกครูที่ปรึกษาไม่สำเร็จ'
	);

export interface StudentYearFilters {
	studentId?: string;
	gradeLevelId?: string;
	studyProgramId?: string;
	homeroomId?: string;
	status?: Schemas['StudentAcademicYearStatus'];
}

export const listStudentAcademicYears = (
	academicYearId: string,
	filters: StudentYearFilters = {}
) => {
	const selectedYear = academicYearId.trim();
	if (!selectedYear) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	const query = new URLSearchParams({ academicYearId: selectedYear });
	if (filters.studentId) query.set('studentId', filters.studentId);
	if (filters.gradeLevelId) query.set('gradeLevelId', filters.gradeLevelId);
	if (filters.studyProgramId) query.set('studyProgramId', filters.studyProgramId);
	if (filters.homeroomId) query.set('homeroomId', filters.homeroomId);
	if (filters.status) query.set('status', filters.status);
	return academicData(
		apiClient.get<StudentAcademicYear[]>(`/api/academic/student-years?${query}`),
		'ไม่สามารถโหลดข้อมูลนักเรียนประจำปีได้'
	);
};
export const createStudentAcademicYear = (body: CreateStudentAcademicYearRequest) =>
	academicData(
		apiClient.post<StudentAcademicYear>('/api/academic/student-years', body),
		'สร้างข้อมูลนักเรียนประจำปีไม่สำเร็จ'
	);
export const updateStudentAcademicYear = (id: string, body: UpdateStudentAcademicYearRequest) =>
	academicData(
		apiClient.patch<StudentAcademicYear>(`/api/academic/student-years/${id}`, body),
		'แก้ไขข้อมูลนักเรียนประจำปีไม่สำเร็จ'
	);
export const listHomeroomPlacements = (studentYearId: string) =>
	academicData(
		apiClient.get<HomeroomPlacement[]>(`/api/academic/student-years/${studentYearId}/placements`),
		'ไม่สามารถโหลดประวัติการจัดห้องได้'
	);
export const createHomeroomPlacement = (
	studentYearId: string,
	body: CreateHomeroomPlacementRequest
) =>
	academicData(
		apiClient.post<HomeroomPlacement>(
			`/api/academic/student-years/${studentYearId}/placements`,
			body
		),
		'จัดห้องนักเรียนไม่สำเร็จ'
	);
export const transferHomeroomPlacement = (
	placementId: string,
	body: TransferHomeroomPlacementRequest
) =>
	academicData(
		apiClient.post<HomeroomPlacementTransfer>(
			`/api/academic/placements/${placementId}/transfer`,
			body
		),
		'ย้ายห้องนักเรียนไม่สำเร็จ'
	);

type LookupGradeLevelsQuery = NonNullable<
	operations['lookupGradeLevels']['parameters']['query']
>;

export const listGradeLevelOptions = (academicYearId: string) => {
	const query = {
		academicYearId: requiredContextValue(academicYearId, 'ปีการศึกษา')
	} satisfies LookupGradeLevelsQuery;
	return academicData(
		apiClient.get<GradeLevelOption[]>('/api/lookup/grade-levels', { query }),
		'ไม่สามารถโหลดระดับชั้นได้'
	);
};
export const listStudentOptions = (search = '') => {
	const query = new URLSearchParams();
	if (search.trim()) query.set('search', search.trim());
	query.set('limit', '200');
	return academicData(
		apiClient.get<StudentOption[]>(`/api/lookup/students?${query}`),
		'ไม่สามารถโหลดนักเรียนได้'
	);
};
export const listStaffOptions = () =>
	academicData(
		apiClient.get<StaffOption[]>('/api/lookup/staff?limit=300'),
		'ไม่สามารถโหลดรายชื่อครูได้'
	);

export type StudyProgramOption = { id: string; name: string; curriculumName: string };

export async function listStudyProgramOptionsForYear(
	academicYearId: string
): Promise<StudyProgramOption[]> {
	const years = await listAcademicYears();
	const selectedYear = years.find((year) => year.id === academicYearId);
	if (!selectedYear) throw new Error('ไม่พบปีการศึกษาที่เลือก');
	const options: StudyProgramOption[] = [];
	const curricula = await listCurricula();
	for (const curriculum of curricula) {
		const versions = await listCurriculumVersions(curriculum.id);
		for (const version of versions) {
			if (version.status !== 'published') continue;
			const start = years.find((year) => year.id === version.startAcademicYearId)?.year;
			const end = version.endAcademicYearId
				? years.find((year) => year.id === version.endAcademicYearId)?.year
				: undefined;
			if (
				start === undefined ||
				selectedYear.year < start ||
				(end !== undefined && selectedYear.year > end)
			)
				continue;
			const programs = await listStudyPrograms(version.id);
			for (const program of programs)
				options.push({ id: program.id, name: program.nameTh, curriculumName: curriculum.nameTh });
		}
	}
	return options;
}
