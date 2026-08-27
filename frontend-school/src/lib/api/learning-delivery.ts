import {
	ApiClientError,
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '$lib/api/client';
import type { components, operations } from '$lib/api/generated/school-api';

type Schemas = components['schemas'];

export type LearningOffering = Schemas['LearningOffering'];
export type LearningDeliveryOverview = Schemas['LearningDeliveryOverview'];
export type DeliveryManagementOptions = Schemas['DeliveryManagementOptions'];
export type LearningGroup = Schemas['LearningGroup'];
export type TeacherAssignment = Schemas['TeacherAssignmentInput'];
export type RosterPreview = Schemas['RosterPreview'];
export type CurriculumOfferingPreview = Schemas['CurriculumOfferingPreview'];
export type ApplyCurriculumOfferingsResult = Schemas['ApplyCurriculumOfferingsResult'];
export type CreateLearningOfferingRequest = Schemas['CreateLearningOfferingRequest'];
export type UpdateLearningOfferingRequest = Schemas['UpdateLearningOfferingRequest'];
export type PublishLearningOfferingRequest = Schemas['PublishLearningOfferingRequest'];
export type PreviewCurriculumOfferingsRequest = Schemas['PreviewCurriculumOfferingsRequest'];
export type ApplyCurriculumOfferingsRequest = Schemas['ApplyCurriculumOfferingsRequest'];
export type CreateLearningGroupRequest = Schemas['CreateLearningGroupRequest'];
export type UpdateLearningGroupRequest = Schemas['UpdateLearningGroupRequest'];
export type ReplaceLearningGroupHomeroomsRequest = Schemas['ReplaceLearningGroupHomeroomsRequest'];
export type ReplaceLearningGroupTeachersRequest = Schemas['ReplaceLearningGroupTeachersRequest'];
export type ApplyRosterRequest = Schemas['ApplyRosterRequest'];
export type PublishRosterRequest = Schemas['PublishRosterRequest'];

async function deliveryData<T>(request: Promise<ApiResponse<T>>, fallback: string): Promise<T> {
	const response = await request;
	if (response.status === 409) {
		const serverMessage = response.error || fallback;
		throw new ApiClientError(
			`${serverMessage} กรุณาโหลดข้อมูลล่าสุดหรือสร้างตัวอย่างใหม่ก่อนดำเนินการ`,
			409
		);
	}
	return requireApiData(response, fallback);
}

function selectedTerm(academicTermId: string): string {
	const value = academicTermId.trim();
	if (!value) throw new Error('กรุณาเลือกภาคเรียนก่อน');
	return value;
}

type ListLearningOfferingsQuery = NonNullable<
	operations['listLearningOfferings']['parameters']['query']
>;
type ListLearningGroupsForTermQuery = NonNullable<
	operations['listLearningGroupsForTerm']['parameters']['query']
>;
type DeliveryWorkspaceQuery = NonNullable<
	operations['getLearningDeliveryOverview']['parameters']['query']
>;
type DeliveryManagementOptionsQuery = NonNullable<
	operations['getLearningDeliveryManagementOptions']['parameters']['query']
>;

export const getLearningDeliveryOverview = (
	academicTermId: string,
	options: ApiRequestOptions = {}
) => {
	const query = {
		academicTermId: selectedTerm(academicTermId)
	} satisfies DeliveryWorkspaceQuery;
	return deliveryData(
		apiClient.get<LearningDeliveryOverview>('/api/academic/delivery/workspace', {
			...options,
			query
		}),
		'ไม่สามารถโหลดภาพรวมรายการเปิดสอนได้'
	);
};

export const getLearningDeliveryManagementOptions = (
	academicTermId: string,
	options: ApiRequestOptions = {}
) => {
	const query = {
		academicTermId: selectedTerm(academicTermId)
	} satisfies DeliveryManagementOptionsQuery;
	return deliveryData(
		apiClient.get<DeliveryManagementOptions>('/api/academic/delivery/management-options', {
			...options,
			query
		}),
		'ไม่สามารถโหลดตัวเลือกสำหรับจัดการรายการเปิดสอนได้'
	);
};

export const listLearningOfferings = (academicTermId: string, options: ApiRequestOptions = {}) => {
	const query = {
		academicTermId: selectedTerm(academicTermId)
	} satisfies ListLearningOfferingsQuery;
	return deliveryData(
		apiClient.get<LearningOffering[]>('/api/academic/offerings', { ...options, query }),
		'ไม่สามารถโหลดรายการเปิดสอนได้'
	);
};
export const getLearningOffering = (id: string, options: ApiRequestOptions = {}) =>
	deliveryData(
		apiClient.get<LearningOffering>(`/api/academic/offerings/${id}`, options),
		'ไม่สามารถโหลดรายการเปิดสอนได้'
	);
export const createLearningOffering = (body: CreateLearningOfferingRequest) =>
	deliveryData(
		apiClient.post<LearningOffering>('/api/academic/offerings', body),
		'สร้างรายการเปิดสอนไม่สำเร็จ'
	);
export const updateLearningOffering = (id: string, body: UpdateLearningOfferingRequest) =>
	deliveryData(
		apiClient.patch<LearningOffering>(`/api/academic/offerings/${id}`, body),
		'แก้ไขรายการเปิดสอนไม่สำเร็จ'
	);
export const publishLearningOffering = (id: string, body: PublishLearningOfferingRequest) =>
	deliveryData(
		apiClient.post<LearningOffering>(`/api/academic/offerings/${id}/publish`, body),
		'เผยแพร่รายการเปิดสอนไม่สำเร็จ'
	);
export const previewLearningOfferingsFromCurriculum = (body: PreviewCurriculumOfferingsRequest) =>
	deliveryData(
		apiClient.post<CurriculumOfferingPreview>(
			'/api/academic/offerings/preview-from-curriculum',
			body
		),
		'สร้างตัวอย่างจากหลักสูตรไม่สำเร็จ'
	);
export const applyLearningOfferingsFromCurriculum = (body: ApplyCurriculumOfferingsRequest) =>
	deliveryData(
		apiClient.post<ApplyCurriculumOfferingsResult>(
			'/api/academic/offerings/apply-from-curriculum',
			body
		),
		'นำรายการเปิดสอนจากหลักสูตรมาใช้ไม่สำเร็จ'
	);

export const listLearningGroups = (offeringId: string, options: ApiRequestOptions = {}) =>
	deliveryData(
		apiClient.get<LearningGroup[]>(`/api/academic/offerings/${offeringId}/groups`, options),
		'ไม่สามารถโหลดกลุ่มเรียนได้'
	);
export const listLearningGroupsForTerm = (
	academicTermId: string,
	options: ApiRequestOptions = {}
) => {
	const query = {
		academicTermId: selectedTerm(academicTermId)
	} satisfies ListLearningGroupsForTermQuery;
	return deliveryData(
		apiClient.get<LearningGroup[]>('/api/academic/learning-groups', { ...options, query }),
		'ไม่สามารถโหลดกลุ่มเรียนของภาคเรียนได้'
	);
};
export const getLearningGroup = (id: string, options: ApiRequestOptions = {}) =>
	deliveryData(
		apiClient.get<LearningGroup>(`/api/academic/learning-groups/${id}`, options),
		'ไม่สามารถโหลดกลุ่มเรียนได้'
	);
export const createLearningGroup = (offeringId: string, body: CreateLearningGroupRequest) =>
	deliveryData(
		apiClient.post<LearningGroup>(`/api/academic/offerings/${offeringId}/groups`, body),
		'สร้างกลุ่มเรียนไม่สำเร็จ'
	);
export const updateLearningGroup = (id: string, body: UpdateLearningGroupRequest) =>
	deliveryData(
		apiClient.patch<LearningGroup>(`/api/academic/learning-groups/${id}`, body),
		'แก้ไขกลุ่มเรียนไม่สำเร็จ'
	);
export const replaceLearningGroupHomerooms = (
	id: string,
	body: ReplaceLearningGroupHomeroomsRequest
) =>
	deliveryData(
		apiClient.put<LearningGroup>(`/api/academic/learning-groups/${id}/homerooms`, body),
		'บันทึกห้องต้นทางไม่สำเร็จ'
	);
export const replaceLearningGroupTeachers = (
	id: string,
	body: ReplaceLearningGroupTeachersRequest
) =>
	deliveryData(
		apiClient.put<LearningGroup>(`/api/academic/learning-groups/${id}/teachers`, body),
		'บันทึกครูผู้สอนไม่สำเร็จ'
	);
export const previewLearningGroupRoster = (id: string) =>
	deliveryData(
		apiClient.get<RosterPreview>(`/api/academic/learning-groups/${id}/roster`),
		'สร้างตัวอย่างรายชื่อนักเรียนไม่สำเร็จ'
	);
export const applyLearningGroupRoster = (id: string, body: ApplyRosterRequest) =>
	deliveryData(
		apiClient.put<LearningGroup>(`/api/academic/learning-groups/${id}/roster`, body),
		'ยืนยันรายชื่อนักเรียนไม่สำเร็จ'
	);
export const publishLearningGroupRoster = (id: string, body: PublishRosterRequest) =>
	deliveryData(
		apiClient.post<LearningGroup>(`/api/academic/learning-groups/${id}/roster/publish`, body),
		'เผยแพร่รายชื่อนักเรียนไม่สำเร็จ'
	);
