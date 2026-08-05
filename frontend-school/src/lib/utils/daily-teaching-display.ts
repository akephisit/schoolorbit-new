import type { DailyTeachingEntry } from '$lib/api/timetable';

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
