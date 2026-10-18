import { apiClient, requireApiData, type ApiRequestOptions } from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';
type Schemas = components['schemas'];
export type CareFamily = Schemas['CareFamily'];
export type CareRespondent = Schemas['CareRespondent'];
export type CareInstrumentMode = Schemas['CareInstrumentMode'];
export type CareAssignmentStatus = Schemas['CareAssignmentStatus'];
export type CareRoundStatus = Schemas['CareRoundStatus'];
export type CareVisitStatus = Schemas['CareVisitStatus'];
export type CareCaseStatus = Schemas['CareCaseStatus'];
export type CareEventKind = Schemas['CareEventKind'];
export type CareOption = Schemas['CareOption'];
export type CareQuestion = Schemas['CareQuestion'];
export type CareBand = Schemas['CareBand'];
export type CareScale = Schemas['CareScale'];
export type CareInstrumentDefinition = Schemas['CareInstrumentDefinition'];
export type CareInstrumentVersion = Schemas['CareInstrumentVersion'];
export type CreateCareInstrumentVersion = Schemas['CreateCareInstrumentVersion'];
export type CareRound = Schemas['CareRound'];
export type CreateCareRound = Schemas['CreateCareRound'];
export type UpdateCareRound = Schemas['UpdateCareRound'];
export type CareStudentOption = Schemas['CareStudentOption'];
export type CareStaffOption = Schemas['CareStaffOption'];
export type CareAnswer = Schemas['CareAnswer'];
export type CareScaleResult = Schemas['CareScaleResult'];
export type CareManualScore = Schemas['CareManualScore'];
export type CareAssignment = Schemas['CareAssignment'];
export type CareAssignmentDetail = Schemas['CareAssignmentDetail'];
export type CareAssignmentTarget = Schemas['CareAssignmentTarget'];
export type CreateCareAssignments = Schemas['CreateCareAssignments'];
export type SaveCareResponse = Schemas['SaveCareResponse'];
export type CareRevisionCommand = Schemas['CareRevisionCommand'];
export type CareVisit = Schemas['CareVisit'];
export type CareVisitRecord = Schemas['CareVisitRecord'];
export type SaveCareVisit = Schemas['SaveCareVisit'];
export type CareCase = Schemas['CareCase'];
export type CareCaseEvent = Schemas['CareCaseEvent'];
export type CareCaseDetail = Schemas['CareCaseDetail'];
export type SaveCareCase = Schemas['SaveCareCase'];
export type AddCareCaseEvent = Schemas['AddCareCaseEvent'];
export type CareOverview = Schemas['CareOverview'];

const root = '/api/student-care';
const path = (resource: string, id: string) => `${root}/${resource}/${encodeURIComponent(id)}`;
export async function careOverview(options: ApiRequestOptions = {}): Promise<CareOverview> {
	return requireApiData(
		await apiClient.get<CareOverview>(`${root}/overview`, options),
		'โหลดภาพรวมไม่ได้'
	);
}
export async function careInstrument(
	id: string,
	options: ApiRequestOptions = {}
): Promise<CareInstrumentVersion> {
	return requireApiData(
		await apiClient.get<CareInstrumentVersion>(path('instruments', id), options),
		'โหลดแบบประเมินไม่ได้'
	);
}
export async function createCareInstrument(
	payload: CreateCareInstrumentVersion
): Promise<CareInstrumentVersion> {
	return requireApiData(
		await apiClient.post<CareInstrumentVersion>(`${root}/instruments`, payload),
		'บันทึกแบบประเมินไม่ได้'
	);
}
export async function createCareRound(payload: CreateCareRound): Promise<CareRound> {
	return requireApiData(
		await apiClient.post<CareRound>(`${root}/rounds`, payload),
		'สร้างรอบไม่ได้'
	);
}
export async function updateCareRound(id: string, payload: UpdateCareRound): Promise<CareRound> {
	return requireApiData(
		await apiClient.put<CareRound>(path('rounds', id), payload),
		'บันทึกรอบไม่ได้'
	);
}
export async function careStaff(options: ApiRequestOptions = {}): Promise<CareStaffOption[]> {
	return requireApiData(
		await apiClient.get<CareStaffOption[]>(`${root}/staff`, options),
		'โหลดบุคลากรไม่ได้'
	);
}
export async function createCareAssignments(
	payload: CreateCareAssignments
): Promise<CareAssignment[]> {
	return requireApiData(
		await apiClient.post<CareAssignment[]>(`${root}/assignments`, payload),
		'มอบหมายแบบประเมินไม่ได้'
	);
}
export async function careAssignment(
	id: string,
	options: ApiRequestOptions = {}
): Promise<CareAssignmentDetail> {
	return requireApiData(
		await apiClient.get<CareAssignmentDetail>(path('assignments', id), options),
		'โหลดแบบตอบไม่ได้'
	);
}
export async function saveCareResponse(
	id: string,
	payload: SaveCareResponse,
	submit = false
): Promise<CareAssignmentDetail> {
	const endpoint = `${path('assignments', id)}/${submit ? 'submit' : 'draft'}`;
	return requireApiData(
		submit
			? await apiClient.post<CareAssignmentDetail>(endpoint, payload)
			: await apiClient.put<CareAssignmentDetail>(endpoint, payload),
		'บันทึกคำตอบไม่ได้'
	);
}
export async function acknowledgeCareAssignment(
	id: string,
	payload: CareRevisionCommand
): Promise<CareAssignment> {
	return requireApiData(
		await apiClient.post<CareAssignment>(`${path('assignments', id)}/acknowledge`, payload),
		'รับทราบผลไม่ได้'
	);
}
export async function cancelCareAssignment(
	id: string,
	payload: CareRevisionCommand
): Promise<CareAssignment> {
	return requireApiData(
		await apiClient.post<CareAssignment>(`${path('assignments', id)}/cancel`, payload),
		'ยกเลิกการมอบหมายไม่ได้'
	);
}
export async function careVisit(
	id: string,
	options: ApiRequestOptions = {}
): Promise<CareVisitRecord> {
	return requireApiData(
		await apiClient.get<CareVisitRecord>(path('visits', id), options),
		'โหลดบันทึกเยี่ยมบ้านไม่ได้'
	);
}
export async function saveCareVisit(payload: SaveCareVisit, id?: string): Promise<CareVisitRecord> {
	return requireApiData(
		id
			? await apiClient.put<CareVisitRecord>(path('visits', id), payload)
			: await apiClient.post<CareVisitRecord>(`${root}/visits`, payload),
		'บันทึกเยี่ยมบ้านไม่ได้'
	);
}
export async function careCase(
	id: string,
	options: ApiRequestOptions = {}
): Promise<CareCaseDetail> {
	return requireApiData(
		await apiClient.get<CareCaseDetail>(path('cases', id), options),
		'โหลดการดูแลไม่ได้'
	);
}
export async function saveCareCase(payload: SaveCareCase, id?: string): Promise<CareCaseDetail> {
	return requireApiData(
		id
			? await apiClient.put<CareCaseDetail>(path('cases', id), payload)
			: await apiClient.post<CareCaseDetail>(`${root}/cases`, payload),
		'บันทึกการดูแลไม่ได้'
	);
}
export async function addCareCaseEvent(
	id: string,
	payload: AddCareCaseEvent
): Promise<CareCaseDetail> {
	return requireApiData(
		await apiClient.post<CareCaseDetail>(`${path('cases', id)}/events`, payload),
		'บันทึกเหตุการณ์ไม่ได้'
	);
}

export type CareListQuery = NonNullable<
	operations['student_care_list_assignments']['parameters']['query']
>;
export type CareStudentQuery = NonNullable<
	operations['student_care_list_students']['parameters']['query']
>;
export async function careInstruments(
	query: CareListQuery = {},
	options: ApiRequestOptions = {}
): Promise<CareInstrumentVersion[]> {
	return requireApiData(
		await apiClient.get<CareInstrumentVersion[]>(`${root}/instruments`, {
			...options,
			query: { ...query }
		}),
		'โหลดรายการไม่ได้'
	);
}
export async function careRounds(
	query: CareListQuery = {},
	options: ApiRequestOptions = {}
): Promise<CareRound[]> {
	return requireApiData(
		await apiClient.get<CareRound[]>(`${root}/rounds`, { ...options, query: { ...query } }),
		'โหลดรายการไม่ได้'
	);
}
export async function careAssignments(
	query: CareListQuery = {},
	options: ApiRequestOptions = {}
): Promise<CareAssignment[]> {
	return requireApiData(
		await apiClient.get<CareAssignment[]>(`${root}/assignments`, {
			...options,
			query: { ...query }
		}),
		'โหลดรายการไม่ได้'
	);
}
export async function careVisits(
	query: CareListQuery = {},
	options: ApiRequestOptions = {}
): Promise<CareVisit[]> {
	return requireApiData(
		await apiClient.get<CareVisit[]>(`${root}/visits`, { ...options, query: { ...query } }),
		'โหลดรายการไม่ได้'
	);
}
export async function careCases(
	query: CareListQuery = {},
	options: ApiRequestOptions = {}
): Promise<CareCase[]> {
	return requireApiData(
		await apiClient.get<CareCase[]>(`${root}/cases`, { ...options, query: { ...query } }),
		'โหลดรายการไม่ได้'
	);
}
export async function careStudents(
	query: CareStudentQuery,
	options: ApiRequestOptions = {}
): Promise<CareStudentOption[]> {
	return requireApiData(
		await apiClient.get<CareStudentOption[]>(`${root}/students`, {
			...options,
			query: { ...query }
		}),
		'โหลดนักเรียนไม่ได้'
	);
}

export async function careRound(id: string, options: ApiRequestOptions = {}): Promise<CareRound> {
	return requireApiData(
		await apiClient.get<CareRound>(path('rounds', id), options),
		'โหลดรอบประเมินไม่ได้'
	);
}
export type CareEventQuery = NonNullable<
	operations['student_care_list_case_events']['parameters']['query']
>;
export async function careCaseEvents(
	id: string,
	query: CareEventQuery = {},
	options: ApiRequestOptions = {}
): Promise<CareCaseEvent[]> {
	return requireApiData(
		await apiClient.get<CareCaseEvent[]>(`${path('cases', id)}/events`, {
			...options,
			query: { ...query }
		}),
		'โหลดประวัติก่อนหน้าไม่ได้'
	);
}
export type UpdateCareAssignmentReviewers = Schemas['UpdateCareAssignmentReviewers'];
export async function updateCareAssignmentReviewers(
	id: string,
	payload: UpdateCareAssignmentReviewers
): Promise<CareAssignment> {
	return requireApiData(
		await apiClient.post<CareAssignment>(`${path('assignments', id)}/reviewers`, payload),
		'เปลี่ยนผู้รับผิดชอบติดตามไม่ได้'
	);
}
