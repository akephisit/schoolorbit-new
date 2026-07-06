import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import test from 'node:test';
import ts from 'typescript';

const projectRoot = path.resolve(import.meta.dirname, '../..');

async function importTimeHelpers() {
	const sourcePath = path.join(projectRoot, 'src/lib/utils/examScheduleTime.ts');
	const source = await readFile(sourcePath, 'utf8');
	const transpiled = ts.transpileModule(source, {
		compilerOptions: {
			module: ts.ModuleKind.ESNext,
			target: ts.ScriptTarget.ES2022,
			sourceMap: false
		},
		fileName: sourcePath
	}).outputText;

	const dataUrl = `data:text/javascript;base64,${Buffer.from(
		`${transpiled}\n//# sourceURL=${pathToFileURL(sourcePath).href}`
	).toString('base64')}`;
	return import(dataUrl);
}

test('exam schedule time helpers use half-open minute ranges', async () => {
	const { addMinutes, minutesBetween, timeToMinutes } = await importTimeHelpers();

	assert.equal(timeToMinutes('10:00'), 600);
	assert.equal(minutesBetween('08:30', '10:00'), 90);
	assert.equal(addMinutes('08:30', 90), '10:00');
});

test('exam schedule range overlap treats touching endpoints as non-overlap', async () => {
	const { rangesOverlap } = await importTimeHelpers();

	assert.equal(rangesOverlap('08:30', '10:00', '09:45', '10:30'), true);
	assert.equal(rangesOverlap('08:30', '10:00', '10:00', '10:30'), false);
	assert.equal(rangesOverlap('08:30', '10:00', '08:00', '08:30'), false);
});

test('timeline placement rejects outside-day and blocked-window overlap', async () => {
	const { validateTimelinePlacement } = await importTimeHelpers();
	const blockedWindows = [{ label: 'Lunch', startTime: '12:00', endTime: '13:00' }];

	assert.deepEqual(
		validateTimelinePlacement({
			dayStartTime: '08:30',
			dayEndTime: '16:00',
			startTime: '09:00',
			durationMinutes: 90,
			blockedWindows
		}),
		{ ok: true }
	);

	assert.equal(
		validateTimelinePlacement({
			dayStartTime: '08:30',
			dayEndTime: '16:00',
			startTime: '07:45',
			durationMinutes: 60,
			blockedWindows
		}).ok,
		false
	);

	assert.match(
		validateTimelinePlacement({
			dayStartTime: '08:30',
			dayEndTime: '16:00',
			startTime: '11:30',
			durationMinutes: 60,
			blockedWindows
		}).reason ?? '',
		/blocked|unavailable/i
	);

	assert.deepEqual(
		validateTimelinePlacement({
			dayStartTime: '08:30',
			dayEndTime: '16:00',
			startTime: '13:00',
			durationMinutes: 60,
			blockedWindows
		}),
		{ ok: true }
	);
});

test('timeline snap helper rounds to 15-minute increments', async () => {
	const { snapMinutesToSlot } = await importTimeHelpers();

	assert.equal(snapMinutesToSlot(602), 600);
	assert.equal(snapMinutesToSlot(608), 615);
	assert.equal(snapMinutesToSlot(622), 615);
});

test('drag coordinate helper anchors existing blocks from their left edge', async () => {
	const { clientXToTimelineStartTime } = await importTimeHelpers();

	assert.equal(
		clientXToTimelineStartTime({
			clientX: 216,
			trackLeft: 100,
			dragOffsetPx: 20,
			dayStartTime: '08:30',
			slotWidthPx: 24
		}),
		'09:30'
	);
});

test('timeline drag preview reports snapped start end left width and validity', async () => {
	const { buildTimelineDragPreview } = await importTimeHelpers();
	const day = {
		id: 'day-1',
		startTime: '08:30:00',
		endTime: '12:00:00',
		gradeLevelIds: [],
		blockedWindows: [],
		roomAssignments: [{ classroomId: 'classroom-1', roomId: 'room-1' }]
	};

	const preview = buildTimelineDragPreview({
		day,
		clientX: 156,
		trackLeft: 100,
		dragOffsetPx: 0,
		slotWidthPx: 24,
		durationMinutes: 60,
		candidate: {
			examScheduleItemId: 'item-1',
			classroomId: 'classroom-1',
			gradeLevelId: 'grade-1'
		},
		scheduledSessions: []
	});

	assert.equal(preview.startTime, '09:00');
	assert.equal(preview.endTime, '10:00');
	assert.equal(preview.leftPx, 48);
	assert.equal(preview.widthPx, 96);
	assert.equal(preview.valid, true);
});

test('timeline placement rejects manually typed off-grid times', async () => {
	const { validateTimelinePlacement } = await importTimeHelpers();

	const result = validateTimelinePlacement({
		dayStartTime: '08:30',
		dayEndTime: '16:00',
		startTime: '08:37',
		durationMinutes: 60,
		blockedWindows: []
	});

	assert.equal(result.ok, false);
	assert.match(result.reason ?? '', /15|นาที/);
});

test('shared exam session validation rejects same-classroom and same-room conflicts', async () => {
	const { validateExamSessionPlacement } = await importTimeHelpers();
	const day = {
		id: 'day-1',
		startTime: '08:30',
		endTime: '16:00',
		gradeLevelIds: ['g1'],
		blockedWindows: [],
		roomAssignments: [
			{ classroomId: 'class-a', roomId: 'room-1' },
			{ classroomId: 'class-b', roomId: 'room-2' }
		]
	};

	assert.match(
		validateExamSessionPlacement({
			day,
			candidate: {
				examScheduleItemId: 'item-new',
				classroomId: 'class-a',
				gradeLevelId: 'g1',
				startTime: '09:15',
				durationMinutes: 45
			},
			scheduledSessions: [
				{
					id: 'session-existing',
					examDayId: 'day-1',
					classroomId: 'class-a',
					roomId: 'room-1',
					startsAt: '09:00',
					endsAt: '10:00'
				}
			]
		}).reason ?? '',
		/ห้องเรียน/
	);

	assert.match(
		validateExamSessionPlacement({
			day: {
				...day,
				roomAssignments: [
					{ classroomId: 'class-a', roomId: 'room-1' },
					{ classroomId: 'class-b', roomId: 'room-1' }
				]
			},
			candidate: {
				examScheduleItemId: 'item-new',
				classroomId: 'class-a',
				gradeLevelId: 'g1',
				startTime: '09:15',
				durationMinutes: 45
			},
			scheduledSessions: [
				{
					id: 'session-existing',
					examDayId: 'day-1',
					classroomId: 'class-b',
					roomId: 'room-1',
					startsAt: '09:00',
					endsAt: '10:00'
				}
			]
		}).reason ?? '',
		/ห้องสอบ/
	);
});
