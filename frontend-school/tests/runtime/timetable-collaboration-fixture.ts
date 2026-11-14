import type { TimetableLookupChanges, TimetableBlockWorkspace } from '../../src/lib/api/timetable.js';
import { makeTimetable, timetableIds } from '../e2e/timetable-test-harness.ts';

export function collaborationWorkspace(): TimetableBlockWorkspace {
	return {
		homerooms: [
			{
				id: timetableIds.homeroom,
				code: 'M1-1',
				name: 'ม.1/1',
				gradeLevelId: '81000000-0000-4000-8000-000000000301',
				gradeLevelType: 'secondary',
				gradeLevelYear: 1,
				roomNumber: '1',
				isActive: true
			}
		],
		learningGroups: [timetableIds.groupA, timetableIds.groupB].map((id, index) => ({
			id,
			learningOfferingId: index ? timetableIds.offeringB : timetableIds.offeringA,
			code: `GROUP-${index}`,
			name: `กลุ่ม ${index + 1}`,
			registrationOpen: false,
			offeringKind: 'course',
			subjectType: 'BASIC',
			offeringCode: `ค${index + 1}`,
			offeringName: `รายวิชา ${index + 1}`,
			homeroomIds: [timetableIds.homeroom],
			preferredRoomIds: [],
			eligibleInstructors: []
		})),
		ordinaryDemands: [timetableIds.groupA, timetableIds.groupB].map((learningGroupId, index) => ({
			learningGroupId,
			learningOfferingId: index ? timetableIds.offeringB : timetableIds.offeringA,
			offeringCode: `ค${index + 1}`,
			offeringName: `รายวิชา ${index + 1}`,
			requiredPeriods: 3,
			scheduledPeriods: 0,
			remainingPeriods: 3,
			homeroomIds: [timetableIds.homeroom],
			eligibleInstructors: []
		})),
		synchronizedDemands: [],
		timetable: makeTimetable(),
		blocks: [],
		bellPeriods: [],
		rooms: [],
		staff: [],
		offeringSubjectGroups: [],
		deliveryRowVersion: 1,
		sourceIssues: [],
		totalBlockCount: 0,
		summary: {
			blockCount: 0,
			ordinaryDemandCount: 1,
			synchronizedDemandCount: 0,
			linkedGroupCount: 0,
			waitingGroupCount: 0,
			conflictGroupCount: 0,
			excludedGroupCount: 0
		}
	};
}

export function emptyLookupChanges(): TimetableLookupChanges {
	return {
		upsertedStaff: [],
		removedStaffIds: [],
		upsertedHomerooms: [],
		removedHomeroomIds: [],
		upsertedRooms: [],
		removedRoomIds: [],
		upsertedBellPeriods: [],
		removedBellPeriodIds: [],
		upsertedOfferingSubjectGroups: [],
		removedSubjectGroupOfferingIds: []
	};
}
