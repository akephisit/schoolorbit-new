import type { DailyTeachingEntry, DailyTeachingTeacher } from '#lib/api/timetable.js';
import { timetableTeacherLabel } from '../academic/timetable/teacher-label.ts';

export const DAILY_TEACHING_TEACHER_COLUMN_WIDTH = 104;
export const DAILY_TEACHING_MIN_PERIOD_COLUMN_WIDTH = 84;

type DailyTeachingTeacherIdentity = {
	displayName: string;
};

export interface DailyTeachingCardPresentation {
	code: string | null;
	title: string;
	titleLineLimit: 1 | 3;
	targetLabel: string | null;
	teacherLabel: string | null;
	teacherTitle: string | null;
	roomLabel: string | null;
}

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

function isStructuralEntry(entry: DailyTeachingEntry): boolean {
	return ['structural', 'break', 'homeroom'].includes(normalizedEntryType(entry.entryType));
}

function structuralPresentationKey(entry: DailyTeachingEntry): string | null {
	if (!isStructuralEntry(entry)) return null;
	// The daily contract has allocation identities, not a source block identity. This groups
	// visually equivalent allocations only; every source entry remains available in details.
	return `structural:${JSON.stringify([
		normalizedEntryType(entry.entryType),
		textOrNull(entry.title),
		entry.offeringId ?? null,
		textOrNull(entry.offeringCode),
		textOrNull(entry.offeringName),
		entry.learningGroupId ?? null,
		textOrNull(entry.learningGroupName),
		entry.activityId ?? null,
		entry.subjectId ?? null,
		textOrNull(entry.activityVersionDisplayLabel),
		textOrNull(entry.subjectVersionDisplayLabel),
		entry.roomId,
		textOrNull(entry.roomName),
		textOrNull(entry.note)
	])}`;
}

export function groupDailyTeachingEntries(
	entries: DailyTeachingEntry[],
	view: DailyTeachingView = 'teacher'
): DailyTeachingDisplayGroup[] {
	const groups: DailyTeachingDisplayGroup[] = [];
	const sharedGroups = new Map<string, DailyTeachingDisplayGroup>();

	for (const entry of entries) {
		const synchronizedKey = synchronizedActivityKey(entry);
		const presentationKey =
			synchronizedKey ?? (view === 'homeroom' ? structuralPresentationKey(entry) : null);
		if (!presentationKey) {
			groups.push(displayGroup(`entry:${entry.entryId}`, entry, false));
			continue;
		}

		const existingGroup = sharedGroups.get(presentationKey);
		if (existingGroup) {
			existingGroup.entries.push(entry);
			appendLocation(existingGroup, entry);
			continue;
		}

		const group = displayGroup(presentationKey, entry, synchronizedKey !== null);
		sharedGroups.set(presentationKey, group);
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

export function dailyTeachingEntryTitle(entry: DailyTeachingEntry): string {
	return (
		(normalizedEntryType(entry.entryType) === 'course' ? textOrNull(entry.offeringName) : null) ??
		textOrNull(entry.title) ??
		textOrNull(entry.offeringName) ??
		textOrNull(entry.subjectVersionDisplayLabel) ??
		textOrNull(entry.activityVersionDisplayLabel) ??
		(normalizedEntryType(entry.entryType) === 'break' ? 'พัก' : null) ??
		(normalizedEntryType(entry.entryType) === 'homeroom' ? 'โฮมรูม' : null) ??
		'คาบพิเศษ'
	);
}

export function dailyTeachingCardPresentation(
	group: DailyTeachingDisplayGroup,
	cell: DailyTeachingDisplayCell,
	view: DailyTeachingView
): DailyTeachingCardPresentation {
	const entry = group.entries[0];
	const shared = isStructuralEntry(entry) || group.isSynchronizedActivity;
	const homeroomNames = [
		...new Set(
			group.entries.flatMap((item) =>
				item.homeroomNames.map(textOrNull).filter((name) => name !== null)
			)
		)
	].sort(thaiNaturalCollator.compare);
	const targetLabel =
		homeroomNames.length > 1
			? shared
				? `${homeroomNames.length} ห้อง`
				: `${homeroomNames[0]} +${homeroomNames.length - 1}`
			: (homeroomNames[0] ?? textOrNull(entry.learningGroupName));
	const instructors =
		view === 'homeroom' && !shared ? dailyTeachingGroupInstructors(group.entries, cell) : [];
	const firstInstructor = instructors[0];
	const rooms = [
		...new Set(group.locations.map((location) => location.roomName).filter((name) => name !== null))
	];
	return {
		code: normalizedEntryType(entry.entryType) === 'course' ? textOrNull(entry.offeringCode) : null,
		title: dailyTeachingEntryTitle(entry),
		titleLineLimit: shared ? 3 : 1,
		targetLabel: view === 'teacher' ? targetLabel : null,
		teacherLabel: firstInstructor
			? `${timetableTeacherLabel(firstInstructor.displayName)}${instructors.length > 1 ? ` +${instructors.length - 1}` : ''}`
			: null,
		teacherTitle: firstInstructor ? instructors.map((item) => item.displayName).join(', ') : null,
		roomLabel: rooms.join(', ') || (shared ? null : 'ไม่ระบุห้อง')
	};
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

export function dailyTeachingEmptyCellLabel(
	teacherName: string,
	periodName: string,
	periodTime: string
): string {
	return `${teacherName} ${periodName} ${periodTime}: ว่าง`;
}
