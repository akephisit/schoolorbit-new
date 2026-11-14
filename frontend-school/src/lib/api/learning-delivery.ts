import {
	ApiClientError,
	apiClient,
	requireApiData,
	type ApiRequestOptions,
	type ApiResponse
} from '#lib/api/client.js';
import type { components, operations } from '#lib/api/generated/school-api.js';

type Schemas = components['schemas'];

export type DeliveryWorkspace = Schemas['DeliveryWorkspace'];
export type DeliveryOffering = Schemas['DeliveryOffering'];
export type DeliveryGroup = Schemas['DeliveryGroup'];
export type DeliveryTeacher = Schemas['DeliveryTeacher'];
export type DeliveryDeletionPreview = Schemas['DeliveryDeletionPreview'];
export type DeleteDeliveryResourceRequest = Schemas['DeleteDeliveryResourceRequest'];
export type DeletedDeliveryResource = Schemas['DeletedDeliveryResource'];
export type UpdateWeeklyPeriodTargetRequest = Schemas['UpdateWeeklyPeriodTargetRequest'];
export type LearningOffering = Schemas['LearningOffering'];
export type LearningDeliveryOverview = Schemas['LearningDeliveryOverview'];
export type LearningOfferingOverviewItem = Schemas['LearningOfferingOverviewItem'];
export type HomeroomDeliveryWorkspace = Schemas['HomeroomDeliveryWorkspace'];
export type HomeroomDeliveryRoom = Schemas['HomeroomDeliveryRoom'];
export type HomeroomDeliveryItem = Schemas['HomeroomDeliveryItem'];
export type CurriculumDeliveryAlignmentState = Schemas['CurriculumDeliveryAlignmentState'];
export type CurriculumDeliveryExtraOffering = Schemas['CurriculumDeliveryExtraOffering'];
export type DeliveryManagementOptions = Schemas['DeliveryManagementOptions'];
export type LearningGroup = Schemas['LearningGroup'];
export type LearningTeacherRole = Schemas['LearningTeacherRole'];
export type TeacherAssignment = Schemas['TeacherAssignmentInput'];
export type RosterPreview = Schemas['RosterPreview'];
export type CurriculumOfferingPreview = Schemas['CurriculumOfferingPreview'];
export type CurriculumPreparationProposal = Schemas['CurriculumPreparationProposal'];
export type CurriculumPreparationChoice = Schemas['CurriculumPreparationChoice'];
export type CurriculumGroupProposal = Schemas['CurriculumGroupProposal'];
export type ApplyCurriculumOfferingsResult = Schemas['ApplyCurriculumOfferingsResult'];
export type CreateLearningOfferingRequest = Schemas['CreateLearningOfferingRequest'];
export type UpdateLearningOfferingRequest = Schemas['UpdateLearningOfferingRequest'];
export type PreviewCurriculumOfferingsRequest = Schemas['PreviewCurriculumOfferingsRequest'];
export type ApplyCurriculumOfferingsRequest = Schemas['ApplyCurriculumOfferingsRequest'];
export type CreateLearningGroupRequest = Schemas['CreateLearningGroupRequest'];
export type UpdateLearningGroupRequest = Schemas['UpdateLearningGroupRequest'];
export type ReplaceLearningGroupHomeroomsRequest = Schemas['ReplaceLearningGroupHomeroomsRequest'];
export type ReplaceLearningGroupTeachersRequest = Schemas['ReplaceLearningGroupTeachersRequest'];
export type ApplyRosterRequest = Schemas['ApplyRosterRequest'];
export type DatedRosterMembership = Schemas['DatedRosterMembership'];
export type AddDatedRosterMembershipRequest = Schemas['AddDatedRosterMembershipRequest'];
export type RemoveDatedRosterMembershipRequest = Schemas['RemoveDatedRosterMembershipRequest'];

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
	operations['getDeliveryWorkspace']['parameters']['query']
>;
type DeliveryManagementOptionsQuery = NonNullable<
	operations['getLearningDeliveryManagementOptions']['parameters']['query']
>;
type HomeroomDeliveryWorkspaceQuery = NonNullable<
	operations['getHomeroomDeliveryWorkspace']['parameters']['query']
>;
type ListDatedRosterMembershipsOperation = operations['listDatedRosterMemberships'];
type AddDatedRosterMembershipOperation = operations['addDatedRosterMembership'];
type EndDatedRosterMembershipOperation = operations['endDatedRosterMembership'];

type OperationPath<T> = T extends { parameters: { path: infer Path } } ? Path : never;

export const getHomeroomDeliveryWorkspace = (
	academicYearId: string,
	academicTermId: string,
	options: ApiRequestOptions = {}
) => {
	const yearId = academicYearId.trim();
	if (!yearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	const query = {
		academicYearId: yearId,
		academicTermId: selectedTerm(academicTermId)
	} satisfies HomeroomDeliveryWorkspaceQuery;
	return deliveryData(
		apiClient.get<HomeroomDeliveryWorkspace>('/api/academic/delivery/homerooms', {
			...options,
			query
		}),
		'ไม่สามารถโหลดภาพรวมรายห้องประจำชั้นได้'
	);
};

export const getDeliveryWorkspace = (
	academicYearId: string,
	academicTermId: string,
	options: ApiRequestOptions = {}
) => {
	const query = {
		academicYearId,
		academicTermId: selectedTerm(academicTermId)
	} satisfies DeliveryWorkspaceQuery;
	return deliveryData(
		apiClient.get<DeliveryWorkspace>('/api/academic/delivery/workspace', { ...options, query }),
		'โหลดรายการเปิดสอนไม่สำเร็จ'
	);
};
export const updateWeeklyPeriodTarget = (id: string, body: UpdateWeeklyPeriodTargetRequest) =>
	deliveryData(
		apiClient.patch<LearningOffering>(
			`/api/academic/offerings/${encodeURIComponent(id)}/weekly-period-target`,
			body
		),
		'บันทึกจำนวนคาบไม่สำเร็จ'
	);
export const previewOfferingDeletion = (
	id: string,
	context: Schemas['DeliveryWorkspaceQuery'],
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.get<DeliveryDeletionPreview>(
			`/api/academic/offerings/${encodeURIComponent(id)}/deletion-preview`,
			{ ...options, query: context }
		),
		'ตรวจผลกระทบการลบไม่สำเร็จ'
	);
export const previewGroupDeletion = (
	id: string,
	context: Schemas['DeliveryWorkspaceQuery'],
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.get<DeliveryDeletionPreview>(
			`/api/academic/learning-groups/${encodeURIComponent(id)}/deletion-preview`,
			{ ...options, query: context }
		),
		'ตรวจผลกระทบการลบกลุ่มไม่สำเร็จ'
	);
export const deleteLearningOffering = (id: string, body: DeleteDeliveryResourceRequest) =>
	deliveryData(
		apiClient.deleteWithBody<DeletedDeliveryResource>(
			`/api/academic/offerings/${encodeURIComponent(id)}`,
			body
		),
		'ลบรายการเปิดสอนไม่สำเร็จ'
	);
export const deleteLearningGroup = (id: string, body: DeleteDeliveryResourceRequest) =>
	deliveryData(
		apiClient.deleteWithBody<DeletedDeliveryResource>(
			`/api/academic/learning-groups/${encodeURIComponent(id)}`,
			body
		),
		'ลบกลุ่มเรียนไม่สำเร็จ'
	);
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
export const listDatedRosterMemberships = (
	id: OperationPath<ListDatedRosterMembershipsOperation>['id'],
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.get<DatedRosterMembership[]>(
			`/api/academic/learning-groups/${id}/memberships`,
			options
		),
		'ไม่สามารถโหลดประวัติรายชื่อนักเรียนได้'
	);

export const addDatedRosterMembership = (
	id: OperationPath<AddDatedRosterMembershipOperation>['id'],
	body: AddDatedRosterMembershipRequest &
		AddDatedRosterMembershipOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.post<DatedRosterMembership>(`/api/academic/learning-groups/${id}/memberships`, body),
		'เพิ่มนักเรียนเข้ากลุ่มไม่สำเร็จ'
	);

export const endDatedRosterMembership = (
	id: OperationPath<EndDatedRosterMembershipOperation>['id'],
	membershipId: OperationPath<EndDatedRosterMembershipOperation>['membershipId'],
	body: RemoveDatedRosterMembershipRequest &
		EndDatedRosterMembershipOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.post<DatedRosterMembership>(
			`/api/academic/learning-groups/${id}/memberships/${membershipId}/end`,
			body
		),
		'กำหนดวันสิ้นสุดสมาชิกกลุ่มไม่สำเร็จ'
	);

export type LearningGroupRosterTracking = Schemas['LearningGroupRosterTracking'];
export type UpdateRosterTrackingRequest = Schemas['UpdateRosterTrackingRequest'];
export const getLearningGroupRosterTracking = (id: string, options: ApiRequestOptions = {}) =>
	deliveryData(
		apiClient.get<LearningGroupRosterTracking>(
			`/api/academic/learning-groups/${encodeURIComponent(id)}/roster-tracking`,
			options
		),
		'โหลดวิธีจัดรายชื่อไม่สำเร็จ'
	);
export const updateLearningGroupRosterTracking = (
	id: string,
	request: UpdateRosterTrackingRequest
) =>
	deliveryData(
		apiClient.put<LearningGroupRosterTracking>(
			`/api/academic/learning-groups/${encodeURIComponent(id)}/roster-tracking`,
			request
		),
		'บันทึกวิธีจัดรายชื่อไม่สำเร็จ'
	);

export type BulkGroupSetupRequest = Schemas['BulkGroupSetupRequest'];
export type BulkGroupSetupPreview = Schemas['BulkGroupSetupPreview'];
export type BulkGroupSetupResponse = Schemas['BulkGroupSetupResponse'];
export const previewBulkGroupSetup = (
	offeringId: string,
	request: BulkGroupSetupRequest,
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.post<BulkGroupSetupPreview>(
			`/api/academic/offerings/${encodeURIComponent(offeringId)}/groups/bulk-setup/preview`,
			request,
			options
		),
		'ตรวจการตั้งค่าหลายกลุ่มไม่สำเร็จ'
	);
export const applyBulkGroupSetup = (offeringId: string, request: BulkGroupSetupRequest) =>
	deliveryData(
		apiClient.put<BulkGroupSetupResponse>(
			`/api/academic/offerings/${encodeURIComponent(offeringId)}/groups/bulk-setup`,
			request
		),
		'บันทึกการตั้งค่าหลายกลุ่มไม่สำเร็จ'
	);
