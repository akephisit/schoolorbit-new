import type {
	TimetableBlockWorkspaceHomeroom,
	TimetableBlockWorkspaceLearningGroup,
	TimetableOrdinaryDemand,
	TimetableSynchronizedDemand
} from '#lib/api/timetable.js';
import type { TimetableBoardView } from '#lib/academic/timetable/board-state.js';

const natural = new Intl.Collator('th', { numeric: true });
const gradeTypes = ['kindergarten', 'primary', 'secondary'];
const gradePrefixes: Record<string, string> = {
	kindergarten: 'อ.',
	primary: 'ป.',
	secondary: 'ม.'
};
const categoryOrder = ['BASIC', 'ADDITIONAL', 'OTHER', 'ACTIVITY'] as const;
export type TimetableDemandCategory = (typeof categoryOrder)[number];
export type TimetableTrayItem =
	| { kind: 'ordinary'; id: string; demand: TimetableOrdinaryDemand }
	| { kind: 'synchronized'; id: string; demand: TimetableSynchronizedDemand };

export interface TimetableTraySection {
	id: string;
	gradeLabel: string | null;
	categories: { category: TimetableDemandCategory; items: TimetableTrayItem[] }[];
}

function gradeRank(type: string): number {
	const rank = gradeTypes.indexOf(type);
	return rank < 0 ? gradeTypes.length : rank;
}

function compareGrades(
	a: TimetableBlockWorkspaceHomeroom,
	b: TimetableBlockWorkspaceHomeroom
): number {
	return (
		gradeRank(a.gradeLevelType) - gradeRank(b.gradeLevelType) ||
		natural.compare(a.gradeLevelType, b.gradeLevelType) ||
		a.gradeLevelYear - b.gradeLevelYear
	);
}

export function compareTimetableHomerooms(
	a: TimetableBlockWorkspaceHomeroom,
	b: TimetableBlockWorkspaceHomeroom
): number {
	return (
		compareGrades(a, b) ||
		Number(!a.roomNumber) - Number(!b.roomNumber) ||
		natural.compare(a.roomNumber ?? '', b.roomNumber ?? '') ||
		natural.compare(a.name, b.name) ||
		a.id.localeCompare(b.id)
	);
}

export function orderTimetableHomerooms(
	homerooms: readonly TimetableBlockWorkspaceHomeroom[]
): TimetableBlockWorkspaceHomeroom[] {
	return [...homerooms].sort(compareTimetableHomerooms);
}

export function relatedSynchronizedDemands(
	group: TimetableBlockWorkspaceLearningGroup | undefined,
	demands: readonly TimetableSynchronizedDemand[]
): TimetableSynchronizedDemand[] {
	const targets = new Set(group?.homeroomIds ?? []);
	return demands.filter((demand) => demand.intendedHomeroomIds.some((id) => targets.has(id)));
}

export function timetableDemandCategory(
	group: TimetableBlockWorkspaceLearningGroup | undefined
): TimetableDemandCategory {
	if (group?.offeringKind === 'activity' || group?.subjectType === 'ACTIVITY') return 'ACTIVITY';
	if (group?.subjectType === 'BASIC' || group?.subjectType === 'ADDITIONAL')
		return group.subjectType;
	return 'OTHER';
}

/** The earliest known target provides one sort key; multi-target demands remain one item. */
export function timetableTraySections(
	view: TimetableBoardView,
	ordinaryDemands: readonly TimetableOrdinaryDemand[],
	synchronizedDemands: readonly TimetableSynchronizedDemand[],
	groups: readonly TimetableBlockWorkspaceLearningGroup[],
	homerooms: readonly TimetableBlockWorkspaceHomeroom[]
): TimetableTraySection[] {
	const groupById = new Map(groups.map((group) => [group.id, group]));
	const roomById = new Map(homerooms.map((room) => [room.id, room]));
	const items: TimetableTrayItem[] = [
		...ordinaryDemands
			.filter((demand) => demand.remainingPeriods > 0)
			.map((demand) => ({
				kind: 'ordinary' as const,
				id: `group:${demand.learningGroupId}`,
				demand
			})),
		...synchronizedDemands
			.filter((demand) => demand.scheduledPeriods < demand.requiredPeriods)
			.map((demand) => ({
				kind: 'synchronized' as const,
				id: `offering:${demand.learningOfferingId}`,
				demand
			}))
	];
	const category = (item: TimetableTrayItem) =>
		item.kind === 'synchronized'
			? 'ACTIVITY'
			: timetableDemandCategory(groupById.get(item.demand.learningGroupId));
	const primaryTargets = new Map(
		items.map((item) => {
			const ids =
				item.kind === 'ordinary' ? item.demand.homeroomIds : item.demand.intendedHomeroomIds;
			const targets = ids.flatMap((id) => {
				const room = roomById.get(id);
				return room ? [room] : [];
			});
			return [item.id, orderTimetableHomerooms(targets)[0]];
		})
	);
	items.sort((a, b) => {
		const roomA = primaryTargets.get(a.id),
			roomB = primaryTargets.get(b.id);
		const grade = roomA && roomB ? compareGrades(roomA, roomB) : Number(!roomA) - Number(!roomB);
		return (
			(view === 'teacher' ? grade : 0) ||
			categoryOrder.indexOf(category(a)) - categoryOrder.indexOf(category(b)) ||
			natural.compare(a.demand.offeringCode, b.demand.offeringCode) ||
			(view === 'teacher' && roomA && roomB ? compareTimetableHomerooms(roomA, roomB) : 0) ||
			natural.compare(a.demand.offeringName, b.demand.offeringName) ||
			a.id.localeCompare(b.id)
		);
	});
	const sections = new Map<string, TimetableTraySection>();
	for (const item of items) {
		const room = primaryTargets.get(item.id);
		const id =
			view !== 'teacher'
				? 'context'
				: room
					? `${room.gradeLevelType}:${room.gradeLevelYear}`
					: 'missing-grade';
		let section = sections.get(id);
		if (!section) {
			section = {
				id,
				gradeLabel:
					view !== 'teacher'
						? null
						: room
							? `${gradePrefixes[room.gradeLevelType] ?? room.gradeLevelType}${room.gradeLevelYear}`
							: 'ไม่ระบุระดับชั้น',
				categories: []
			};
			sections.set(id, section);
		}
		const itemCategory = category(item);
		let categorySection = section.categories.find((entry) => entry.category === itemCategory);
		if (!categorySection) {
			categorySection = { category: itemCategory, items: [] };
			section.categories.push(categorySection);
		}
		categorySection.items.push(item);
	}
	return [...sections.values()];
}
