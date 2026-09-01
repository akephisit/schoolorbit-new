import type { TimetableBlock, TimetableBlockInstructor } from '$lib/api/timetable';

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

export interface TeacherLoadColumnWidthOptions {
	minWidths?: readonly number[];
	maxWidths?: readonly number[];
	padding?: number;
	defaultMinWidth?: number;
	defaultMaxWidth?: number;
}

export const TEACHER_LOAD_SUMMARY_COLUMN_WIDTH_OPTIONS = {
	minWidths: [12, 14, 8, 8, 8, 8, 8, 8, 8, 8],
	maxWidths: [20, 24, 14, 14, 14, 14, 14, 14, 14, 10],
	padding: 2
} satisfies TeacherLoadColumnWidthOptions;

export const TEACHER_LOAD_DETAIL_COLUMN_WIDTH_OPTIONS = {
	minWidths: [12, 14, 12, 12, 7, 7, 9, 10, 16],
	maxWidths: [20, 24, 20, 22, 10, 12, 13, 18, 42],
	padding: 2
} satisfies TeacherLoadColumnWidthOptions;

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
	if (entry.blockKind === 'course') return 'course';
	if (entry.blockKind !== 'activity') return null;
	if (entry.schedulingMode === 'independent') return 'independentActivity';
	if (entry.schedulingMode === 'synchronized') return 'synchronizedActivity';
	return 'unspecifiedActivity';
}

export function buildTeacherLoadExportRows(entries: TeacherLoadEntry[]): TeacherLoadExportRows {
	const summaries = new Map<string, TeacherLoadSummaryRow>();
	const details = new Map<string, TeacherLoadDetailRow & { homeroomNames: string[] }>();

	for (const entry of entries) {
		const category = teacherLoadCategoryForEntry(entry);
		if (!category) continue;

		for (const instructor of instructorsForBlock(entry)) {
			const teacherId = instructor.teacherId;
			const teacherName = instructor.displayName;
			const teacherSubjectGroup = teacherSubjectGroupForInstructor(instructor);
			const instructorRole = instructor.role === 'primary' ? 'primary' : 'secondary';
			const detailKind = detailKindForEntry(
				entry,
				category,
				teacherSubjectGroup.id,
				instructorRole
			);
			const detailKey = teacherLoadDetailKey(entry, category, teacherId);
			const existingDetail = details.get(detailKey);

			if (existingDetail) {
				for (const name of blockTargetNames(entry))
					appendUnique(existingDetail.homeroomNames, name);
				existingDetail.homeroomName = existingDetail.homeroomNames.join(', ');
				continue;
			}

			const summary = getOrCreateSummary(summaries, teacherId, teacherName, teacherSubjectGroup);
			incrementSummary(summary, detailKind);

			const itemSubjectGroup = itemSubjectGroupForEntry(entry, category);
			const homeroomNames = uniqueNonEmpty(blockTargetNames(entry));
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
				periodOrderIndex: null,
				timeLabel: formatTimeRange(entry.startTime, entry.endTime),
				homeroomName: homeroomNames.join(', '),
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
				row.unspecifiedActivityPeriods
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
		const logicalActivityId = entry.learningOfferingId || entry.id;
		return [
			teacherId,
			category,
			logicalActivityId,
			entry.dayOfWeek,
			entry.bellSchedulePeriodId
		].join('|');
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

	const isHomeGroup = false;
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

function teacherSubjectGroupForInstructor(_instructor: TimetableBlockInstructor): SubjectGroupMeta {
	return {
		id: null,
		name: UNKNOWN_SUBJECT_GROUP_NAME,
		displayOrder: null
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
		id: null,
		name: UNKNOWN_SUBJECT_GROUP_NAME,
		displayOrder: null
	};
}

function entryTitle(entry: TeacherLoadEntry, category: TeacherLoadCategory): string {
	if (category === 'course') {
		return [entry.offeringCode, entry.offeringName].filter(Boolean).join(' - ');
	}
	return entry.offeringName || entry.title || CATEGORY_LABELS[category];
}

function instructorsForBlock(entry: TeacherLoadEntry): TimetableBlockInstructor[] {
	const byTeacher = new Map<string, TimetableBlockInstructor>();
	for (const instructor of entry.groups.flatMap((group) => group.instructors)) {
		const current = byTeacher.get(instructor.teacherId);
		if (!current || instructor.role === 'primary') byTeacher.set(instructor.teacherId, instructor);
	}
	return [...byTeacher.values()];
}

function blockTargetNames(entry: TeacherLoadEntry): string[] {
	return [
		...entry.groups.map((group) => group.name),
		...entry.homerooms.map((homeroom) => homeroom.name)
	];
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
				row.homeroomName,
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
