import assert from 'node:assert/strict';
import test from 'node:test';

import {
	filterHomeroomDeliveryRooms,
	sortHomeroomDeliveryItems,
	summarizeHomeroomDelivery
} from '../../src/lib/academic/homeroom-delivery.ts';

const rooms = [
	{
		homeroom: { id: 'room-1', name: 'ม.1/1' },
		gradeLevel: {
			id: 'grade-1',
			code: 'M1',
			name: 'มัธยมศึกษาปีที่ 1',
			levelType: 'secondary',
			levelOrder: 301
		},
		studyProgram: {
			id: 'program-1',
			code: 'SCI',
			name: 'วิทย์-คณิต',
			editionId: 'curriculum-1',
			levelName: 'หลักสูตร 2569'
		},
		expectedCount: 2,
		readyCount: 1,
		blockers: [],
		items: []
	},
	{
		homeroom: { id: 'room-2', name: 'ม.1/2' },
		gradeLevel: {
			id: 'grade-1',
			code: 'M1',
			name: 'มัธยมศึกษาปีที่ 1',
			levelType: 'secondary',
			levelOrder: 301
		},
		studyProgram: {
			id: 'program-2',
			code: 'ART',
			name: 'ศิลป์ภาษา',
			editionId: 'curriculum-1',
			levelName: 'หลักสูตร 2569'
		},
		expectedCount: 1,
		readyCount: 1,
		blockers: [],
		items: []
	}
] as const;

test('filters rooms by Thai room or program text and readiness', () => {
	assert.deepEqual(
		filterHomeroomDeliveryRooms(rooms, 'วิทย์', 'all').map((room) => room.homeroom.id),
		['room-1']
	);
	assert.deepEqual(
		filterHomeroomDeliveryRooms(rooms, '', 'attention').map((room) => room.homeroom.id),
		['room-1']
	);
});

test('summarizes readiness without treating missing work as ready', () => {
	assert.deepEqual(summarizeHomeroomDelivery(rooms), {
		roomCount: 2,
		expectedCount: 3,
		readyCount: 2,
		attentionRoomCount: 1
	});
});

test('room items use pinned BASIC then ADDITIONAL classification independently of required/elective and code digits', () => {
	const course = (
		requirementId: string,
		code: string,
		subjectType: string | null,
		requirementKind = 'required'
	) => ({
		requirementId,
		code,
		name: requirementId,
		subjectType,
		requirementKind,
		resourceKind: 'course' as const
	});
	const items = [
		{
			...course('activity-b', 'AA000', null),
			resourceKind: 'activity' as const,
			name: 'กิจกรรม ข'
		},
		course('additional', 'ค21101', 'ADDITIONAL'),
		course('legacy', 'ค11101', 'LEGACY-UNKNOWN'),
		course('basic-23', 'ค23101', 'BASIC', 'elective'),
		{
			...course('activity-a', 'ZZ000', null),
			resourceKind: 'activity' as const,
			name: 'กิจกรรม ก'
		},
		course('basic-21', 'ค21102', 'BASIC', 'elective'),
		course('unknown', 'ค01101', null)
	];
	const original = [...items];
	assert.deepEqual(
		sortHomeroomDeliveryItems(items).map((item) => item.requirementId),
		['basic-21', 'basic-23', 'additional', 'unknown', 'legacy', 'activity-a', 'activity-b']
	);
	assert.deepEqual(items, original);
});

import { buildDeliveryAlignmentFacts } from '../../src/lib/academic/homeroom-delivery.ts';
import type {
	HomeroomDeliveryItem,
	HomeroomDeliveryWorkspace
} from '../../src/lib/api/learning-delivery.js';

function comparisonWorkspace(): HomeroomDeliveryWorkspace {
	const missing: HomeroomDeliveryItem = {
		requirementId: 'new-requirement',
		catalogVersionId: 'new-version',
		resourceKind: 'course',
		subjectType: 'BASIC',
		code: 'ว30104',
		name: 'วิทยาศาสตร์',
		requirementKind: 'required',
		schedulingMode: null,
		standardPeriodsPerWeek: 2,
		weeklyPeriodTarget: null,
		offeringId: null,
		offeringState: 'missing',
		groupMode: 'missing',
		teacherState: 'missing_primary',
		alignmentStates: ['curriculum_requirement_not_offered'],
		groups: []
	};
	const old = {
		offeringId: 'old-offering',
		catalogVersionId: 'old-version',
		resourceKind: 'course' as const,
		code: 'ว30104',
		name: 'วิทยาศาสตร์*',
		startsOn: null,
		endsOn: null,
		weeklyPeriodTarget: 2,
		alignmentStates: ['extra_offering' as const]
	};
	const room = (id: string): HomeroomDeliveryWorkspace['homerooms'][number] => ({
		homeroom: { id, name: id },
		gradeLevel: {
			id: 'grade',
			code: 'M4',
			name: 'ม.4',
			level_type: 'secondary',
			level_order: 4,
			short_name: null
		},
		studyProgram: {
			id: 'program',
			code: 'SCI',
			name: 'วิทย์',
			editionId: 'edition',
			levelName: 'มัธยม',
			curriculumLevelId: 'level',
			editionName: 'หลักสูตร',
			displayOrder: 0,
			gradeLevelIds: ['grade']
		},
		curriculumLevelId: 'level',
		expectedCount: 1,
		readyCount: 0,
		items: [missing],
		extraOfferings: [old],
		blockers: []
	});
	return {
		academicYearId: 'year',
		academicTermId: 'term',
		deliveryVersionId: 'version',
		deliveryVersionStatus: 'draft',
		deliveryVersionEffectiveFrom: null,
		homerooms: [room('room-a'), room('room-b')],
		unlinked: [
			{
				offeringId: 'old-offering',
				groupId: 'old-group',
				resourceKind: 'course',
				code: 'ว30104',
				name: 'วิทยาศาสตร์*',
				reason: 'กลุ่มนี้ยังไม่เชื่อมห้อง'
			}
		]
	};
}

test('comparison keeps same-code old and missing new versions separate and deduplicates overlap across rooms', () => {
	const workspace = comparisonWorkspace();
	const original = structuredClone(workspace);
	const facts = buildDeliveryAlignmentFacts(workspace);
	assert.equal(facts.missingCount, 1);
	assert.equal(facts.extraCount, 1);
	assert.equal(facts.unlinkedCount, 1);
	assert.equal(facts.offerings.size, 1);
	assert.equal(facts.affectedExistingCount, 1);
	assert.deepEqual(facts.missingRequirements[0].requirementIds, ['new-requirement']);
	assert.deepEqual(
		facts.missingRequirements[0].rooms.map((room) => room.id),
		['room-a', 'room-b']
	);
	assert.equal(facts.missingRequirements[0].catalogVersionId, 'new-version');
	assert.equal(facts.offerings.get('old-offering')?.name, 'วิทยาศาสตร์*');
	assert.equal(facts.offerings.get('old-offering')?.unlinkedReasons.length, 1);
	assert.deepEqual(workspace, original);
});

test('comparison preserves mixed room states and unlinked-only unknown evidence without claiming a match', () => {
	const workspace = comparisonWorkspace();
	workspace.homerooms[1].extraOfferings = [];
	workspace.homerooms[1].items = [
		{
			...workspace.homerooms[1].items[0],
			catalogVersionId: 'old-version',
			offeringId: 'old-offering',
			offeringState: 'draft',
			alignmentStates: ['matches_curriculum']
		}
	];
	workspace.unlinked.push({
		offeringId: 'unknown-offering',
		resourceKind: 'activity',
		code: 'INTERNAL',
		name: 'กิจกรรมเพิ่มเติม',
		reason: 'ยังไม่กำหนดเป้าหมาย'
	});
	const facts = buildDeliveryAlignmentFacts(workspace);
	assert.deepEqual(
		facts.offerings
			.get('old-offering')
			?.states.map((entry) => [entry.state, entry.rooms.map((room) => room.id)]),
		[
			['extra_offering', ['room-a']],
			['matches_curriculum', ['room-b']]
		]
	);
	assert.deepEqual(facts.offerings.get('unknown-offering')?.states, []);
	assert.equal(facts.unlinkedCount, 2);
	assert.equal(facts.missingRequirements.length, 1);
});
