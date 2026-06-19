import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '../../..');

async function readRepoFile(relativePath) {
	return readFile(path.join(repoRoot, relativePath), 'utf8');
}

async function listRepoFiles(relativeDir, predicate) {
	const entries = await readdir(path.join(repoRoot, relativeDir), { withFileTypes: true });
	return entries
		.filter((entry) => entry.isFile())
		.map((entry) => path.join(relativeDir, entry.name).replaceAll(path.sep, '/'))
		.filter(predicate)
		.sort();
}

test('project rules require a single JSON API response envelope', async () => {
	const source = await readRepoFile('.rules');

	assert.match(source, /API Response Contract/);
	assert.match(source, /success:\s*true/);
	assert.match(source, /data:\s*T/);
	assert.match(source, /success:\s*false/);
	assert.match(source, /error:\s*string/);
});

test('backend auth success handlers return enveloped data', async () => {
	const source = await readRepoFile('backend-school/src/modules/auth/handlers.rs');

	assert.doesNotMatch(source, /Json\(user_response\)/);
	assert.doesNotMatch(source, /Json\(profile_response\)/);
	assert.doesNotMatch(source, /Json\(LoginResponse\s*\{/);
	assert.match(source, /ApiResponse::with_message\(\s*LoginData\s*\{[\s\S]*?\buser:/);
	assert.match(source, /ApiResponse::ok\(user_response\)/);
	assert.match(source, /ApiResponse::ok\(profile_response\)/);
});

test('backend app errors return the shared error envelope', async () => {
	const errorSource = await readRepoFile('backend-school/src/error.rs');
	const responseSource = await readRepoFile('backend-school/src/api_response.rs');

	assert.match(responseSource, /struct\s+ApiErrorResponse/);
	assert.match(responseSource, /success:\s*false/);
	assert.match(responseSource, /pub\s+error:\s+String/);
	assert.match(errorSource, /ApiErrorResponse::new\(error_message\)/);
	assert.doesNotMatch(errorSource, /json!\s*\(\s*\{/);
});

test('frontend auth consumes the shared envelope through apiClient', async () => {
	const source = await readRepoFile('frontend-school/src/lib/api/auth.ts');

	assert.match(source, /import\s+\{[^}]*\bapiClient\b[^}]*\}\s+from\s+['"]\$lib\/api\/client['"]/);
	assert.doesNotMatch(source, /\bfetch\s*\(/);
	assert.doesNotMatch(source, /\b(getRaw|postRaw|putRaw)\b/);
	assert.match(source, /\.data\?\.user/);
});

test('user role assignment API contract stays aligned across backend and frontend', async () => {
	const backendModels = await readRepoFile('backend-school/src/modules/staff/models.rs');
	const backendService = await readRepoFile(
		'backend-school/src/modules/staff/services/user_role_service.rs'
	);
	const delegationService = await readRepoFile(
		'backend-school/src/modules/staff/services/organization_delegation_service.rs'
	);
	const staffService = await readRepoFile(
		'backend-school/src/modules/staff/services/staff_service.rs'
	);
	const frontendApi = await readRepoFile('frontend-school/src/lib/api/roles.ts');
	const frontendStaffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const frontendComponent = await readRepoFile(
		'frontend-school/src/lib/components/UserRoleManager.svelte'
	);
	const publicStaffPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/view/[id]/+page.svelte'
	);

	assert.match(backendModels, /struct\s+UserRoleAssignmentResponse/);
	assert.match(backendModels, /pub\s+role:\s+Role/);
	assert.match(backendService, /Result<Vec<UserRoleAssignmentResponse>,\s*AppError>/);
	assert.doesNotMatch(backendService, /Result<Vec<Role>/);
	assert.match(backendService, /FROM user_roles ur/);
	assert.match(backendService, /ur\.role_id/);
	assert.match(backendService, /LEFT JOIN role_permissions rp/);
	assert.match(backendService, /AS role_permissions/);
	assert.match(backendService, /role:\s+Role\s*\{/);

	assert.match(frontendApi, /interface\s+UserRoleAssignment/);
	assert.match(frontendApi, /role:\s+Role/);
	assert.match(
		frontendApi,
		/getUserRoles\(userId:\s*string\):\s*Promise<ApiResponse<UserRoleAssignment\[\]>>/
	);
	assert.doesNotMatch(frontendApi, /interface\s+UserRole\s*\{/);
	assert.match(frontendStaffApi, /permissions:\s*string\[\]/);
	assert.doesNotMatch(frontendStaffApi, /permissions:\s*Record<string,\s*unknown>/);
	assert.match(delegationService, /struct\s+DelegatablePermission/);
	assert.match(delegationService, /Result<Vec<DelegatablePermission>,\s*AppError>/);
	assert.doesNotMatch(delegationService, /Result<Vec<serde_json::Value>,\s*AppError>/);
	assert.match(staffService, /struct\s+PublicStaffProfile/);
	assert.match(staffService, /Result<PublicStaffProfile,\s*AppError>/);
	assert.doesNotMatch(
		staffService,
		/get_public_staff_profile[\s\S]*?Result<serde_json::Value,\s*AppError>/
	);
	assert.match(frontendStaffApi, /interface\s+PublicStaffProfileResponse/);
	assert.match(
		frontendStaffApi,
		/getPublicStaffProfile[\s\S]*ApiResponse<PublicStaffProfileResponse>/
	);

	assert.match(frontendComponent, /type\s+UserRoleAssignment/);
	assert.match(frontendComponent, /userRole\.role/);
	assert.doesNotMatch(frontendComponent, /getRoleById\(userRole\.role_id\)/);
	assert.match(publicStaffPage, /PublicStaffProfileResponse/);
});

test('admission application detail contract returns application and documents in data', async () => {
	const backendHandler = await readRepoFile(
		'backend-school/src/modules/admission/handlers/applications.rs'
	);
	const examRoomService = await readRepoFile(
		'backend-school/src/modules/admission/services/exam_room_service.rs'
	);
	const selectionService = await readRepoFile(
		'backend-school/src/modules/admission/services/selection_service.rs'
	);
	const portalService = await readRepoFile(
		'backend-school/src/modules/admission/services/portal_service.rs'
	);
	const applicationService = await readRepoFile(
		'backend-school/src/modules/admission/services/application_service.rs'
	);
	const frontendApi = await readRepoFile('frontend-school/src/lib/api/admission.ts');
	const portalStatusPage = await readRepoFile(
		'frontend-school/src/routes/(public)/apply/status/+page.svelte'
	);

	assert.match(
		backendHandler,
		/struct\s+ApplicationWithDocumentsData\s*\{[\s\S]*application:\s*AdmissionApplication,[\s\S]*documents:\s*Vec<ApplicationDocument>,[\s\S]*\}/
	);
	assert.match(
		backendHandler,
		/ApiResponse::ok\(ApplicationWithDocumentsData\s*\{[\s\S]*application,[\s\S]*documents,[\s\S]*\}\)/
	);
	assert.doesNotMatch(
		backendHandler,
		/"data":\s*\{\s*"items": application,\s*"documents": documents\s*\}/
	);

	assert.match(frontendApi, /interface\s+ApplicationDetailResponse/);
	assert.match(frontendApi, /application:\s*AdmissionApplication/);
	assert.match(frontendApi, /documents:\s*ApplicationDocument\[\]/);
	assert.match(frontendApi, /apiClient\.get<ApplicationDetailResponse>/);
	assert.doesNotMatch(
		frontendApi,
		/ApiResponse<AdmissionApplication>[\s\S]*documents\?: ApplicationDocument\[\]/
	);

	assert.match(
		backendHandler,
		/#\[serde\(rename_all = "camelCase"\)\][\s\S]*struct\s+SubmitApplicationData\s*\{[\s\S]*application_number:\s*String,/
	);
	assert.doesNotMatch(backendHandler, /"application_number": application_number/);
	assert.match(frontendApi, /apiClient\.post<\{\s*applicationNumber:\s*string\s*\}>/);
	assert.match(frontendApi, /interface\s+PortalStatusResult/);
	assert.match(frontendApi, /application:\s*AdmissionApplication/);
	assert.match(frontendApi, /assignment:\s*RoomAssignment \| null/);
	assert.match(frontendApi, /scores:\s*ExamScore\[\] \| null/);
	assert.match(frontendApi, /enrollmentForm:\s*EnrollmentForm \| null/);
	assert.match(
		frontendApi,
		/portalGetStatus[\s\S]*requireApiData\(res,\s*'ไม่สามารถโหลดสถานะใบสมัครได้'\)/
	);
	assert.match(portalStatusPage, /PortalStatusResult/);

	assert.match(
		backendHandler,
		/#\[serde\(rename_all = "camelCase"\)\][\s\S]*struct\s+CompleteEnrollmentData\s*\{[\s\S]*user_id:\s*Uuid,[\s\S]*student_code:\s*String,/
	);
	assert.match(backendHandler, /user_id:\s*result\.user_id/);
	assert.match(backendHandler, /student_code:\s*result\.student_code/);
	assert.doesNotMatch(backendHandler, /"user_id": result\.user_id/);
	assert.doesNotMatch(backendHandler, /"student_code": result\.student_code/);
	assert.match(frontendApi, /interface\s+CompleteEnrollmentResponse/);
	assert.match(frontendApi, /apiClient\.post<CompleteEnrollmentResponse>/);
	assert.match(
		frontendApi,
		/copyExamRoomsFromRound[\s\S]*res\.message \?\? 'copy ห้องสอบเรียบร้อย'/
	);
	assert.match(
		frontendApi,
		/assignExamSeats[\s\S]*message: res\.message \?\? 'จัดที่นั่งสอบเรียบร้อย'/
	);
	assert.match(frontendApi, /apiClient\.post<\{\s*updated:\s*number\s*\}>/);
	assert.match(frontendApi, /sortRoomStudents[\s\S]*res\.data\?\.updated \?\? 0/);
	assert.match(frontendApi, /apiClient\.post<\{\s*assigned:\s*number\s*\}>/);
	assert.match(frontendApi, /autoAssignStudentIds[\s\S]*res\.data\?\.assigned \?\? 0/);
	assert.match(
		frontendApi,
		/apiClient\.post<ExamSeatDetail \| null>\('\/api\/admission\/portal\/exam-seat'/
	);
	assert.match(frontendApi, /apiClient\.get<ExamRoomsResponse>/);
	assert.match(frontendApi, /interface\s+RoundRankingResult/);
	assert.match(frontendApi, /apiClient\.get<RoundRankingResult\[\]>/);
	assert.match(frontendApi, /apiClient\.get<TrackRankingResult>/);
	assert.match(frontendApi, /apiClient\.get<GlobalRankingResult>/);
	assert.match(frontendApi, /apiClient\.patch<\{\s*updated:\s*number\s*\}>/);
	assert.doesNotMatch(frontendApi, /ApiResponse<unknown>/);
	assert.doesNotMatch(frontendApi, /apiClient\.get<unknown\[\]>/);
	assert.doesNotMatch(frontendApi, /res\.data as/);

	assert.match(examRoomService, /struct\s+ExamConfigStorage/);
	assert.match(examRoomService, /struct\s+ExamConfigResponse/);
	assert.match(examRoomService, /struct\s+AssignSeatsRoomSummary/);
	assert.match(examRoomService, /Result<ExamConfigResponse,\s*AppError>/);
	assert.match(examRoomService, /pub\s+rooms:\s+Vec<AssignSeatsRoomSummary>/);
	assert.doesNotMatch(
		examRoomService,
		/get_exam_config[\s\S]*?Result<serde_json::Value,\s*AppError>/
	);
	assert.doesNotMatch(examRoomService, /config\["exam_id_type"\]/);
	assert.doesNotMatch(examRoomService, /json!\(\{\s*"roomName"/);

	assert.match(selectionService, /struct\s+RoundRankingResult/);
	assert.match(selectionService, /struct\s+TrackRankingResult/);
	assert.match(selectionService, /struct\s+GlobalRankingResult/);
	assert.match(
		selectionService,
		/get_round_ranking[\s\S]*?Result<Vec<RoundRankingResult>,\s*AppError>/
	);
	assert.match(selectionService, /get_track_ranking[\s\S]*?Result<TrackRankingResult,\s*AppError>/);
	assert.match(
		selectionService,
		/get_global_ranking[\s\S]*?Result<GlobalRankingResult,\s*AppError>/
	);
	assert.doesNotMatch(
		selectionService,
		/get_round_ranking[\s\S]*?Result<Vec<serde_json::Value>,\s*AppError>/
	);
	assert.doesNotMatch(
		selectionService,
		/get_track_ranking[\s\S]*?Result<serde_json::Value,\s*AppError>/
	);
	assert.doesNotMatch(
		selectionService,
		/get_global_ranking[\s\S]*?Result<serde_json::Value,\s*AppError>/
	);
	assert.match(portalService, /struct\s+PortalStatusResult/);
	assert.match(portalService, /get_status[\s\S]*?Result<PortalStatusResult,\s*AppError>/);
	assert.doesNotMatch(portalService, /get_status[\s\S]*?Result<serde_json::Value,\s*AppError>/);
	assert.match(applicationService, /struct\s+DocumentUploadResponse/);
	assert.match(
		applicationService,
		/document_upload_response[\s\S]*?Result<DocumentUploadResponse,\s*AppError>/
	);
	assert.doesNotMatch(applicationService, /document_upload_response_json/);
});

test('parent self-service API uses typed student and timetable responses', async () => {
	const parentsApi = await readRepoFile('frontend-school/src/lib/api/parents.ts');
	const childPage = await readRepoFile(
		'frontend-school/src/routes/(app)/parent/student/[id]/+page.svelte'
	);
	const timetablePage = await readRepoFile(
		'frontend-school/src/routes/(app)/parent/student/[id]/timetable/+page.svelte'
	);

	assert.match(parentsApi, /import type \{ Student \} from '\.\/students'/);
	assert.match(parentsApi, /getChildProfile[\s\S]*Promise<LoadedApiResponse<Student>>/);
	assert.match(parentsApi, /apiClient\.get<Student>/);
	assert.match(
		parentsApi,
		/getChildTimetable[\s\S]*Promise<LoadedApiResponse<TimetableEntry\[\]>>/
	);
	assert.doesNotMatch(parentsApi, /apiClient\.get<unknown>/);
	assert.doesNotMatch(parentsApi, /return response as/);

	assert.match(childPage, /import type \{ Student \} from '\$lib\/api\/students'/);
	assert.match(childPage, /student = response\.data/);
	assert.doesNotMatch(childPage, /response\.data as/);
	assert.match(timetablePage, /child = childRes\.data/);
	assert.doesNotMatch(timetablePage, /childData as/);
});

test('school settings API consumes typed envelope data without casts', async () => {
	const schoolApi = await readRepoFile('frontend-school/src/lib/api/school.ts');

	assert.match(schoolApi, /apiClient\.get<SchoolSettings>/);
	assert.match(schoolApi, /apiClient\.patch<Record<string, never>>/);
	assert.match(schoolApi, /apiClient\.delete<Record<string, never>>/);
	assert.match(schoolApi, /apiClient\.get<PublicSchoolInfo>/);
	assert.doesNotMatch(schoolApi, /res\.data as/);
});

test('work inbox API uses typed envelope data and SSE only signals refresh', async () => {
	const workApi = await readRepoFile('frontend-school/src/lib/api/work.ts');
	const workStore = await readRepoFile('frontend-school/src/lib/stores/work.ts');
	const notificationStore = await readRepoFile('frontend-school/src/lib/stores/notification.ts');
	const sidebar = await readRepoFile('frontend-school/src/lib/components/layout/Sidebar.svelte');
	const workInboxPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/work/+page.svelte'
	);
	const workManagePage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/work/manage/+page.svelte'
	);

	assert.match(workApi, /export\s+type\s+WorkItemState/);
	assert.match(workApi, /export\s+interface\s+WorkItem\s*\{/);
	assert.match(workApi, /export\s+interface\s+WorkItemCounts\s*\{/);
	assert.match(workApi, /apiClient\.get<\{\s*items:\s*WorkItem\[\]\s*\}>/);
	assert.match(workApi, /apiClient\.get<WorkItemCounts>/);
	assert.match(workApi, /apiClient\.post<\{\s*id:\s*string\s*\}>/);
	assert.match(workApi, /listManageableWorkflowWindows/);
	assert.match(workApi, /createWorkflowWindow/);
	assert.match(workApi, /updateWorkflowWindowStatus/);
	assert.match(workApi, /apiClient\.get<\{\s*items:\s*WorkflowWindow\[\]\s*\}>/);
	assert.match(workApi, /apiClient\.post<WorkflowWindow>/);
	assert.match(workApi, /apiClient\.patch<WorkflowWindow>/);
	assert.doesNotMatch(workApi, /ApiResponse<unknown>/);
	assert.doesNotMatch(workApi, /Record<string,\s*unknown>/);
	assert.doesNotMatch(workApi, /res\.data as/);

	assert.match(workStore, /getMyWorkCounts/);
	assert.match(workStore, /getMyWorkItems/);
	assert.match(workStore, /refreshSilently/);
	assert.doesNotMatch(workStore, /\bfetch\s*\(/);

	assert.match(notificationStore, /addEventListener\(['"]work_items_changed['"]/);
	assert.match(notificationStore, /addEventListener\(['"]workflow_window_changed['"]/);
	assert.match(notificationStore, /workStore\.refreshSilently\(\)/);
	assert.doesNotMatch(
		notificationStore,
		/addEventListener\(['"]work_items_changed['"],\s*\([^)]*event/
	);
	assert.doesNotMatch(
		notificationStore,
		/addEventListener\(['"]workflow_window_changed['"],\s*\([^)]*event/
	);

	assert.match(sidebar, /workStore/);
	assert.match(sidebar, /\/staff\/work/);
	assert.match(workInboxPage, /from '\$lib\/stores\/permissions'/);
	assert.match(workInboxPage, /\$can\.hasWorkflowManage\(\)/);
	assert.match(workInboxPage, /\/staff\/work\/manage/);
	assert.doesNotMatch(workInboxPage, /PERMISSION_MODULES\.ORGANIZATION_WORK/);
	assert.match(workManagePage, /listManageableWorkflowWindows/);
	assert.match(workManagePage, /createWorkflowWindow/);
	assert.match(workManagePage, /createWorkItem/);
	assert.match(workManagePage, /lookupStaff/);
	assert.match(workManagePage, /lookupOrganizationUnits/);
	assert.match(workManagePage, /from '\$lib\/components\/ui\/select'/);
	assert.match(workManagePage, /<Select\.Root/);
	assert.doesNotMatch(workManagePage, /<select\b/);
	assert.doesNotMatch(workManagePage, /\bfetch\s*\(/);
});

test('teaching supervision frontend contract uses typed API and permission metadata', async () => {
	const supervisionApi = await readRepoFile('frontend-school/src/lib/api/supervision.ts');
	const supervisionRoute = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.ts'
	);
	const supervisionPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/supervision/+page.svelte'
	);

	assert.match(supervisionApi, /export\s+type\s+SupervisionObservationStatus/);
	assert.match(supervisionApi, /apiClient\.get<\{\s*items:\s*SupervisionCycle\[\]\s*\}>/);
	assert.match(supervisionApi, /apiClient\.post<SupervisionObservation>/);
	assert.doesNotMatch(supervisionApi, /ApiResponse<unknown>/);
	assert.doesNotMatch(supervisionApi, /Record<string,\s*unknown>/);
	assert.doesNotMatch(supervisionApi, /res\.data as/);
	assert.match(supervisionRoute, /PERMISSION_MODULES\.SUPERVISION/);
	assert.match(supervisionPage, /listSupervisionCycles/);
	assert.match(supervisionPage, /requestSupervisionObservation/);
	assert.match(supervisionPage, /updateSupervisionCycle/);
	assert.match(supervisionPage, /approveSupervisionObservationRequest/);
	assert.match(supervisionPage, /saveMySupervisionEvaluation/);
	assert.match(supervisionPage, /acknowledgeSupervisionObservation/);
	assert.match(supervisionPage, /getMyTimetable/);
	assert.match(supervisionPage, /academic_semester_id:\s*cycle\?\.academicSemesterId/);
	assert.match(supervisionPage, /getSchoolDays/);
	assert.match(supervisionPage, /timetableGridDays/);
	assert.match(supervisionPage, /timetablePeriodRows/);
	assert.match(supervisionPage, /selectTimetableEntry/);
	assert.match(supervisionPage, /entry\.period_order_index/);
	assert.doesNotMatch(supervisionPage, /period_name\?\.match\(/);
	assert.match(supervisionPage, /class="overflow-x-auto rounded-md border"/);
	assert.match(
		supervisionPage,
		/<Table\.Head\s+class="sticky left-0 z-10 w-\[112px\] bg-background"[\s\S]*>วัน<\/Table\.Head/
	);
	assert.match(
		supervisionPage,
		/<Table\.Header>[\s\S]*\{#each timetablePeriodRows\(\) as row \(row\.key\)\}/
	);
	assert.match(
		supervisionPage,
		/<Table\.Body>[\s\S]*\{#each timetableSchoolDays as day \(day\.value\)\}/
	);
	assert.doesNotMatch(
		supervisionPage,
		/grid gap-2 md:hidden[\s\S]*timetableEntriesForSelectedCycle/
	);
	assert.match(supervisionPage, /cycleStatusCreateOptions/);
	assert.match(supervisionPage, /status:\s*cycleForm\.status/);
	assert.match(supervisionPage, /setCycleStatus/);
	assert.match(supervisionPage, /createPaperSupervisionRubricSections/);
	assert.match(supervisionPage, /templateForm\.sections/);
	assert.match(supervisionPage, /addTemplateSection/);
	assert.match(supervisionPage, /addTemplateItem/);
	assert.match(supervisionPage, /moveTemplateItem/);
	assert.match(supervisionPage, /calculateRubricDraftSummary/);
	assert.match(supervisionPage, /sectionRubricProgress/);
	assert.doesNotMatch(supervisionPage, /ratingLabel/);
	assert.doesNotMatch(supervisionPage, /textLabel/);
	assert.match(supervisionPage, /canManageSchool/);
	assert.match(supervisionPage, /canManageRequests/);
	assert.match(supervisionPage, /canReadObservations/);
	assert.match(supervisionPage, /SUPERVISION_READ_OWN/);
	assert.match(supervisionPage, /SUPERVISION_READ_ASSIGNED/);
	assert.match(supervisionPage, /SUPERVISION_READ_ORGANIZATION_UNIT/);
	assert.match(supervisionPage, /SUPERVISION_READ_ORGANIZATION_TREE/);
	assert.match(supervisionPage, /SUPERVISION_READ_SCHOOL/);
	assert.match(supervisionPage, /SUPERVISION_MANAGE_ORGANIZATION_UNIT/);
	assert.match(supervisionPage, /SUPERVISION_MANAGE_ORGANIZATION_TREE/);
	assert.match(
		supervisionPage,
		/canReadObservations[\s\S]*await\s+listSupervisionObservations\(\)[\s\S]*:\s*\[\]/
	);
	assert.match(supervisionPage, /canManageRequests[\s\S]*await\s+lookupStaff/);
	assert.match(supervisionPage, /getAcademicStructure/);
	assert.match(supervisionPage, /\* as Select/);
	assert.match(supervisionPage, /\* as Dialog/);
	assert.match(supervisionPage, /\* as Table/);
	assert.match(supervisionPage, /\* as Alert/);
	assert.match(supervisionPage, /Progress/);
	assert.doesNotMatch(supervisionPage, /<select\b/);
	assert.doesNotMatch(supervisionPage, /type="datetime-local"/);
	assert.doesNotMatch(supervisionPage, /status:\s*'draft',\s*\n\s*targets:/);
	assert.doesNotMatch(supervisionPage, /Select\.Root[^>]*bind:value=\{selectedTimetableEntryId\}/);
	assert.doesNotMatch(
		supervisionPage,
		/Promise\.all\(\[\s*listSupervisionCycles\(\),\s*listSupervisionTemplates\(\),\s*listSupervisionObservations\(\),\s*lookupStaff/
	);
	assert.doesNotMatch(supervisionPage, /\bfetch\s*\(/);
});

test('scheduling API uses backend envelope data types without response casts', async () => {
	const schedulingApi = await readRepoFile('frontend-school/src/lib/api/scheduling.ts');

	assert.match(
		schedulingApi,
		/updateInstructorConstraints[\s\S]*apiClient\.put<Record<string, never>>/
	);
	assert.match(
		schedulingApi,
		/reorderInstructorPriority[\s\S]*apiClient\.put<Record<string, never>>/
	);
	assert.match(schedulingApi, /updateSchoolSettings[\s\S]*apiClient\.put<Record<string, never>>/);
	assert.match(
		schedulingApi,
		/updateSubjectConstraints[\s\S]*apiClient\.put<Record<string, never>>/
	);
	assert.match(
		schedulingApi,
		/updateClassroomCourseConstraints[\s\S]*apiClient\.put<Record<string, never>>/
	);
	assert.match(schedulingApi, /setCcPreferredRooms[\s\S]*apiClient\.put<Record<string, never>>/);
	assert.match(
		schedulingApi,
		/updateTimetableTemplate[\s\S]*apiClient\.put<Record<string, never>>/
	);
	assert.match(
		schedulingApi,
		/deleteTimetableTemplate[\s\S]*apiClient\.delete<Record<string, never>>/
	);
	assert.match(schedulingApi, /apiClient\.deleteWithBody<\{\s*deleted:\s*number\s*\}>/);
	assert.doesNotMatch(schedulingApi, /apiClient\.(put|delete)<unknown>/);
	assert.doesNotMatch(
		schedulingApi,
		/return response as \{ success: boolean; data: \{ deleted: number \} \}/
	);
});

test('facility API returns typed loaded envelope data without helper casts', async () => {
	const facilityApi = await readRepoFile('frontend-school/src/lib/api/facility.ts');

	assert.match(facilityApi, /type\s+LoadedApiResponse<T>/);
	assert.match(facilityApi, /Promise<LoadedApiResponse<T>>/);
	assert.match(
		facilityApi,
		/return \{ success: true, data: response\.data, message: response\.message \}/
	);
	assert.match(facilityApi, /fetchApi<Building\[\]>/);
	assert.match(facilityApi, /fetchApi<Room\[\]>/);
	assert.match(facilityApi, /fetchApi<Record<string, never>>/);
	assert.doesNotMatch(facilityApi, /return response as T/);
});

test('timetable API exposes typed loaded responses and conflict unions without response casts', async () => {
	const timetableApi = await readRepoFile('frontend-school/src/lib/api/timetable.ts');
	const timetableService = await readRepoFile(
		'backend-school/src/modules/academic/services/timetable_service.rs'
	);
	const timetablePage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte'
	);

	assert.match(timetableApi, /type\s+LoadedApiResponse<T>/);
	assert.match(timetableApi, /Promise<LoadedApiResponse<T>>/);
	assert.match(timetableApi, /interface\s+TimetableConflictResponse/);
	assert.match(timetableApi, /type\s+TimetableMutationResponse/);
	assert.match(timetableApi, /apiClient\.post<TimetableEntry \| ConflictPayload>/);
	assert.match(timetableApi, /apiClient\.put<TimetableEntry \| ConflictPayload>/);
	assert.match(timetableApi, /fetchApi<AcademicPeriod\[\]>/);
	assert.match(timetableApi, /period_order_index\?:\s*number/);
	assert.match(timetableApi, /fetchApi<MoveValidityCell\[\]>/);
	assert.match(timetableApi, /fetchApi<OccupancyEntry\[\]>/);
	assert.match(timetableApi, /interface\s+MyActivityForEntry/);
	assert.match(timetableApi, /fetchApi<MyActivityForEntry \| null>/);
	assert.doesNotMatch(timetableApi, /return response as T/);
	assert.doesNotMatch(timetableApi, /ApiResponse<unknown>/);
	assert.doesNotMatch(timetableApi, /response\.data as/);
	assert.match(timetableService, /struct\s+MyActivityForEntry/);
	assert.match(timetableService, /ap\.order_index\s+AS\s+period_order_index/);
	assert.match(
		timetableService,
		/get_my_activity_for_entry[\s\S]*?Result<Option<MyActivityForEntry>,\s*AppError>/
	);
	assert.doesNotMatch(
		timetableService,
		/get_my_activity_for_entry[\s\S]*?Result<serde_json::Value,\s*AppError>/
	);
	assert.match(timetableService, /struct\s+BatchSkippedCell/);
	assert.match(timetableService, /pub\s+skipped:\s+Vec<BatchSkippedCell>/);
	assert.match(timetableService, /pub\s+blocked:\s+Vec<BatchBlockedCell>/);
	assert.match(timetableService, /pub\s+deleted:\s+Vec<BatchDeletedEntry>/);
	assert.match(timetableService, /pub\s+excluded_instructors:\s+Vec<BatchExcludedInstructor>/);
	assert.doesNotMatch(timetableService, /pub\s+skipped:\s+Vec<serde_json::Value>/);
	assert.match(timetableService, /conflicts:\s+Vec<ConflictInfo>/);
	assert.match(timetableService, /let mut conflict_list:\s+Vec<ConflictInfo>/);
	assert.doesNotMatch(timetableService, /let mut conflict_list:\s+Vec<serde_json::Value>/);

	assert.doesNotMatch(timetablePage, /await createTimetableEntry\([^)]*\)\) as/);
	assert.doesNotMatch(timetablePage, /await updateTimetableEntry\([^)]*\)\) as/);
	assert.doesNotMatch(
		timetablePage,
		/res as \{ success\?: boolean; conflicts\?: ConflictInfo\[\] \}/
	);
});

test('academic API uses typed loaded responses and unwraps generate-plan payloads', async () => {
	const academicApi = await readRepoFile('frontend-school/src/lib/api/academic.ts');
	const planningPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/planning/+page.svelte'
	);
	const activitiesPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/activities/+page.svelte'
	);

	assert.match(academicApi, /type\s+LoadedApiResponse<T>/);
	assert.match(academicApi, /Promise<LoadedApiResponse<T>>/);
	assert.match(
		academicApi,
		/return \{ success: true, data: response\.data, message: response\.message \}/
	);
	assert.match(academicApi, /fetchApi<AcademicStructureData>/);
	assert.match(academicApi, /fetchApi<ClassroomCourse\[\]>/);
	assert.match(academicApi, /fetchApi<StudyPlan\[\]>/);
	assert.match(academicApi, /fetchApi<ActivitySlot\[\]>/);
	assert.match(academicApi, /fetchApi<ActivityGroup\[\]>/);
	assert.match(academicApi, /fetchApi<GenerateCoursesFromPlanResponse>/);
	assert.match(academicApi, /return response\.data/);
	assert.match(academicApi, /fetchApi<GenerateActivitiesFromPlanResponse>/);
	assert.doesNotMatch(academicApi, /return response as T/);
	assert.doesNotMatch(academicApi, /ApiResponse<unknown>/);
	assert.doesNotMatch(academicApi, /res\.data as/);

	assert.match(planningPage, /result\.courses_created \?\? result\.items\.added_count/);
	assert.doesNotMatch(planningPage, /result\.data\.added_count/);
	assert.match(activitiesPage, /res\.created/);
});

test('academic course instructor batch API sends ids in POST body', async () => {
	const academicApi = await readRepoFile('frontend-school/src/lib/api/academic.ts');

	assert.match(
		academicApi,
		/batchListCourseInstructors[\s\S]*?fetchApi<Record<string,\s*CourseInstructor\[\]>>\(\s*['"]\/api\/academic\/planning\/courses\/instructors\/batch['"][\s\S]*?method:\s*['"]POST['"][\s\S]*?body:\s*JSON\.stringify\(\{\s*course_ids:\s*courseIds\s*\}\)/
	);
	assert.doesNotMatch(
		academicApi,
		/batchListCourseInstructors[\s\S]*?new URLSearchParams\(\{\s*course_ids:\s*courseIds\.join\(['"],['"]\)\s*\}\)/
	);
});

test('frontend API contracts use named dynamic JSON types instead of raw Record unknown', async () => {
	const rules = await readRepoFile('.rules');
	const checkedApiFiles = await listRepoFiles(
		'frontend-school/src/lib/api',
		(relativePath) => relativePath.endsWith('.ts') && !relativePath.endsWith('/client.ts')
	);
	const forbiddenPatterns = [
		[
			/Record<string,\s*unknown>/,
			'use a named dynamic JSON contract instead of Record<string, unknown>'
		],
		[/ApiResponse<unknown>/, 'use a concrete ApiResponse<T> contract'],
		[
			/apiClient\.(?:get|post|put|patch|delete)<unknown(?:\[\])?>/,
			'use concrete apiClient<T> generics'
		],
		[/fetchApi<unknown(?:\[\])?>/, 'use concrete fetchApi<T> generics'],
		[/\b(?:res|response)\.data\s+as\b/, 'type the API response instead of casting response.data'],
		[/return\s+response\s+as\b/, 'return a typed envelope instead of casting the full response']
	];

	assert.match(rules, /named contract/);
	assert.match(rules, /Record<string,\s*unknown>/);
	assert.ok(
		checkedApiFiles.includes('frontend-school/src/lib/api/academic.ts'),
		'frontend API contract guard should scan academic.ts'
	);
	assert.ok(
		checkedApiFiles.includes('frontend-school/src/lib/api/admission.ts'),
		'frontend API contract guard should scan admission.ts'
	);
	assert.ok(
		!checkedApiFiles.includes('frontend-school/src/lib/api/client.ts'),
		'apiClient envelope parser is the only frontend API file allowed to inspect unknown JSON'
	);

	for (const relativePath of checkedApiFiles) {
		const source = await readRepoFile(relativePath);
		for (const [pattern, message] of forbiddenPatterns) {
			assert.doesNotMatch(source, pattern, `${relativePath}: ${message}`);
		}
	}
});
