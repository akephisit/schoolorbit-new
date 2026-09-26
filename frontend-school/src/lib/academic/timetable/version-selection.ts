import type { TimetableVersion } from '$lib/api/timetable';

export function selectPreferredBoardVersion(
	versions: TimetableVersion[],
	requestedId: string | null
): TimetableVersion | null {
	return (
		versions.find((version) => version.id === requestedId) ??
		versions.find((version) => version.status === 'draft') ??
		versions.find((version) => version.displayState === 'current') ??
		versions[0] ??
		null
	);
}

export function selectPreferredTemplateVersion(
	versions: TimetableVersion[],
	requestedId: string | null
): TimetableVersion | null {
	return (
		versions.find((version) => version.id === requestedId) ??
		versions.find(
			(version) => version.status === 'published' && version.displayState === 'current'
		) ??
		versions.find(
			(version) => version.status === 'published' && version.displayState === 'upcoming'
		) ??
		versions.find((version) => version.status === 'draft') ??
		null
	);
}
