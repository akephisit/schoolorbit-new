import type {
	TimetableBlock,
	TimetableBlockInstructor,
	TimetableBlockWorkspace
} from '#lib/api/timetable.js';

export type TeacherLoadCategory =
	| 'course'
	| 'independentActivity'
	| 'synchronizedActivity'
	| 'unspecifiedActivity'
	| 'specialPeriod';

export type TeacherLoadDetailKind =
	| 'homeGroupPrimaryCourse'
	| 'homeGroupSecondaryCourse'
	| 'sharedPrimaryCourse'
	| 'sharedSecondaryCourse'
	| 'independentActivity'
	| 'synchronizedActivity'
	| 'unspecifiedActivity'
	| 'unclassifiedCourse'
	| 'specialPeriod';

export type TeacherLoadEntry = TimetableBlock;

export interface TeacherLoadSummaryRow {
	teacherId: string;
	teacherName: string;
	teacherSubjectGroupId: string | null;
	teacherSubjectGroupName: string;
	teacherSubjectGroupDisplayOrder: number | null;
	homeGroupPrimaryCoursePeriods: number;
	homeGroupSecondaryCoursePeriods: number;
	sharedPrimaryCoursePeriods: number;
	sharedSecondaryCoursePeriods: number;
	independentActivityPeriods: number;
	synchronizedActivityPeriods: number;
	unspecifiedActivityPeriods: number;
	unclassifiedCoursePeriods: number;
	specialPeriods: number;
	totalPeriods: number;
}

export interface TeacherLoadDetailRow {
	teacherId: string;
	teacherName: string;
	teacherSubjectGroupId: string | null;
	teacherSubjectGroupName: string;
	teacherSubjectGroupDisplayOrder: number | null;
	subjectGroupId: string | null;
	subjectGroupName: string;
	subjectGroupDisplayOrder: number | null;
	instructorRole: string;
	category: TeacherLoadCategory;
	detailKind: TeacherLoadDetailKind;
	categoryLabel: string;
	dayOfWeek: string;
	dayLabel: string;
	periodName: string;
	periodOrderIndex: number | null;
	timeLabel: string;
	homeroomName: string;
	roomName: string;
	title: string;
}

export interface TeacherLoadSummaryGroup {
	subjectGroupId: string | null;
	subjectGroupName: string;
	subjectGroupDisplayOrder: number | null;
	rows: TeacherLoadSummaryRow[];
	totals: {
		homeGroupPrimaryCoursePeriods: number;
		homeGroupSecondaryCoursePeriods: number;
		sharedPrimaryCoursePeriods: number;
		sharedSecondaryCoursePeriods: number;
		independentActivityPeriods: number;
		synchronizedActivityPeriods: number;
		unspecifiedActivityPeriods: number;
		unclassifiedCoursePeriods: number;
		specialPeriods: number;
		totalPeriods: number;
	};
}

export interface TeacherLoadDetailGroup {
	subjectGroupId: string | null;
	subjectGroupName: string;
	subjectGroupDisplayOrder: number | null;
	rows: TeacherLoadDetailRow[];
}

export interface TeacherLoadExportRows {
	summaryRows: TeacherLoadSummaryRow[];
	detailRows: TeacherLoadDetailRow[];
	summaryGroups: TeacherLoadSummaryGroup[];
	detailGroups: TeacherLoadDetailGroup[];
	summarySheetRows: Array<Array<string | number>>;
	detailSheetRows: Array<Array<string | number>>;
}

export interface TeacherLoadColumnWidthOptions {
	minWidths?: readonly number[];
	maxWidths?: readonly number[];
	padding?: number;
	defaultMinWidth?: number;
	defaultMaxWidth?: number;
}

export const TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS = {
	minWidths: [12, 14, 8, 8, 8, 8, 8, 8, 8, 8, 8, 8],
	maxWidths: [20, 24, 14, 14, 14, 14, 14, 14, 14, 14, 14, 12],
	padding: 2
} satisfies TeacherLoadColumnWidthOptions;

export const TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS = {
	minWidths: [12, 14, 12, 12, 8, 7, 7, 9, 10, 10, 16],
	maxWidths: [20, 24, 20, 22, 14, 10, 12, 13, 24, 24, 42],
	padding: 2
} satisfies TeacherLoadColumnWidthOptions;

const UNKNOWN_SUBJECT_GROUP_NAME = 'ไม่ระบุกลุ่มสาระ';
const ACTIVITY_SUBJECT_GROUP_NAME = 'กิจกรรม';

const CATEGORY_LABELS: Record<TeacherLoadDetailKind, string> = {
	homeGroupPrimaryCourse: 'วิชาในกลุ่มสาระ (ครูหลัก)',
	homeGroupSecondaryCourse: 'วิชาในกลุ่มสาระ (ครูรอง)',
	sharedPrimaryCourse: 'วิชานอกกลุ่มสาระ (ครูหลัก)',
	sharedSecondaryCourse: 'วิชานอกกลุ่มสาระ (ครูรอง)',
	independentActivity: 'กิจกรรมอิสระ',
	synchronizedActivity: 'กิจกรรมพร้อมกัน',
	unspecifiedActivity: 'กิจกรรมไม่ระบุประเภท',
	unclassifiedCourse: 'วิชายังไม่ทราบกลุ่มสาระ',
	specialPeriod: 'คาบพิเศษ/ภารกิจ'
};

const DETAIL_KIND_ORDER: Record<TeacherLoadDetailKind, number> = {
	homeGroupPrimaryCourse: 1,
	homeGroupSecondaryCourse: 2,
	sharedPrimaryCourse: 3,
	sharedSecondaryCourse: 4,
	independentActivity: 5,
	synchronizedActivity: 6,
	unspecifiedActivity: 7,
	unclassifiedCourse: 8,
	specialPeriod: 9
};

const DAY_LABELS: Record<string, string> = {
	MON: 'จันทร์',
	TUE: 'อังคาร',
	WED: 'พุธ',
	THU: 'พฤหัสบดี',
	FRI: 'ศุกร์',
	SAT: 'เสาร์',
	SUN: 'อาทิตย์'
};

const DAY_ORDER: Record<string, number> = {
	MON: 1,
	TUE: 2,
	WED: 3,
	THU: 4,
	FRI: 5,
	SAT: 6,
	SUN: 7
};

export function teacherLoadCategoryForEntry(entry: TeacherLoadEntry): TeacherLoadCategory | null {
	if (entry.blockKind === 'course') return 'course';
	if (entry.blockKind === 'structural') return 'specialPeriod';
	if (entry.schedulingMode === 'independent') return 'independentActivity';
	if (entry.schedulingMode === 'synchronized') return 'synchronizedActivity';
	return 'unspecifiedActivity';
}

export type TeacherLoadWorkspace = Pick<
	TimetableBlockWorkspace,
	'blocks' | 'staff' | 'offeringSubjectGroups' | 'homerooms' | 'rooms' | 'bellPeriods'
>;

export function buildTeacherLoadExportRows(workspace: TeacherLoadWorkspace): TeacherLoadExportRows {
	const { blocks: entries } = workspace;
	const staffById = new Map(workspace.staff.map((teacher) => [teacher.id, teacher]));
	const offeringGroups = new Map(
		workspace.offeringSubjectGroups.map((offering) => [
			offering.learningOfferingId,
			offering.subjectGroup
		])
	);
	const homerooms = new Map(workspace.homerooms.map((room) => [room.id, room.name]));
	const rooms = new Map(workspace.rooms.map((room) => [room.id, room.name]));
	const periodOrders = new Map(
		workspace.bellPeriods.map((period) => [period.id, period.orderIndex])
	);
	const summaries = new Map<string, TeacherLoadSummaryRow>();
	const details = new Map<string, TeacherLoadDetailRow & { homeroomNames: string[] }>();

	for (const entry of entries) {
		if (!entry.isActive) continue;
		const category = teacherLoadCategoryForEntry(entry);
		if (!category) continue;

		for (const instructor of instructorsForBlock(entry)) {
			const teacherId = instructor.teacherId;
			const teacherName = instructor.displayName;
			const teacherGroups = staffById.get(teacherId)?.subjectGroups ?? [];
			const teacherSubjectGroup = teacherSubjectGroupForInstructor(teacherGroups);
			const itemSubjectGroup = itemSubjectGroupForEntry(entry, category, offeringGroups);
			const targetNames = blockTargetNames(entry, teacherId, homerooms);
			const instructorRole = instructor.role;
			const detailKind = detailKindForEntry(
				category,
				teacherGroups.map((group) => group.id),
				itemSubjectGroup.id,
				instructorRole
			);
			const detailKey = `${teacherId}|${entry.id}`;
			const existingDetail = details.get(detailKey);

			if (existingDetail) {
				for (const name of targetNames) appendUnique(existingDetail.homeroomNames, name);
				existingDetail.homeroomName = existingDetail.homeroomNames.join(', ');
				continue;
			}

			const summary = getOrCreateSummary(summaries, teacherId, teacherName, teacherSubjectGroup);
			incrementSummary(summary, detailKind);

			const homeroomNames = uniqueNonEmpty(targetNames);
			details.set(detailKey, {
				teacherId,
				teacherName,
				teacherSubjectGroupId: teacherSubjectGroup.id,
				teacherSubjectGroupName: teacherSubjectGroup.name,
				teacherSubjectGroupDisplayOrder: teacherSubjectGroup.displayOrder,
				subjectGroupId: itemSubjectGroup.id,
				subjectGroupName: itemSubjectGroup.name,
				subjectGroupDisplayOrder: itemSubjectGroup.displayOrder,
				instructorRole,
				category,
				detailKind,
				categoryLabel: CATEGORY_LABELS[detailKind],
				dayOfWeek: entry.dayOfWeek,
				dayLabel: DAY_LABELS[entry.dayOfWeek] ?? entry.dayOfWeek,
				periodName: entry.periodName ?? '',
				periodOrderIndex: periodOrders.get(entry.bellSchedulePeriodId) ?? null,
				timeLabel: formatTimeRange(entry.startTime, entry.endTime),
				homeroomName: homeroomNames.join(', '),
				roomName: blockRoomNames(entry, teacherId, rooms).join(', '),
				title: entryTitle(entry, category),
				homeroomNames
			});
		}
	}

	const summaryRows = Array.from(summaries.values())
		.map((row) => ({
			...row,
			totalPeriods:
				row.homeGroupPrimaryCoursePeriods +
				row.homeGroupSecondaryCoursePeriods +
				row.sharedPrimaryCoursePeriods +
				row.sharedSecondaryCoursePeriods +
				row.independentActivityPeriods +
				row.synchronizedActivityPeriods +
				row.unspecifiedActivityPeriods +
				row.unclassifiedCoursePeriods +
				row.specialPeriods
		}))
		.sort(compareSummaryRows);

	const detailRows = Array.from(details.values())
		.map(({ homeroomNames: _homeroomNames, ...row }) => row)
		.sort(compareDetailRows);

	const summaryGroups = groupSummaryRows(summaryRows);
	const detailGroups = groupDetailRows(detailRows);

	return {
		summaryRows,
		detailRows,
		summaryGroups,
		detailGroups,
		summarySheetRows: buildSummarySheetRows(summaryGroups),
		detailSheetRows: buildDetailSheetRows(detailGroups)
	};
}

export function calculateTeacherLoadColumnWidths(
	rows: Array<Array<string | number>>,
	options: TeacherLoadColumnWidthOptions = {}
): number[] {
	const columnCount = rows.reduce((max, row) => Math.max(max, row.length), 0);
	const padding = options.padding ?? 2;
	const defaultMinWidth = options.defaultMinWidth ?? 8;
	const defaultMaxWidth = options.defaultMaxWidth ?? 36;

	return Array.from({ length: columnCount }, (_, index) => {
		const minWidth = options.minWidths?.[index] ?? defaultMinWidth;
		const maxWidth = options.maxWidths?.[index] ?? defaultMaxWidth;
		const contentWidth = rows.reduce((max, row) => {
			const value = row[index];
			return Math.max(max, teacherLoadCellDisplayWidth(value));
		}, 0);

		return Math.min(maxWidth, Math.max(minWidth, Math.ceil(contentWidth + padding)));
	});
}

function teacherLoadCellDisplayWidth(value: string | number | undefined): number {
	if (value === undefined) return 0;
	return String(value)
		.split(/\r?\n/)
		.reduce((max, line) => Math.max(max, Array.from(line).length), 0);
}

function getOrCreateSummary(
	summaries: Map<string, TeacherLoadSummaryRow>,
	teacherId: string,
	teacherName: string,
	teacherSubjectGroup: SubjectGroupMeta
): TeacherLoadSummaryRow {
	const existing = summaries.get(teacherId);
	if (existing) return existing;

	const row = {
		teacherId,
		teacherName,
		teacherSubjectGroupId: teacherSubjectGroup.id,
		teacherSubjectGroupName: teacherSubjectGroup.name,
		teacherSubjectGroupDisplayOrder: teacherSubjectGroup.displayOrder,
		homeGroupPrimaryCoursePeriods: 0,
		homeGroupSecondaryCoursePeriods: 0,
		sharedPrimaryCoursePeriods: 0,
		sharedSecondaryCoursePeriods: 0,
		independentActivityPeriods: 0,
		synchronizedActivityPeriods: 0,
		unspecifiedActivityPeriods: 0,
		unclassifiedCoursePeriods: 0,
		specialPeriods: 0,
		totalPeriods: 0
	};
	summaries.set(teacherId, row);
	return row;
}

function incrementSummary(summary: TeacherLoadSummaryRow, detailKind: TeacherLoadDetailKind) {
	if (detailKind === 'homeGroupPrimaryCourse') summary.homeGroupPrimaryCoursePeriods += 1;
	else if (detailKind === 'homeGroupSecondaryCourse') summary.homeGroupSecondaryCoursePeriods += 1;
	else if (detailKind === 'sharedPrimaryCourse') summary.sharedPrimaryCoursePeriods += 1;
	else if (detailKind === 'sharedSecondaryCourse') summary.sharedSecondaryCoursePeriods += 1;
	else if (detailKind === 'independentActivity') summary.independentActivityPeriods += 1;
	else if (detailKind === 'synchronizedActivity') summary.synchronizedActivityPeriods += 1;
	else if (detailKind === 'unspecifiedActivity') summary.unspecifiedActivityPeriods += 1;
	else if (detailKind === 'unclassifiedCourse') summary.unclassifiedCoursePeriods += 1;
	else summary.specialPeriods += 1;
}

function detailKindForEntry(
	category: TeacherLoadCategory,
	teacherSubjectGroupIds: string[],
	itemSubjectGroupId: string | null,
	instructorRole: string
): TeacherLoadDetailKind {
	if (category === 'independentActivity') return 'independentActivity';
	if (category === 'synchronizedActivity') return 'synchronizedActivity';
	if (category === 'unspecifiedActivity') return 'unspecifiedActivity';

	if (category === 'specialPeriod') return 'specialPeriod';
	if (!itemSubjectGroupId || teacherSubjectGroupIds.length === 0) return 'unclassifiedCourse';
	const isHomeGroup = teacherSubjectGroupIds.includes(itemSubjectGroupId);
	const isPrimary = instructorRole === 'primary';

	if (isHomeGroup && isPrimary) return 'homeGroupPrimaryCourse';
	if (isHomeGroup) return 'homeGroupSecondaryCourse';
	if (isPrimary) return 'sharedPrimaryCourse';
	return 'sharedSecondaryCourse';
}

interface SubjectGroupMeta {
	id: string | null;
	name: string;
	displayOrder: number | null;
}

function teacherSubjectGroupForInstructor(
	groups: TimetableBlockWorkspace['staff'][number]['subjectGroups']
): SubjectGroupMeta {
	const unique = [...new Map(groups.map((group) => [group.id, group])).values()].sort(
		(a, b) =>
			(a.displayOrder ?? 9999) - (b.displayOrder ?? 9999) ||
			a.name.localeCompare(b.name, 'th') ||
			a.id.localeCompare(b.id)
	);
	return {
		id: unique.length ? unique.map((group) => group.id).join('|') : null,
		name: unique.length ? unique.map((group) => group.name).join(', ') : UNKNOWN_SUBJECT_GROUP_NAME,
		displayOrder: unique[0]?.displayOrder ?? null
	};
}

function itemSubjectGroupForEntry(
	entry: TeacherLoadEntry,
	category: TeacherLoadCategory,
	groups: Map<string, TimetableBlockWorkspace['offeringSubjectGroups'][number]['subjectGroup']>
): SubjectGroupMeta {
	if (category !== 'course')
		return {
			id: null,
			name: category === 'specialPeriod' ? 'คาบพิเศษ/ภารกิจ' : ACTIVITY_SUBJECT_GROUP_NAME,
			displayOrder: null
		};
	return (
		groups.get(entry.learningOfferingId ?? '') ?? {
			id: null,
			name: UNKNOWN_SUBJECT_GROUP_NAME,
			displayOrder: null
		}
	);
}

function entryTitle(entry: TeacherLoadEntry, category: TeacherLoadCategory): string {
	if (category === 'specialPeriod') return entry.title || 'คาบพิเศษ/ภารกิจ';
	if (category === 'course') {
		return [entry.offeringCode, entry.offeringName].filter(Boolean).join(' - ');
	}
	return entry.offeringName || entry.title || CATEGORY_LABELS[category];
}

function instructorsForBlock(entry: TeacherLoadEntry): TimetableBlockInstructor[] {
	const byTeacher = new Map<string, TimetableBlockInstructor>();
	for (const instructor of entry.groups
		.filter((group) => group.isActive)
		.flatMap((group) => group.instructors)) {
		const current = byTeacher.get(instructor.teacherId);
		if (!current || instructor.role === 'primary') byTeacher.set(instructor.teacherId, instructor);
	}
	for (const teacher of entry.teachers.filter((target) => target.isActive)) {
		if (!byTeacher.has(teacher.teacherId))
			byTeacher.set(teacher.teacherId, {
				teacherId: teacher.teacherId,
				displayName: teacher.displayName,
				role: 'target',
				orderIndex: 0
			});
	}
	return [...byTeacher.values()];
}

function teacherGroups(entry: TeacherLoadEntry, teacherId: string) {
	return entry.groups.filter(
		(group) =>
			group.isActive && group.instructors.some((teacher) => teacher.teacherId === teacherId)
	);
}

function blockTargetNames(
	entry: TeacherLoadEntry,
	teacherId: string,
	homerooms: Map<string, string>
): string[] {
	const groups = teacherGroups(entry, teacherId);
	if (groups.length)
		return groups.flatMap((group) => {
			const names = group.homeroomIds.flatMap((id) =>
				homerooms.has(id) ? [homerooms.get(id)!] : []
			);
			return names.length ? names : [group.name];
		});
	return entry.homerooms.filter((target) => target.isActive).map((target) => target.name);
}

function blockRoomNames(
	entry: TeacherLoadEntry,
	teacherId: string,
	rooms: Map<string, string>
): string[] {
	const groups = teacherGroups(entry, teacherId);
	const targets = groups.length ? groups : entry.homerooms.filter((target) => target.isActive);
	return uniqueNonEmpty(
		targets.flatMap((target) =>
			target.roomId ? [rooms.get(target.roomId) ?? target.roomCode ?? ''] : []
		)
	);
}

function formatTimeRange(start?: string | null, end?: string | null): string {
	if (!start && !end) return '';
	if (!start) return formatTime(end);
	if (!end) return formatTime(start);
	return `${formatTime(start)}-${formatTime(end)}`;
}

function formatTime(value?: string | null): string {
	return value ? value.slice(0, 5) : '';
}

function uniqueNonEmpty(values: string[]): string[] {
	const result: string[] = [];
	for (const value of values) appendUnique(result, value);
	return result;
}

function appendUnique(values: string[], value: string) {
	if (value && !values.includes(value)) values.push(value);
}

function groupSummaryRows(rows: TeacherLoadSummaryRow[]): TeacherLoadSummaryGroup[] {
	const groups = new Map<string, TeacherLoadSummaryGroup>();

	for (const row of rows) {
		const key = subjectGroupKey(row.teacherSubjectGroupId, row.teacherSubjectGroupName);
		const group =
			groups.get(key) ??
			createSummaryGroup(
				row.teacherSubjectGroupId,
				row.teacherSubjectGroupName,
				row.teacherSubjectGroupDisplayOrder
			);
		group.rows.push(row);
		group.totals.homeGroupPrimaryCoursePeriods += row.homeGroupPrimaryCoursePeriods;
		group.totals.homeGroupSecondaryCoursePeriods += row.homeGroupSecondaryCoursePeriods;
		group.totals.sharedPrimaryCoursePeriods += row.sharedPrimaryCoursePeriods;
		group.totals.sharedSecondaryCoursePeriods += row.sharedSecondaryCoursePeriods;
		group.totals.independentActivityPeriods += row.independentActivityPeriods;
		group.totals.synchronizedActivityPeriods += row.synchronizedActivityPeriods;
		group.totals.unspecifiedActivityPeriods += row.unspecifiedActivityPeriods;
		group.totals.unclassifiedCoursePeriods += row.unclassifiedCoursePeriods;
		group.totals.specialPeriods += row.specialPeriods;
		group.totals.totalPeriods += row.totalPeriods;
		groups.set(key, group);
	}

	return Array.from(groups.values()).sort(compareGroups);
}

function createSummaryGroup(
	subjectGroupId: string | null,
	subjectGroupName: string,
	subjectGroupDisplayOrder: number | null
): TeacherLoadSummaryGroup {
	return {
		subjectGroupId,
		subjectGroupName,
		subjectGroupDisplayOrder,
		rows: [],
		totals: {
			homeGroupPrimaryCoursePeriods: 0,
			homeGroupSecondaryCoursePeriods: 0,
			sharedPrimaryCoursePeriods: 0,
			sharedSecondaryCoursePeriods: 0,
			independentActivityPeriods: 0,
			synchronizedActivityPeriods: 0,
			unspecifiedActivityPeriods: 0,
			unclassifiedCoursePeriods: 0,
			specialPeriods: 0,
			totalPeriods: 0
		}
	};
}

function groupDetailRows(rows: TeacherLoadDetailRow[]): TeacherLoadDetailGroup[] {
	const groups = new Map<string, TeacherLoadDetailGroup>();

	for (const row of rows) {
		const key = subjectGroupKey(row.teacherSubjectGroupId, row.teacherSubjectGroupName);
		const group =
			groups.get(key) ??
			({
				subjectGroupId: row.teacherSubjectGroupId,
				subjectGroupName: row.teacherSubjectGroupName,
				subjectGroupDisplayOrder: row.teacherSubjectGroupDisplayOrder,
				rows: []
			} satisfies TeacherLoadDetailGroup);
		group.rows.push(row);
		groups.set(key, group);
	}

	return Array.from(groups.values()).sort(compareGroups);
}

function subjectGroupKey(subjectGroupId: string | null, subjectGroupName: string): string {
	return subjectGroupId ?? `missing:${subjectGroupName}`;
}

function buildSummarySheetRows(groups: TeacherLoadSummaryGroup[]): Array<Array<string | number>> {
	return [
		[
			'กลุ่มสาระครู',
			'ครูผู้สอน',
			'วิชาในกลุ่มสาระ (ครูหลัก)',
			'วิชาในกลุ่มสาระ (ครูรอง)',
			'วิชานอกกลุ่มสาระ (ครูหลัก)',
			'วิชานอกกลุ่มสาระ (ครูรอง)',
			'กิจกรรมอิสระ (คาบ)',
			'กิจกรรมพร้อมกัน (คาบ)',
			'กิจกรรมไม่ระบุประเภท (คาบ)',
			'วิชายังไม่ทราบกลุ่มสาระ (คาบ)',
			'คาบพิเศษ/ภารกิจ (คาบ)',
			'รวมคาบที่รับผิดชอบ'
		],
		...groups.flatMap((group) => [
			[
				`กลุ่มสาระ: ${group.subjectGroupName}`,
				'',
				group.totals.homeGroupPrimaryCoursePeriods,
				group.totals.homeGroupSecondaryCoursePeriods,
				group.totals.sharedPrimaryCoursePeriods,
				group.totals.sharedSecondaryCoursePeriods,
				group.totals.independentActivityPeriods,
				group.totals.synchronizedActivityPeriods,
				group.totals.unspecifiedActivityPeriods,
				group.totals.unclassifiedCoursePeriods,
				group.totals.specialPeriods,
				group.totals.totalPeriods
			],
			...group.rows.map((row) => [
				row.teacherSubjectGroupName,
				row.teacherName,
				row.homeGroupPrimaryCoursePeriods,
				row.homeGroupSecondaryCoursePeriods,
				row.sharedPrimaryCoursePeriods,
				row.sharedSecondaryCoursePeriods,
				row.independentActivityPeriods,
				row.synchronizedActivityPeriods,
				row.unspecifiedActivityPeriods,
				row.unclassifiedCoursePeriods,
				row.specialPeriods,
				row.totalPeriods
			])
		])
	];
}

function buildDetailSheetRows(groups: TeacherLoadDetailGroup[]): Array<Array<string | number>> {
	return [
		[
			'กลุ่มสาระครู',
			'ครูผู้สอน',
			'กลุ่มสาระรายการ',
			'ประเภท',
			'บทบาท',
			'วัน',
			'คาบ',
			'เวลา',
			'ชั้น/กลุ่มเรียน',
			'ห้องเรียน',
			'รายการ'
		],
		...groups.flatMap((group) => [
			[`กลุ่มสาระ: ${group.subjectGroupName}`, '', '', '', '', '', '', '', '', '', ''],
			...group.rows.map((row) => [
				row.teacherSubjectGroupName,
				row.teacherName,
				row.subjectGroupName,
				row.categoryLabel,
				row.instructorRole === 'target'
					? 'ผู้รับผิดชอบ'
					: row.instructorRole === 'primary'
						? 'ครูหลัก'
						: 'ครูรอง',
				row.dayLabel,
				row.periodName,
				row.timeLabel,
				row.homeroomName,
				row.roomName,
				row.title
			])
		])
	];
}

function compareSummaryRows(a: TeacherLoadSummaryRow, b: TeacherLoadSummaryRow): number {
	return (
		compareSubjectGroupMeta(
			a.teacherSubjectGroupDisplayOrder,
			a.teacherSubjectGroupName,
			b.teacherSubjectGroupDisplayOrder,
			b.teacherSubjectGroupName
		) ||
		b.totalPeriods - a.totalPeriods ||
		a.teacherName.localeCompare(b.teacherName, 'th') ||
		a.teacherId.localeCompare(b.teacherId)
	);
}

function compareDetailRows(a: TeacherLoadDetailRow, b: TeacherLoadDetailRow): number {
	return (
		compareSubjectGroupMeta(
			a.teacherSubjectGroupDisplayOrder,
			a.teacherSubjectGroupName,
			b.teacherSubjectGroupDisplayOrder,
			b.teacherSubjectGroupName
		) ||
		(DAY_ORDER[a.dayOfWeek] ?? 99) - (DAY_ORDER[b.dayOfWeek] ?? 99) ||
		(a.periodOrderIndex ?? 999) - (b.periodOrderIndex ?? 999) ||
		a.timeLabel.localeCompare(b.timeLabel) ||
		DETAIL_KIND_ORDER[a.detailKind] - DETAIL_KIND_ORDER[b.detailKind] ||
		a.teacherName.localeCompare(b.teacherName, 'th') ||
		a.title.localeCompare(b.title, 'th')
	);
}

function compareGroups(
	a: Pick<
		TeacherLoadSummaryGroup | TeacherLoadDetailGroup,
		'subjectGroupDisplayOrder' | 'subjectGroupName'
	>,
	b: Pick<
		TeacherLoadSummaryGroup | TeacherLoadDetailGroup,
		'subjectGroupDisplayOrder' | 'subjectGroupName'
	>
): number {
	return compareSubjectGroupMeta(
		a.subjectGroupDisplayOrder,
		a.subjectGroupName,
		b.subjectGroupDisplayOrder,
		b.subjectGroupName
	);
}

function compareSubjectGroupMeta(
	aOrder: number | null,
	aName: string,
	bOrder: number | null,
	bName: string
): number {
	return (aOrder ?? 9999) - (bOrder ?? 9999) || aName.localeCompare(bName, 'th');
}
