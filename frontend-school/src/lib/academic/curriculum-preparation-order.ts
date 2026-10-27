import type {
	CurriculumGroupProposal,
	DeliveryManagementOptions
} from '#lib/api/learning-delivery.js';

/** Presentation only: keep UUID ordering in preview identities and submitted choices intact. */
export function curriculumPreparationOrder(
	options: Pick<DeliveryManagementOptions, 'homerooms' | 'gradeLevels'>
) {
	const rooms = new Map(options.homerooms.map((room) => [room.id, room]));
	const grades = new Map(options.gradeLevels.map((grade) => [grade.id, grade.level_order]));
	const compareNames = (left: string, right: string) =>
		left.localeCompare(right, 'th-TH', { numeric: true });

	function compareRooms(leftId: string, rightId: string): number {
		const left = rooms.get(leftId);
		const right = rooms.get(rightId);
		if (!left || !right) {
			if (left) return -1;
			if (right) return 1;
			return compareNames(leftId, rightId);
		}
		const leftGrade = grades.get(left.gradeLevelId ?? '') ?? Infinity;
		const rightGrade = grades.get(right.gradeLevelId ?? '') ?? Infinity;
		if (leftGrade !== rightGrade) return leftGrade < rightGrade ? -1 : 1;
		return compareNames(left.name, right.name) || compareNames(leftId, rightId);
	}

	function homeroomIds(ids: readonly string[]): string[] {
		return [...ids].sort(compareRooms);
	}

	function groups(items: readonly CurriculumGroupProposal[]): CurriculumGroupProposal[] {
		const anchored = items.map((group) => ({ group, anchor: homeroomIds(group.homeroomIds)[0] }));
		return anchored
			.sort((left, right) => {
				if (left.anchor === undefined) return right.anchor === undefined ? 0 : 1;
				if (right.anchor === undefined) return -1;
				// Stable sorting preserves the user's group sequence for a shared room anchor.
				return compareRooms(left.anchor, right.anchor);
			})
			.map(({ group }) => group);
	}

	return { homeroomIds, groups };
}
