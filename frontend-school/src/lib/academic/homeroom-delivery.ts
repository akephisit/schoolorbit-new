import type {
	HomeroomDeliveryItem,
	HomeroomDeliveryWorkspace,
	HomeroomDeliveryRoom,
	CurriculumDeliveryAlignmentState
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

type DeliveryRoomShape = {
	homeroom: { name: string };
	gradeLevel: { name: string };
	studyProgram: { code: string; name: string; levelName: string };
	expectedCount: number;
	readyCount: number;
	items?: ReadonlyArray<{ alignmentStates: readonly string[] }>;
	extraOfferings?: readonly unknown[];
};

function normalize(value: string): string {
	return value.trim().toLocaleLowerCase('th-TH');
}

function needsAttention(room: DeliveryRoomShape): boolean {
	return (
		room.expectedCount === 0 ||
		room.readyCount < room.expectedCount ||
		(room.extraOfferings?.length ?? 0) > 0 ||
		(room.items?.some((item) => !item.alignmentStates.includes('matches_curriculum')) ?? false)
	);
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
	matches_curriculum: 'ตรงกับหลักสูตร',
	curriculum_requirement_not_offered: 'หลักสูตรกำหนดไว้แต่ยังไม่เปิดสอน',
	extra_offering: 'เปิดสอนเพิ่มเติมนอกหลักสูตร',
	ended_early: 'หยุดสอนก่อนรุ่นเปิดสอนนี้มีผล',
	operational_periods_differ: 'คาบจริงต่างจากค่ามาตรฐานในหลักสูตร'
};

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
