export type TeacherLoadCategory =
	| 'course'
	| 'independentActivity'
	| 'synchronizedActivity'
	| 'unspecifiedActivity';

export type TeacherLoadDetailKind =
	| 'homeGroupPrimaryCourse'
	| 'homeGroupSecondaryCourse'
	| 'sharedPrimaryCourse'
	| 'sharedSecondaryCourse'
	| 'independentActivity'
	| 'synchronizedActivity'
	| 'unspecifiedActivity';

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
	instructor_roles?: string[] | null;
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
	homeGroupPrimaryCoursePeriods: number;
	homeGroupSecondaryCoursePeriods: number;
	sharedPrimaryCoursePeriods: number;
	sharedSecondaryCoursePeriods: number;
	independentActivityPeriods: number;
	synchronizedActivityPeriods: number;
	unspecifiedActivityPeriods: number;
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
	classroomName: string;
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
	homeGroupPrimaryCourse: 'วิชาในกลุ่มสาระ (ครูหลัก)',
	homeGroupSecondaryCourse: 'วิชาในกลุ่มสาระ (ครูรอง)',
	sharedPrimaryCourse: 'วิชานอกกลุ่มสาระ (ครูหลัก)',
	sharedSecondaryCourse: 'วิชานอกกลุ่มสาระ (ครูรอง)',
	independentActivity: 'กิจกรรม independent',
	synchronizedActivity: 'กิจกรรม synchronized',
	unspecifiedActivity: 'กิจกรรมไม่ระบุประเภท'
};

const DETAIL_KIND_ORDER: Record<TeacherLoadDetailKind, number> = {
	homeGroupPrimaryCourse: 1,
	homeGroupSecondaryCourse: 2,
	sharedPrimaryCourse: 3,
	sharedSecondaryCourse: 4,
	independentActivity: 5,
	synchronizedActivity: 6,
	unspecifiedActivity: 7
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
	if (entry.activity_scheduling_mode === 'independent') return 'independentActivity';
	if (entry.activity_scheduling_mode === 'synchronized') return 'synchronizedActivity';
	return 'unspecifiedActivity';
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
			const instructorRole = instructorRoleForEntry(entry, index);
			const detailKind = detailKindForEntry(
				entry,
				category,
				teacherSubjectGroup.id,
				instructorRole
			);
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
				instructorRole,
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
				row.homeGroupPrimaryCoursePeriods +
				row.homeGroupSecondaryCoursePeriods +
				row.sharedPrimaryCoursePeriods +
				row.sharedSecondaryCoursePeriods +
				row.independentActivityPeriods +
				row.synchronizedActivityPeriods +
				row.unspecifiedActivityPeriods
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
		homeGroupPrimaryCoursePeriods: 0,
		homeGroupSecondaryCoursePeriods: 0,
		sharedPrimaryCoursePeriods: 0,
		sharedSecondaryCoursePeriods: 0,
		independentActivityPeriods: 0,
		synchronizedActivityPeriods: 0,
		unspecifiedActivityPeriods: 0,
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
	else summary.unspecifiedActivityPeriods += 1;
}

function teacherLoadDetailKey(
	entry: TeacherLoadEntry,
	category: TeacherLoadCategory,
	teacherId: string
): string {
	if (category === 'synchronizedActivity' || category === 'unspecifiedActivity') {
		const logicalActivityId = entry.activity_slot_id || entry.id;
		return [teacherId, category, logicalActivityId, entry.day_of_week, entry.period_id].join('|');
	}
	return [teacherId, category, entry.id].join('|');
}

function detailKindForEntry(
	entry: TeacherLoadEntry,
	category: TeacherLoadCategory,
	teacherSubjectGroupId: string | null,
	instructorRole: string
): TeacherLoadDetailKind {
	if (category === 'independentActivity') return 'independentActivity';
	if (category === 'synchronizedActivity') return 'synchronizedActivity';
	if (category === 'unspecifiedActivity') return 'unspecifiedActivity';

	const isHomeGroup =
		!!entry.subject_group_id &&
		!!teacherSubjectGroupId &&
		entry.subject_group_id === teacherSubjectGroupId;
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

function instructorRoleForEntry(entry: TeacherLoadEntry, index: number): string {
	return entry.instructor_roles?.[index] === 'primary' ? 'primary' : 'secondary';
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
		group.totals.homeGroupPrimaryCoursePeriods += row.homeGroupPrimaryCoursePeriods;
		group.totals.homeGroupSecondaryCoursePeriods += row.homeGroupSecondaryCoursePeriods;
		group.totals.sharedPrimaryCoursePeriods += row.sharedPrimaryCoursePeriods;
		group.totals.sharedSecondaryCoursePeriods += row.sharedSecondaryCoursePeriods;
		group.totals.independentActivityPeriods += row.independentActivityPeriods;
		group.totals.synchronizedActivityPeriods += row.synchronizedActivityPeriods;
		group.totals.unspecifiedActivityPeriods += row.unspecifiedActivityPeriods;
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
			'กิจกรรม independent (คาบ)',
			'กิจกรรม synchronized (คาบ)',
			'กิจกรรมไม่ระบุประเภท (คาบ)',
			'รวม (คาบ)'
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
