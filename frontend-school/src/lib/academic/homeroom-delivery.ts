import type {
	HomeroomDeliveryItem,
	HomeroomDeliveryWorkspace,
	HomeroomDeliveryRoom,
	CurriculumDeliveryAlignmentState,
	DeliveryVersion
} from '#lib/api/learning-delivery.js';

export type HomeroomReadinessFilter = 'all' | 'ready' | 'attention';

type HomeroomItemOrder = Pick<
	HomeroomDeliveryItem,
	'resourceKind' | 'subjectType' | 'code' | 'name' | 'requirementId'
>;

export function sortHomeroomDeliveryItems<T extends HomeroomItemOrder>(items: readonly T[]): T[] {
	const order = new Intl.Collator('th-TH', { numeric: true });
	const rank = (item: HomeroomItemOrder) =>
		item.resourceKind === 'activity'
			? 3
			: item.subjectType === 'BASIC'
				? 0
				: item.subjectType === 'ADDITIONAL'
					? 1
					: 2;
	return [...items].sort(
		(left, right) =>
			rank(left) - rank(right) ||
			(left.resourceKind === 'course' ? order.compare(left.code, right.code) : 0) ||
			order.compare(left.name, right.name) ||
			left.requirementId.localeCompare(right.requirementId)
	);
}

type DeliveryRoomShape = Pick<
	HomeroomDeliveryRoom,
	| 'homeroom'
	| 'gradeLevel'
	| 'studyProgram'
	| 'expectedCount'
	| 'readyCount'
	| 'items'
	| 'extraOfferings'
	| 'blockers'
>;

export type DeliveryReviewStatusKey =
	| CurriculumDeliveryAlignmentState
	| 'missing_period_target'
	| 'zero_weekly_periods'
	| 'groups_present'
	| 'groups_missing'
	| 'groups_deferred'
	| 'central_activity'
	| 'groups_absent'
	| 'primary_missing'
	| 'primary_assigned'
	| 'teachers_deferred'
	| 'closed_offering'
	| 'comparison_unknown'
	| 'coverage_incomplete'
	| 'prerequisites'
	| 'structure_unknown';
export type DeliveryReviewStatus = {
	key: DeliveryReviewStatusKey;
	label: string;
	count: number;
	unit: 'รายการ' | 'กลุ่ม' | 'ห้อง';
	tone: 'success' | 'warning' | 'info' | 'neutral';
	review: boolean;
};
type StatusDefinition = Omit<DeliveryReviewStatus, 'key' | 'count'>;
const statusDefinitions: Record<DeliveryReviewStatusKey, StatusDefinition> = {
	curriculum_requirement_not_offered: {
		label: 'หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน',
		unit: 'รายการ',
		tone: 'warning',
		review: true
	},
	operational_periods_differ: {
		label: 'คาบจริงต่างจากค่ามาตรฐานในหลักสูตร',
		unit: 'รายการ',
		tone: 'info',
		review: true
	},
	extra_offering: {
		label: 'เปิดสอนเพิ่มเติมนอกหลักสูตร',
		unit: 'รายการ',
		tone: 'info',
		review: true
	},
	ended_early: {
		label: 'หยุดสอนก่อนรุ่นเปิดสอนนี้มีผล',
		unit: 'รายการ',
		tone: 'warning',
		review: true
	},
	closed_offering: { label: 'รายการเปิดสอนปิดแล้ว', unit: 'รายการ', tone: 'warning', review: true },
	missing_period_target: {
		label: 'ยังไม่กำหนดคาบที่จัดจริง',
		unit: 'รายการ',
		tone: 'warning',
		review: true
	},
	groups_missing: { label: 'ยังไม่จัดกลุ่ม', unit: 'รายการ', tone: 'warning', review: true },
	groups_deferred: { label: 'รอจัดกลุ่ม', unit: 'รายการ', tone: 'info', review: true },
	primary_missing: { label: 'ยังไม่มีครูหลัก', unit: 'กลุ่ม', tone: 'warning', review: true },
	prerequisites: {
		label: 'ขาดข้อมูลโครงสร้างหลักสูตร',
		unit: 'ห้อง',
		tone: 'warning',
		review: true
	},
	coverage_incomplete: {
		label: 'ยังเปิดสอนรองรับไม่ครบ',
		unit: 'รายการ',
		tone: 'warning',
		review: true
	},
	structure_unknown: {
		label: 'ยังตรวจโครงสร้างไม่ได้',
		unit: 'ห้อง',
		tone: 'neutral',
		review: true
	},
	comparison_unknown: {
		label: 'ยังไม่มีผลเทียบหลักสูตร',
		unit: 'รายการ',
		tone: 'neutral',
		review: true
	},
	groups_absent: { label: 'ยังไม่มีกลุ่มเรียน', unit: 'รายการ', tone: 'neutral', review: true },
	central_activity: { label: 'กิจกรรมคาบกลาง', unit: 'รายการ', tone: 'info', review: false },
	teachers_deferred: { label: 'กำหนดครูภายหลัง', unit: 'รายการ', tone: 'info', review: false },
	zero_weekly_periods: {
		label: 'ไม่จัดคาบประจำสัปดาห์',
		unit: 'รายการ',
		tone: 'info',
		review: false
	},
	groups_present: { label: 'มีกลุ่มเรียน', unit: 'กลุ่ม', tone: 'neutral', review: false },
	primary_assigned: { label: 'มีครูหลัก', unit: 'กลุ่ม', tone: 'success', review: false },
	matches_curriculum: { label: 'ตรงกับหลักสูตร', unit: 'รายการ', tone: 'success', review: false }
};
type StatusEvidence = Map<DeliveryReviewStatusKey, Set<string>>;
function addStatus(evidence: StatusEvidence, key: DeliveryReviewStatusKey, id: string) {
	let identities = evidence.get(key);
	if (!identities) {
		identities = new Set();
		evidence.set(key, identities);
	}
	identities.add(id);
}
function statusBadges(evidence: StatusEvidence): DeliveryReviewStatus[] {
	return (Object.keys(statusDefinitions) as DeliveryReviewStatusKey[]).flatMap((key) => {
		const count = evidence.get(key)?.size ?? 0;
		if (!count) return [];
		const definition = statusDefinitions[key];
		return [
			{
				key,
				...definition,
				count,
				label:
					key === 'primary_assigned' && !evidence.has('primary_missing')
						? 'ครูหลักครบใน'
						: definition.label
			}
		];
	});
}
function itemStatusEvidence(item: HomeroomDeliveryItem, evidence: StatusEvidence) {
	const id = item.offeringId ?? `requirement:${item.requirementId}`;
	for (const state of item.alignmentStates) addStatus(evidence, state, id);
	if (!item.offeringId) {
		addStatus(evidence, 'curriculum_requirement_not_offered', id);
		return;
	}
	if (!item.alignmentStates.length) addStatus(evidence, 'comparison_unknown', id);
	if (item.offeringState === 'closed') addStatus(evidence, 'closed_offering', id);
	if (item.weeklyPeriodTarget === null) addStatus(evidence, 'missing_period_target', id);
	if (item.weeklyPeriodTarget === 0) addStatus(evidence, 'zero_weekly_periods', id);
	if (item.groupMode === 'missing') addStatus(evidence, 'groups_missing', id);
	if (item.groupMode === 'deferred' && item.groups.length === 0)
		addStatus(evidence, 'groups_deferred', id);
	if (item.groupMode === 'central') addStatus(evidence, 'central_activity', id);
	if (
		item.groups.length === 0 &&
		(item.groupMode === 'central' ||
			item.groupMode === 'deferred' ||
			item.teacherState === 'deferred')
	)
		addStatus(evidence, 'teachers_deferred', id);
	for (const group of item.groups) {
		addStatus(evidence, 'groups_present', group.id);
		addStatus(
			evidence,
			group.primaryTeacherCount > 0 ? 'primary_assigned' : 'primary_missing',
			group.id
		);
	}
}

export function deliveryItemStatuses(item: HomeroomDeliveryItem): DeliveryReviewStatus[] {
	const evidence: StatusEvidence = new Map();
	itemStatusEvidence(item, evidence);
	return statusBadges(evidence);
}

/** Coverage is the provider's opening/group count, separate from this UI review projection. */
export function deliveryRoomStatus(room: DeliveryRoomShape) {
	const evidence: StatusEvidence = new Map();
	for (const item of room.items) itemStatusEvidence(item, evidence);
	for (const item of room.extraOfferings) {
		for (const state of item.alignmentStates) addStatus(evidence, state, item.offeringId);
	}
	// A room may match one requirement while another differs. Coverage has its own scoped badge.
	evidence.delete('matches_curriculum');
	if (room.blockers.length) addStatus(evidence, 'prerequisites', room.homeroom.id);
	if (room.expectedCount === 0 || room.items.length === 0)
		addStatus(evidence, 'structure_unknown', room.homeroom.id);
	const coverageComplete =
		room.expectedCount > 0 &&
		room.items.length > 0 &&
		room.blockers.length === 0 &&
		room.readyCount === room.expectedCount;
	const statuses = statusBadges(evidence);
	if (room.expectedCount > room.readyCount)
		statuses.push({
			key: 'coverage_incomplete',
			...statusDefinitions.coverage_incomplete,
			count: room.expectedCount - room.readyCount
		});
	return {
		coverageComplete,
		statuses,
		needsAttention: !coverageComplete || statuses.some((status) => status.review)
	};
}

/** Only the selected version's real groups prove group/teacher facts; they do not prove target-room coverage. */
export function deliveryOfferingStatuses(
	offering: DeliveryVersion['snapshot']['offerings'][number],
	workspace: HomeroomDeliveryWorkspace | null = null
): DeliveryReviewStatus[] {
	const evidence: StatusEvidence = new Map();
	const matching =
		workspace?.homerooms.flatMap((room) =>
			room.items.filter((item) => item.offeringId === offering.id)
		) ?? [];
	for (const item of matching) {
		if (item.groupMode === 'missing') addStatus(evidence, 'groups_missing', offering.id);
		if (item.groupMode === 'deferred' && item.groups.length === 0)
			addStatus(evidence, 'groups_deferred', offering.id);
		if (item.groupMode === 'central') addStatus(evidence, 'central_activity', offering.id);
		if (item.weeklyPeriodTarget === null) addStatus(evidence, 'missing_period_target', offering.id);
	}
	if (offering.weeklyPeriodTarget === 0) addStatus(evidence, 'zero_weekly_periods', offering.id);
	for (const group of offering.groups) {
		addStatus(evidence, 'groups_present', group.id);
		addStatus(
			evidence,
			group.teachers.some((teacher) => teacher.role === 'primary')
				? 'primary_assigned'
				: 'primary_missing',
			group.id
		);
	}
	if (offering.groups.length === 0) {
		const central =
			offering.catalog.kind === 'activity' && offering.catalog.schedulingMode === 'synchronized';
		if (central) addStatus(evidence, 'central_activity', offering.id);
		if (
			central ||
			matching.some((item) => item.groupMode === 'deferred' || item.teacherState === 'deferred')
		)
			addStatus(evidence, 'teachers_deferred', offering.id);
		if (
			!central &&
			!matching.some((item) => ['central', 'missing', 'deferred'].includes(item.groupMode))
		)
			addStatus(evidence, 'groups_absent', offering.id);
	}
	return statusBadges(evidence);
}

function normalize(value: string): string {
	return value.trim().toLocaleLowerCase('th-TH');
}
function needsAttention(room: DeliveryRoomShape): boolean {
	return deliveryRoomStatus(room).needsAttention;
}

export function filterHomeroomDeliveryRooms<T extends DeliveryRoomShape>(
	rooms: readonly T[],
	search: string,
	readiness: HomeroomReadinessFilter
): T[] {
	const needle = normalize(search);
	return rooms.filter((room) => {
		const matchesReadiness =
			readiness === 'all' ||
			(readiness === 'ready' && !needsAttention(room)) ||
			(readiness === 'attention' && needsAttention(room));
		if (!matchesReadiness) return false;
		if (!needle) return true;
		return normalize(
			[
				room.homeroom.name,
				room.gradeLevel.name,
				room.studyProgram.code,
				room.studyProgram.name,
				room.studyProgram.levelName
			].join(' ')
		).includes(needle);
	});
}

export function summarizeHomeroomDelivery(rooms: readonly DeliveryRoomShape[]) {
	return rooms.reduce(
		(summary, room) => ({
			roomCount: summary.roomCount + 1,
			expectedCount: summary.expectedCount + room.expectedCount,
			readyCount: summary.readyCount + room.readyCount,
			attentionRoomCount: summary.attentionRoomCount + (needsAttention(room) ? 1 : 0)
		}),
		{ roomCount: 0, expectedCount: 0, readyCount: 0, attentionRoomCount: 0 }
	);
}

export const deliveryAlignmentLabels: Record<CurriculumDeliveryAlignmentState, string> = {
	matches_curriculum: statusDefinitions.matches_curriculum.label,
	curriculum_requirement_not_offered: statusDefinitions.curriculum_requirement_not_offered.label,
	extra_offering: statusDefinitions.extra_offering.label,
	ended_early: statusDefinitions.ended_early.label,
	operational_periods_differ: statusDefinitions.operational_periods_differ.label
};

/** Presentation-only classification; the full comparison keeps normal evidence too. */
export function deliveryAlignmentNeedsReview(state: CurriculumDeliveryAlignmentState): boolean {
	return statusDefinitions[state].review;
}

export function deliveryAlignmentBadgeClass(state: CurriculumDeliveryAlignmentState): string {
	if (state === 'matches_curriculum') return 'border-success/35 text-success';
	if (state === 'extra_offering' || state === 'operational_periods_differ')
		return 'border-primary/25 text-primary';
	return 'border-warning/40 text-warning';
}

export type DeliveryAlignmentRoom = {
	id: string;
	name: string;
	studyProgramId: string;
	studyProgramName: string;
};
export type DeliveryOfferingAlignment = {
	offeringId: string;
	resourceKind: HomeroomDeliveryItem['resourceKind'];
	code: string;
	name: string;
	states: { state: CurriculumDeliveryAlignmentState; rooms: DeliveryAlignmentRoom[] }[];
	rooms: DeliveryAlignmentRoom[];
	unlinkedReasons: { groupId: string | null; reason: string }[];
};
export type MissingDeliveryRequirement = Pick<
	HomeroomDeliveryItem,
	'resourceKind' | 'catalogVersionId' | 'code' | 'name' | 'schedulingMode'
> & {
	key: string;
	requirementIds: string[];
	rooms: DeliveryAlignmentRoom[];
};
export type DeliveryAlignmentFacts = {
	offerings: Map<string, DeliveryOfferingAlignment>;
	missingRequirements: MissingDeliveryRequirement[];
	prerequisites: { room: DeliveryAlignmentRoom; blockers: HomeroomDeliveryRoom['blockers'] }[];
	missingCount: number;
	extraCount: number;
	unlinkedCount: number;
	affectedExistingCount: number;
};

/** Present the workspace provider's comparisons; identities, names and equivalence remain provider-owned. */
export function buildDeliveryAlignmentFacts(
	workspace: HomeroomDeliveryWorkspace
): DeliveryAlignmentFacts {
	const offerings = new Map<string, DeliveryOfferingAlignment>();
	const missing = new Map<string, MissingDeliveryRequirement>();
	const prerequisites: DeliveryAlignmentFacts['prerequisites'] = [];
	function offering(
		id: string,
		source: Pick<DeliveryOfferingAlignment, 'resourceKind' | 'code' | 'name'>
	) {
		let fact = offerings.get(id);
		if (!fact) {
			fact = { offeringId: id, ...source, states: [], rooms: [], unlinkedReasons: [] };
			offerings.set(id, fact);
		}
		return fact;
	}
	function addRoom(rooms: DeliveryAlignmentRoom[], room: DeliveryAlignmentRoom) {
		if (!rooms.some((item) => item.id === room.id)) rooms.push(room);
	}
	for (const source of workspace.homerooms) {
		const room: DeliveryAlignmentRoom = {
			id: source.homeroom.id,
			name: source.homeroom.name,
			studyProgramId: source.studyProgram.id,
			studyProgramName: source.studyProgram.name
		};
		if (source.blockers.length) prerequisites.push({ room, blockers: source.blockers });
		for (const item of [...source.items, ...source.extraOfferings]) {
			const id = 'offeringId' in item ? item.offeringId : null;
			if (id) {
				const fact = offering(id, item);
				addRoom(fact.rooms, room);
				for (const state of item.alignmentStates) {
					let entry = fact.states.find((entry) => entry.state === state);
					if (!entry) {
						entry = { state, rooms: [] };
						fact.states.push(entry);
					}
					addRoom(entry.rooms, room);
				}
			}
			if (
				'requirementId' in item &&
				item.alignmentStates.includes('curriculum_requirement_not_offered')
			) {
				const key = `${item.resourceKind}:${item.catalogVersionId}`;
				let fact = missing.get(key);
				if (!fact) {
					fact = {
						key,
						resourceKind: item.resourceKind,
						catalogVersionId: item.catalogVersionId,
						code: item.code,
						name: item.name,
						schedulingMode: item.schedulingMode,
						requirementIds: [],
						rooms: []
					};
					missing.set(key, fact);
				}
				if (!fact.requirementIds.includes(item.requirementId))
					fact.requirementIds.push(item.requirementId);
				addRoom(fact.rooms, room);
			}
		}
	}
	for (const source of workspace.unlinked) {
		const fact = offering(source.offeringId, source);
		const groupId = source.groupId ?? null;
		if (
			!fact.unlinkedReasons.some(
				(item) => item.groupId === groupId && item.reason === source.reason
			)
		)
			fact.unlinkedReasons.push({ groupId, reason: source.reason });
	}
	return {
		offerings,
		missingRequirements: [...missing.values()],
		prerequisites,
		missingCount: missing.size,
		affectedExistingCount: [...offerings.values()].filter(
			(item) =>
				item.unlinkedReasons.length > 0 ||
				item.states.some((entry) => entry.state !== 'matches_curriculum')
		).length,
		extraCount: [...offerings.values()].filter((item) =>
			item.states.some((entry) => entry.state === 'extra_offering')
		).length,
		unlinkedCount: [...offerings.values()].filter((item) => item.unlinkedReasons.length > 0).length
	};
}
