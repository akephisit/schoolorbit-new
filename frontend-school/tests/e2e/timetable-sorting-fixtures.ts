import type { TimetableBlockWorkspace } from '#lib/api/timetable.js';
import { timetableIds } from './timetable-test-harness.ts';

export const sortingIds = {
	primary: 'room-primary-2',
	room2: timetableIds.homeroom,
	room10: 'room-secondary-1-10',
	grade2: 'room-secondary-2',
	additional: 'group-additional',
	unknown: 'group-unknown',
	activity: 'group-activity',
	courseActivity: 'group-course-activity',
	primaryBasic: 'group-primary-basic',
	primaryAdditional: 'group-primary-additional',
	room10Group: 'group-room-10',
	multi: 'group-multi',
	missing: 'group-missing',
	sync: timetableIds.offeringSync,
	syncPrimary: 'offering-sync-primary',
	syncGrade2: 'offering-sync-grade2'
} as const;

// All sixteen identities fell within 160–162 degrees under the retired independent hue hash.
export const closeHueGroupIds = [
	'group-color-3',
	'group-color-200',
	'group-color-396',
	'group-color-541',
	'group-color-917',
	'group-color-1111',
	'group-color-1592',
	'group-color-1655',
	'group-color-1947',
	'group-color-1990',
	'group-color-2003',
	'group-color-2217',
	'group-color-2454',
	'group-color-2542',
	'group-color-2662',
	'group-color-2928'
];

export function makeColorCohort(count = 16): TimetableBlockWorkspace['learningGroups'] {
	const template = makeSortingFixture().learningGroups.find(
		(group) => group.id === timetableIds.groupA
	)!;
	return Array.from({ length: count }, (_, index) => ({
		...template,
		id: closeHueGroupIds[index] ?? `color-overflow-${index}`,
		learningOfferingId: `offering-color-${index}`,
		offeringCode: `ค${index + 1}`,
		offeringName: `วิชาทดสอบสี ${index + 1}`
	}));
}

export function makeSortingFixture(): Pick<
	TimetableBlockWorkspace,
	'homerooms' | 'learningGroups' | 'ordinaryDemands' | 'synchronizedDemands'
> {
	const homeroom = (
		id: string,
		type: string,
		year: number,
		roomNumber: string
	): TimetableBlockWorkspace['homerooms'][number] => ({
		id,
		code: `custom-${id}`,
		name: `${type === 'primary' ? 'ป.' : 'ม.'}${year}/${roomNumber}`,
		gradeLevelId: `${type}-${year}`,
		gradeLevelType: type,
		gradeLevelYear: year,
		roomNumber,
		isActive: true
	});
	const homerooms = [
		homeroom(sortingIds.grade2, 'secondary', 2, '1'),
		homeroom(sortingIds.room10, 'secondary', 1, '10'),
		homeroom(sortingIds.primary, 'primary', 2, '1'),
		homeroom(sortingIds.room2, 'secondary', 1, '2')
	];
	const group = (
		id: string,
		subjectType: string | null,
		code: string,
		targets: string[],
		offeringKind = 'course'
	): TimetableBlockWorkspace['learningGroups'][number] => ({
		id,
		learningOfferingId: `offering-${id}`,
		code: `group-${id}`,
		name: `${homerooms.find((room) => room.id === targets[0])?.name ?? 'ไม่ระบุห้อง'} ${code}`,
		status: 'published',
		rosterStatus: 'published',
		offeringKind,
		subjectType,
		offeringCode: code,
		offeringName: `วิชา ${code}`,
		homeroomIds: targets,
		preferredRoomIds: [],
		eligibleInstructors: [
			{
				teacherId: timetableIds.teacherA,
				displayName: 'ครูคณิตศาสตร์ A',
				role: 'primary',
				orderIndex: 0
			},
			{
				teacherId: timetableIds.teacherB,
				displayName: 'ครูคณิตศาสตร์ B',
				role: 'secondary',
				orderIndex: 1
			}
		]
	});
	const learningGroups = [
		group(timetableIds.groupA, 'BASIC', 'ค2', [sortingIds.room2]),
		group(timetableIds.groupB, 'BASIC', 'ค10', [sortingIds.room2]),
		group(sortingIds.additional, 'ADDITIONAL', 'ก1', [sortingIds.room2]),
		group(sortingIds.unknown, null, 'ข1', [sortingIds.room2]),
		group(sortingIds.activity, 'BASIC', 'ก20', [sortingIds.room2], 'activity'),
		group(sortingIds.courseActivity, 'ACTIVITY', 'ก2', [sortingIds.room2]),
		group(sortingIds.primaryBasic, 'BASIC', 'ก99', [sortingIds.primary]),
		group(sortingIds.primaryAdditional, 'ADDITIONAL', 'ก1', [sortingIds.primary]),
		group(sortingIds.room10Group, 'BASIC', 'ค2', [sortingIds.room10]),
		group(sortingIds.multi, 'BASIC', 'ก10', [sortingIds.grade2, sortingIds.primary]),
		group(sortingIds.missing, 'BASIC', 'ก1', ['missing-room'])
	].reverse();
	const ordinaryDemands = learningGroups.map((item) => ({
		learningGroupId: item.id,
		learningOfferingId: item.learningOfferingId,
		offeringCode: item.offeringCode,
		offeringName: item.offeringName,
		requiredPeriods: 3,
		scheduledPeriods: 0,
		remainingPeriods: 3,
		homeroomIds: item.homeroomIds,
		eligibleInstructors: item.eligibleInstructors
	}));
	const synchronizedDemands = [
		{
			learningOfferingId: sortingIds.syncGrade2,
			offeringCode: 'ก3',
			offeringName: 'กิจกรรม ม.2',
			requiredPeriods: 1,
			scheduledPeriods: 0,
			intendedHomeroomIds: [sortingIds.grade2]
		},
		{
			learningOfferingId: sortingIds.sync,
			offeringCode: 'ก10',
			offeringName: 'กิจกรรม ม.1',
			requiredPeriods: 1,
			scheduledPeriods: 0,
			intendedHomeroomIds: [sortingIds.room2]
		},
		{
			learningOfferingId: sortingIds.syncPrimary,
			offeringCode: 'ก2',
			offeringName: 'กิจกรรมประถม',
			requiredPeriods: 1,
			scheduledPeriods: 0,
			intendedHomeroomIds: [sortingIds.primary]
		}
	];
	return {
		homerooms,
		learningGroups,
		ordinaryDemands,
		synchronizedDemands: synchronizedDemands.map((demand) => ({
			...demand,
			conflictGroupCount: 0,
			excludedGroupCount: 0,
			linkedGroupCount: 0,
			pendingGroupCount: 0
		}))
	};
}
