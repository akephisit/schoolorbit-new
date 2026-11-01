import type { TimetableBlockWorkspace } from '#lib/api/timetable.js';

export function initialTimetablePublicationDate(
	workspace: Pick<TimetableBlockWorkspace, 'publicationDateRules' | 'latestDeliveryPublication'>
): string {
	const { today, minimumEffectiveFrom } = workspace.publicationDateRules;
	const latestStart = workspace.latestDeliveryPublication.effectiveFrom;
	return [today, latestStart, minimumEffectiveFrom ?? today].toSorted().at(-1) ?? today;
}

export function timetableSourceIsCurrent(
	workspace: Pick<TimetableBlockWorkspace, 'version' | 'latestDeliveryVersionId'>
): boolean {
	return (
		workspace.version.status !== 'draft' ||
		workspace.version.deliveryVersionId === workspace.latestDeliveryVersionId
	);
}
