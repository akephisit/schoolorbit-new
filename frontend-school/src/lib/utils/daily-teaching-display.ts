import type { DailyTeachingEntry, DailyTeachingTeacher } from '#lib/api/timetable.js';

export const DAILY_TEACHING_TEACHER_COLUMN_WIDTH = 104;
export const DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH = 84;

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
	roomName: string | null;
	label: string;
}

export interface DailyTeachingDisplayGroup {
	key: string;
	entries: DailyTeachingEntry[];
	isSynchronizedActivity: boolean;
	locations: DailyTeachingDisplayLocation[];
	classroomLabels: string[];
}

export type DailyTeachingView = 'teacher' | 'homeroom';
export type DailyTeachingInstructor = Pick<DailyTeachingTeacher, 'id' | 'displayName'>;

export interface DailyTeachingDisplayCell {
	bellSchedulePeriodId: string;
	entries: DailyTeachingEntry[];
	instructorsByEntry: Map<string, DailyTeachingInstructor[]>;
}

export interface DailyTeachingDisplayRow {
	id: string;
	displayName: string;
	kind: DailyTeachingView;
	periods: DailyTeachingDisplayCell[];
}

const thaiNaturalCollator = new Intl.Collator('th', {
	numeric: true,
	sensitivity: 'base'
});

/** Project the already-authorized daily response without additional resource reads. */
export function dailyTeachingRows(
	teachers: DailyTeachingTeacher[],
	view: DailyTeachingView
): DailyTeachingDisplayRow[] {
	const instructorsByPeriod = new Map<string, Map<string, DailyTeachingInstructor[]>>();
	for (const teacher of teachers) {
		for (const cell of teacher.periods) {
			let instructors = instructorsByPeriod.get(cell.bellSchedulePeriodId);
			if (!instructors) {
				instructors = new Map();
				instructorsByPeriod.set(cell.bellSchedulePeriodId, instructors);
			}
			for (const entry of cell.entries) {
				const assigned = instructors.get(entry.entryId) ?? [];
				if (!assigned.some((item) => item.id === teacher.id)) {
					assigned.push({ id: teacher.id, displayName: teacher.displayName });
				}
				instructors.set(entry.entryId, assigned);
			}
		}
	}
	const displayCell = (
		periodId: string,
		entries: DailyTeachingEntry[]
	): DailyTeachingDisplayCell => ({
		bellSchedulePeriodId: periodId,
		entries,
		instructorsByEntry: instructorsByPeriod.get(periodId) ?? new Map()
	});
	if (view === 'teacher') {
		return teachers.map((teacher) => ({
			id: teacher.id,
			displayName: teacher.displayName,
			kind: view,
			periods: teacher.periods.map((cell) => displayCell(cell.bellSchedulePeriodId, cell.entries))
		}));
	}
	const homerooms = new Map<string, Map<string, Map<string, DailyTeachingEntry>>>();
	for (const teacher of teachers) {
		for (const cell of teacher.periods) {
			for (const entry of cell.entries) {
				for (const name of new Set(
					entry.homeroomNames.map(textOrNull).filter((name) => name !== null)
				)) {
					let periods = homerooms.get(name);
					if (!periods) {
						periods = new Map();
						homerooms.set(name, periods);
					}
					let entries = periods.get(cell.bellSchedulePeriodId);
					if (!entries) {
						entries = new Map();
						periods.set(cell.bellSchedulePeriodId, entries);
					}
					entries.set(entry.entryId, entry);
				}
			}
		}
	}
	return [...homerooms]
		.sort(([left], [right]) => thaiNaturalCollator.compare(left, right))
		.map(([name, periods]) => ({
			id: name,
			displayName: name,
			kind: view,
			periods: [...periods].map(([periodId, entries]) =>
				displayCell(periodId, [...entries.values()])
			)
		}));
}

export function dailyTeachingGroupInstructors(
	entries: DailyTeachingEntry[],
	cell: DailyTeachingDisplayCell
): DailyTeachingInstructor[] {
	const instructors = new Map<string, DailyTeachingInstructor>();
	for (const entry of entries) {
		for (const instructor of cell.instructorsByEntry.get(entry.entryId) ?? []) {
			instructors.set(instructor.id, instructor);
		}
	}
	return [...instructors.values()].sort(
		(left, right) =>
			thaiNaturalCollator.compare(left.displayName, right.displayName) ||
			left.id.localeCompare(right.id)
	);
}

export function filterDailyTeachingRows(
	rows: DailyTeachingDisplayRow[],
	ownerId: string,
	search: string
): DailyTeachingDisplayRow[] {
	const query = search.trim().toLocaleLowerCase('th');
	return rows.filter((row) => {
		if (ownerId !== 'all' && row.id !== ownerId) return false;
		if (!query || row.displayName.toLocaleLowerCase('th').includes(query)) return true;
		return row.periods.some((cell) =>
			cell.entries.some((entry) =>
				[
					entry.title,
					entry.offeringCode,
					entry.offeringName,
					entry.learningGroupName,
					entry.subjectVersionDisplayLabel,
					entry.activityVersionDisplayLabel,
					entry.roomName,
					...entry.homeroomNames,
					...(cell.instructorsByEntry.get(entry.entryId) ?? []).map((item) => item.displayName)
				].some((value) => value?.toLocaleLowerCase('th').includes(query))
			)
		);
	});
}

export function dailyTeachingHomeroomGroupMeta(
	group: DailyTeachingDisplayGroup,
	cell: DailyTeachingDisplayCell
): string {
	const instructors = dailyTeachingGroupInstructors(group.entries, cell);
	const teacherLabel = instructors.map((item) => item.displayName).join(', ');
	const rooms = [...new Set(group.locations.map((location) => location.roomName).filter(Boolean))];
	return [teacherLabel, rooms.join(', ')].filter(Boolean).join(' · ');
}

function synchronizedActivityKey(entry: DailyTeachingEntry): string | null {
	if (
		normalizedEntryType(entry.entryType) !== 'activity' ||
		entry.activitySchedulingMode !== 'synchronized' ||
		!entry.offeringId
	) {
		return null;
	}

	return `activity:${entry.offeringId}`;
}

function normalizedEntryType(entryType: DailyTeachingEntry['entryType']): string {
	return entryType.trim().toLowerCase();
}

function textOrNull(value: string | null | undefined): string | null {
	const normalized = value?.trim();
	return normalized ? normalized : null;
}

function locationFromEntry(entry: DailyTeachingEntry): DailyTeachingDisplayLocation | null {
	const homeroomNames = entry.homeroomNames.map(textOrNull).filter((name) => name !== null);
	const roomName = textOrNull(entry.roomName) ?? (entry.roomId ? 'ไม่พบชื่อห้อง' : null);
	if (homeroomNames.length === 0 && !roomName) return null;
	const homeroomLabel = homeroomNames.join(', ');

	return {
		key: `${homeroomLabel}\u0000${entry.roomId ?? ''}`,
		homeroomNames,
		roomName,
		label: [homeroomLabel, roomName].filter(Boolean).join(' / ')
	};
}

function compareLocations(
	left: DailyTeachingDisplayLocation,
	right: DailyTeachingDisplayLocation
): number {
	return (
		Number(left.homeroomNames.length === 0) - Number(right.homeroomNames.length === 0) ||
		thaiNaturalCollator.compare(left.homeroomNames.join(', '), right.homeroomNames.join(', ')) ||
		thaiNaturalCollator.compare(left.roomName ?? '', right.roomName ?? '') ||
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

export function dailyTeachingCompactTitle(entry: DailyTeachingEntry): string {
	if (normalizedEntryType(entry.entryType) === 'course') {
		return (
			textOrNull(entry.offeringCode) ??
			textOrNull(entry.offeringName) ??
			textOrNull(entry.subjectVersionDisplayLabel) ??
			textOrNull(entry.title) ??
			entry.entryType
		);
	}

	return (
		textOrNull(entry.title) ??
		textOrNull(entry.offeringName) ??
		textOrNull(entry.activityVersionDisplayLabel) ??
		textOrNull(entry.subjectVersionDisplayLabel) ??
		entry.entryType
	);
}

export function dailyTeachingCompactGroupMeta(group: DailyTeachingDisplayGroup): string {
	const entry = group.entries[0];
	if (!entry) return '';

	const homeroomNames = Array.from(
		new Set(
			group.entries.flatMap((item) =>
				item.homeroomNames.map(textOrNull).filter((name) => name !== null)
			)
		)
	);
	if (
		group.isSynchronizedActivity ||
		(normalizedEntryType(entry.entryType) !== 'course' && homeroomNames.length > 1)
	) {
		if (homeroomNames.length > 0) return `${homeroomNames.length} ห้อง`;
		return displayGroupCountLabel(group);
	}

	const targetLabel =
		homeroomNames.length > 0
			? `${homeroomNames[0]}${homeroomNames.length > 1 ? ` +${homeroomNames.length - 1}` : ''}`
			: textOrNull(entry.learningGroupName);
	const roomLabel = textOrNull(entry.roomName) ?? (entry.roomId ? 'ไม่พบชื่อห้อง' : null);
	return [targetLabel, roomLabel].filter(Boolean).join(' · ');
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
	const entryType = normalizedEntryType(entry.entryType);
	if (entryType === 'course') {
		return { tone: 'course', layout: 'details', titleLineLimit: 2 };
	}
	switch (entryType) {
		case 'break':
			return { tone: 'break', layout: 'centered', titleLineLimit: 3 };
		case 'activity':
			return entry.activitySchedulingMode === 'independent'
				? { tone: 'activity', layout: 'details', titleLineLimit: 2 }
				: { tone: 'activity', layout: 'centered', titleLineLimit: 3 };
		case 'homeroom':
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
