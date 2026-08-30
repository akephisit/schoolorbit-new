import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';
import ts from 'typescript';

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

function extractObjectBlock(source, marker) {
	const markerStart = source.indexOf(marker);
	assert.notEqual(markerStart, -1, `missing generated block marker: ${marker}`);
	const opening = source.indexOf('{', markerStart);
	assert.notEqual(opening, -1, `missing opening brace after: ${marker}`);
	let depth = 0;
	for (let index = opening; index < source.length; index += 1) {
		if (source[index] === '{') depth += 1;
		if (source[index] === '}') depth -= 1;
		if (depth === 0) return source.slice(opening, index + 1);
	}
	assert.fail(`unterminated generated block: ${marker}`);
}

function extractGeneratedSchemaBlock(source, schemaName) {
	const escapedName = schemaName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
	const property = new RegExp(`^[\\t ]*${escapedName}:\\s*\\{`, 'm').exec(source);
	assert.ok(property, `missing generated schema property: ${schemaName}`);
	return extractObjectBlock(source, property[0]);
}

async function importStaffApiWithRequestRecorder(requests) {
	const source = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const compiled = ts.transpileModule(source, {
		compilerOptions: {
			module: ts.ModuleKind.ESNext,
			target: ts.ScriptTarget.ES2022,
			verbatimModuleSyntax: true
		},
		fileName: 'staff.ts'
	}).outputText;
	const recorderKey = `__staffApiRequests_${process.pid}_${Math.random()}`;
	globalThis[recorderKey] = requests;
	const clientModule = [
		`const requests = globalThis[${JSON.stringify(recorderKey)}];`,
		'export const apiClient = {',
		'  async get(url) { requests.push(url); return { success: true, data: { totalStaff: 1, totalStudents: 2, activeHomerooms: 3 } }; }',
		'};',
		'export function requireApiData(response) { return response.data; }'
	].join('\n');
	const clientUrl = `data:text/javascript;base64,${Buffer.from(clientModule).toString('base64')}`;
	const moduleUrl = `data:text/javascript;base64,${Buffer.from(
		compiled.replace('$lib/api/client', clientUrl)
	).toString('base64')}#${Date.now()}`;

	try {
		return await import(moduleUrl);
	} finally {
		delete globalThis[recorderKey];
	}
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
	const sessionSource = await readRepoFile('backend-school/src/modules/auth/session_handlers.rs');
	const profileSource = await readRepoFile('backend-school/src/modules/auth/handlers.rs');

	assert.doesNotMatch(sessionSource, /Json\(current_user_response\)/);
	assert.doesNotMatch(profileSource, /Json\(profile_response\)/);
	assert.doesNotMatch(sessionSource, /Json\(LoginResponse\s*\{/);
	assert.match(sessionSource, /LoginData\s*\{\s*user:\s*current_user_response/);
	assert.match(sessionSource, /Json\(ApiResponse::ok\(data\)\)/);
	assert.match(sessionSource, /ApiResponse::ok\(current_user_response\(user\)\)/);
	assert.match(sessionSource, /ApiResponse::ok\(SessionListData\s*\{\s*sessions\s*\}\)/);
	assert.match(profileSource, /ApiResponse::ok\(profile_response\)/);
});

test('backend app errors return the shared error envelope', async () => {
	const errorSource = await readRepoFile('backend-school/src/error.rs');
	const responseSource = await readRepoFile('backend-school/src/api_response.rs');

	assert.match(responseSource, /struct\s+ApiErrorResponse/);
	assert.match(responseSource, /success:\s*false/);
	assert.match(responseSource, /pub\s+error:\s+String/);
	assert.match(errorSource, /ApiErrorResponse::new\(self\.public_message\(\)\.to_string\(\)\)/);
	assert.doesNotMatch(errorSource, /json!\s*\(\s*\{/);
});

test('frontend API client preserves typed error data on thrown errors', async () => {
	const source = await readRepoFile('frontend-school/src/lib/api/client.ts');

	assert.match(source, /interface\s+ApiResponse<T,\s*E\s*=\s*never>/);
	assert.match(source, /errorData\?:\s*E/);
	assert.match(source, /class\s+ApiClientError<E\s*=\s*never>/);
	assert.match(source, /readonly\s+data\?:\s*E/);
	assert.match(source, /errorData:\s*payload\.data\s+as\s+E/);
	assert.match(source, /new\s+ApiClientError<E>[\s\S]*response\.errorData/);
});

test('frontend auth consumes the shared envelope through apiClient', async () => {
	const source = await readRepoFile('frontend-school/src/lib/api/auth.ts');

	assert.match(source, /import\s+\{[^}]*\bapiClient\b[^}]*\}\s+from\s+['"]\$lib\/api\/client['"]/);
	assert.match(
		source,
		/import\s+type\s+\{\s*components\s*\}\s+from\s+['"]\$lib\/api\/generated\/school-api['"]/
	);
	assert.match(
		source,
		/type\s+Schemas\s*=\s*components\['schemas'\][\s\S]*export\s+type\s+CurrentUserDto\s*=\s*Schemas\['CurrentUserResponse'\]/
	);
	assert.match(source, /export\s+type\s+SessionDto\s*=\s*Schemas\['SessionResponse'\]/);
	assert.match(source, /type\s+SessionListData\s*=\s*Schemas\['SessionListData'\]/);
	assert.match(
		source,
		/function\s+normalizeCurrentUser\(userData:\s*CurrentUserDto\):\s*\{[\s\S]*user:\s*User;[\s\S]*permissions:\s*string\[\];[\s\S]*\}/
	);
	assert.match(source, /authStore\.setUser\(currentUser\.user,\s*currentUser\.permissions\)/);
	assert.doesNotMatch(source, /interface\s+BackendUser/);
	assert.doesNotMatch(source, /userData\.user_type/);
	assert.doesNotMatch(source, /\.\.\.userData/);
	assert.doesNotMatch(source, /\b(?:nationalId|email|phone|createdAt):\s*userData\./);
	assert.match(source, /profileImageFileId:\s*userData\.profileImageFileId\s*\?\?\s*undefined/);
	assert.doesNotMatch(source, /\bfetch\s*\(/);
	assert.doesNotMatch(source, /\b(getRaw|postRaw|putRaw)\b/);
	assert.match(source, /requireApiData\([\s\S]*?\)\.user/);
});

test('generated current-user schemas keep concrete envelope and payload types', async () => {
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const userResponse = extractGeneratedSchemaBlock(generated, 'CurrentUserResponse');
	const successEnvelope = extractGeneratedSchemaBlock(generated, 'ApiResponse_CurrentUserResponse');

	for (const block of [userResponse, successEnvelope]) {
		assert.doesNotMatch(block, /\b(?:any|unknown)\b/);
	}
	for (const schemaName of ['SessionResponse', 'SessionListData']) {
		assert.doesNotMatch(extractGeneratedSchemaBlock(generated, schemaName), /\b(?:any|unknown)\b/);
	}
	assert.match(successEnvelope, /data:\s*\{/);
	assert.match(successEnvelope, /success:\s*boolean/);
	assert.match(
		generated,
		/'application\/json':\s*components\['schemas'\]\['ApiResponse_CurrentUserResponse'\]/
	);
	assert.match(generated, /'application\/json':\s*components\['schemas'\]\['ApiErrorResponse'\]/);
});

test('generated authorization contracts cover implemented routes and frontend DTO ownership', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const authApi = await readRepoFile('frontend-school/src/lib/api/auth.ts');
	const rolesApi = await readRepoFile('frontend-school/src/lib/api/roles.ts');
	const staffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const expected = [
		['/api/auth/login', 'post', 'login'],
		['/api/auth/logout', 'post', 'logout'],
		['/api/auth/me', 'get', 'getCurrentUser'],
		['/api/auth/sessions', 'get', 'listAuthSessions'],
		['/api/auth/sessions/{id}', 'delete', 'revokeAuthSession'],
		['/api/auth/logout-all', 'post', 'logoutAllSessions'],
		['/api/auth/me/profile', 'get', 'getCurrentUserProfile'],
		['/api/auth/me/profile', 'put', 'updateCurrentUserProfile'],
		['/api/auth/me/change-password', 'post', 'changeCurrentUserPassword'],
		['/api/roles', 'get', 'listRoles'],
		['/api/roles/{id}', 'get', 'getRole'],
		['/api/roles', 'post', 'createRole'],
		['/api/roles/{id}', 'put', 'updateRole'],
		['/api/roles/{id}', 'delete', 'deleteRole'],
		['/api/permissions', 'get', 'listPermissions'],
		['/api/permissions/modules', 'get', 'listPermissionsByModule'],
		['/api/users/{id}/roles', 'get', 'getUserRoles'],
		['/api/users/{id}/roles', 'post', 'assignUserRole'],
		['/api/users/{id}/roles/{role_id}', 'delete', 'removeUserRole'],
		['/api/users/{id}/permissions', 'get', 'listUserEffectivePermissions'],
		['/api/organization/units', 'get', 'listOrganizationUnits'],
		['/api/organization/units/{id}', 'get', 'getOrganizationUnit'],
		['/api/organization/units', 'post', 'createOrganizationUnit'],
		['/api/organization/units/{id}', 'put', 'updateOrganizationUnit'],
		['/api/organization/units/{id}', 'delete', 'deactivateOrganizationUnit'],
		['/api/organization/units/{id}/permissions', 'get', 'getOrganizationPermissions'],
		['/api/organization/units/{id}/permissions', 'put', 'updateOrganizationPermissions'],
		['/api/organization/units/{id}/delegatable-permissions', 'get', 'listDelegatablePermissions'],
		['/api/organization/units/{id}/delegations', 'get', 'listOrganizationDelegations'],
		['/api/organization/units/{id}/delegations', 'post', 'createOrganizationDelegation'],
		['/api/organization/delegations/{id}', 'delete', 'revokeOrganizationDelegation'],
		['/api/organization/units/{id}/members', 'get', 'listOrganizationMembers'],
		['/api/organization/units/{id}/members', 'post', 'addOrganizationMember'],
		['/api/organization/units/{id}/members/{user_id}', 'put', 'updateOrganizationMember'],
		['/api/organization/units/{id}/members/{user_id}', 'delete', 'removeOrganizationMember']
	];

	assert.equal(expected.length, 35);
	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
	}

	for (const schemaName of [
		'LoginRequest',
		'CurrentUserResponse',
		'SessionResponse',
		'SessionListData',
		'ProfileResponse',
		'Role',
		'Permission',
		'UserRoleAssignmentResponse',
		'OrganizationUnit',
		'OrganizationPermissionGrant',
		'DelegationItem',
		'OrganizationMemberItem'
	]) {
		assert.doesNotMatch(extractGeneratedSchemaBlock(generated, schemaName), /\b(?:any|unknown)\b/);
	}

	assert.doesNotMatch(authApi, /export\s+interface\s+(?:LoginRequest|ProfileResponse)\b/);
	assert.match(rolesApi, /import\s+type\s+\{\s*components\s*\}/);
	assert.doesNotMatch(rolesApi, /export\s+interface\s+(?:Role|Permission|UserRoleAssignment)\b/);
	assert.match(staffApi, /import\s+type\s+\{\s*components\s*\}/);
	assert.match(
		staffApi,
		/OrganizationUnitLookupItem\s*=\s*Schemas\['OrganizationUnitLookupItem'\]/
	);
	assert.match(
		staffApi,
		/listOrganizationUnitsLookup[\s\S]*Promise<ApiResponse<OrganizationUnitLookupItem\[\]>>/
	);
	assert.match(
		staffApi,
		/getOrganizationUnitLookup[\s\S]*Promise<ApiResponse<OrganizationUnitLookupItem>>/
	);
	assert.doesNotMatch(
		staffApi,
		/export\s+interface\s+(?:Role|OrganizationUnit|OrganizationPermissionGrant|DelegationItem|DelegatablePermission|OrganizationMemberItem)\b/
	);
});

test('migrated people mutation wrappers use the generated empty response DTO', async () => {
	const staffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const studentsApi = await readRepoFile('frontend-school/src/lib/api/students.ts');
	const achievementApi = await readRepoFile('frontend-school/src/lib/api/achievement.ts');

	for (const source of [staffApi, studentsApi, achievementApi]) {
		assert.match(source, /type\s+EmptyData\s*=\s*Schemas\['EmptyData'\]/);
	}
	assert.doesNotMatch(staffApi, /(?:updateStaff|deleteStaff)[\s\S]{0,180}Record<string, never>/);
	assert.doesNotMatch(
		studentsApi,
		/(?:updateStudent|deleteStudent|updateOwnProfile|addParentToStudent|removeParentFromStudent)[\s\S]{0,220}Record<string, never>/
	);
	assert.doesNotMatch(achievementApi, /deleteAchievement[\s\S]{0,180}Record<string, never>/);
});

test('generated schema lookup uses the complete property name', () => {
	const source = `
		ApiResponse_UserResponse: {
			data: unknown;
		};
		UserResponse: {
			id: string;
		};`;

	assert.match(extractGeneratedSchemaBlock(source, 'UserResponse'), /id:\s*string/);
	assert.doesNotMatch(extractGeneratedSchemaBlock(source, 'UserResponse'), /data:\s*unknown/);
});

test('generated lookup, menu, and feature contracts own read transport DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const lookupApi = await readRepoFile('frontend-school/src/lib/api/lookup.ts');
	const staffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const menuApi = await readRepoFile('frontend-school/src/lib/api/menu.ts');
	const menuAdminApi = await readRepoFile('frontend-school/src/lib/api/menu-admin.ts');
	const featureApi = await readRepoFile('frontend-school/src/lib/api/feature-toggles.ts');
	const expected = [
		['/api/menu/user', 'get', 'getUserMenu'],
		['/api/admin/features', 'get', 'listFeatures'],
		['/api/admin/features/{id}', 'get', 'getFeature'],
		['/api/admin/menu/workspaces', 'get', 'listMenuWorkspaces'],
		['/api/admin/menu/groups', 'get', 'listMenuGroups'],
		['/api/admin/menu/items', 'get', 'listMenuItems'],
		['/api/lookup/staff', 'get', 'lookupStaff'],
		['/api/lookup/students', 'get', 'lookupStudents'],
		['/api/lookup/rooms', 'get', 'lookupRooms'],
		['/api/lookup/roles', 'get', 'lookupRoles'],
		['/api/lookup/organization-units', 'get', 'lookupOrganizationUnits'],
		['/api/lookup/organization-units/{id}', 'get', 'getLookupOrganizationUnit'],
		['/api/lookup/grade-levels', 'get', 'lookupGradeLevels'],
		['/api/lookup/homerooms', 'get', 'lookupHomerooms'],
		['/api/lookup/academic-years', 'get', 'lookupAcademicYears'],
		['/api/lookup/subjects', 'get', 'lookupSubjects']
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
	}

	for (const source of [lookupApi, staffApi, menuApi, menuAdminApi, featureApi]) {
		assert.match(source, /import\s+type\s+\{[^}]*\bcomponents\b[^}]*\}/);
	}
	assert.match(menuAdminApi, /MenuWorkspace\s*=\s*Schemas\['MenuWorkspace'\]/);
	assert.match(
		menuAdminApi,
		/CreateMenuWorkspaceRequest\s*=\s*Schemas\['CreateMenuWorkspaceRequest'\]/
	);
	assert.doesNotMatch(
		lookupApi,
		/export\s+interface\s+(?:LookupItem|StaffLookupItem|RoleLookupItem|OrganizationUnitLookupItem|GradeLevelLookupItem|HomeroomLookupItem|AcademicYearLookupItem|StudentLookupItem|RoomLookupItem)\b/
	);
	assert.match(
		staffApi,
		/OrganizationUnitLookupItem\s*=\s*Schemas\['OrganizationUnitLookupItem'\]/
	);
	assert.doesNotMatch(menuApi, /export\s+interface\s+(?:MenuItem|MenuGroup)\b/);
	assert.doesNotMatch(menuAdminApi, /export\s+interface\s+(?:MenuGroup|MenuItem)\b/);
	assert.doesNotMatch(featureApi, /export\s+interface\s+FeatureToggle\b/);
});

test('generated staff, student, and parent profile contracts own read transport DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const staffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const studentApi = await readRepoFile('frontend-school/src/lib/api/students.ts');
	const parentApi = await readRepoFile('frontend-school/src/lib/api/parents.ts');
	const expected = [
		['/api/staff', 'get', 'listStaff'],
		['/api/staff/dashboard', 'get', 'getStaffDashboard'],
		['/api/staff/{id}', 'get', 'getStaffProfile'],
		['/api/staff/{id}/public-profile', 'get', 'getPublicStaffProfile'],
		['/api/student/profile', 'get', 'getStudentProfile'],
		['/api/parent/profile', 'get', 'getParentProfile'],
		['/api/parent/students/{student_id}', 'get', 'getParentChildProfile']
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
	}
	for (const source of [staffApi, studentApi, parentApi]) {
		assert.match(source, /import\s+type\s+\{[^}]*\bcomponents\b[^}]*\}/);
	}
	assert.doesNotMatch(
		staffApi,
		/export\s+interface\s+(?:StaffListItem|StaffDashboardOverview|RoleResponse|OrganizationUnitResponse|TeachingAssignmentItem|AdvisorHomeroomItem|StaffInfoResponse|StaffProfileResponse|PublicStaffRoleResponse|PublicStaffOrganizationUnitResponse|PublicStaffProfileResponse)\b/
	);
	assert.doesNotMatch(studentApi, /export\s+interface\s+(?:StudentParent|Student)\b/);
	assert.doesNotMatch(parentApi, /export\s+interface\s+(?:ChildDto|ParentProfile)\b/);
});

test('generated self-service schedule contracts own timetable, exam, and calendar wire DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const timetableApi = await readRepoFile('frontend-school/src/lib/api/timetable.ts');
	const examApi = await readRepoFile('frontend-school/src/lib/api/examSchedule.ts');
	const calendarApi = await readRepoFile('frontend-school/src/lib/api/calendar.ts');
	const expected = [
		['/api/parent/students/{student_id}/timetable', 'get', 'getParentChildTimetable'],
		['/api/parent/students/{student_id}/exam-schedules', 'get', 'getParentChildExamSchedule'],
		['/api/parent/students/{student_id}/calendar/events', 'get', 'getParentChildCalendarEvents'],
		['/api/me/timetable', 'get', 'getMyTimetable'],
		['/api/me/exam-schedules', 'get', 'listMyExamSchedules'],
		['/api/staff/exam-schedules', 'get', 'listStaffExamSchedules'],
		['/api/me/calendar/events', 'get', 'listMyCalendarEvents']
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
	}

	assert.match(timetableApi, /export\s+type\s+TimetableEntry\s*=\s*Schemas\['TimetableEntry'\]/);
	assert.doesNotMatch(timetableApi, /export\s+interface\s+TimetableEntry\b/);

	for (const schemaName of ['PersonalExamScheduleRound', 'PersonalExamSessionView']) {
		assert.match(
			examApi,
			new RegExp(`export\\s+type\\s+${schemaName}\\s*=\\s*Schemas\\['${schemaName}'\\]`)
		);
		assert.doesNotMatch(examApi, new RegExp(`export\\s+interface\\s+${schemaName}\\b`));
	}

	assert.match(
		calendarApi,
		/export\s+type\s+CalendarViewerEvent\s*=\s*Schemas\['CalendarViewerEvent'\]/
	);
	assert.match(calendarApi, /export\s+type\s+CalendarEventTag\s*=\s*Schemas\['CalendarEventTag'\]/);
	assert.doesNotMatch(calendarApi, /export\s+interface\s+CalendarViewerEvent\b/);
	assert.doesNotMatch(calendarApi, /export\s+interface\s+CalendarEventTag\b/);

	for (const schemaName of [
		'TimetableEntry',
		'PersonalExamScheduleRound',
		'PersonalExamSessionView',
		'CalendarViewerEvent',
		'CalendarEventTag'
	]) {
		assert.doesNotMatch(extractGeneratedSchemaBlock(generated, schemaName), /\b(?:any|unknown)\b/);
	}
});

test('generated calendar, school, and notification contracts own final read DTOs', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const calendarApi = await readRepoFile('frontend-school/src/lib/api/calendar.ts');
	const schoolApi = await readRepoFile('frontend-school/src/lib/api/school.ts');
	const notificationStore = await readRepoFile('frontend-school/src/lib/stores/notification.ts');
	const expected = [
		['/api/public/calendar/events', 'get', 'listPublicCalendarEvents'],
		['/api/calendar/events', 'get', 'listCalendarEvents'],
		['/api/calendar/categories', 'get', 'listCalendarCategories'],
		['/api/calendar/tags', 'get', 'listCalendarTags'],
		['/api/school/public', 'get', 'getPublicSchoolInfo'],
		['/api/school/settings', 'get', 'getSchoolSettings'],
		['/api/notifications', 'get', 'listNotifications']
	];

	for (const [route, method, operationId] of expected) {
		assert.equal(contract.paths?.[route]?.[method]?.operationId, operationId, `${method} ${route}`);
	}
	assert.equal(contract.paths?.['/api/notifications/stream'], undefined);

	for (const schemaName of ['CalendarCategory', 'CalendarTag', 'CalendarPublicEvent']) {
		assert.match(
			calendarApi,
			new RegExp(`export\\s+type\\s+${schemaName}\\s*=\\s*Schemas\\['${schemaName}'\\]`)
		);
	}
	for (const schemaName of [
		'CalendarEventDto',
		'CalendarEventTargetDto',
		'CalendarEventReminderDto'
	]) {
		const generatedName = schemaName.replace(/Dto$/, '');
		assert.match(
			calendarApi,
			new RegExp(`export\\s+type\\s+${schemaName}\\s*=\\s*Schemas\\['${generatedName}'\\]`)
		);
	}
	assert.match(calendarApi, /export\s+type\s+CalendarEvent\s*=\s*Omit<CalendarEventDto,/);

	assert.match(
		schoolApi,
		/export\s+type\s+SchoolSettingsDto\s*=\s*Schemas\['SchoolSettingsResponse'\]/
	);
	assert.match(
		schoolApi,
		/export\s+type\s+PublicSchoolInfoDto\s*=\s*Schemas\['PublicSchoolInfoData'\]/
	);
	assert.match(schoolApi, /apiClient\.get<SchoolSettingsDto>\('\/api\/school\/settings'\)/);
	assert.match(schoolApi, /apiClient\.get<PublicSchoolInfoDto>\('\/api\/school\/public'\)/);

	assert.match(notificationStore, /import\s+type\s+\{\s*components\s*\}/);
	assert.match(notificationStore, /export\s+type\s+Notification\s*=\s*Schemas\['Notification'\]/);
	assert.match(
		notificationStore,
		/type\s+ListNotificationsResponse\s*=\s*Schemas\['ListNotificationsResponse'\]/
	);
	assert.match(notificationStore, /apiClient\.get<ListNotificationsResponse>/);
	assert.doesNotMatch(notificationStore, /export\s+interface\s+Notification\b/);

	for (const schemaName of [
		'CalendarEvent',
		'CalendarPublicEvent',
		'SchoolSettingsResponse',
		'PublicSchoolInfoData',
		'Notification',
		'ListNotificationsResponse'
	]) {
		assert.doesNotMatch(extractGeneratedSchemaBlock(generated, schemaName), /\b(?:any|unknown)\b/);
	}
});

test('project rules document generated API contract ownership', async () => {
	const rules = await readRepoFile('.rules');
	const testing = await readRepoFile('docs/TESTING.md');

	for (const source of [rules, testing]) {
		assert.match(source, /generate:api-contracts/);
		assert.match(source, /check:api-contracts/);
		assert.match(source, /contracts\/openapi\/school-api\.json/);
		assert.match(source, /generated files?[^\n]*do not edit|do not edit[^\n]*generated files?/i);
	}
});

test('API contract CI protects the offline exporter boundary', async () => {
	const workflow = await readRepoFile('.github/workflows/api-contract.yml');

	assert.match(workflow, /backend-school\/src\/main\.rs/);
	assert.match(workflow, /backend-school\/tests\/static_architecture\.rs/);
	assert.match(workflow, /cargo test structured_logging --test static_architecture/);
	assert.match(workflow, /env -i PATH="\$PATH" HOME="\$HOME"[\s\S]*export-openapi/);
	assert.match(workflow, /JSON\.parse/);
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
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
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

	assert.match(
		frontendApi,
		/export\s+type\s+UserRoleAssignment\s*=\s*Schemas\['UserRoleAssignmentResponse'\]/
	);
	assert.match(
		extractGeneratedSchemaBlock(generated, 'UserRoleAssignmentResponse'),
		/role:\s*components\['schemas'\]\['Role'\]/
	);
	assert.match(
		frontendApi,
		/getUserRoles\(userId:\s*string\):\s*Promise<ApiResponse<UserRoleAssignment\[\]>>/
	);
	assert.doesNotMatch(frontendApi, /interface\s+UserRole\s*\{/);
	assert.match(
		frontendStaffApi,
		/export\s+type\s+StaffProfileResponse\s*=\s*Schemas\['StaffProfileResponse'\]/
	);
	assert.match(
		extractGeneratedSchemaBlock(generated, 'StaffProfileResponse'),
		/permissions:\s*string\[\]/
	);
	assert.doesNotMatch(
		extractGeneratedSchemaBlock(generated, 'StaffProfileResponse'),
		/permissions:\s*Record<string,\s*unknown>/
	);
	assert.match(delegationService, /struct\s+DelegatablePermission/);
	assert.match(delegationService, /Result<Vec<DelegatablePermission>,\s*AppError>/);
	assert.doesNotMatch(delegationService, /Result<Vec<serde_json::Value>,\s*AppError>/);
	assert.match(staffService, /struct\s+PublicStaffProfile/);
	assert.match(staffService, /Result<PublicStaffProfile,\s*AppError>/);
	assert.doesNotMatch(
		staffService,
		/get_public_staff_profile[\s\S]*?Result<serde_json::Value,\s*AppError>/
	);
	assert.match(
		frontendStaffApi,
		/export\s+type\s+PublicStaffProfileResponse\s*=\s*Schemas\['PublicStaffProfile'\]/
	);
	assert.match(
		frontendStaffApi,
		/getPublicStaffProfile[\s\S]*ApiResponse<PublicStaffProfileResponse>/
	);

	assert.match(frontendComponent, /type\s+UserRoleAssignment/);
	assert.match(frontendComponent, /userRole\.role/);
	assert.doesNotMatch(frontendComponent, /getRoleById\(userRole\.role_id\)/);
	assert.match(publicStaffPage, /PublicStaffProfileResponse/);
});

test('staff dashboard API uses a typed aggregate response scoped to the selected year', async () => {
	const frontendStaffApi = await readRepoFile('frontend-school/src/lib/api/staff.ts');
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const backendService = await readRepoFile(
		'backend-school/src/modules/staff/services/dashboard_service.rs'
	);
	const backendHandler = await readRepoFile('backend-school/src/modules/staff/handlers/staff.rs');

	assert.match(
		frontendStaffApi,
		/export\s+type\s+StaffDashboardOverview\s*=\s*Schemas\['StaffDashboardOverview'\]/
	);
	const dashboardSchema = extractGeneratedSchemaBlock(generated, 'StaffDashboardOverview');
	assert.match(dashboardSchema, /totalStaff:\s*number/);
	assert.match(dashboardSchema, /totalStudents:\s*number/);
	assert.match(dashboardSchema, /activeHomerooms:\s*number/);
	const requests = [];
	const staffApi = await importStaffApiWithRequestRecorder(requests);
	await staffApi.getStaffDashboard('10000000-0000-4000-8000-000000000001');
	assert.deepEqual(requests, [
		'/api/staff/dashboard?academicYearId=10000000-0000-4000-8000-000000000001'
	]);

	assert.match(backendService, /struct\s+StaffDashboardOverview/);
	assert.match(backendService, /#\[serde\(rename_all = "camelCase"\)\]/);
	assert.match(backendHandler, /ApiResponse::ok\(data\)/);

	assert.doesNotMatch(frontendStaffApi, /listStaff\(\{[\s\S]*page_size:\s*1/);
	assert.doesNotMatch(frontendStaffApi, /listStudents\(\{[\s\S]*page_size:\s*1/);
});

test('daily teaching overview API uses typed response contracts', async () => {
	const contract = JSON.parse(await readRepoFile('contracts/openapi/school-api.json'));
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const frontendTimetableApi = await readRepoFile('frontend-school/src/lib/api/timetable.ts');
	const backendService = await readRepoFile(
		'backend-school/src/modules/academic/services/daily_teaching_service.rs'
	);
	const backendHandler = await readRepoFile(
		'backend-school/src/modules/academic/handlers/timetable.rs'
	);

	const dailyTeachingPath = '/api/academic/timetable/daily-teaching';
	assert.ok(contract.paths[dailyTeachingPath], 'daily teaching route must be generated');
	assert.equal(contract.paths[dailyTeachingPath].get.operationId, 'getDailyTeachingOverview');

	const dailyEntrySchema = extractGeneratedSchemaBlock(generated, 'DailyTeachingEntry');
	for (const field of ['learningGroupId', 'offeringId', 'subjectId', 'activityId']) {
		assert.match(dailyEntrySchema, new RegExp(`${field}\\?:\\s*string \\| null`));
	}
	for (const retiredField of [
		['activity', 'SlotId'].join(''),
		['classroom', 'CourseId'].join(''),
		['semester', 'Id'].join('')
	]) {
		assert.doesNotMatch(dailyEntrySchema, new RegExp(retiredField));
	}

	assert.doesNotMatch(frontendTimetableApi, /interface\s+DailyTeachingOverview/);
	assert.doesNotMatch(frontendTimetableApi, /interface\s+DailyTeachingTeacher/);
	assert.doesNotMatch(frontendTimetableApi, /interface\s+DailyTeachingEntry/);
	assert.match(
		frontendTimetableApi,
		/export\s+type\s+DailyTeachingEntry\s*=\s*Schemas\['DailyTeachingEntry'\]/
	);
	assert.match(frontendTimetableApi, /getDailyTeachingOverview/);
	assert.match(frontendTimetableApi, /apiClient\.get<DailyTeachingOverview>/);
	assert.match(frontendTimetableApi, /\/api\/academic\/timetable\/daily-teaching/);
	assert.match(backendService, /struct\s+DailyTeachingOverview/);
	assert.match(backendService, /#\[serde\(rename_all = "camelCase"\)\]/);
	assert.match(backendHandler, /ApiResponse::ok\(overview\)/);
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
	assert.match(applicationService, /document_upload_response[\s\S]*?->\s*DocumentUploadResponse/);
	assert.doesNotMatch(applicationService, /document_upload_response_json/);
	assert.match(backendHandler, /file_access_policy::authorize_create/);
	assert.match(backendHandler, /state[\s\S]*?\.file_platform[\s\S]*?\.upload\(/);
	assert.match(backendHandler, /application_service::attach_document/);
	assert.match(
		backendHandler,
		/request_deletions\(state\.file_platform\.as_ref\(\),\s*&pool,\s*\[file\.id\]\)/
	);
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
	assert.match(parentsApi, /getChildProfile[\s\S]*Promise<Student>/);
	assert.match(parentsApi, /operations\['getParentChildProfile'\]\['parameters'\]\['query'\]/);
	assert.match(parentsApi, /apiClient\.get<Student>/);
	assert.match(parentsApi, /getChildTimetable[\s\S]*Promise<TimetableEntry\[\]>/);
	assert.match(parentsApi, /requireApiData\(/);
	assert.match(parentsApi, /operations\['getParentChildTimetable'\]\['parameters'\]\['query'\]/);
	assert.match(parentsApi, /\{ query \}/);
	assert.doesNotMatch(parentsApi, /\?academicTermId=/);
	assert.doesNotMatch(parentsApi, /apiClient\.get<unknown>/);
	assert.doesNotMatch(parentsApi, /return response as/);

	assert.match(childPage, /import type \{ Student \} from '\$lib\/api\/students'/);
	assert.match(childPage, /student = loaded/);
	assert.doesNotMatch(childPage, /response\.data as/);
	assert.match(timetablePage, /getChildProfile\(studentId, selectedYearId\)/);
	assert.match(timetablePage, /child = loadedChild/);
	assert.match(
		timetablePage,
		/const loaded = await getChildTimetable\(studentId, termId, currentLocalDate\(\)\)/
	);
	assert.doesNotMatch(timetablePage, /childData as/);
});

test('school settings API consumes typed envelope data without casts', async () => {
	const schoolApi = await readRepoFile('frontend-school/src/lib/api/school.ts');

	assert.match(schoolApi, /apiClient\.get<SchoolSettingsDto>/);
	assert.match(schoolApi, /apiClient\.patch<Record<string, never>>/);
	assert.match(schoolApi, /apiClient\.delete<Record<string, never>>/);
	assert.match(schoolApi, /apiClient\.get<PublicSchoolInfoDto>/);
	assert.match(schoolApi, /schoolSettingsFromDto\(res\.data\)/);
	assert.match(schoolApi, /publicSchoolInfoFromDto\(res\.data\)/);
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
	const supervisionWorkspace = await readRepoFile(
		'frontend-school/src/lib/components/supervision/SupervisionWorkspace.svelte'
	);

	assert.match(supervisionApi, /export\s+type\s+SupervisionObservationStatus/);
	assert.match(
		supervisionApi,
		/type SupervisionCycleItems = Schemas\['ItemsData_SupervisionCycle'\]/
	);
	assert.match(supervisionApi, /apiClient\.get<SupervisionCycleItems>/);
	assert.match(supervisionApi, /apiClient\.post<SupervisionObservation>/);
	assert.doesNotMatch(supervisionApi, /ApiResponse<unknown>/);
	assert.doesNotMatch(supervisionApi, /Record<string,\s*unknown>/);
	assert.doesNotMatch(supervisionApi, /res\.data as/);
	assert.match(supervisionRoute, /PERMISSION_MODULES\.SUPERVISION/);
	assert.match(supervisionWorkspace, /listSupervisionCycles/);
	assert.match(supervisionWorkspace, /requestSupervisionObservation/);
	assert.match(supervisionWorkspace, /updateSupervisionCycle/);
	assert.match(supervisionWorkspace, /approveSupervisionObservationRequest/);
	assert.match(supervisionWorkspace, /submitMySupervisionEvaluation/);
	assert.doesNotMatch(supervisionWorkspace, /saveMySupervisionEvaluation/);
	assert.match(supervisionWorkspace, /acknowledgeSupervisionObservation/);
	assert.match(supervisionWorkspace, /getMyTimetable/);
	assert.match(supervisionWorkspace, /academicTermId:\s*termId/);
	assert.match(
		supervisionWorkspace,
		/date:\s*cycle\s*\?\s*defaultBookingWeekStartDate\(cycle\)\s*:\s*currentLocalDate\(\)/
	);
	assert.match(supervisionWorkspace, /\{ signal \}/);
	assert.match(supervisionWorkspace, /entry\.academicTermId === termId/);
	assert.match(supervisionWorkspace, /timetableGridDays/);
	assert.match(supervisionWorkspace, /timetablePeriodRows/);
	assert.match(supervisionWorkspace, /selectTimetableEntry/);
	assert.match(supervisionWorkspace, /entry\.periodName/);
	assert.doesNotMatch(supervisionWorkspace, /period_name\?\.match\(/);
	assert.match(supervisionWorkspace, /class="overflow-x-auto rounded-md border"/);
	assert.match(
		supervisionWorkspace,
		/<Table\.Head\s+class="sticky left-0 z-10 w-\[112px\] bg-background"[\s\S]*>วัน<\/Table\.Head/
	);
	assert.match(
		supervisionWorkspace,
		/<Table\.Header>[\s\S]*\{#each timetablePeriodRows\(\) as row \(row\.key\)\}/
	);
	assert.match(
		supervisionWorkspace,
		/<Table\.Body>[\s\S]*\{#each bookingWeekDays as day \(day\.value\)\}/
	);
	assert.match(supervisionWorkspace, /formatShortDate\(day\.date\)/);
	assert.doesNotMatch(
		supervisionWorkspace,
		/grid gap-2 md:hidden[\s\S]*timetableEntriesForSelectedCycle/
	);
	assert.match(supervisionWorkspace, /cycleStatusCreateOptions/);
	assert.match(supervisionWorkspace, /status:\s*cycleForm\.status/);
	assert.match(supervisionWorkspace, /setCycleStatus/);
	assert.match(supervisionWorkspace, /createPaperSupervisionRubricSections/);
	assert.match(supervisionWorkspace, /templateForm\.sections/);
	assert.match(supervisionWorkspace, /addTemplateSection/);
	assert.match(supervisionWorkspace, /addTemplateItem/);
	assert.match(supervisionWorkspace, /moveTemplateItem/);
	assert.match(supervisionWorkspace, /calculateRubricDraftSummary/);
	assert.match(supervisionWorkspace, /sectionRubricProgress/);
	assert.match(supervisionWorkspace, /overflow-x-hidden/);
	assert.match(supervisionWorkspace, /min-w-0/);
	assert.match(supervisionWorkspace, /LoadingButton/);
	assert.match(supervisionWorkspace, /savingAction/);
	assert.match(supervisionWorkspace, /savingTemplate/);
	assert.match(supervisionWorkspace, /savingEvaluation/);
	assert.match(supervisionWorkspace, /function replaceCycle/);
	assert.match(supervisionWorkspace, /function replaceTemplate/);
	assert.match(supervisionWorkspace, /function replaceObservation/);
	assert.match(supervisionWorkspace, /async function refreshTemplates/);
	assert.match(
		supervisionWorkspace,
		/<div class="min-w-0 space-y-2 md:col-span-3">\s*<Label>ชื่อแบบประเมิน<\/Label>/
	);
	assert.doesNotMatch(supervisionWorkspace, /lg:grid-cols-\[120px_1fr_auto\]/);
	assert.doesNotMatch(supervisionWorkspace, /md:grid-cols-\[1fr_220px\]/);
	assert.doesNotMatch(supervisionWorkspace, /ratingLabel/);
	assert.doesNotMatch(supervisionWorkspace, /\btextLabel\b/);
	assert.match(supervisionWorkspace, /canManageSchool/);
	assert.match(supervisionWorkspace, /canManageRequests/);
	assert.match(supervisionWorkspace, /canReadObservations/);
	assert.match(supervisionWorkspace, /SUPERVISION_READ_OWN/);
	assert.match(supervisionWorkspace, /SUPERVISION_READ_ASSIGNED/);
	assert.match(supervisionWorkspace, /SUPERVISION_READ_ORGANIZATION_UNIT/);
	assert.match(supervisionWorkspace, /SUPERVISION_READ_ORGANIZATION_TREE/);
	assert.match(supervisionWorkspace, /SUPERVISION_READ_SCHOOL/);
	assert.match(supervisionWorkspace, /SUPERVISION_MANAGE_ORGANIZATION_UNIT/);
	assert.match(supervisionWorkspace, /SUPERVISION_MANAGE_ORGANIZATION_TREE/);
	assert.match(
		supervisionWorkspace,
		/shouldLoadObservations[\s\S]*listSupervisionObservations\([\s\S]*academicYearId:\s*yearId[\s\S]*\.\.\.\(termId \? \{ academicTermId:\s*termId \} : \{\}\)[\s\S]*\{ signal \}[\s\S]*:\s*\[\]/
	);
	assert.match(supervisionWorkspace, /getSupervisionEvaluatorAvailability/);
	assert.match(supervisionWorkspace, /requestEvaluatorAvailability/);
	assert.doesNotMatch(supervisionWorkspace, /lookupStaff/);
	assert.match(supervisionWorkspace, /getAcademicContextStore/);
	assert.match(supervisionWorkspace, /academicContextOptions/);
	assert.match(supervisionWorkspace, /\* as Select/);
	assert.match(supervisionWorkspace, /\* as Dialog/);
	assert.match(supervisionWorkspace, /\* as Table/);
	assert.match(supervisionWorkspace, /\* as Alert/);
	assert.match(supervisionWorkspace, /Progress/);
	assert.doesNotMatch(supervisionWorkspace, /<select\b/);
	assert.doesNotMatch(supervisionWorkspace, /type="datetime-local"/);
	assert.doesNotMatch(supervisionWorkspace, /status:\s*'draft',\s*\n\s*targets:/);
	assert.doesNotMatch(
		supervisionWorkspace,
		/Select\.Root[^>]*bind:value=\{selectedTimetableEntryId\}/
	);
	assert.doesNotMatch(
		supervisionWorkspace,
		/Promise\.all\(\[\s*listSupervisionCycles\(\),\s*listSupervisionTemplates\(\),\s*listSupervisionObservations\(\),\s*lookupStaff/
	);
	const createTemplateBody =
		supervisionWorkspace.match(/async function createTemplate\(\) \{[\s\S]*?\n\t\}/)?.[0] ?? '';
	const saveEvaluationBody =
		supervisionWorkspace.match(
			/async function saveEvaluation\(submit = false\) \{[\s\S]*?\n\t\}/
		)?.[0] ?? '';
	assert.doesNotMatch(createTemplateBody, /await refreshAll\(\)/);
	assert.doesNotMatch(saveEvaluationBody, /await refreshAll\(\)/);
	assert.doesNotMatch(supervisionWorkspace, /disabled=\{saving\}/);
	assert.doesNotMatch(supervisionWorkspace, /\bfetch\s*\(/);
});

test('canonical timetable API keeps generated response types after scheduler removal', async () => {
	const timetableApi = await readRepoFile('frontend-school/src/lib/api/timetable.ts');
	assert.match(timetableApi, /updateTimetableTemplate[\s\S]*apiClient\.put<TimetableTemplate>/);
	assert.match(
		timetableApi,
		/deleteTimetableTemplate[\s\S]*apiClient\.delete<Schemas\['EmptyData'\]>/
	);
	assert.doesNotMatch(
		timetableApi,
		/autoScheduleTimetable|SchedulingJobResponse|saveSchedulingConfiguration/
	);
});

test('new academic workspace mutations use typed returned resources', async () => {
	const coreApi = await readRepoFile('frontend-school/src/lib/api/academic-core.ts');
	const deliveryApi = await readRepoFile('frontend-school/src/lib/api/learning-delivery.ts');
	const corePage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/core/+page.svelte'
	);
	const deliveryPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/delivery/+page.svelte'
	);
	const deliveryCreateDialog = await readRepoFile(
		'frontend-school/src/lib/components/learning-delivery/OfferingCreateDialog.svelte'
	);
	const deliveryDetailPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/delivery/[offeringId]/+page.svelte'
	);

	assert.match(coreApi, /apiClient\.post<AcademicYear>/);
	assert.match(coreApi, /apiClient\.post<AcademicTerm>/);
	assert.match(deliveryApi, /apiClient\.post<LearningOffering>/);
	assert.match(deliveryApi, /apiClient\.put<LearningGroup>/);
	assert.doesNotMatch(
		`${coreApi}\n${deliveryApi}`,
		/ApiResponse<unknown>|Record<string,\s*unknown>|\sas\s/
	);
	assert.match(corePage, /years = \[created, \.\.\.years\]/);
	assert.match(deliveryCreateDialog, /const offering = await createLearningOffering\(/);
	assert.match(deliveryCreateDialog, /onCreated\(\{\s*offering,/);
	assert.match(deliveryPage, /function addCreated\(item: LearningOfferingOverviewItem\)/);
	assert.match(deliveryPage, /offerings: \[\.\.\.overview\.offerings, item\]\.sort/);
	assert.match(deliveryDetailPage, /const created = await createLearningGroup\(/);
	assert.match(deliveryDetailPage, /groups = \[\.\.\.groups, created\]\.sort/);
	assert.match(deliveryDetailPage, /const updated = await updateLearningGroup\(/);
	assert.match(deliveryDetailPage, /updateGroupState\(updated\)/);
});

test('admission exam-room mutations return typed data and patch local state', async () => {
	const admissionApi = await readRepoFile('frontend-school/src/lib/api/admission.ts');
	const examRoomsPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/admission/[id]/exam-rooms/+page.svelte'
	);
	const examRoomsHandler = await readRepoFile(
		'backend-school/src/modules/admission/handlers/exam_rooms.rs'
	);

	assert.match(admissionApi, /addExamRoom[\s\S]*apiClient\.post<ExamRoom>/);
	assert.match(admissionApi, /updateExamRoom[\s\S]*apiClient\.put<ExamRoom>/);
	assert.match(admissionApi, /copyExamRoomsFromRound[\s\S]*apiClient\.post<ExamRoomsResponse>/);
	for (const functionName of ['addExamRoom', 'updateExamRoom', 'copyExamRoomsFromRound']) {
		const body =
			admissionApi.match(new RegExp(`export async function ${functionName}\\([^]*?\\n\\}`))?.[0] ??
			'';
		assert.notEqual(body, '', `${functionName} should exist`);
		assert.doesNotMatch(body, /apiClient\.(post|put)<Record<string, never>>/);
	}

	for (const helper of [
		'replaceExamRoom',
		'removeExamRoomFromList',
		'replaceExamRooms',
		'applySeatAssignmentsToRooms'
	]) {
		assert.match(examRoomsPage, new RegExp(`function ${helper}\\b`));
	}

	for (const functionName of [
		'handleAddRoom',
		'handleRemoveRoom',
		'saveCapacity',
		'handleCopyFromRound'
	]) {
		const body =
			examRoomsPage.match(
				new RegExp(`async function ${functionName}\\([^)]*\\) \\{[\\s\\S]*?\\n\\t\\}`)
			)?.[0] ?? '';
		assert.notEqual(body, '', `${functionName} should exist`);
		assert.doesNotMatch(
			body,
			/await refreshRooms\(\)/,
			`${functionName} should patch rooms locally`
		);
	}

	const assignBody =
		examRoomsPage.match(/async function handleAssignSeats\(\) \{[\s\S]*?\n\t\}/)?.[0] ?? '';
	assert.doesNotMatch(assignBody, /refreshRooms\(\)/);
	assert.match(assignBody, /applySeatAssignmentsToRooms/);

	assert.match(examRoomsHandler, /ApiResponse::ok\(room\)/);
	assert.match(examRoomsHandler, /ApiResponse::with_message\(\s*ListExamRoomsData/);
});

test('facility workspace mutations patch buildings and rooms locally', async () => {
	const facilityPage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/facility/buildings/+page.svelte'
	);

	for (const helper of ['replaceBuilding', 'removeBuilding', 'replaceRoom', 'removeRoom']) {
		assert.match(facilityPage, new RegExp(`function ${helper}\\b`));
	}

	for (const functionName of ['handleSaveBuilding', 'handleSaveRoom', 'handleDelete']) {
		const body =
			facilityPage.match(
				new RegExp(`async function ${functionName}\\([^)]*\\) \\{[\\s\\S]*?\\n\\t\\}`)
			)?.[0] ?? '';
		assert.notEqual(body, '', `${functionName} should exist`);
		assert.doesNotMatch(
			body,
			/\b(loadData|refreshRooms)\(\)/,
			`${functionName} should patch local state`
		);
	}
});

test('achievement workspace mutations patch saved and deleted rows locally', async () => {
	const achievementPage = await readRepoFile(
		'frontend-school/src/lib/components/achievement/SelfRecordedAchievements.svelte'
	);

	for (const helper of ['replaceAchievement', 'removeAchievement']) {
		assert.match(achievementPage, new RegExp(`function ${helper}\\b`));
	}

	for (const functionName of ['handleSave', 'confirmDelete']) {
		const body =
			achievementPage.match(
				new RegExp(`async function ${functionName}\\([^)]*\\) \\{[\\s\\S]*?\\n\\t\\}`)
			)?.[0] ?? '';
		assert.notEqual(body, '', `${functionName} should exist`);
		assert.doesNotMatch(body, /\bloadData\(\)/, `${functionName} should patch local state`);
	}
});

test('facility API returns typed loaded envelope data without helper casts', async () => {
	const facilityApi = await readRepoFile('frontend-school/src/lib/api/facility.ts');

	assert.match(facilityApi, /type\s+LoadedApiResponse<T>/);
	assert.match(facilityApi, /Promise<LoadedApiResponse<T>>/);
	assert.match(facilityApi, /return \{ \.\.\.response, success: true, data: response\.data \}/);
	assert.match(facilityApi, /fetchApi<Building\[\]>/);
	assert.match(facilityApi, /fetchApi<Room\[\]>/);
	assert.match(facilityApi, /fetchApi<Record<string, never>>/);
	assert.doesNotMatch(facilityApi, /return response as T/);
});

test('timetable API exposes typed loaded responses and conflict unions without response casts', async () => {
	const timetableApi = await readRepoFile('frontend-school/src/lib/api/timetable.ts');
	const generated = await readRepoFile('frontend-school/src/lib/api/generated/school-api.ts');
	const timetableService = await readRepoFile(
		'backend-school/src/modules/academic/services/timetable_service.rs'
	);
	const timetablePage = await readRepoFile(
		'frontend-school/src/routes/(app)/staff/academic/timetable/+page.svelte'
	);

	assert.match(timetableApi, /TimetableEntry\s*=\s*Schemas\['TimetableEntry'\]/);
	assert.match(timetableApi, /TimetableFilters\s*=\s*operations\['listTimetableEntries'\]/);
	assert.match(timetableApi, /MyTimetableFilters\s*=\s*operations\['getMyTimetable'\]/);
	assert.match(timetableApi, /async function timetableData<T>/);
	assert.match(timetableApi, /response\.status === 409/);
	assert.match(timetableApi, /requireApiData\(response, fallback\)/);
	assert.match(timetableApi, /academicTermId:\s*requiredTerm\(filters\.academicTermId\)/);
	assert.match(timetableApi, /apiClient\.post<TimetableEntry>/);
	assert.match(timetableApi, /apiClient\.put<TimetableEntry>/);
	assert.match(timetableApi, /periodsFromTimetableEntries/);
	assert.match(
		extractGeneratedSchemaBlock(generated, 'TimetableEntry'),
		/bellSchedulePeriodId:\s*string/
	);
	assert.match(timetableApi, /apiClient\.post<MoveValidityCell\[\]>/);
	assert.match(timetableApi, /apiClient\.get<TimetableOccupancyCell\[\]>/);
	assert.doesNotMatch(timetableApi, /return response as T/);
	assert.doesNotMatch(timetableApi, /ApiResponse<unknown>/);
	assert.doesNotMatch(timetableApi, /response\.data as/);
	assert.match(timetableService, /Result<Vec<TimetableEntry>,\s*AppError>/);
	assert.match(timetableService, /period\.order_index/);
	assert.match(timetableService, /conflicts:\s*&mut Vec<ConflictInfo>/);
	assert.doesNotMatch(timetableService, /serde_json::Value/);

	assert.doesNotMatch(timetablePage, /await createTimetableEntry\([^)]*\)\) as/);
	assert.doesNotMatch(timetablePage, /await updateTimetableEntry\([^)]*\)\) as/);
	assert.doesNotMatch(
		timetablePage,
		/res as \{ success\?: boolean; conflicts\?: ConflictInfo\[\] \}/
	);
});

test('academic core and delivery APIs consume generated DTOs without response casts', async () => {
	const coreApi = await readRepoFile('frontend-school/src/lib/api/academic-core.ts');
	const deliveryApi = await readRepoFile('frontend-school/src/lib/api/learning-delivery.ts');
	for (const source of [coreApi, deliveryApi]) {
		assert.match(source, /generated\/school-api/);
		assert.match(source, /requireApiData/);
		assert.doesNotMatch(source, /return response as|ApiResponse<unknown>|res\.data as/);
	}
});

test('legacy academic batch wrapper is absent after the hard cutover', async () => {
	await assert.rejects(readRepoFile('frontend-school/src/lib/api/academic.ts'));
	const deliveryApi = await readRepoFile('frontend-school/src/lib/api/learning-delivery.ts');
	assert.match(deliveryApi, /generated\/school-api/);
	assert.match(deliveryApi, /TeacherAssignment\s*=\s*Schemas\['TeacherAssignmentInput'\]/);
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
	assert.ok(!checkedApiFiles.includes('frontend-school/src/lib/api/academic.ts'));
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
