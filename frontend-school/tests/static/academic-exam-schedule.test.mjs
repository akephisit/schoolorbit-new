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
	const listRoute = await readProjectFile('src/routes/(app)/staff/academic/exam-schedules/+page.ts');
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

test('staff workspace wires setup, import, room assignment, and publish actions', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	const expectedWorkspaceWiring = [
		'ExamDaySetupPanel',
		'ExamRoomAssignmentPanel',
		'ExamScheduleTimeline',
		'ReadinessPanel',
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
	assert.match(page, /await refreshWorkspace\(\)/);
	assert.match(page, /<ExamScheduleTimeline/);
	assert.match(page, /onPlaceSession=\{handlePlaceExamSession\}/);

	for (const expected of [
		'validateExamSessionPlacement',
		'clientXToTimelineStartTime',
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
	assert.match(listPage, /async function handleCreateRound\(input: CreateExamRoundInput\): Promise<boolean>/);

	assert.match(dayPanel, /onSaveDay\?: \(input: UpsertExamDayInput\) => Promise<boolean> \| boolean/);
	assert.match(dayPanel, /const saved = await onSaveDay\?\.\(/);
	assert.match(dayPanel, /if \(saved\) resetForm\(\)/);

	assert.match(
		roomPanel,
		/onSaveAssignment\?: \(\s*examDayId: string,\s*input: UpsertDayRoomAssignmentInput\s*\) => Promise<boolean> \| boolean/
	);
	assert.match(roomPanel, /const saved = await onSaveAssignment\?\.\(/);
	assert.match(roomPanel, /if \(saved\) resetForm\(\)/);

	assert.match(
		roundDialog,
		/onCreate\?: \(input: CreateExamRoundInput\) => Promise<boolean> \| boolean/
	);
	assert.match(roundDialog, /const created = await onCreate\?\.\(/);
	assert.match(roundDialog, /if \(created\) resetForm\(\)/);
});

test('exam room invigilator search is server-driven and preserves selected staff options', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const roomPanel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte'),
		'utf8'
	);

	assert.match(page, /async function searchStaffOptions\(search: string\): Promise<StaffListItem\[]>/);
	assert.match(page, /listStaff\(\{\s*status: 'active',\s*search:/);
	assert.match(page, /onSearchStaff=\{searchStaffOptions\}/);

	assert.match(
		roomPanel,
		/onSearchStaff\?: \(search: string\) => Promise<StaffListItem\[]>/
	);
	assert.match(roomPanel, /setTimeout\(\(\) => \{/);
	assert.match(roomPanel, /onSearchStaff\(staffSearch\.trim\(\)\)/);
	assert.match(roomPanel, /selectedInvigilatorOptions/);
	assert.match(roomPanel, /assignment\.invigilators\.map/);
	assert.match(roomPanel, /staffOptionsForDisplay/);
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
	assert.match(page, /isCurrentManagementOptionsRequest\(requestToken,\s*roundId,\s*semesterId,\s*yearId\)/);
	assert.match(page, /if \(!isCurrentManagementOptionsRequest\(requestToken,\s*roundId,\s*semesterId,\s*yearId\)\) return/);
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
	const personalViewPath = 'src/lib/components/academic/exam-schedule/PersonalExamScheduleView.svelte';
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
		assert.match(personalView, new RegExp(escapeRegExp(expected)), `${expected} should be rendered`);
	}

	assert.match(personalView, /\{#each dateGroup\.sessions as session\}/);
	assert.doesNotMatch(personalView, /\{#each dateGroup\.sessions as session \(/);
	assert.doesNotMatch(personalView, /session\.examDate\}-\$\{session\.startsAt\}-\$\{session\.subjectName/);
	assert.doesNotMatch(personalView, /สำหรับนักเรียนคนนี้/);

	for (const forbidden of ['invigilator', 'Invigilator', 'กรรมการคุมสอบ']) {
		assert.doesNotMatch(
			combinedPersonalSources,
			new RegExp(escapeRegExp(forbidden)),
			`${forbidden} should not appear in personal exam schedule sources`
		);
	}
});
