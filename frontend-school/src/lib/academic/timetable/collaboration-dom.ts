import type { TimetableBlockPlacementSource } from '#lib/api/timetable.js';

export function findTimetableBlockElement(
	root: HTMLElement,
	blockId: string,
	ownerId?: string | null
): HTMLElement | null {
	const matches = [...root.querySelectorAll<HTMLElement>('[data-timetable-anchor="block"]')].filter(
		(element) =>
			element.dataset.blockId === blockId && (!ownerId || element.dataset.rowId === ownerId)
	);
	return matches.length === 1 ? matches[0] : null;
}

export function findTimetableCellElement(
	root: HTMLElement,
	dayOfWeek: string,
	periodId: string,
	ownerId?: string | null
): HTMLElement | null {
	return (
		[...root.querySelectorAll<HTMLElement>('[data-timetable-anchor="cell"]')].find(
			(element) =>
				element.dataset.timetableDay === dayOfWeek &&
				element.dataset.timetablePeriodId === periodId &&
				(!ownerId || element.dataset.timetableOwnerId === ownerId)
		) ?? null
	);
}

export function findTimetableDemandElement(
	root: HTMLElement,
	source: TimetableBlockPlacementSource
): HTMLElement | null {
	if (source.kind === 'existing_block') return findTimetableBlockElement(root, source.blockId);
	return (
		[...root.querySelectorAll<HTMLElement>('[data-timetable-anchor="tray"]')].find(
			(element) =>
				element.dataset.timetableSourceKind === source.kind &&
				element.dataset.timetableOfferingId === source.learningOfferingId &&
				(source.kind !== 'ordinary_demand' ||
					element.dataset.timetableGroupId === source.learningGroupId)
		) ?? null
	);
}

export function pointerTimetableSubject(
	root: HTMLElement,
	clientX: number,
	clientY: number
): HTMLElement | null {
	const element = root.ownerDocument
		.elementFromPoint(clientX, clientY)
		?.closest<HTMLElement>('[data-timetable-anchor]');
	return element && root.contains(element) ? element : null;
}
