import type { DailyTeachingEntry } from '$lib/api/timetable';

export const DAILY_TEACHING_TEACHER_COLUMN_WIDTH = 128;
export const DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH = 132;

type DailyTeachingTeacherIdentity = {
	displayName: string;
};

export type DailyTeachingEntryTone = 'course' | 'activity' | 'break';
export type DailyTeachingEntryLayout = 'details' | 'centered';
export type DailyTeachingTitleLineLimit = 2 | 3;

export type DailyTeachingEntryCardPresentation = {
	tone: DailyTeachingEntryTone;
	layout: DailyTeachingEntryLayout;
	titleLineLimit: DailyTeachingTitleLineLimit;
};

export interface DailyTeachingDisplayLocation {
	key: string;
	homeroomNames: string[];
	roomCode: string | null;
	label: string;
}

export interface DailyTeachingDisplayGroup {
	key: string;
	entries: DailyTeachingEntry[];
	isSynchronizedActivity: boolean;
	locations: DailyTeachingDisplayLocation[];
	classroomLabels: string[];
}

const thaiNaturalCollator = new Intl.Collator('th', {
	numeric: true,
	sensitivity: 'base'
});

function synchronizedActivityKey(entry: DailyTeachingEntry): string | null {
	if (
		entry.entryType !== 'ACTIVITY' ||
		entry.activitySchedulingMode !== 'synchronized' ||
		!entry.offeringId
	) {
		return null;
	}

	return `activity:${entry.offeringId}`;
}

function textOrNull(value: string | null | undefined): string | null {
	const normalized = value?.trim();
	return normalized ? normalized : null;
}

function locationFromEntry(entry: DailyTeachingEntry): DailyTeachingDisplayLocation | null {
	const homeroomNames = entry.homeroomNames.map(textOrNull).filter((name) => name !== null);
	const roomCode = textOrNull(entry.roomCode);
	if (homeroomNames.length === 0 && !roomCode) return null;
	const homeroomLabel = homeroomNames.join(', ');

	return {
		key: `${homeroomLabel}\u0000${roomCode ?? ''}`,
		homeroomNames,
		roomCode,
		label: [homeroomLabel, roomCode].filter(Boolean).join(' / ')
	};
}

function compareLocations(
	left: DailyTeachingDisplayLocation,
	right: DailyTeachingDisplayLocation
): number {
	return (
		Number(left.homeroomNames.length === 0) - Number(right.homeroomNames.length === 0) ||
		thaiNaturalCollator.compare(left.homeroomNames.join(', '), right.homeroomNames.join(', ')) ||
		thaiNaturalCollator.compare(left.roomCode ?? '', right.roomCode ?? '') ||
		left.key.localeCompare(right.key)
	);
}

function appendLocation(group: DailyTeachingDisplayGroup, entry: DailyTeachingEntry) {
	const location = locationFromEntry(entry);
	if (!location || group.locations.some((item) => item.key === location.key)) return;

	group.locations.push(location);
	group.locations.sort(compareLocations);
	group.classroomLabels = group.locations.map((item) => item.label);
}

function displayGroup(
	key: string,
	entry: DailyTeachingEntry,
	isSynchronizedActivity: boolean
): DailyTeachingDisplayGroup {
	const group: DailyTeachingDisplayGroup = {
		key,
		entries: [entry],
		isSynchronizedActivity,
		locations: [],
		classroomLabels: []
	};
	appendLocation(group, entry);
	return group;
}

export function groupDailyTeachingEntries(
	entries: DailyTeachingEntry[]
): DailyTeachingDisplayGroup[] {
	const groups: DailyTeachingDisplayGroup[] = [];
	const synchronizedGroups = new Map<string, DailyTeachingDisplayGroup>();

	for (const entry of entries) {
		const synchronizedKey = synchronizedActivityKey(entry);
		if (!synchronizedKey) {
			groups.push(displayGroup(`entry:${entry.entryId}`, entry, false));
			continue;
		}

		const existingGroup = synchronizedGroups.get(synchronizedKey);
		if (existingGroup) {
			existingGroup.entries.push(entry);
			appendLocation(existingGroup, entry);
			continue;
		}

		const group = displayGroup(synchronizedKey, entry, true);
		synchronizedGroups.set(synchronizedKey, group);
		groups.push(group);
	}

	return groups;
}

export function displayGroupCountLabel(group: DailyTeachingDisplayGroup): string {
	if (group.classroomLabels.length > 0) {
		return `${group.classroomLabels.length} ห้อง`;
	}
	if (group.entries.length > 1) {
		return `${group.entries.length} รายการ`;
	}
	return '';
}

export function dailyTeachingTableMinWidth(periodCount: number): number {
	return DAILY_TEACHING_TEACHER_COLUMN_WIDTH + periodCount * DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH;
}

export function dailyTeachingTeacherCell(teacher: DailyTeachingTeacherIdentity): {
	label: string;
	title: string;
} {
	return {
		label: teacher.displayName,
		title: teacher.displayName
	};
}

export function dailyTeachingEntryCardPresentation(
	entry: Pick<DailyTeachingEntry, 'entryType' | 'activitySchedulingMode'>
): DailyTeachingEntryCardPresentation {
	if (entry.entryType === 'COURSE') {
		return { tone: 'course', layout: 'details', titleLineLimit: 2 };
	}
	switch (entry.entryType) {
		case 'BREAK':
			return { tone: 'break', layout: 'centered', titleLineLimit: 3 };
		case 'ACTIVITY':
			return entry.activitySchedulingMode === 'independent'
				? { tone: 'activity', layout: 'details', titleLineLimit: 2 }
				: { tone: 'activity', layout: 'centered', titleLineLimit: 3 };
		case 'HOMEROOM':
			return { tone: 'activity', layout: 'centered', titleLineLimit: 3 };
		default:
			return { tone: 'course', layout: 'centered', titleLineLimit: 3 };
	}
}

export function dailyTeachingEmptyCellLabel(
	teacherName: string,
	periodName: string,
	periodTime: string
): string {
	return `${teacherName} ${periodName} ${periodTime}: ว่าง`;
}
