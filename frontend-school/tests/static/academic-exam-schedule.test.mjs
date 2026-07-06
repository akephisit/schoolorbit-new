import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import test from 'node:test';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(__dirname, '../..');

function projectPath(relativePath) {
	return path.join(projectRoot, relativePath);
}

async function readProjectFile(relativePath) {
	return readFile(projectPath(relativePath), 'utf8');
}

function escapeRegExp(value) {
	return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function exportedFunctionSource(source, functionName) {
	const startPattern = new RegExp(`export\\s+async\\s+function\\s+${functionName}\\b`);
	const startMatch = startPattern.exec(source);

	assert.ok(startMatch, `${functionName} should be exported`);

	const rest = source.slice(startMatch.index);
	const nextFunctionIndex = rest.search(/\nexport\s+async\s+function\s+\w+\b/);
	return nextFunctionIndex === -1 ? rest : rest.slice(0, nextFunctionIndex);
}

function localFunctionSource(source, functionName) {
	const startPattern = new RegExp(`(?:async\\s+)?function\\s+${functionName}\\b`);
	const startMatch = startPattern.exec(source);

	assert.ok(startMatch, `${functionName} should exist`);

	const bodyStart = source.indexOf('{', startMatch.index);
	assert.notEqual(bodyStart, -1, `${functionName} should have a function body`);

	let depth = 0;
	for (let index = bodyStart; index < source.length; index += 1) {
		const character = source[index];
		if (character === '{') depth += 1;
		if (character === '}') {
			depth -= 1;
			if (depth === 0) {
				return source.slice(startMatch.index, index + 1);
			}
		}
	}

	assert.fail(`${functionName} should have a closed function body`);
}

test('exam schedule refresh uses shared shadcn sheet primitive instead of feature-local drawers', async () => {
	const sheetIndexPath = 'src/lib/components/ui/sheet/index.ts';
	assert.equal(existsSync(projectPath(sheetIndexPath)), true, `${sheetIndexPath} should exist`);

	const sheetIndex = await readProjectFile(sheetIndexPath);
	const requiredExports = [
		'Content as SheetContent',
		'Header as SheetHeader',
		'Footer as SheetFooter',
		'Title as SheetTitle',
		'Description as SheetDescription',
		'Close as SheetClose',
		'Trigger as SheetTrigger'
	];

	for (const requiredExport of requiredExports) {
		assert.match(
			sheetIndex,
			new RegExp(escapeRegExp(requiredExport)),
			`${sheetIndexPath} should export ${requiredExport}`
		);
	}
});

test('academic exam schedule routes have compile-ready page placeholders', () => {
	const pageFiles = [
		'src/routes/(app)/staff/academic/exam-schedules/+page.svelte',
		'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte',
		'src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte',
		'src/lib/components/academic/exam-schedule/ExamItemTray.svelte',
		'src/lib/components/academic/exam-schedule/ExamSessionBlock.svelte',
		'src/routes/(app)/student/exams/+page.svelte',
		'src/routes/(app)/parent/student/[id]/exams/+page.svelte'
	];

	for (const pageFile of pageFiles) {
		assert.equal(existsSync(projectPath(pageFile)), true, `${pageFile} should exist`);
	}
});

test('academic exam schedule staff routes are guarded by read-school permission', async () => {
	const listRoute = await readProjectFile(
		'src/routes/(app)/staff/academic/exam-schedules/+page.ts'
	);
	const detailRoute = await readProjectFile(
		'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.ts'
	);

	assert.match(listRoute, /PERMISSIONS\.ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL/);
	assert.match(detailRoute, /PERMISSIONS\.ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL/);
});

test('academic exam schedule API client maps functions to backend routes and methods', async () => {
	const clientPath = 'src/lib/api/examSchedule.ts';
	const endpointMatrix = [
		{
			functionName: 'listExamRounds',
			method: 'get',
			routeFragment: '/api/academic/exam-schedules'
		},
		{
			functionName: 'createExamRound',
			method: 'post',
			routeFragment: '/api/academic/exam-schedules'
		},
		{
			functionName: 'updateExamRound',
			method: 'patch',
			routeFragment: '/api/academic/exam-schedules/${roundId}'
		},
		{
			functionName: 'getExamScheduleWorkspace',
			method: 'get',
			routeFragment: '/api/academic/exam-schedules/${roundId}'
		},
		{
			functionName: 'getExamInvigilatorWorkspace',
			method: 'get',
			routeFragment: '/api/academic/exam-schedules/${roundId}/invigilators'
		},
		{
			functionName: 'importExamItems',
			method: 'post',
			routeFragment: '/import-items'
		},
		{
			functionName: 'upsertExamDay',
			method: 'post',
			routeFragment: '/${roundId}/days'
		},
		{
			functionName: 'deleteExamDay',
			method: 'delete',
			routeFragment: '/days/${examDayId}'
		},
		{
			functionName: 'listDayRoomAssignments',
			method: 'get',
			routeFragment: '/days/${examDayId}/room-assignments'
		},
		{
			functionName: 'upsertDayRoomAssignment',
			method: 'post',
			routeFragment: '/days/${examDayId}/room-assignments'
		},
		{
			functionName: 'updateExamAssignmentInvigilators',
			method: 'put',
			routeFragment: '/room-assignments/${assignmentId}/invigilators'
		},
		{
			functionName: 'generateSeatsForAssignment',
			method: 'post',
			routeFragment: '/room-assignments/${assignmentId}/seats'
		},
		{
			functionName: 'placeExamSession',
			method: 'post',
			routeFragment: '/sessions'
		},
		{
			functionName: 'deleteExamSession',
			method: 'delete',
			routeFragment: '/sessions/${sessionId}'
		},
		{
			functionName: 'publishExamRound',
			method: 'post',
			routeFragment: '/${roundId}/publish'
		},
		{
			functionName: 'listMyExamSchedules',
			method: 'get',
			routeFragment: '/api/me/exam-schedules'
		},
		{
			functionName: 'listChildExamSchedules',
			method: 'get',
			routeFragment: '/api/parent/students/${studentId}/exam-schedules'
		}
	];

	assert.equal(existsSync(projectPath(clientPath)), true, `${clientPath} should exist`);

	const client = await readProjectFile(clientPath);

	assert.match(client, /requireApiData/);

	for (const { functionName, method, routeFragment } of endpointMatrix) {
		const functionSource = exportedFunctionSource(client, functionName);
		const methodPattern = new RegExp(`\\bapiClient\\.${method}\\s*(?:<[^>]+>)?\\s*\\(`);
		const routePattern = new RegExp(escapeRegExp(routeFragment));
		const callPattern = new RegExp(
			`\\bapiClient\\.${method}\\s*(?:<[^>]+>)?\\s*\\([\\s\\S]*?${escapeRegExp(routeFragment)}`
		);

		assert.match(functionSource, methodPattern, `${functionName} should call apiClient.${method}`);
		assert.match(functionSource, routePattern, `${functionName} should target ${routeFragment}`);
		assert.match(
			functionSource,
			callPattern,
			`${functionName} should call apiClient.${method} with ${routeFragment}`
		);
	}
});

test('exam schedule API exposes invigilator workspace and updates separately from room assignment', async () => {
	const api = await readProjectFile('src/lib/api/examSchedule.ts');

	assert.match(api, /export interface ExamInvigilatorWorkspace/);
	assert.match(api, /export async function getExamInvigilatorWorkspace/);
	assert.match(api, /export async function updateExamAssignmentInvigilators/);
	assert.match(api, /room-assignments\/\$\{assignmentId\}\/invigilators/);

	const roomInputStart = api.indexOf('export interface UpsertDayRoomAssignmentInput');
	const roomInputEnd = api.indexOf('export interface GenerateSeatsInput');
	const roomInput = api.slice(roomInputStart, roomInputEnd);
	assert.doesNotMatch(roomInput, /invigilatorStaffIds/);
});

test('exam room assignment panel is room and seat only with sheet editing', async () => {
	const panel = await readProjectFile(
		'src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte'
	);

	assert.match(panel, /\$lib\/components\/ui\/sheet/);
	assert.doesNotMatch(panel, /staffSearch/);
	assert.doesNotMatch(panel, /selectedInvigilatorIds/);
	assert.doesNotMatch(panel, /invigilatorStaffIds/);
	assert.match(panel, /บันทึกห้องสอบ/);
	assert.match(panel, /sticky|mt-auto/);
});

test('exam invigilator panel exposes room-first workflow and workload summary', () => {
	const panelPath = 'src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte';
	assert.equal(existsSync(projectPath(panelPath)), true, `${panelPath} should exist`);

	const panel = readFileSync(projectPath(panelPath), 'utf8');
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	assert.match(panel, /staffWorkloads/);
	assert.match(panel, /sessionMinutes/);
	assert.match(panel, /จัดกรรมการ/);
	assert.match(panel, /แนะนำ 2 คน/);
	assert.match(panel, /updateExamAssignmentInvigilators|onSaveInvigilators/);
	assert.match(page, /getExamInvigilatorWorkspace/);
	assert.match(page, /<ExamInvigilatorPanel/);
});

test('exam invigilator staff loading is split from room option loading', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const loadManagementOptions = localFunctionSource(page, 'loadManagementOptions');
	const loadInvigilatorStaffOptions = localFunctionSource(page, 'loadInvigilatorStaffOptions');

	assert.doesNotMatch(loadManagementOptions, /listStaff/);
	assert.doesNotMatch(loadManagementOptions, /staffResponse/);
	assert.match(loadManagementOptions, /listClassrooms/);
	assert.match(loadManagementOptions, /listRooms/);
	assert.match(loadInvigilatorStaffOptions, /listStaff\(\{\s*status: 'active'/);
	assert.match(loadInvigilatorStaffOptions, /page_size: 40/);
	assert.match(page, /loadInvigilatorStaffOptions\(\)/);
});

test('staff workspace refreshes or invalidates invigilator workspace after source changes', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const refreshWorkspace = localFunctionSource(page, 'refreshWorkspace');
	const refreshOrInvalidateInvigilators = localFunctionSource(
		page,
		'refreshOrInvalidateInvigilators'
	);

	assert.match(refreshWorkspace, /refreshOrInvalidateInvigilators/);
	assert.match(refreshOrInvalidateInvigilators, /activeTab === 'invigilators'/);
	assert.match(refreshOrInvalidateInvigilators, /loadInvigilators/);
	assert.match(refreshOrInvalidateInvigilators, /invigilatorWorkspace = null/);

	for (const handlerName of [
		'handleImportItems',
		'handleSaveDay',
		'handleDeleteDay',
		'handleSaveAssignment',
		'handlePlaceExamSession'
	]) {
		const handler = localFunctionSource(page, handlerName);
		assert.match(handler, /refreshWorkspace\(true\)/, `${handlerName} should refresh invigilators`);
	}
});

test('invigilator workspace load exposes recoverable error and retry state', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const panel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte'),
		'utf8'
	);
	const loadInvigilators = localFunctionSource(page, 'loadInvigilators');

	assert.match(page, /let invigilatorLoadError = \$state\(''\)/);
	assert.match(loadInvigilators, /invigilatorLoadError = ''/);
	assert.match(loadInvigilators, /invigilatorLoadError =/);
	assert.match(page, /loadError={invigilatorLoadError}/);
	assert.match(page, /onRetry={\(\) => loadInvigilators\(\)}/);
	assert.match(panel, /loading = false/);
	assert.match(panel, /loadError = ''/);
	assert.match(panel, /onRetry/);
	assert.match(panel, /variant="error"/);
	assert.match(panel, /actionLabel="ลองอีกครั้ง"/);
	assert.match(panel, /onaction={onRetry}/);
});

test('invigilator staff search invalidates stale requests when reset to default options', () => {
	const panel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte'),
		'utf8'
	);
	const cancelStaffSearchRequest = localFunctionSource(panel, 'cancelStaffSearchRequest');
	const syncDefaultStaffOptions = localFunctionSource(panel, 'syncDefaultStaffOptions');
	const resetStaffSearch = localFunctionSource(panel, 'resetStaffSearch');
	const loadAssignment = localFunctionSource(panel, 'loadAssignment');

	assert.match(cancelStaffSearchRequest, /staffSearchRequestToken \+= 1/);
	assert.doesNotMatch(syncDefaultStaffOptions, /staffSearchRequestToken/);
	assert.match(syncDefaultStaffOptions, /staffOptions = staff/);
	assert.match(syncDefaultStaffOptions, /staffSearchLoading = false/);
	assert.match(resetStaffSearch, /cancelStaffSearchRequest\(\)/);
	assert.match(resetStaffSearch, /syncDefaultStaffOptions\(\)/);
	assert.match(loadAssignment, /resetStaffSearch\(\)/);
	assert.match(panel, /syncDefaultStaffOptions\(\);\s*return;/);
	assert.doesNotMatch(panel, /resetStaffSearch\(\);\s*return;/);
	assert.match(panel, /return \(\) => \{/);
	assert.match(panel, /cancelStaffSearchRequest\(\)/);
	assert.match(panel, /bind:open={editorOpen}/);
});

test('staff workspace wires setup, import, room assignment, and publish actions', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	const expectedWorkspaceWiring = [
		'ExamDaySetupPanel',
		'ExamRoomAssignmentPanel',
		'ExamScheduleTimeline',
		'CompactExamScheduleStatus',
		'getExamScheduleWorkspace',
		'placeExamSession',
		'upsertExamDay',
		'deleteExamDay',
		'upsertDayRoomAssignment',
		'generateSeatsForAssignment',
		'importExamItems',
		'publishExamRound',
		'ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL',
		'ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL'
	];

	for (const expected of expectedWorkspaceWiring) {
		assert.match(page, new RegExp(escapeRegExp(expected)), `${expected} should be wired`);
	}
});

test('staff exam schedule detail uses compact status and removes large readiness aside', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	assert.match(page, /CompactExamScheduleStatus/);
	assert.doesNotMatch(page, /<aside class="min-w-0 xl:sticky/);
	assert.doesNotMatch(page, /xl:grid-cols-\[minmax\(0,1fr\)_22rem\]/);
	assert.doesNotMatch(page, /ReadinessPanel/);
	assert.doesNotMatch(page, /value="review"/);
	assert.match(page, /value="invigilators"/);
	assert.match(page, /<Tabs\.Trigger value="invigilators">กรรมการ<\/Tabs\.Trigger>/);
});

test('compact exam schedule status derives invigilator counts from room assignments by default', () => {
	const statusComponent = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/CompactExamScheduleStatus.svelte'),
		'utf8'
	);

	assert.doesNotMatch(statusComponent, /invigilatorAssignedCount = 0/);
	assert.doesNotMatch(statusComponent, /invigilatorAssignmentCount = 0/);
	assert.match(statusComponent, /invigilatorAssignmentFallback = \$derived\(/);
	assert.match(statusComponent, /day\.roomAssignments\.length/);
	assert.match(statusComponent, /invigilatorAssignedFallback = \$derived\(/);
	assert.match(statusComponent, /assignment\.invigilators\.length > 0/);
	assert.match(statusComponent, /invigilatorAssignedCount \?\? invigilatorAssignedFallback/);
	assert.match(statusComponent, /invigilatorAssignmentCount \?\? invigilatorAssignmentFallback/);
});

test('staff timeline wires drag drop placement and accessible schedule dialog', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);
	const tray = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamItemTray.svelte'),
		'utf8'
	);
	const block = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamSessionBlock.svelte'),
		'utf8'
	);

	assert.match(page, /async function handlePlaceExamSession\(/);
	assert.match(page, /placeExamSession\(\{/);
	assert.match(page, /await refreshWorkspace\(true\)/);
	assert.match(page, /<ExamScheduleTimeline/);
	assert.match(page, /onPlaceSession=\{handlePlaceExamSession\}/);

	for (const expected of [
		'validateExamSessionPlacement',
		'buildTimelineDragPreview',
		'ondrop=',
		'blocked-window',
		'--slot-width: 24px',
		'<Dialog.Root',
		'onPlaceSession?.('
	]) {
		assert.match(timeline, new RegExp(escapeRegExp(expected)), `${expected} should be wired`);
	}

	assert.match(tray, /unscheduledItems/);
	assert.match(tray, /scheduledSessions/);
	assert.match(tray, /validateExamSessionPlacement/);
	assert.match(tray, /draggable=/);
	assert.match(tray, /<Dialog.Root/);
	assert.match(block, /draggable=/);
	assert.match(block, /session-block/);
	assert.match(block, /min-height: 2\.25rem/);
	assert.match(block, /overflow: hidden/);
});

test('staff timeline renders duration-aware drag preview states', () => {
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);

	assert.match(timeline, /buildTimelineDragPreview/);
	assert.match(timeline, /dragPreview/);
	assert.match(timeline, /preview\.valid/);
	assert.match(timeline, /preview\.startTime/);
	assert.match(timeline, /preview\.endTime/);
});

test('staff workspace reloads by route round id and keeps form input on failed saves', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const listPage = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/+page.svelte'),
		'utf8'
	);
	const dayPanel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamDaySetupPanel.svelte'),
		'utf8'
	);
	const roomPanel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte'),
		'utf8'
	);
	const roundDialog = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamRoundDialog.svelte'),
		'utf8'
	);

	assert.match(page, /async function loadWorkspace\(roundId: string,\s*initial = false\)/);
	assert.match(page, /getExamScheduleWorkspace\(roundId\)/);
	assert.match(page, /resetWorkspaceForRound\(roundId\)/);
	assert.match(page, /loadWorkspace\(roundId,\s*true\)/);
	assert.doesNotMatch(page, /onMount\(\(\) => \{\s*loadWorkspace\(true\)/);

	assert.match(page, /async function handleSaveDay\(input: UpsertExamDayInput\): Promise<boolean>/);
	assert.match(page, /async function handleSaveAssignment\(/);
	assert.match(page, /input: UpsertDayRoomAssignmentInput/);
	assert.match(page, /\): Promise<boolean> \{/);
	assert.match(
		listPage,
		/async function handleCreateRound\(input: CreateExamRoundInput\): Promise<boolean>/
	);

	assert.match(
		dayPanel,
		/onSaveDay\?: \(input: UpsertExamDayInput\) => Promise<boolean> \| boolean/
	);
	assert.match(dayPanel, /const saved = await onSaveDay\?\.\(/);
	assert.match(dayPanel, /if \(saved\) resetForm\(\)/);

	assert.match(
		roomPanel,
		/onSaveAssignment\?: \(\s*examDayId: string,\s*input: UpsertDayRoomAssignmentInput\s*\) => Promise<boolean> \| boolean/
	);
	assert.match(roomPanel, /const saved = await onSaveAssignment\?\.\(/);
	assert.match(roomPanel, /if \(saved\) \{\s*resetForm\(\);\s*editorOpen = false;\s*\}/);

	assert.match(
		roundDialog,
		/onCreate\?: \(input: CreateExamRoundInput\) => Promise<boolean> \| boolean/
	);
	assert.match(roundDialog, /const created = await onCreate\?\.\(/);
	assert.match(roundDialog, /if \(created\) resetForm\(\)/);
});

test('exam room assignment panel leaves invigilator search out of room editing', () => {
	const roomPanel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte'),
		'utf8'
	);

	assert.doesNotMatch(roomPanel, /StaffListItem/);
	assert.doesNotMatch(roomPanel, /onSearchStaff/);
	assert.doesNotMatch(roomPanel, /setTimeout\(\(\) => \{/);
	assert.doesNotMatch(roomPanel, /assignment\.invigilators\.map/);
	assert.doesNotMatch(roomPanel, /staffOptionsForDisplay/);
});

test('staff workspace ignores stale management option responses after route changes', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	assert.match(page, /let managementOptionsRequestToken = 0/);
	assert.match(page, /managementOptionsRequestToken \+= 1/);
	assert.match(page, /const requestToken = \+\+managementOptionsRequestToken/);
	assert.match(page, /const roundId = workspace\.round\.id/);
	assert.match(page, /const semesterId = workspace\.round\.academicSemesterId/);
	assert.match(page, /const yearId = currentSemester\?\.academic_year_id/);
	assert.match(
		page,
		/isCurrentManagementOptionsRequest\(requestToken,\s*roundId,\s*semesterId,\s*yearId\)/
	);
	assert.match(
		page,
		/if \(!isCurrentManagementOptionsRequest\(requestToken,\s*roundId,\s*semesterId,\s*yearId\)\) return/
	);
});

test('personal exam schedule pages use the published schedule APIs and shared view', () => {
	const studentPage = readFileSync(
		projectPath('src/routes/(app)/student/exams/+page.svelte'),
		'utf8'
	);
	const parentPage = readFileSync(
		projectPath('src/routes/(app)/parent/student/[id]/exams/+page.svelte'),
		'utf8'
	);

	assert.match(studentPage, /listMyExamSchedules/);
	assert.doesNotMatch(studentPage, /listChildExamSchedules/);
	assert.match(studentPage, /PersonalExamScheduleView/);
	assert.match(studentPage, /PageSkeleton/);
	assert.match(studentPage, /PageState/);

	assert.match(parentPage, /listChildExamSchedules\(studentId\)/);
	assert.doesNotMatch(parentPage, /listMyExamSchedules/);
	assert.match(parentPage, /PersonalExamScheduleView/);
	assert.match(parentPage, /PageSkeleton/);
	assert.match(parentPage, /PageState/);
	assert.match(parentPage, /data\.studentId/);
	assert.match(parentPage, /let scheduleRequestToken = 0/);
	assert.match(parentPage, /\$effect\(\(\) => \{/);
	assert.match(parentPage, /const requestToken = \+\+scheduleRequestToken/);
	assert.match(parentPage, /rounds = \[\]/);
	assert.match(parentPage, /listChildExamSchedules\(studentId\)/);
	assert.match(parentPage, /requestToken !== scheduleRequestToken/);
	assert.doesNotMatch(parentPage, /onMount/);
});

test('personal exam schedule view groups published sessions and hides staff supervision data', () => {
	const personalViewPath =
		'src/lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte';
	const personalView = readFileSync(projectPath(personalViewPath), 'utf8');
	const studentPage = readFileSync(
		projectPath('src/routes/(app)/student/exams/+page.svelte'),
		'utf8'
	);
	const parentPage = readFileSync(
		projectPath('src/routes/(app)/parent/student/[id]/exams/+page.svelte'),
		'utf8'
	);
	const combinedPersonalSources = [personalView, studentPage, parentPage].join('\n');

	for (const expected of [
		'PersonalExamScheduleRound',
		'PersonalExamSessionView',
		'groupSessionsByDate',
		'round.sessions',
		'session.examDate',
		'session.startsAt',
		'session.endsAt',
		'session.subjectName',
		'session.assessmentCategoryName',
		'session.classroomName',
		'session.buildingName',
		'session.roomName',
		'session.seatNumber',
		'ไม่มีตารางสอบที่เผยแพร่'
	]) {
		assert.match(
			personalView,
			new RegExp(escapeRegExp(expected)),
			`${expected} should be rendered`
		);
	}

	assert.match(personalView, /\{#each dateGroup\.sessions as session\}/);
	assert.doesNotMatch(personalView, /\{#each dateGroup\.sessions as session \(/);
	assert.doesNotMatch(
		personalView,
		/session\.examDate\}-\$\{session\.startsAt\}-\$\{session\.subjectName/
	);
	assert.doesNotMatch(personalView, /สำหรับนักเรียนคนนี้/);

	for (const forbidden of ['invigilator', 'Invigilator', 'กรรมการคุมสอบ']) {
		assert.doesNotMatch(
			combinedPersonalSources,
			new RegExp(escapeRegExp(forbidden)),
			`${forbidden} should not appear in personal exam schedule sources`
		);
	}
});
