import type { DailyTeachingEntry } from '$lib/api/timetable';

export const DAILY_TEACHING_TEACHER_COLUMN_WIDTH = 128;
export const DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH = 132;

type DailyTeachingTeacherIdentity = {
	displayName: string;
};

export type DailyTeachingEntryTone = 'course' | 'activity' | 'break';
export type DailyTeachingEntryLayout = 'details' | 'centered';

export type DailyTeachingEntryCardPresentation = {
	tone: DailyTeachingEntryTone;
	layout: DailyTeachingEntryLayout;
};

export interface DailyTeachingDisplayGroup {
	key: string;
	entries: DailyTeachingEntry[];
	isSynchronizedActivity: boolean;
	classroomLabels: string[];
}

function synchronizedActivityKey(entry: DailyTeachingEntry): string | null {
	if (
		entry.entryType !== 'ACTIVITY' ||
		entry.activitySchedulingMode !== 'synchronized' ||
		!entry.activitySlotId
	) {
		return null;
	}

	return `synchronized:${entry.activitySlotId}`;
}

function classroomLabel(entry: DailyTeachingEntry): string {
	return [entry.classroomName, entry.roomCode].filter(Boolean).join(' / ');
}

function appendClassroomLabel(group: DailyTeachingDisplayGroup, entry: DailyTeachingEntry) {
	const label = classroomLabel(entry);
	if (label && !group.classroomLabels.includes(label)) {
		group.classroomLabels.push(label);
	}
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
		classroomLabels: []
	};
	appendClassroomLabel(group, entry);
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
			appendClassroomLabel(existingGroup, entry);
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
	entryType: DailyTeachingEntry['entryType']
): DailyTeachingEntryCardPresentation {
	switch (entryType) {
		case 'COURSE':
			return { tone: 'course', layout: 'details' };
		case 'BREAK':
			return { tone: 'break', layout: 'centered' };
		case 'ACTIVITY':
		case 'HOMEROOM':
			return { tone: 'activity', layout: 'centered' };
		default:
			return { tone: 'course', layout: 'centered' };
	}
}

export function dailyTeachingEmptyCellLabel(
	teacherName: string,
	periodName: string,
	periodTime: string
): string {
	return `${teacherName} ${periodName} ${periodTime}: ว่าง`;
}
