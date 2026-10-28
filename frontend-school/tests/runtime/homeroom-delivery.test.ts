import assert from 'node:assert/strict';
import test from 'node:test';

import {
	deliveryAlignmentNeedsReview,
	deliveryItemStatuses,
	deliveryRoomStatus,
	deliveryOfferingStatuses,
	filterHomeroomDeliveryRooms,
	sortHomeroomDeliveryItems,
	summarizeHomeroomDelivery
} from '../../src/lib/academic/homeroom-delivery.ts';

function statusItem(patch: Partial<HomeroomDeliveryItem> = {}): HomeroomDeliveryItem {
	return {
		...comparisonWorkspace().homerooms[0].items[0],
		offeringId: 'opened',
		offeringState: 'draft',
		groupMode: 'normal',
		teacherState: 'assigned',
		weeklyPeriodTarget: 2,
		alignmentStates: ['matches_curriculum'],
		groups: [
			{
				id: 'group',
				code: 'INTERNAL',
				name: 'กลุ่มเรียน',
				status: 'draft',
				rosterStatus: 'draft',
				teachersLocked: false,
				homeroomIds: ['room-a'],
				homeroomNames: ['ม.1/1'],
				primaryTeacherCount: 1
			}
		],
		...patch
	};
}
const rooms = comparisonWorkspace().homerooms.map((room, index) => ({
	...room,
	homeroom: { ...room.homeroom, id: `room-${index + 1}`, name: `ม.1/${index + 1}` },
	studyProgram: { ...room.studyProgram, name: index === 0 ? 'วิทย์-คณิต' : 'ศิลป์ภาษา' },
	expectedCount: index === 0 ? 2 : 1,
	readyCount: 1,
	extraOfferings: [],
	items:
		index === 0
			? [
					statusItem(),
					statusItem({
						requirementId: 'missing',
						offeringId: null,
						groups: [],
						teacherState: 'missing_primary',
						groupMode: 'missing',
						offeringState: 'missing',
						alignmentStates: ['curriculum_requirement_not_offered']
					})
				]
			: [statusItem()]
}));

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

function statusRoom(items: HomeroomDeliveryItem[]): HomeroomDeliveryWorkspace['homerooms'][number] {
	return {
		...comparisonWorkspace().homerooms[0],
		items,
		extraOfferings: [],
		expectedCount: items.length,
		readyCount: items.filter(
			(item) => item.offeringId && (item.groups.length || item.groupMode === 'central')
		).length
	};
}
const statusByKey = (statuses: ReturnType<typeof deliveryItemStatuses>, key: string) =>
	statuses.find((status) => status.key === key);

test('full opening coverage coexists with real missing-primary warnings and drives the same review filters and count', () => {
	const item = statusItem({
		teacherState: 'missing_primary',
		groups: [{ ...statusItem().groups[0], primaryTeacherCount: 0 }]
	});
	const room = statusRoom([item]);
	const state = deliveryRoomStatus(room);
	assert.equal(state.coverageComplete, true);
	assert.equal(state.needsAttention, true);
	assert.equal(statusByKey(state.statuses, 'primary_missing')?.count, 1);
	assert.equal(statusByKey(state.statuses, 'primary_missing')?.unit, 'กลุ่ม');
	assert.deepEqual(filterHomeroomDeliveryRooms([room], '', 'attention'), [room]);
	assert.deepEqual(filterHomeroomDeliveryRooms([room], '', 'ready'), []);
	assert.equal(summarizeHomeroomDelivery([room]).attentionRoomCount, 1);
	assert.equal(summarizeHomeroomDelivery([room]).readyCount, 1);
});

test('room status deduplicates real group and offering identities across requirements without mutating provider facts', () => {
	const item = statusItem({
		weeklyPeriodTarget: 0,
		alignmentStates: ['operational_periods_differ'],
		teacherState: 'missing_primary',
		groups: [{ ...statusItem().groups[0], primaryTeacherCount: 0 }]
	});
	const room = statusRoom([item, { ...item, requirementId: 'second-requirement' }]);
	const extra = comparisonWorkspace().homerooms[0].extraOfferings[0];
	room.extraOfferings = [extra, extra];
	const original = structuredClone(room);
	const state = deliveryRoomStatus(room);
	for (const key of [
		'primary_missing',
		'groups_present',
		'operational_periods_differ',
		'zero_weekly_periods',
		'extra_offering'
	])
		assert.equal(statusByKey(state.statuses, key)?.count, 1, key);
	assert.equal(statusByKey(state.statuses, 'missing_period_target'), undefined);
	assert.equal(statusByKey(state.statuses, 'extra_offering')?.tone, 'info');
	assert.deepEqual(room, original);
});

for (const mode of ['central', 'deferred'] as const) {
	test(`${mode} zero-group activity does not invent a missing primary teacher`, () => {
		const item = statusItem({
			resourceKind: 'activity',
			schedulingMode: mode === 'central' ? 'synchronized' : 'independent',
			groupMode: mode,
			groups: [],
			teacherState: mode === 'central' ? 'deferred' : 'missing_primary',
			weeklyPeriodTarget: 0
		});
		const statuses = deliveryItemStatuses(item);
		assert.equal(statusByKey(statuses, 'primary_missing'), undefined);
		assert.equal(statusByKey(statuses, 'primary_assigned'), undefined);
		assert.equal(statusByKey(statuses, 'teachers_deferred')?.count, 1);
		assert.equal(
			statusByKey(statuses, mode === 'central' ? 'central_activity' : 'groups_deferred')?.count,
			1
		);
		assert.equal(deliveryRoomStatus(statusRoom([item])).needsAttention, mode !== 'central');
	});
}

test('existing synchronized groups still require primary teachers and null weekly targets stay distinct from zero', () => {
	const item = statusItem({
		resourceKind: 'activity',
		schedulingMode: 'synchronized',
		teacherState: 'missing_primary',
		weeklyPeriodTarget: null,
		groups: [{ ...statusItem().groups[0], primaryTeacherCount: 0 }]
	});
	const statuses = deliveryItemStatuses(item);
	assert.equal(statusByKey(statuses, 'primary_missing')?.count, 1);
	assert.equal(statusByKey(statuses, 'missing_period_target')?.count, 1);
	assert.equal(statusByKey(statuses, 'zero_weekly_periods'), undefined);
});

test('missing offerings and unknown structures never fabricate teacher defects or ready zero-of-zero status', () => {
	const missing = statusItem({
		offeringId: null,
		offeringState: 'missing',
		weeklyPeriodTarget: null,
		groupMode: 'missing',
		teacherState: 'missing_primary',
		groups: [],
		alignmentStates: ['curriculum_requirement_not_offered']
	});
	assert.deepEqual(
		deliveryItemStatuses(missing).map((status) => status.key),
		['curriculum_requirement_not_offered']
	);
	const room = statusRoom([]);
	room.blockers = [
		{
			code: 'structure_missing',
			message: 'ยังไม่มีโครงสร้าง',
			recoveryPath: '/staff/academic/curricula'
		}
	];
	assert.equal(deliveryRoomStatus(room).coverageComplete, false);
	assert.equal(deliveryRoomStatus(room).needsAttention, true);
	assert.equal(statusByKey(deliveryRoomStatus(room).statuses, 'prerequisites')?.unit, 'ห้อง');
	assert.equal(
		deliveryRoomStatus({ ...room, expectedCount: 1, readyCount: 1 }).coverageComplete,
		false
	);
});

function statusOffering(): Parameters<typeof deliveryOfferingStatuses>[0] {
	return {
		id: 'opened',
		kind: 'activity',
		code: 'ACT',
		name: 'กิจกรรม',
		owningOrganizationUnitId: 'unit',
		weeklyPeriodTarget: 0,
		targets: [],
		homeroomIds: [],
		catalog: {
			kind: 'activity',
			activityId: 'activity',
			activityVersionId: 'version',
			registrationType: 'assigned',
			schedulingMode: 'synchronized',
			hours: '20',
			attendanceRequirement: {},
			passCriteria: {
				requireAttendance: true,
				requireTeacherConfirmation: true,
				outcomes: ['pass', 'fail']
			}
		},
		groups: [
			{
				id: 'group',
				code: 'INTERNAL',
				name: 'กลุ่มเรียน',
				homeroomIds: ['room-a'],
				preferredRoomIds: [],
				teachers: []
			}
		]
	};
}

test('offering statuses use only selected real groups and retain missing target-room grouping evidence alongside known teacher completeness', () => {
	const offering = statusOffering();
	offering.groups[0].teachers = [
		{ assignmentId: 'assignment', teacherId: 'teacher', displayName: 'ครูหลัก', role: 'primary' }
	];
	const workspace = comparisonWorkspace();
	workspace.homerooms[0] = statusRoom([statusItem()]);
	workspace.homerooms[1] = statusRoom([
		statusItem({ groups: [], groupMode: 'missing', teacherState: 'missing_primary' })
	]);
	const statuses = deliveryOfferingStatuses(offering, workspace);
	assert.equal(statusByKey(statuses, 'primary_assigned')?.label, 'ครูหลักครบใน');
	assert.equal(statusByKey(statuses, 'primary_assigned')?.count, 1);
	assert.equal(statusByKey(statuses, 'groups_present')?.count, 1);
	assert.equal(statusByKey(statuses, 'groups_missing')?.count, 1);
	assert.equal(statusByKey(statuses, 'primary_missing'), undefined);
	assert.equal(
		statusByKey(statuses, 'matches_curriculum'),
		undefined,
		'operational badges cannot fabricate a curriculum comparison'
	);
});

test('offering groups outside curriculum remain inspectable and missing-primary groups count once without any room comparison', () => {
	const offering = statusOffering();
	offering.groups.push({ ...offering.groups[0] });
	const statuses = deliveryOfferingStatuses(offering);
	assert.equal(statusByKey(statuses, 'primary_missing')?.count, 1);
	assert.equal(statusByKey(statuses, 'groups_present')?.count, 1);
	assert.equal(statusByKey(statuses, 'primary_assigned'), undefined);
	assert.equal(statusByKey(statuses, 'matches_curriculum'), undefined);
	offering.groups = [];
	const central = deliveryOfferingStatuses(offering);
	assert.equal(statusByKey(central, 'central_activity')?.count, 1);
	assert.equal(statusByKey(central, 'teachers_deferred')?.count, 1);
	assert.equal(statusByKey(central, 'primary_missing'), undefined);
	if (offering.catalog.kind === 'activity') offering.catalog.schedulingMode = 'independent';
	assert.equal(statusByKey(deliveryOfferingStatuses(offering), 'groups_absent')?.count, 1);
});

test('matching coverage numbers with unresolved structure blockers keep the verdict unknown', () => {
	const room = statusRoom([statusItem()]);
	room.blockers = [
		{
			code: 'needs_structure_review',
			message: 'ต้องตรวจโครงสร้าง',
			recoveryPath: '/staff/academic/curricula'
		}
	];
	assert.equal(room.readyCount, room.expectedCount);
	const status = deliveryRoomStatus(room);
	assert.equal(status.coverageComplete, false);
	assert.equal(status.needsAttention, true);
	assert.equal(statusByKey(status.statuses, 'prerequisites')?.count, 1);
});

test('zero real groups prove absence without proving the required grouping mode or missing primary teachers', () => {
	const offering = statusOffering();
	offering.groups = [];
	if (offering.catalog.kind === 'activity') offering.catalog.schedulingMode = 'independent';
	const statuses = deliveryOfferingStatuses(offering);
	assert.equal(statusByKey(statuses, 'groups_absent')?.label, 'ยังไม่มีกลุ่มเรียน');
	assert.equal(statusByKey(statuses, 'groups_absent')?.count, 1);
	assert.equal(statusByKey(statuses, 'groups_missing'), undefined);
	assert.equal(statusByKey(statuses, 'primary_missing'), undefined);
	assert.equal(statusByKey(statuses, 'primary_assigned'), undefined);
});

test('alignment badge review filtering suppresses only known normal evidence and preserves all comparison issues', () => {
	assert.equal(deliveryAlignmentNeedsReview('matches_curriculum'), false);
	for (const state of [
		'curriculum_requirement_not_offered',
		'operational_periods_differ',
		'extra_offering',
		'ended_early'
	] as const)
		assert.equal(deliveryAlignmentNeedsReview(state), true);
});
