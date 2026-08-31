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
export type PublishLearningOfferingRequest = Schemas['PublishLearningOfferingRequest'];
export type PreviewCurriculumOfferingsRequest = Schemas['PreviewCurriculumOfferingsRequest'];
export type ApplyCurriculumOfferingsRequest = Schemas['ApplyCurriculumOfferingsRequest'];
export type CreateLearningGroupRequest = Schemas['CreateLearningGroupRequest'];
export type UpdateLearningGroupRequest = Schemas['UpdateLearningGroupRequest'];
export type ReplaceLearningGroupHomeroomsRequest = Schemas['ReplaceLearningGroupHomeroomsRequest'];
export type ReplaceLearningGroupTeachersRequest = Schemas['ReplaceLearningGroupTeachersRequest'];
export type ApplyRosterRequest = Schemas['ApplyRosterRequest'];
export type PublishRosterRequest = Schemas['PublishRosterRequest'];
export type AcademicTermChangeSet = Schemas['AcademicTermChangeSet'];
export type AcademicTermChangeSetPreview = Schemas['AcademicTermChangeSetPreview'];
export type AcademicChangeFinding = Schemas['AcademicChangeFinding'];
export type AcademicChangeFindingCode = Schemas['AcademicChangeFindingCode'];
export type CreateAcademicTermChangeSetRequest = Schemas['CreateAcademicTermChangeSetRequest'];
export type UpdateAcademicTermChangeSetRequest = Schemas['UpdateAcademicTermChangeSetRequest'];
export type CancelAcademicTermChangeSetRequest = Schemas['CancelAcademicTermChangeSetRequest'];
export type UpsertAcademicTermChangeItemRequest = Schemas['UpsertAcademicTermChangeItemRequest'];
export type DeleteAcademicTermChangeItemRequest = Schemas['DeleteAcademicTermChangeItemRequest'];
export type PublishAcademicTermChangeSetRequest = Schemas['PublishAcademicTermChangeSetRequest'];
export type TeacherHandoffMode = Schemas['TeacherHandoffMode'];
export type PreviewTeacherHandoffRequest = Schemas['PreviewTeacherHandoffRequest'];
export type ApplyTeacherHandoffRequest = Schemas['ApplyTeacherHandoffRequest'];
export type TeacherHandoffPreview = Schemas['TeacherHandoffPreview'];
export type ApplyTeacherHandoffResponse = Schemas['ApplyTeacherHandoffResponse'];
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
	operations['getLearningDeliveryOverview']['parameters']['query']
>;
type DeliveryManagementOptionsQuery = NonNullable<
	operations['getLearningDeliveryManagementOptions']['parameters']['query']
>;
type HomeroomDeliveryWorkspaceQuery = NonNullable<
	operations['getHomeroomDeliveryWorkspace']['parameters']['query']
>;
type HomeroomDeliveryRequestOptions = ApiRequestOptions & { timetableVersionId?: string };
type ListAcademicTermChangeSetsQuery = NonNullable<
	operations['listAcademicTermChangeSets']['parameters']['query']
>;

type CreateAcademicTermChangeSetOperation = operations['createAcademicTermChangeSet'];
type GetAcademicTermChangeSetOperation = operations['getAcademicTermChangeSet'];
type UpdateAcademicTermChangeSetOperation = operations['updateAcademicTermChangeSet'];
type CancelAcademicTermChangeSetOperation = operations['cancelAcademicTermChangeSet'];
type UpsertAcademicTermChangeItemOperation = operations['upsertAcademicTermChangeItem'];
type DeleteAcademicTermChangeItemOperation = operations['deleteAcademicTermChangeItem'];
type PreviewAcademicTermChangeSetOperation = operations['previewAcademicTermChangeSet'];
type PublishAcademicTermChangeSetOperation = operations['publishAcademicTermChangeSet'];
type PreviewTeacherHandoffOperation = operations['previewTeacherHandoff'];
type ApplyTeacherHandoffOperation = operations['applyTeacherHandoff'];
type ListDatedRosterMembershipsOperation = operations['listDatedRosterMemberships'];
type AddDatedRosterMembershipOperation = operations['addDatedRosterMembership'];
type EndDatedRosterMembershipOperation = operations['endDatedRosterMembership'];

type OperationPath<T> = T extends { parameters: { path: infer Path } } ? Path : never;

function changeSetPath(id: OperationPath<GetAcademicTermChangeSetOperation>['id']): string {
	return `/api/academic/term-change-sets/${id}`;
}

export const getHomeroomDeliveryWorkspace = (
	academicYearId: string,
	academicTermId: string,
	options: HomeroomDeliveryRequestOptions = {}
) => {
	const yearId = academicYearId.trim();
	if (!yearId) throw new Error('กรุณาเลือกปีการศึกษาก่อน');
	const timetableVersionId = options.timetableVersionId?.trim();
	const { timetableVersionId: _selectedVersion, ...requestOptions } = options;
	const query = {
		academicYearId: yearId,
		academicTermId: selectedTerm(academicTermId),
		...(timetableVersionId ? { timetableVersionId } : {})
	} satisfies HomeroomDeliveryWorkspaceQuery;
	return deliveryData(
		apiClient.get<HomeroomDeliveryWorkspace>('/api/academic/delivery/homerooms', {
			...requestOptions,
			query
		}),
		'ไม่สามารถโหลดภาพรวมรายห้องประจำชั้นได้'
	);
};

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

export const listAcademicTermChangeSets = (
	academicTermId: string,
	options: ApiRequestOptions = {}
) => {
	const query = {
		academicTermId: selectedTerm(academicTermId)
	} satisfies ListAcademicTermChangeSetsQuery;
	return deliveryData(
		apiClient.get<AcademicTermChangeSet[]>('/api/academic/term-change-sets', {
			...options,
			query
		}),
		'ไม่สามารถโหลดรายการเปลี่ยนแปลงกลางภาคได้'
	);
};

export const createAcademicTermChangeSet = (
	body: CreateAcademicTermChangeSetRequest &
		CreateAcademicTermChangeSetOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.post<AcademicTermChangeSet>('/api/academic/term-change-sets', body),
		'สร้างแบบร่างการเปลี่ยนแปลงกลางภาคไม่สำเร็จ'
	);

export const getAcademicTermChangeSet = (
	id: OperationPath<GetAcademicTermChangeSetOperation>['id'],
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.get<AcademicTermChangeSet>(changeSetPath(id), options),
		'ไม่สามารถโหลดแบบร่างการเปลี่ยนแปลงกลางภาคได้'
	);

export const updateAcademicTermChangeSet = (
	id: OperationPath<UpdateAcademicTermChangeSetOperation>['id'],
	body: UpdateAcademicTermChangeSetRequest &
		UpdateAcademicTermChangeSetOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.patch<AcademicTermChangeSet>(changeSetPath(id), body),
		'แก้ไขแบบร่างการเปลี่ยนแปลงกลางภาคไม่สำเร็จ'
	);

export const cancelAcademicTermChangeSet = (
	id: OperationPath<CancelAcademicTermChangeSetOperation>['id'],
	body: CancelAcademicTermChangeSetRequest &
		CancelAcademicTermChangeSetOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.post<AcademicTermChangeSet>(`${changeSetPath(id)}/cancel`, body),
		'ยกเลิกแบบร่างการเปลี่ยนแปลงกลางภาคไม่สำเร็จ'
	);

export const upsertAcademicTermChangeItem = (
	id: OperationPath<UpsertAcademicTermChangeItemOperation>['id'],
	body: UpsertAcademicTermChangeItemRequest &
		UpsertAcademicTermChangeItemOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.put<AcademicTermChangeSet>(`${changeSetPath(id)}/items`, body),
		'บันทึกรายการเปลี่ยนแปลงไม่สำเร็จ'
	);

export const deleteAcademicTermChangeItem = (
	id: OperationPath<DeleteAcademicTermChangeItemOperation>['id'],
	itemId: OperationPath<DeleteAcademicTermChangeItemOperation>['itemId'],
	body: DeleteAcademicTermChangeItemRequest &
		DeleteAcademicTermChangeItemOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.deleteWithBody<AcademicTermChangeSet>(`${changeSetPath(id)}/items/${itemId}`, body),
		'ลบรายการเปลี่ยนแปลงไม่สำเร็จ'
	);

export const previewAcademicTermChangeSet = (
	id: OperationPath<PreviewAcademicTermChangeSetOperation>['id'],
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.get<AcademicTermChangeSetPreview>(`${changeSetPath(id)}/preview`, options),
		'ตรวจความพร้อมของการเปลี่ยนแปลงไม่สำเร็จ'
	);

export const publishAcademicTermChangeSet = (
	id: OperationPath<PublishAcademicTermChangeSetOperation>['id'],
	body: PublishAcademicTermChangeSetRequest &
		PublishAcademicTermChangeSetOperation['requestBody']['content']['application/json']
) =>
	deliveryData(
		apiClient.post<AcademicTermChangeSet>(`${changeSetPath(id)}/publish`, body),
		'เผยแพร่การเปลี่ยนแปลงกลางภาคไม่สำเร็จ'
	);

export const previewTeacherHandoff = (
	id: OperationPath<PreviewTeacherHandoffOperation>['id'],
	body: PreviewTeacherHandoffRequest &
		PreviewTeacherHandoffOperation['requestBody']['content']['application/json'],
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.post<TeacherHandoffPreview>(
			`${changeSetPath(id)}/teacher-handoff/preview`,
			body,
			options
		),
		'ตรวจผลกระทบของการส่งต่อคาบไม่สำเร็จ'
	);

export const applyTeacherHandoff = (
	id: OperationPath<ApplyTeacherHandoffOperation>['id'],
	body: ApplyTeacherHandoffRequest &
		ApplyTeacherHandoffOperation['requestBody']['content']['application/json'],
	options: ApiRequestOptions = {}
) =>
	deliveryData(
		apiClient.post<ApplyTeacherHandoffResponse>(
			`${changeSetPath(id)}/teacher-handoff/apply`,
			body,
			options
		),
		'ส่งต่อคาบให้ครูชุดใหม่ไม่สำเร็จ'
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
