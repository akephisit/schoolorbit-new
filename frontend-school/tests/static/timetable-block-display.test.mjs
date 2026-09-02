import assert from 'node:assert/strict';
import test from 'node:test';

const displayModule = await import('../../src/lib/academic/timetable/block-display.ts').catch(
	() => ({})
);

function block(overrides = {}) {
	return {
		academicTermId: 'term-1',
		academicYearId: 'year-1',
		bellScheduleId: 'bell-1',
		bellSchedulePeriodId: 'period-1',
		blockKind: 'course',
		createdAt: '2026-09-02T00:00:00Z',
		dayOfWeek: 'MON',
		endTime: '09:20:00',
		groups: [],
		homerooms: [],
		id: 'block-1',
		isActive: true,
		learningOfferingId: 'offering-1',
		note: null,
		offeringCode: 'ค21101',
		offeringName: 'คณิตศาสตร์พื้นฐาน',
		periodName: 'คาบ 1',
		rowVersion: 1,
		schedulingMode: null,
		seriesId: null,
		startTime: '08:30:00',
		structuralKind: null,
		syncStates: [],
		teachers: [],
		timetableVersionId: 'version-1',
		title: null,
		updatedAt: '2026-09-02T00:00:00Z',
		...overrides
	};
}

function teacher(teacherId, displayName) {
	return { teacherId, displayName, role: 'primary', orderIndex: 0 };
}

function homeroom(index) {
	return {
		id: `target-${index}`,
		homeroomId: `homeroom-${index}`,
		code: `M1-${index}`,
		name: `ม.1/${index}`,
		roomId: `room-${index}`,
		roomCode: `10${index}`,
		isActive: true,
		rowVersion: 1
	};
}

test('shared scheduler blocks compact long teacher and homeroom lists', () => {
	const result = displayModule.buildTimetableBlockDisplay?.(
		block({
			blockKind: 'structural',
			learningOfferingId: null,
			offeringCode: null,
			offeringName: null,
			title: 'กิจกรรมหน้าเสาธง',
			structuralKind: 'flag_ceremony',
			homerooms: [1, 2, 3, 4, 5, 6].map(homeroom),
			teachers: [
				teacher('teacher-1', 'ครูหนึ่ง'),
				teacher('teacher-2', 'ครูสอง'),
				teacher('teacher-3', 'ครูสาม'),
				teacher('teacher-4', 'ครูสี่')
			]
		}),
		'scheduler'
	);

	assert.deepEqual(result, {
		shared: true,
		contextLabel: 'กิจกรรมรวม',
		teacherLabel: 'ครูหนึ่ง, ครูสอง +อีก 2 คน',
		scopeLabel: '6 ห้อง',
		groupLabel: null,
		roomLabel: null
	});
});

test('personal timetable hides shared activity classroom and room lists', () => {
	const result = displayModule.buildTimetableBlockDisplay?.(
		block({
			blockKind: 'activity',
			schedulingMode: 'synchronized',
			groups: [
				{
					id: 'target-group-1',
					learningGroupId: 'group-1',
					learningOfferingId: 'offering-1',
					code: 'CLUB-1',
					name: 'ชุมนุมหุ่นยนต์ กลุ่ม 1',
					homeroomIds: ['homeroom-1', 'homeroom-2'],
					instructors: [teacher('teacher-1', 'ครูหนึ่ง')],
					roomId: 'room-1',
					roomCode: '101',
					isActive: true,
					rowVersion: 1,
					syncStatus: 'linked'
				},
				{
					id: 'target-group-2',
					learningGroupId: 'group-2',
					learningOfferingId: 'offering-1',
					code: 'CLUB-2',
					name: 'ชุมนุมหุ่นยนต์ กลุ่ม 2',
					homeroomIds: ['homeroom-3', 'homeroom-4'],
					instructors: [teacher('teacher-1', 'ครูหนึ่ง')],
					roomId: 'room-2',
					roomCode: '102',
					isActive: true,
					rowVersion: 1,
					syncStatus: 'linked'
				}
			]
		}),
		'personal'
	);

	assert.equal(result?.shared, true);
	assert.equal(result?.contextLabel, 'กิจกรรมพร้อมกัน');
	assert.equal(result?.groupLabel, null);
	assert.equal(result?.roomLabel, null);
});

test('ordinary course keeps the teacher classroom and room information', () => {
	const result = displayModule.buildTimetableBlockDisplay?.(
		block({
			groups: [
				{
					id: 'target-group-1',
					learningGroupId: 'group-1',
					learningOfferingId: 'offering-1',
					code: 'M1-1-MATH',
					name: 'ม.1/1 คณิตศาสตร์',
					homeroomIds: ['homeroom-1'],
					instructors: [teacher('teacher-1', 'ครูหนึ่ง')],
					roomId: 'room-1',
					roomCode: '101',
					isActive: true,
					rowVersion: 1,
					syncStatus: null
				}
			]
		}),
		'personal'
	);

	assert.deepEqual(result, {
		shared: false,
		contextLabel: null,
		teacherLabel: 'ครูหนึ่ง',
		scopeLabel: null,
		groupLabel: 'ม.1/1 คณิตศาสตร์',
		roomLabel: '101'
	});
});
