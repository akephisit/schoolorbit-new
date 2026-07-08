export type TeacherLoadCategory = 'course' | 'independentActivity' | 'synchronizedActivity';

export type TeacherLoadDetailKind =
	| 'homeGroupCourse'
	| 'sharedCourse'
	| 'independentActivity'
	| 'synchronizedActivity';

export interface TeacherLoadEntry {
	id: string;
	entry_type: string;
	day_of_week: string;
	period_id: string;
	period_name?: string | null;
	period_order_index?: number | null;
	start_time?: string | null;
	end_time?: string | null;
	classroom_name?: string | null;
	subject_code?: string | null;
	subject_name_th?: string | null;
	subject_group_id?: string | null;
	subject_group_name?: string | null;
	subject_group_display_order?: number | null;
	title?: string | null;
	activity_slot_id?: string | null;
	activity_slot_name?: string | null;
	activity_scheduling_mode?: string | null;
	instructor_ids?: string[] | null;
	instructor_names?: string[] | null;
	instructor_name?: string | null;
	instructor_subject_group_ids?: Array<string | null> | null;
	instructor_subject_group_names?: Array<string | null> | null;
	instructor_subject_group_display_orders?: Array<number | null> | null;
}

export interface TeacherLoadSummaryRow {
	teacherId: string;
	teacherName: string;
	teacherSubjectGroupId: string | null;
	teacherSubjectGroupName: string;
	teacherSubjectGroupDisplayOrder: number | null;
	homeGroupCoursePeriods: number;
	sharedCoursePeriods: number;
	independentActivityPeriods: number;
	synchronizedActivityPeriods: number;
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
	category: TeacherLoadCategory;
	detailKind: TeacherLoadDetailKind;
	categoryLabel: string;
	dayOfWeek: string;
	dayLabel: string;
	periodName: string;
	periodOrderIndex: number | null;
	timeLabel: string;
	classroomName: string;
	title: string;
}

export interface TeacherLoadSummaryGroup {
	subjectGroupId: string | null;
	subjectGroupName: string;
	subjectGroupDisplayOrder: number | null;
	rows: TeacherLoadSummaryRow[];
	totals: {
		homeGroupCoursePeriods: number;
		sharedCoursePeriods: number;
		independentActivityPeriods: number;
		synchronizedActivityPeriods: number;
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

const UNKNOWN_SUBJECT_GROUP_NAME = 'ไม่ระบุกลุ่มสาระ';
const ACTIVITY_SUBJECT_GROUP_NAME = 'กิจกรรม';

const CATEGORY_LABELS: Record<TeacherLoadDetailKind, string> = {
	homeGroupCourse: 'วิชาในกลุ่มสาระ',
	sharedCourse: 'วิชานอกกลุ่มสาระ/สอนร่วม',
	independentActivity: 'กิจกรรม independent',
	synchronizedActivity: 'กิจกรรม synchronized'
};

const DETAIL_KIND_ORDER: Record<TeacherLoadDetailKind, number> = {
	homeGroupCourse: 1,
	sharedCourse: 2,
	independentActivity: 3,
	synchronizedActivity: 4
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
	if (entry.entry_type === 'COURSE') return 'course';
	if (entry.entry_type !== 'ACTIVITY') return null;
	return entry.activity_scheduling_mode === 'independent'
		? 'independentActivity'
		: 'synchronizedActivity';
}

export function buildTeacherLoadExportRows(entries: TeacherLoadEntry[]): TeacherLoadExportRows {
	const summaries = new Map<string, TeacherLoadSummaryRow>();
	const details = new Map<string, TeacherLoadDetailRow & { classroomNames: string[] }>();

	for (const entry of entries) {
		const category = teacherLoadCategoryForEntry(entry);
		if (!category) continue;

		const instructorIds = entry.instructor_ids ?? [];
		for (let index = 0; index < instructorIds.length; index += 1) {
			const teacherId = instructorIds[index];
			const teacherName = teacherNameForEntry(entry, index, teacherId);
			const teacherSubjectGroup = teacherSubjectGroupForEntry(entry, index);
			const detailKind = detailKindForEntry(entry, category, teacherSubjectGroup.id);
			const detailKey = teacherLoadDetailKey(entry, category, teacherId);
			const existingDetail = details.get(detailKey);

			if (existingDetail) {
				appendUnique(existingDetail.classroomNames, entry.classroom_name ?? '');
				existingDetail.classroomName = existingDetail.classroomNames.join(', ');
				continue;
			}

			const summary = getOrCreateSummary(summaries, teacherId, teacherName, teacherSubjectGroup);
			incrementSummary(summary, detailKind);

			const itemSubjectGroup = itemSubjectGroupForEntry(entry, category);
			const classroomNames = uniqueNonEmpty([entry.classroom_name ?? '']);
			details.set(detailKey, {
				teacherId,
				teacherName,
				teacherSubjectGroupId: teacherSubjectGroup.id,
				teacherSubjectGroupName: teacherSubjectGroup.name,
				teacherSubjectGroupDisplayOrder: teacherSubjectGroup.displayOrder,
				subjectGroupId: itemSubjectGroup.id,
				subjectGroupName: itemSubjectGroup.name,
				subjectGroupDisplayOrder: itemSubjectGroup.displayOrder,
				category,
				detailKind,
				categoryLabel: CATEGORY_LABELS[detailKind],
				dayOfWeek: entry.day_of_week,
				dayLabel: DAY_LABELS[entry.day_of_week] ?? entry.day_of_week,
				periodName: entry.period_name ?? '',
				periodOrderIndex: entry.period_order_index ?? null,
				timeLabel: formatTimeRange(entry.start_time, entry.end_time),
				classroomName: classroomNames.join(', '),
				title: entryTitle(entry, category),
				classroomNames
			});
		}
	}

	const summaryRows = Array.from(summaries.values())
		.map((row) => ({
			...row,
			totalPeriods:
				row.homeGroupCoursePeriods +
				row.sharedCoursePeriods +
				row.independentActivityPeriods +
				row.synchronizedActivityPeriods
		}))
		.sort(compareSummaryRows);

	const detailRows = Array.from(details.values())
		.map(({ classroomNames: _classroomNames, ...row }) => row)
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
		homeGroupCoursePeriods: 0,
		sharedCoursePeriods: 0,
		independentActivityPeriods: 0,
		synchronizedActivityPeriods: 0,
		totalPeriods: 0
	};
	summaries.set(teacherId, row);
	return row;
}

function incrementSummary(summary: TeacherLoadSummaryRow, detailKind: TeacherLoadDetailKind) {
	if (detailKind === 'homeGroupCourse') summary.homeGroupCoursePeriods += 1;
	else if (detailKind === 'sharedCourse') summary.sharedCoursePeriods += 1;
	else if (detailKind === 'independentActivity') summary.independentActivityPeriods += 1;
	else summary.synchronizedActivityPeriods += 1;
}

function teacherLoadDetailKey(
	entry: TeacherLoadEntry,
	category: TeacherLoadCategory,
	teacherId: string
): string {
	if (category === 'synchronizedActivity') {
		const logicalActivityId = entry.activity_slot_id || entry.id;
		return [teacherId, category, logicalActivityId, entry.day_of_week, entry.period_id].join('|');
	}
	return [teacherId, category, entry.id].join('|');
}

function detailKindForEntry(
	entry: TeacherLoadEntry,
	category: TeacherLoadCategory,
	teacherSubjectGroupId: string | null
): TeacherLoadDetailKind {
	if (category === 'independentActivity') return 'independentActivity';
	if (category === 'synchronizedActivity') return 'synchronizedActivity';
	return entry.subject_group_id &&
		teacherSubjectGroupId &&
		entry.subject_group_id === teacherSubjectGroupId
		? 'homeGroupCourse'
		: 'sharedCourse';
}

interface SubjectGroupMeta {
	id: string | null;
	name: string;
	displayOrder: number | null;
}

function teacherSubjectGroupForEntry(entry: TeacherLoadEntry, index: number): SubjectGroupMeta {
	const id = entry.instructor_subject_group_ids?.[index] ?? null;
	const name = entry.instructor_subject_group_names?.[index] ?? null;
	const displayOrder = entry.instructor_subject_group_display_orders?.[index] ?? null;

	return {
		id,
		name: name || UNKNOWN_SUBJECT_GROUP_NAME,
		displayOrder
	};
}

function itemSubjectGroupForEntry(
	entry: TeacherLoadEntry,
	category: TeacherLoadCategory
): SubjectGroupMeta {
	if (category !== 'course') {
		return {
			id: null,
			name: ACTIVITY_SUBJECT_GROUP_NAME,
			displayOrder: null
		};
	}

	return {
		id: entry.subject_group_id ?? null,
		name: entry.subject_group_name || UNKNOWN_SUBJECT_GROUP_NAME,
		displayOrder: entry.subject_group_display_order ?? null
	};
}

function teacherNameForEntry(entry: TeacherLoadEntry, index: number, teacherId: string): string {
	const parallelName = entry.instructor_names?.[index];
	if (parallelName) return parallelName;
	if ((entry.instructor_ids?.length ?? 0) === 1 && entry.instructor_name)
		return entry.instructor_name;
	return teacherId;
}

function entryTitle(entry: TeacherLoadEntry, category: TeacherLoadCategory): string {
	if (category === 'course') {
		return [entry.subject_code, entry.subject_name_th].filter(Boolean).join(' - ');
	}
	return entry.activity_slot_name || entry.title || CATEGORY_LABELS[category];
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
		group.totals.homeGroupCoursePeriods += row.homeGroupCoursePeriods;
		group.totals.sharedCoursePeriods += row.sharedCoursePeriods;
		group.totals.independentActivityPeriods += row.independentActivityPeriods;
		group.totals.synchronizedActivityPeriods += row.synchronizedActivityPeriods;
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
			homeGroupCoursePeriods: 0,
			sharedCoursePeriods: 0,
			independentActivityPeriods: 0,
			synchronizedActivityPeriods: 0,
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
			'วิชาในกลุ่มสาระ (คาบ)',
			'วิชานอกกลุ่มสาระ/สอนร่วม (คาบ)',
			'กิจกรรม independent (คาบ)',
			'กิจกรรม synchronized (คาบ)',
			'รวม (คาบ)'
		],
		...groups.flatMap((group) => [
			[
				`กลุ่มสาระ: ${group.subjectGroupName}`,
				'',
				group.totals.homeGroupCoursePeriods,
				group.totals.sharedCoursePeriods,
				group.totals.independentActivityPeriods,
				group.totals.synchronizedActivityPeriods,
				group.totals.totalPeriods
			],
			...group.rows.map((row) => [
				row.teacherSubjectGroupName,
				row.teacherName,
				row.homeGroupCoursePeriods,
				row.sharedCoursePeriods,
				row.independentActivityPeriods,
				row.synchronizedActivityPeriods,
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
			'วัน',
			'คาบ',
			'เวลา',
			'ห้อง',
			'รายการ'
		],
		...groups.flatMap((group) => [
			[`กลุ่มสาระ: ${group.subjectGroupName}`, '', '', '', '', '', '', '', ''],
			...group.rows.map((row) => [
				row.teacherSubjectGroupName,
				row.teacherName,
				row.subjectGroupName,
				row.categoryLabel,
				row.dayLabel,
				row.periodName,
				row.timeLabel,
				row.classroomName,
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
		a.teacherName.localeCompare(b.teacherName, 'th') ||
		(DAY_ORDER[a.dayOfWeek] ?? 99) - (DAY_ORDER[b.dayOfWeek] ?? 99) ||
		(a.periodOrderIndex ?? 999) - (b.periodOrderIndex ?? 999) ||
		DETAIL_KIND_ORDER[a.detailKind] - DETAIL_KIND_ORDER[b.detailKind] ||
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
