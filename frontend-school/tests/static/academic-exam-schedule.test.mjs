import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
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
