import type { TimetableBlockWorkspace } from '#lib/api/timetable.js';
import { timetableIds } from './timetable-test-harness.ts';

export const sortingIds = {
	primary: '72000000-0000-4000-8000-000000000001',
	room2: timetableIds.homeroom,
	room10: '72000000-0000-4000-8000-000000000002',
	grade2: '72000000-0000-4000-8000-000000000003',
	additional: '72000000-0000-4000-8000-000000000004',
	unknown: '72000000-0000-4000-8000-000000000005',
	activity: '72000000-0000-4000-8000-000000000006',
	courseActivity: '72000000-0000-4000-8000-000000000007',
	primaryBasic: '72000000-0000-4000-8000-000000000008',
	primaryAdditional: '72000000-0000-4000-8000-000000000009',
	room10Group: '72000000-0000-4000-8000-000000000010',
	multi: '72000000-0000-4000-8000-000000000011',
	missing: '72000000-0000-4000-8000-000000000012',
	sync: timetableIds.offeringSync,
	syncPrimary: '72000000-0000-4000-8000-000000000013',
	syncGrade2: '72000000-0000-4000-8000-000000000014'
} as const;

// All sixteen identities fell within 160–162 degrees under the retired independent hue hash.
export const closeHueGroupIds = [
	'73000000-0000-4000-8000-000000000007',
	'73000000-0000-4000-8000-000000000087',
	'73000000-0000-4000-8000-000000000116',
	'73000000-0000-4000-8000-000000000181',
	'73000000-0000-4000-8000-000000000201',
	'73000000-0000-4000-8000-000000000334',
	'73000000-0000-4000-8000-000000000409',
	'73000000-0000-4000-8000-000000000544',
	'73000000-0000-4000-8000-000000000691',
	'73000000-0000-4000-8000-000000001062',
	'73000000-0000-4000-8000-000000001066',
	'73000000-0000-4000-8000-000000001081',
	'73000000-0000-4000-8000-000000001249',
	'73000000-0000-4000-8000-000000001268',
	'73000000-0000-4000-8000-000000001331',
	'73000000-0000-4000-8000-000000001353'
];

export function makeColorCohort(count = 16): TimetableBlockWorkspace['learningGroups'] {
	const template = makeSortingFixture().learningGroups.find(
		(group) => group.id === timetableIds.groupA
	)!;
	return Array.from({ length: count }, (_, index) => ({
		...template,
		id:
			closeHueGroupIds[index] ??
			`73000000-0000-4000-8000-${String(100000 + index).padStart(12, '0')}`,
		learningOfferingId: `74000000-0000-4000-8000-${String(index).padStart(12, '0')}`,
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
		gradeLevelId: `75000000-0000-4000-8000-${String((type === 'primary' ? 100 : 200) + year).padStart(12, '0')}`,
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
		learningOfferingId: `76000000-${id.slice(9)}`,
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
		group(sortingIds.missing, 'BASIC', 'ก1', ['82000000-0000-4000-8000-000000000999'])
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
