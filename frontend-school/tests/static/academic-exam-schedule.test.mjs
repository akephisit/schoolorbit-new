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

function localSnippetSource(source, snippetName) {
	const startPattern = new RegExp(`\\{#snippet\\s+${snippetName}\\(\\)\\}`);
	const startMatch = startPattern.exec(source);

	assert.ok(startMatch, `${snippetName} snippet should exist`);

	const rest = source.slice(startMatch.index);
	const endIndex = rest.indexOf('{/snippet}');
	assert.notEqual(endIndex, -1, `${snippetName} snippet should be closed`);

	return rest.slice(0, endIndex + '{/snippet}'.length);
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

test('exam schedule detail uses shadcn alert dialogs for destructive confirmations', async () => {
	const alertDialogIndexPath = 'src/lib/components/ui/alert-dialog/index.ts';
	assert.equal(
		existsSync(projectPath(alertDialogIndexPath)),
		true,
		`${alertDialogIndexPath} should exist`
	);

	const alertDialogIndex = await readProjectFile(alertDialogIndexPath);
	const page = await readProjectFile(
		'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'
	);
	const handleUpdateExamKind = localFunctionSource(page, 'handleUpdateExamKind');
	const handleClearMismatchedItems = localFunctionSource(page, 'handleClearMismatchedItems');
	const confirmExamKindChange = localFunctionSource(page, 'confirmExamKindChange');
	const confirmClearMismatchedItems = localFunctionSource(page, 'confirmClearMismatchedItems');

	for (const expectedExport of [
		'Content as AlertDialogContent',
		'Action as AlertDialogAction',
		'Cancel as AlertDialogCancel',
		'Title as AlertDialogTitle',
		'Description as AlertDialogDescription'
	]) {
		assert.match(alertDialogIndex, new RegExp(escapeRegExp(expectedExport)));
	}

	assert.match(page, /\$lib\/components\/ui\/alert-dialog/);
	assert.match(page, /let examKindDialogOpen = \$state\(false\)/);
	assert.match(page, /let clearMismatchedDialogOpen = \$state\(false\)/);
	assert.match(page, /<AlertDialog\.Root bind:open=\{examKindDialogOpen\}>/);
	assert.match(page, /<AlertDialog\.Root bind:open=\{clearMismatchedDialogOpen\}>/);
	assert.match(page, /ยืนยันการเปลี่ยนชนิดรอบสอบ/);
	assert.match(page, /ยืนยันการล้างรายการสอบ/);
	assert.match(handleUpdateExamKind, /examKindDialogOpen = true/);
	assert.match(handleClearMismatchedItems, /clearMismatchedDialogOpen = true/);
	assert.match(confirmExamKindChange, /saveExamKind\(nextKind\)/);
	assert.match(confirmClearMismatchedItems, /clearMismatchedExamItems\(workspace\.round\.id\)/);
	assert.doesNotMatch(handleUpdateExamKind, /window\.confirm/);
	assert.doesNotMatch(handleClearMismatchedItems, /window\.confirm/);
});

test('academic exam schedule routes have compile-ready page placeholders', () => {
	const pageFiles = [
		'src/routes/(app)/staff/academic/exam-schedules/+page.svelte',
		'src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte',
		'src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte',
		'src/lib/components/academic/exam-schedule/ExamItemTray.svelte',
		'src/lib/components/academic/exam-schedule/ExamSessionBlock.svelte',
		'src/routes/(app)/staff/exams/+page.svelte',
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
			functionName: 'listExamInvigilatorStaffOptions',
			method: 'get',
			routeFragment: '/api/academic/exam-schedules/${roundId}/invigilator-staff-options'
		},
		{
			functionName: 'importExamItems',
			method: 'post',
			routeFragment: '/import-items'
		},
		{
			functionName: 'clearMismatchedExamItems',
			method: 'post',
			routeFragment: '/clear-mismatched-items'
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
			functionName: 'listStaffExamSchedules',
			method: 'get',
			routeFragment: '/api/staff/exam-schedules'
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

test('exam schedule API exposes staff-level invigilator drag actions', () => {
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');

	assert.match(api, /export async function assignExamAssignmentInvigilator/);
	assert.match(api, /export async function removeExamAssignmentInvigilator/);
	assert.match(api, /room-assignments\/\$\{assignmentId\}\/invigilators\/\$\{staffId\}/);
	assert.match(api, /apiClient\.put<ExamInvigilatorWorkspace>/);
	assert.match(api, /apiClient\.delete<ExamInvigilatorWorkspace>/);
});

test('exam rounds expose exam kind for midterm and final import filtering', () => {
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');
	const listPage = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/+page.svelte'),
		'utf8'
	);
	const roundDialog = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamRoundDialog.svelte'),
		'utf8'
	);
	const detailPage = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);

	assert.match(api, /export type ExamRoundKind = 'midterm' \| 'final'/);
	assert.match(api, /examKind: ExamRoundKind/);
	assert.match(api, /examKind: ExamRoundKind/);
	assert.match(roundDialog, /let examKind = \$state<ExamRoundKind>\('midterm'\)/);
	assert.match(roundDialog, /<Select\.Item value="midterm">กลางภาค<\/Select\.Item>/);
	assert.match(roundDialog, /<Select\.Item value="final">ปลายภาค<\/Select\.Item>/);
	assert.match(roundDialog, /examKind/);
	assert.match(listPage, /function examRoundKindLabel\(kind: ExamRoundKind\): string/);
	assert.match(listPage, /examRoundKindLabel\(round\.examKind\)/);
	assert.match(detailPage, /examRoundKindLabel\(workspace\.round\.examKind\)/);
	assert.match(detailPage, /examKindLabel=\{examRoundKindLabel\(workspace\.round\.examKind\)\}/);
	assert.match(timeline, /นำเข้าเฉพาะ \{examKindLabel\}/);
});

test('staff schedule tab can clear imported items that do not match the round kind', () => {
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');
	const detailPage = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);
	const scheduleTabStart = detailPage.indexOf('<Tabs.Content value="schedule"');
	assert.notEqual(scheduleTabStart, -1, 'schedule tab should exist');
	const scheduleTab = detailPage.slice(
		scheduleTabStart,
		detailPage.indexOf('<Tabs.Content value="invigilators"')
	);
	const handleClearMismatchedItems = localFunctionSource(detailPage, 'handleClearMismatchedItems');
	const resetWorkspaceForRound = localFunctionSource(detailPage, 'resetWorkspaceForRound');

	assert.match(api, /export interface ClearMismatchedExamItemsResult/);
	assert.match(api, /deletedCount: number/);
	assert.match(api, /export async function clearMismatchedExamItems/);
	assert.match(api, /\/clear-mismatched-items/);
	assert.match(detailPage, /clearMismatchedExamItems/);
	assert.match(detailPage, /let clearingMismatchedItems = \$state\(false\)/);
	assert.match(resetWorkspaceForRound, /clearingMismatchedItems = false/);
	assert.match(handleClearMismatchedItems, /clearMismatchedDialogOpen = true/);
	assert.doesNotMatch(handleClearMismatchedItems, /window\.confirm/);
	assert.match(scheduleTab, /onImportItems=\{handleImportItems\}/);
	assert.match(scheduleTab, /onClearMismatchedItems=\{handleClearMismatchedItems\}/);
	assert.match(
		scheduleTab,
		/canManageActions=\{canManageExamSchedules && workspace\.round\.status !== 'published'\}/
	);
	assert.doesNotMatch(scheduleTab, /รายการสอบสำหรับ\{examRoundKindLabel/);
	assert.match(scheduleTab, /<ExamScheduleTimeline/);
	assert.match(timeline, /onImportItems\?: \(\) => void/);
	assert.match(timeline, /onClearMismatchedItems\?: \(\) => void/);
	assert.match(timeline, /onclick=\{onImportItems\}/);
	assert.match(timeline, /onclick=\{onClearMismatchedItems\}/);
	assert.match(timeline, /นำเข้าเฉพาะ/);
	assert.match(timeline, /ล้างรายการไม่ตรงรอบสอบ/);
});

test('exam schedule workspace tabs inherit app content height instead of recalculating viewport height', () => {
	const detailPage = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);
	const invigilatorPanel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte'),
		'utf8'
	);

	assert.match(detailPage, /class="flex h-full min-h-0 flex-col"/);
	assert.match(detailPage, /contentClass="flex min-h-0 flex-1 flex-col"/);
	assert.match(
		detailPage,
		/<Tabs\.Root bind:value=\{activeTab\} class="flex min-h-0 flex-1 flex-col gap-4">/
	);
	assert.match(detailPage, /<Tabs\.Content value="schedule" class="min-h-0 flex-1">/);
	assert.match(detailPage, /<Tabs\.Content value="invigilators" class="min-h-0 flex-1">/);

	for (const [label, component] of [
		['timeline', timeline],
		['invigilator panel', invigilatorPanel]
	]) {
		assert.match(component, /class="flex h-full min-h-0 flex-col overflow-hidden/);
		assert.doesNotMatch(component, /100vh/, `${label} should not calculate against full viewport`);
		assert.doesNotMatch(component, /min-h-\[\d+rem\]/, `${label} should not force page overflow`);
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

test('exam room assignment panel keeps setup actions compact and labels rooms plainly', async () => {
	const panel = await readProjectFile(
		'src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte'
	);

	assert.match(panel, /function classroomLabel\(classroom: Classroom \| undefined\): string/);
	assert.match(panel, /return classroom\?\.name \?\? 'เลือกห้องเรียน'/);
	assert.match(panel, /function roomOptionLabel\(room: Room \| undefined\): string/);
	assert.match(panel, /\$\{building\} \/ \$\{name\} \/ \$\{capacity\} ที่นั่ง/);
	assert.doesNotMatch(panel, /room\.code/);
	assert.doesNotMatch(panel, /classroom\.grade_level_name \? `\$\{classroom\.grade_level_name\}/);
	assert.match(
		panel,
		/<Select\.Root type="single" bind:value=\{selectedDayId\}>[\s\S]*เพิ่มห้องสอบ/
	);
	assert.doesNotMatch(panel, /xl:grid-cols-\[minmax\(0,1fr\)_24rem\]/);
	assert.doesNotMatch(panel, /<div class="p-4">\s*\{#if readonly\}/);
});

test('exam room assignment panel hides already assigned classrooms and rooms from new selections', async () => {
	const panel = await readProjectFile(
		'src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte'
	);

	assert.match(panel, /usedClassroomIds/);
	assert.match(panel, /usedRoomIds/);
	assert.match(panel, /assignment\.id !== editingAssignmentId/);
	assert.match(panel, /availableClassrooms/);
	assert.match(panel, /availableRooms/);
	assert.match(panel, /!usedClassroomIds\.has\(classroom\.id\) \|\| classroom\.id === classroomId/);
	assert.match(panel, /!usedRoomIds\.has\(room\.id\) \|\| room\.id === roomId/);
	assert.match(panel, /\{#each availableClassrooms as classroom \(classroom\.id\)\}/);
	assert.match(panel, /\{#each availableRooms as room \(room\.id\)\}/);
	assert.doesNotMatch(panel, /\{#each filteredClassrooms as classroom \(classroom\.id\)\}/);
	assert.doesNotMatch(panel, /\{#each rooms as room \(room\.id\)\}/);
});

test('exam invigilator panel exposes room-first workflow without summary cards', () => {
	const panelPath = 'src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte';
	assert.equal(existsSync(projectPath(panelPath)), true, `${panelPath} should exist`);

	const panel = readFileSync(projectPath(panelPath), 'utf8');
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	assert.match(panel, /staffWorkloads/);
	assert.match(panel, /จัดกรรมการ/);
	assert.match(panel, /selectedDayMinutes/);
	assert.match(panel, /onAssignInvigilator/);
	assert.match(panel, /onRemoveInvigilator/);
	assert.match(panel, /InvigilatorStaffList/);
	assert.match(panel, /InvigilatorRoomBoard/);
	for (const removedSummaryLabel of [
		'ครูทั้งหมด',
		'คุมวันนี้',
		'ชั่วโมงรวมสูงสุด',
		'ห้องยังไม่มีกรรมการ'
	]) {
		assert.doesNotMatch(panel, new RegExp(escapeRegExp(removedSummaryLabel)));
	}
	assert.doesNotMatch(panel, /workloadSummary/);
	assert.doesNotMatch(panel, /unassignedAssignmentCount/);
	assert.doesNotMatch(panel, /แนะนำ 2 คน|onSaveInvigilators|updateExamAssignmentInvigilators/);
	assert.match(page, /getExamInvigilatorWorkspace/);
	assert.match(page, /<ExamInvigilatorPanel/);
});

test('exam invigilator drag workflow uses teacher cards and room drop targets', () => {
	const panel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte'),
		'utf8'
	);
	const staffListPath = 'src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte';
	const roomBoardPath = 'src/lib/components/academic/exam-schedule/InvigilatorRoomBoard.svelte';
	const roomCardPath = 'src/lib/components/academic/exam-schedule/InvigilatorRoomCard.svelte';
	const dragHelperPath = 'src/lib/components/academic/exam-schedule/invigilatorDrag.ts';

	for (const file of [staffListPath, roomBoardPath, roomCardPath, dragHelperPath]) {
		assert.equal(existsSync(projectPath(file)), true, `${file} should exist`);
	}

	const staffList = readFileSync(projectPath(staffListPath), 'utf8');
	const roomBoard = readFileSync(projectPath(roomBoardPath), 'utf8');
	const roomCard = readFileSync(projectPath(roomCardPath), 'utf8');
	const dragHelper = readFileSync(projectPath(dragHelperPath), 'utf8');

	assert.match(panel, /<InvigilatorStaffList/);
	assert.match(panel, /<InvigilatorRoomBoard/);
	assert.match(panel, /let activeDragStaffId = \$state<string \| null>\(null\)/);
	assert.match(panel, /function handleStaffDragStart/);
	assert.match(panel, /function handleStaffDragEnd/);
	assert.match(panel, /activeDragStaffId=\{activeDragStaffId\}|\{activeDragStaffId\}/);
	assert.match(staffList, /onStaffDragStart/);
	assert.match(staffList, /onStaffDragEnd/);
	assert.match(staffList, /draggable=/);
	assert.match(staffList, /วันนี้/);
	assert.match(staffList, /รวมรอบนี้/);
	assert.match(staffList, /TableBody/);
	assert.doesNotMatch(staffList, /<article/);
	assert.match(roomBoard, /InvigilatorRoomCard/);
	assert.match(roomBoard, /activeDragStaffId/);
	assert.match(roomCard, /ondrop=/);
	assert.match(roomCard, /activeDragStaffId/);
	assert.match(roomCard, /text\/plain/);
	assert.match(roomCard, /กรรมการ \{assignment\.invigilators\.length\} คน/);
	assert.match(roomCard, /onRemoveInvigilator/);
	assert.match(dragHelper, /INVIGILATOR_STAFF_DRAG_TYPE/);
	assert.match(dragHelper, /workloadLevel/);
	assert.doesNotMatch(panel + staffList + roomBoard + roomCard, /แนะนำ 2 คน|2\/2/);
});

test('exam invigilator staff loading is split from room option loading', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');
	const backendRoutes = readFileSync(
		projectPath('../backend-school/src/modules/academic.rs'),
		'utf8'
	);
	const backendHandler = readFileSync(
		projectPath('../backend-school/src/modules/academic/handlers/exam_schedule.rs'),
		'utf8'
	);
	const backendModels = readFileSync(
		projectPath('../backend-school/src/modules/academic/models/exam_schedule.rs'),
		'utf8'
	);
	const loadManagementOptions = localFunctionSource(page, 'loadManagementOptions');
	const loadInvigilatorStaffOptions = localFunctionSource(page, 'loadInvigilatorStaffOptions');

	assert.doesNotMatch(loadManagementOptions, /listStaff/);
	assert.doesNotMatch(loadManagementOptions, /staffResponse/);
	assert.match(loadManagementOptions, /listClassrooms/);
	assert.match(loadManagementOptions, /listRooms/);
	assert.doesNotMatch(page, /\bimport\s+\{\s*listStaff/);
	assert.doesNotMatch(page, /\blistStaff\(/);
	assert.match(page, /listExamInvigilatorStaffOptions/);
	assert.match(loadInvigilatorStaffOptions, /listExamInvigilatorStaffOptions\(roundId/);
	assert.match(loadInvigilatorStaffOptions, /limit: 500/);
	assert.doesNotMatch(page, /async function searchStaffOptions/);
	assert.doesNotMatch(page, /onSearchStaff=/);
	assert.match(api, /export interface ExamInvigilatorStaffOption/);
	assert.match(api, /staffId: string/);
	assert.match(api, /displayName: string/);
	assert.doesNotMatch(api, /ExamInvigilatorStaffOption[\s\S]*username/);
	assert.doesNotMatch(api, /ExamInvigilatorStaffOption[\s\S]*email/);
	assert.match(api, /listExamInvigilatorStaffOptions/);
	assert.match(backendRoutes, /\/exam-schedules\/\{round_id\}\/invigilator-staff-options/);
	assert.match(backendHandler, /get_invigilator_staff_options/);
	assert.match(backendHandler, /ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL/);
	assert.match(backendModels, /pub struct ExamInvigilatorStaffOption/);
	assert.match(backendModels, /pub staff_id: Uuid/);
	assert.match(backendModels, /pub display_name: String/);
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
		'handleSaveAssignment'
	]) {
		const handler = localFunctionSource(page, handlerName);
		assert.match(handler, /refreshWorkspace\(true\)/, `${handlerName} should refresh invigilators`);
	}

	const handlePlaceExamSession = localFunctionSource(page, 'handlePlaceExamSession');
	assert.match(handlePlaceExamSession, /refreshOrInvalidateInvigilators\(session\.examRoundId\)/);
});

test('staff workspace wires staff-level invigilator drag actions', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);

	assert.match(page, /assignExamAssignmentInvigilator/);
	assert.match(page, /removeExamAssignmentInvigilator/);
	assert.match(page, /async function handleAssignInvigilator/);
	assert.match(page, /async function handleRemoveInvigilator/);
	assert.match(page, /onAssignInvigilator={handleAssignInvigilator}/);
	assert.match(page, /onRemoveInvigilator={handleRemoveInvigilator}/);
	assert.doesNotMatch(page, /savingAssignmentId={savingInvigilatorAssignmentId}/);
	assert.doesNotMatch(page, /onSaveInvigilators={handleSaveInvigilators}/);
	assert.doesNotMatch(page, /onSearchStaff={searchStaffOptions}/);
});

test('exam schedule detail exports one editable report workbook', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const exportUtilPath = 'src/lib/utils/exam-schedule-export.ts';
	const packageJson = readFileSync(projectPath('package.json'), 'utf8');

	assert.equal(existsSync(projectPath(exportUtilPath)), true, `${exportUtilPath} should exist`);

	const exportUtil = readFileSync(projectPath(exportUtilPath), 'utf8');
	const handleExport = localFunctionSource(page, 'handleExportExamSchedule');
	const ensureInvigilatorWorkspace = localFunctionSource(
		page,
		'ensureInvigilatorWorkspaceForExport'
	);

	assert.match(page, /buildExamScheduleExportWorkbook/);
	assert.match(page, /examScheduleExportFileName/);
	assert.match(page, /let exportingExamSchedule = \$state\(false\)/);
	assert.match(page, /import\('exceljs'\)/);
	assert.match(page, /new ExcelJS\.Workbook\(\)/);
	assert.match(page, /workbook\.xlsx\.writeBuffer\(\)/);
	assert.match(page, /saveWorkbookBuffer/);
	assert.match(page, /ส่งออก/);
	assert.match(packageJson, /"exceljs"/);
	assert.match(handleExport, /buildExamScheduleExportWorkbook\(workspace,\s*invigilatorData\)/);
	assert.match(handleExport, /for \(const reportSheet of exportWorkbook\.reportSheets\)/);
	assert.match(handleExport, /appendReportSheet\(workbook,\s*reportSheet\)/);
	assert.match(page, /function appendReportSheet/);
	assert.match(page, /function appendObjectSheet/);
	assert.match(page, /TH Sarabun New/);
	assert.match(page, /border/);
	assert.match(page, /alignment/);
	assert.match(page, /printTitlesRow/);
	assert.match(page, /function applyWorksheetPageBreaks/);
	assert.match(page, /addPageBreak/);
	for (const sheetName of ['ห้องสอบ', 'ภาระงานกรรมการ', 'ความพร้อม']) {
		assert.match(handleExport, new RegExp(escapeRegExp(sheetName)));
	}
	for (const removedSheetName of ['ตารางสอบ', 'กรรมการ']) {
		assert.doesNotMatch(
			handleExport,
			new RegExp(`appendObjectSheet\\(workbook,\\s*'${escapeRegExp(removedSheetName)}'`)
		);
	}
	for (const sheetName of [
		'ตารางสอบรวม',
		'ตารางสอบ ม.ต้น',
		'ตารางสอบ ม.ปลาย',
		'ตารางสอบแยกห้อง ม.ต้น',
		'ตารางสอบแยกห้อง ม.ปลาย',
		'กรรมการคุมสอบ',
		'รับส่งข้อสอบ'
	]) {
		assert.match(exportUtil, new RegExp(escapeRegExp(sheetName)));
	}
	assert.match(ensureInvigilatorWorkspace, /getExamInvigilatorWorkspace\(roundId\)/);
	assert.match(exportUtil, /export function buildExamScheduleExportWorkbook/);
	assert.match(exportUtil, /export function examScheduleExportFileName/);
	assert.match(exportUtil, /export type ExamScheduleExportSheet/);
	assert.match(exportUtil, /reportSheets/);
	assert.match(exportUtil, /lowerSecondaryReport/);
	assert.match(exportUtil, /upperSecondaryReport/);
	assert.match(exportUtil, /lowerSecondaryClassroomReport/);
	assert.match(exportUtil, /upperSecondaryClassroomReport/);
	assert.match(exportUtil, /invigilatorSummary/);
	assert.match(exportUtil, /paperTransferReport/);
	assert.match(exportUtil, /function reportClassroomLabel/);
	assert.match(exportUtil, /function compactClassroomLabels/);
	assert.match(exportUtil, /function printableReportSheet/);
	assert.match(exportUtil, /function printableClassroomReportSheet/);
	assert.match(exportUtil, /function classroomReportRows/);
	assert.match(exportUtil, /function printableInvigilatorSummarySheet/);
	assert.match(exportUtil, /function invigilatorSummaryRows/);
	assert.match(exportUtil, /function printablePaperTransferSheet/);
	assert.match(exportUtil, /function paperTransferRows/);
	assert.match(exportUtil, /function invigilatorSummarySheetMerges/);
	assert.match(exportUtil, /function paperTransferSheetMerges/);
	assert.match(exportUtil, /function paperTransferTableHeaderRow/);
	assert.match(exportUtil, /function reportSheetMerges/);
	assert.match(exportUtil, /วันเดือนปี/);
	assert.match(exportUtil, /เวลาสอบ/);
	assert.match(exportUtil, /รหัสวิชา/);
	assert.match(exportUtil, /ชั้น/);
	assert.match(exportUtil, /ลงชื่อรับ\\n\(กรรมการคุมสอบ\)/);
	assert.match(exportUtil, /ลงชื่อส่ง\\n\(กรรมการคุมสอบ\)/);
	assert.match(exportUtil, /ลงชื่อตรวจทาน\\n\(กรรมการกลาง\)/);
	assert.match(exportUtil, /ลงชื่อรับไปตรวจ\\n\(ครูผู้สอน\)/);
	assert.match(exportUtil, /ห้องเรียน/);
	assert.match(exportUtil, /ห้องสอบ/);
	assert.match(exportUtil, /weekday: 'long'/);
	assert.match(exportUtil, /month: 'long'/);
	assert.match(exportUtil, /!printTitlesRow/);
	assert.match(exportUtil, /!rowBreaks/);
	assert.match(exportUtil, /!merges/);
	assert.match(exportUtil, /!cols/);
	for (const builderName of [
		'printableReportRows',
		'scheduleRows',
		'roomRows',
		'invigilatorRows',
		'workloadRows',
		'paperTransferRows',
		'readinessRows'
	]) {
		assert.match(exportUtil, new RegExp(`function ${builderName}\\b`));
	}
	assert.match(page, /reportSheet\.name === 'รับส่งข้อสอบ'/);
	assert.match(page, /headerText === 'กรรมการคุมสอบ'/);
	assert.match(page, /function reportSheetColumnCount/);
	assert.match(page, /function isPaperTransferHeaderRow/);
	assert.match(page, /function isPaperTransferTimeRow/);
	assert.match(page, /isPaperTransferSubjectCell/);
	assert.match(page, /reportIsPaperTransferSheet[\s\S]*headerText === 'วิชา'/);
	assert.match(page, /function autoFitWorksheetColumns/);
	assert.match(page, /function reportCellBorder/);
	assert.match(page, /isInvigilatorSummarySheet/);
	assert.match(page, /isSecondInvigilatorColumn/);
	assert.doesNotMatch(handleExport + exportUtil, /nationalId|national_id|phone|email|username/);
});

test('exam invigilator staff names join title and first name without a middle space', () => {
	const service = readFileSync(
		projectPath('../backend-school/src/modules/academic/services/exam_schedule_service.rs'),
		'utf8'
	);

	assert.match(
		service,
		/concat_ws\('',\s*NULLIF\(TRIM\(user_account\.title\), ''\),\s*NULLIF\(TRIM\(user_account\.first_name\), ''\)\)/
	);
	assert.doesNotMatch(
		service,
		/concat_ws\(' ', user_account\.title, user_account\.first_name, user_account\.last_name\)/
	);
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

test('invigilator staff filtering is local to the drag board', () => {
	const panel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte'),
		'utf8'
	);
	const staffList = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/InvigilatorStaffList.svelte'),
		'utf8'
	);
	const filterStaffCards = localFunctionSource(panel, 'filterStaffCards');

	assert.match(panel, /let staffSearch = \$state\(''\)/);
	assert.match(panel, /let showAvailableOnly = \$state\(false\)/);
	assert.match(filterStaffCards, /showAvailableOnly && card\.assignedAssignment/);
	assert.match(filterStaffCards, /card\.displayName\.toLowerCase\(\)\.includes\(search\)/);
	assert.match(staffList, /onSearchChange/);
	assert.match(staffList, /onShowAvailableOnlyChange/);
	assert.doesNotMatch(panel, /staffSearchRequestToken|onSearchStaff|bind:open={editorOpen}/);
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
	const actionsSnippet = localSnippetSource(page, 'actions');
	const bodyBeforeTabs = page.slice(page.indexOf('{:else}'), page.indexOf('<Tabs.Root'));

	assert.match(page, /CompactExamScheduleStatus/);
	assert.match(actionsSnippet, /<CompactExamScheduleStatus/);
	assert.match(actionsSnippet, /<RefreshCw/);
	assert.doesNotMatch(bodyBeforeTabs, /<CompactExamScheduleStatus/);
	assert.doesNotMatch(page, /<aside class="min-w-0 xl:sticky/);
	assert.doesNotMatch(page, /xl:grid-cols-\[minmax\(0,1fr\)_22rem\]/);
	assert.doesNotMatch(page, /ReadinessPanel/);
	assert.doesNotMatch(page, /value="review"/);
	assert.match(page, /value="invigilators"/);
	const expectedTabs = new Map([
		['setup', 'ตั้งค่า'],
		['rooms', 'ห้องสอบ'],
		['schedule', 'จัดตาราง'],
		['invigilators', 'กรรมการ']
	]);

	for (const [value, label] of expectedTabs) {
		assert.match(page, new RegExp(`<Tabs\\.Trigger value="${value}">${label}<\\/Tabs\\.Trigger>`));
	}

	assert.doesNotMatch(
		page,
		/<Tabs\.Trigger value="(?:setup|rooms|schedule|invigilators)">(?:Setup|Rooms|Schedule|Invigilators)<\/Tabs\.Trigger>/
	);
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
	assert.doesNotMatch(statusComponent, /<section/);
	assert.match(statusComponent, /<Sheet\.Root>/);
	assert.match(statusComponent, /ยังไม่พร้อม \{readiness\.blockers\.length\}/);
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
		'timelineGridTemplate(day)',
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

test('staff schedule placement and unschedule patch local workspace state', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');

	const handlePlaceExamSession = localFunctionSource(page, 'handlePlaceExamSession');
	const handleUnscheduleExamSession = localFunctionSource(page, 'handleUnscheduleExamSession');

	assert.match(page, /function applyPlacedExamSession\(session: ExamSession\)/);
	assert.match(page, /function applyRemovedExamSession\(session: ExamSession\)/);
	assert.match(
		page,
		/function examScheduleItemFromSession\(session: ExamSession\): ExamScheduleItem/
	);
	assert.match(handlePlaceExamSession, /const session = await placeExamSession/);
	assert.match(handlePlaceExamSession, /applyPlacedExamSession\(session\)/);
	assert.doesNotMatch(handlePlaceExamSession, /refreshWorkspace\(true\)/);
	assert.match(handleUnscheduleExamSession, /applyRemovedExamSession\(session\)/);
	assert.doesNotMatch(handleUnscheduleExamSession, /refreshWorkspace\(true\)/);
	for (const field of [
		'importedAt',
		'subjectGroupId',
		'subjectGroupName',
		'subjectGroupDisplayOrder',
		'subjectType',
		'gradeLevelName',
		'gradeLevelType',
		'gradeLevelYear'
	]) {
		assert.match(api, new RegExp(`${field}\\??:`), `ExamSession should expose ${field}`);
	}
});

test('staff timeline can switch between all exam days and one selected day', () => {
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);

	assert.match(timeline, /let dayDisplayMode = \$state<'all' \| 'single'>\('all'\)/);
	assert.match(timeline, /let selectedTimelineDayId = \$state\(''\)/);
	assert.match(timeline, /const visibleDays = \$derived\(/);
	assert.match(timeline, /dayDisplayMode === 'single'/);
	assert.match(timeline, /selectedTimelineDayId/);
	assert.match(timeline, /แสดงทุกวัน/);
	assert.match(timeline, /เฉพาะวัน/);
	assert.match(timeline, /bind:value=\{dayDisplayMode\}/);
	assert.match(timeline, /bind:value=\{selectedTimelineDayId\}/);
	assert.match(timeline, /\{#each visibleDays as day \(day\.id\)\}/);
});

test('staff timeline keeps 5-minute grid compact and room labels narrow', () => {
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);

	assert.match(timeline, /const MIN_SLOT_WIDTH = 8/);
	assert.match(timeline, /const ROOM_LABEL_COLUMN_WIDTH = 'minmax\(7\.5rem, 8\.5rem\)'/);
	assert.match(
		timeline,
		/const SCHEDULE_ROW_GRID_TEMPLATE = `\$\{ROOM_LABEL_COLUMN_WIDTH\} minmax\(0, 1fr\)`/
	);
	assert.match(timeline, /const TIME_LABEL_INTERVAL_MINUTES = 60/);
	assert.match(timeline, /function shouldRenderTimeLabel\(/);
	assert.match(timeline, /let dayTrackWidths = \$state<Record<string, number>>\(\{\}\)/);
	assert.match(timeline, /new ResizeObserver/);
	assert.match(timeline, /function trackSlotWidth\(day: ExamDayDetail\): number/);
	assert.match(timeline, /function timelineGridTemplate\(day: ExamDayDetail\): string/);
	assert.match(timeline, /minmax\(\$\{MIN_SLOT_WIDTH\}px, 1fr\)/);
	assert.match(timeline, /slotWidthPx: trackSlotWidth\(day, rect\.width\)/);
	assert.match(timeline, /leftPx\(day, session\.startsAt\)/);
	assert.match(timeline, /widthPx\(day, session\.durationMinutes\)/);
	assert.match(timeline, /style:min-width=\{`\$\{minimumTrackWidth\(day\)\}px`\}/);
	assert.match(timeline, /style:grid-template-columns=\{SCHEDULE_ROW_GRID_TEMPLATE\}/);
	assert.doesNotMatch(timeline, /const SLOT_WIDTH = 40/);
	assert.doesNotMatch(timeline, /--slot-width: 40px/);
	assert.doesNotMatch(timeline, /grid-cols-\[12rem_minmax\(0,1fr\)\]/);
});

test('exam item tray filters and sorts unscheduled subjects by group grade and type', () => {
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');
	const tray = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamItemTray.svelte'),
		'utf8'
	);

	for (const expectedField of [
		'subjectGroupId?: string | null',
		'subjectGroupName?: string | null',
		'subjectGroupDisplayOrder?: number | null',
		'subjectType?: string | null',
		'gradeLevelType?: string | null',
		'gradeLevelYear?: number | null'
	]) {
		assert.match(
			api,
			new RegExp(escapeRegExp(expectedField)),
			`${expectedField} should be exposed`
		);
	}

	assert.match(tray, /let subjectGroupFilter = \$state/);
	assert.match(tray, /let gradeLevelFilter = \$state/);
	assert.match(tray, /const subjectGroupOptions = \$derived/);
	assert.match(tray, /const gradeLevelOptions = \$derived/);
	assert.match(tray, /const filteredSortedItems = \$derived/);
	assert.match(tray, /function compareExamScheduleItems/);
	assert.match(tray, /subjectGroupDisplayOrder/);
	assert.match(tray, /gradeLevelSortValue/);
	assert.match(tray, /subjectTypeSortValue/);
	assert.match(tray, /subjectGroupFilter === ALL_FILTER_VALUE/);
	assert.match(tray, /gradeLevelFilter === ALL_FILTER_VALUE/);
	assert.match(tray, /ทุกกลุ่มสาระ/);
	assert.match(tray, /ทุกชั้น/);
	assert.match(tray, /\{#each filteredSortedItems as item \(item\.id\)\}/);
	assert.doesNotMatch(tray, /\{#each unscheduledItems as item \(item\.id\)\}/);
	assert.match(tray, /overflow-y-auto/);
	assert.match(tray, /min-h-0/);
});

test('scheduled exam session blocks show action-specific placement and removal state', () => {
	const page = readFileSync(
		projectPath('src/routes/(app)/staff/academic/exam-schedules/[id]/+page.svelte'),
		'utf8'
	);
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);
	const block = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamSessionBlock.svelte'),
		'utf8'
	);
	const tray = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamItemTray.svelte'),
		'utf8'
	);
	const timelineHandleDragOver = localFunctionSource(timeline, 'handleDragOver');
	const timelineHandleDrop = localFunctionSource(timeline, 'handleDrop');
	const trayHandleDragStart = localFunctionSource(tray, 'handleDragStart');
	const trayHandleDragOver = localFunctionSource(tray, 'handleDragOver');
	const trayHandleDrop = localFunctionSource(tray, 'handleDrop');

	assert.match(page, /let placingItemIds = \$state<string\[\]>\(\[\]\)/);
	assert.match(page, /let unschedulingSessionIds = \$state<string\[\]>\(\[\]\)/);
	assert.match(page, /function applyPendingExamSession\(input: PlaceExamSessionInput\)/);
	assert.match(page, /function rollbackPendingExamSession\(/);
	assert.match(timeline, /const placementDisabled = \$derived\(readonly\)/);
	assert.match(timeline, /placingSessionIds/);
	assert.match(timeline, /removing={unschedulingSessionIdSet\.has\(session\.id\)}/);
	assert.match(timeline, /placing={placingSessionIds\.has\(session\.id\)}/);
	assert.match(timeline, /readonly={placementDisabled}/);
	assert.match(tray, /const placingItemIdSet = \$derived\(new Set\(placingItemIds\)\)/);
	assert.match(tray, /draggable={!readonly && !placingItemIdSet\.has\(item\.id\)}/);
	assert.match(tray, /disabled={placingItemIdSet\.has\(item\.id\)}/);
	for (const source of [
		timelineHandleDragOver,
		timelineHandleDrop,
		trayHandleDragStart,
		trayHandleDragOver,
		trayHandleDrop
	]) {
		assert.doesNotMatch(source, /placingItemId\s*\|\|\s*unschedulingSessionId/);
	}
	assert.match(block, /placing = false/);
	assert.match(block, /removing = false/);
	assert.match(block, /draggable={!disabled}/);
	assert.match(block, /aria-busy={busy}/);
	assert.match(block, /กำลังเอาออก|กำลังบันทึก/);
	assert.match(block, /cursor-wait/);
});

test('staff timeline renders duration-aware drag preview states', () => {
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);
	const tray = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamItemTray.svelte'),
		'utf8'
	);

	assert.match(timeline, /buildTimelineDragPreview/);
	assert.match(timeline, /dragPreview/);
	assert.match(timeline, /preview\.valid/);
	assert.match(timeline, /preview\.startTime/);
	assert.match(timeline, /preview\.endTime/);
	assert.match(timeline, /onDragEnd=\{clearActiveDrag\}/);
	assert.match(tray, /onDragEnd/);
	assert.match(tray, /ondragend/);
});

test('exam timeline keeps active drag payload so dragover can allow drops reliably', () => {
	const timeline = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte'),
		'utf8'
	);
	const tray = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamItemTray.svelte'),
		'utf8'
	);
	const handleDragOver = localFunctionSource(timeline, 'handleDragOver');
	const handleDrop = localFunctionSource(timeline, 'handleDrop');

	assert.match(timeline, /let activeDragPayload = \$state<DragPayload \| null>\(null\)/);
	assert.match(timeline, /function setActiveDragPayload\(payload: DragPayload\)/);
	assert.match(timeline, /function currentDragPayload\(event: DragEvent\): DragPayload \| null/);
	assert.match(timeline, /function clearActiveDragForPayload\(payload: DragPayload\)/);
	assert.match(timeline, /activeDragPayload = payload/);
	assert.match(timeline, /activeDragPayload = null/);
	assert.match(timeline, /onDragStart=\{setActiveDragPayload\}/);
	assert.match(timeline, /onDragEnd=\{clearActiveDrag\}/);
	assert.match(tray, /onDragStart\?: \(payload: DragPayload\) => void/);
	assert.match(tray, /onDragStart\?\.\(payload\)/);
	assert.match(handleDragOver, /currentDragPayload\(event\)/);
	assert.match(handleDragOver, /event\.preventDefault\(\)/);
	assert.doesNotMatch(
		handleDragOver,
		/const payload = dragPayload\(event\);\s*if \(!payload\) return;\s*event\.preventDefault\(\)/
	);
	assert.match(handleDrop, /currentDragPayload\(event\)/);
	assert.match(handleDrop, /const placement = placeLocallyValidated\(payload, day, startsAt\)/);
	assert.match(handleDrop, /clearActiveDragForPayload\(payload\)/);
	assert.match(handleDrop, /await placement/);
	assert.doesNotMatch(handleDrop, /finally \{[\s\S]*clearActiveDrag\(\)/);
});

test('scheduled exam sessions can be removed through dialog and tray drop', () => {
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

	assert.match(tray, /onUnscheduleSession/);
	assert.match(tray, /ondrop/);
	assert.match(timeline, /เอาออกจากตาราง/);
	assert.match(timeline, /onUnscheduleSession/);
	assert.match(timeline, /selectedSessionUnscheduling/);
	assert.match(timeline, /loading=\{selectedSessionUnscheduling\}/);
	assert.match(page, /deleteExamSession/);
	assert.match(page, /handleUnscheduleExamSession/);
	assert.match(page, /let unschedulingSessionIds = \$state<string\[\]>\(\[\]\)/);
	assert.match(page, /\{unschedulingSessionIds\}/);

	const resetWorkspaceForRound = localFunctionSource(page, 'resetWorkspaceForRound');
	const handleUnscheduleExamSession = localFunctionSource(page, 'handleUnscheduleExamSession');

	assert.match(resetWorkspaceForRound, /unschedulingSessionIds = \[\]/);
	assert.match(handleUnscheduleExamSession, /addUnschedulingSessionId\(sessionId\)/);
	assert.match(
		handleUnscheduleExamSession,
		/finally \{[\s\S]*removeUnschedulingSessionId\(sessionId\)/
	);
	assert.doesNotMatch(handleUnscheduleExamSession, /placingItemId\s*=/);
	assert.doesNotMatch(handleUnscheduleExamSession, /refreshWorkspace\(true\)/);
	assert.match(handleUnscheduleExamSession, /applyRemovedExamSession\(session\)/);
	assert.match(handleUnscheduleExamSession, /!canManageExamSchedules/);
	assert.match(handleUnscheduleExamSession, /workspace\.round\.status === 'published'/);
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

test('exam day setup uses the shared shadcn date picker for exam date selection', () => {
	const dayPanel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamDaySetupPanel.svelte'),
		'utf8'
	);

	assert.match(dayPanel, /from '\$lib\/components\/ui\/date-picker'/);
	assert.match(dayPanel, /<DatePicker[\s\S]*id="exam-day-date"[\s\S]*bind:value=\{examDate\}/);
	assert.match(dayPanel, /placeholder="เลือกวันสอบ"/);
	assert.doesNotMatch(dayPanel, /<Input[\s\S]*id="exam-day-date"[\s\S]*type="date"/);
});

test('exam day setup derives day ordering from exam dates without sort order payloads', () => {
	const helper = readFileSync(projectPath('src/lib/utils/examScheduleDayOrder.ts'), 'utf8');
	const api = readFileSync(projectPath('src/lib/api/examSchedule.ts'), 'utf8');
	const dayPanel = readFileSync(
		projectPath('src/lib/components/academic/exam-schedule/ExamDaySetupPanel.svelte'),
		'utf8'
	);

	assert.match(helper, /export function compareExamDaysByDate/);
	assert.match(helper, /left\.examDate\.localeCompare\(right\.examDate\)/);
	assert.doesNotMatch(helper, /sortOrder/);
	assert.doesNotMatch(helper, /nextSortOrderForDate/);

	assert.match(dayPanel, /compareExamDaysByDate/);
	assert.doesNotMatch(dayPanel, /nextSortOrderForDate/);
	assert.doesNotMatch(dayPanel, /sortOrder:/);
	assert.doesNotMatch(dayPanel, /let sortOrder = \$state/);
	assert.doesNotMatch(dayPanel, /id="exam-day-order"/);
	assert.doesNotMatch(dayPanel, /bind:value=\{sortOrder\}/);
	assert.doesNotMatch(dayPanel, /ลำดับ \{day\.sortOrder\}/);

	assert.doesNotMatch(api, /sortOrder: number/);
});

test('exam schedule day selectors use the shared date ordering helper consistently', () => {
	const componentPaths = [
		'src/lib/components/academic/exam-schedule/ExamRoomAssignmentPanel.svelte',
		'src/lib/components/academic/exam-schedule/ExamScheduleTimeline.svelte',
		'src/lib/components/academic/exam-schedule/ExamItemTray.svelte',
		'src/lib/components/academic/exam-schedule/ExamInvigilatorPanel.svelte'
	];

	for (const componentPath of componentPaths) {
		const component = readFileSync(projectPath(componentPath), 'utf8');
		assert.match(
			component,
			/from '\$lib\/utils\/examScheduleDayOrder'/,
			`${componentPath} should import the shared exam day ordering helper`
		);
		assert.match(
			component,
			/sort\(compareExamDaysByDate\)/,
			`${componentPath} should sort exam days by date`
		);
		assert.doesNotMatch(
			component,
			/sort\(\(a, b\) => a\.sortOrder - b\.sortOrder\)/,
			`${componentPath} should not manually sort days by sortOrder`
		);
	}
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
	const staffRoute = readFileSync(projectPath('src/routes/(app)/staff/exams/+page.ts'), 'utf8');
	const staffPage = readFileSync(projectPath('src/routes/(app)/staff/exams/+page.svelte'), 'utf8');
	const studentPage = readFileSync(
		projectPath('src/routes/(app)/student/exams/+page.svelte'),
		'utf8'
	);
	const parentPage = readFileSync(
		projectPath('src/routes/(app)/parent/student/[id]/exams/+page.svelte'),
		'utf8'
	);

	assert.match(staffRoute, /user_type: 'staff'/);
	assert.doesNotMatch(staffRoute, /permission:/);
	assert.match(staffPage, /listStaffExamSchedules/);
	assert.doesNotMatch(staffPage, /listMyExamSchedules/);
	assert.doesNotMatch(staffPage, /listChildExamSchedules/);
	assert.match(staffPage, /PersonalExamScheduleView/);
	assert.match(staffPage, /showSeatNumber=\{false\}/);
	assert.match(staffPage, /PageSkeleton/);
	assert.match(staffPage, /PageState/);

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
	const staffPage = readFileSync(projectPath('src/routes/(app)/staff/exams/+page.svelte'), 'utf8');
	const combinedPersonalSources = [personalView, staffPage, studentPage, parentPage].join('\n');

	for (const expected of [
		'PersonalExamScheduleRound',
		'PersonalExamSessionView',
		'groupSessionsByDate',
		'personalExamSessionKey',
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
		'showSeatNumber',
		'ไม่มีตารางสอบที่เผยแพร่'
	]) {
		assert.match(
			personalView,
			new RegExp(escapeRegExp(expected)),
			`${expected} should be rendered`
		);
	}

	assert.match(
		personalView,
		/\{#each dateGroup\.sessions as session \(personalExamSessionKey\(session\)\)\}/
	);
	assert.match(personalView, /\{#if showSeatNumber\}/);
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
