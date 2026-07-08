export type TeacherLoadCategory = 'course' | 'independentActivity' | 'synchronizedActivity';

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
	title?: string | null;
	activity_slot_id?: string | null;
	activity_slot_name?: string | null;
	activity_scheduling_mode?: string | null;
	instructor_ids?: string[] | null;
	instructor_names?: string[] | null;
	instructor_name?: string | null;
}

export interface TeacherLoadSummaryRow {
	teacherId: string;
	teacherName: string;
	coursePeriods: number;
	independentActivityPeriods: number;
	synchronizedActivityPeriods: number;
	totalPeriods: number;
}

export interface TeacherLoadDetailRow {
	teacherId: string;
	teacherName: string;
	category: TeacherLoadCategory;
	categoryLabel: string;
	dayOfWeek: string;
	dayLabel: string;
	periodName: string;
	periodOrderIndex: number | null;
	timeLabel: string;
	classroomName: string;
	title: string;
}

export interface TeacherLoadExportRows {
	summaryRows: TeacherLoadSummaryRow[];
	detailRows: TeacherLoadDetailRow[];
	summarySheetRows: Array<Array<string | number>>;
	detailSheetRows: Array<Array<string | number>>;
}

const CATEGORY_LABELS: Record<TeacherLoadCategory, string> = {
	course: 'วิชา',
	independentActivity: 'กิจกรรม independent',
	synchronizedActivity: 'กิจกรรม synchronized'
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
			const detailKey = teacherLoadDetailKey(entry, category, teacherId);
			const existingDetail = details.get(detailKey);

			if (existingDetail) {
				appendUnique(existingDetail.classroomNames, entry.classroom_name ?? '');
				existingDetail.classroomName = existingDetail.classroomNames.join(', ');
				continue;
			}

			const summary = getOrCreateSummary(summaries, teacherId, teacherName);
			incrementSummary(summary, category);

			const classroomNames = uniqueNonEmpty([entry.classroom_name ?? '']);
			details.set(detailKey, {
				teacherId,
				teacherName,
				category,
				categoryLabel: CATEGORY_LABELS[category],
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
				row.coursePeriods + row.independentActivityPeriods + row.synchronizedActivityPeriods
		}))
		.sort(compareSummaryRows);

	const detailRows = Array.from(details.values())
		.map(({ classroomNames: _classroomNames, ...row }) => row)
		.sort(compareDetailRows);

	return {
		summaryRows,
		detailRows,
		summarySheetRows: buildSummarySheetRows(summaryRows),
		detailSheetRows: buildDetailSheetRows(detailRows)
	};
}

function getOrCreateSummary(
	summaries: Map<string, TeacherLoadSummaryRow>,
	teacherId: string,
	teacherName: string
): TeacherLoadSummaryRow {
	const existing = summaries.get(teacherId);
	if (existing) return existing;

	const row = {
		teacherId,
		teacherName,
		coursePeriods: 0,
		independentActivityPeriods: 0,
		synchronizedActivityPeriods: 0,
		totalPeriods: 0
	};
	summaries.set(teacherId, row);
	return row;
}

function incrementSummary(summary: TeacherLoadSummaryRow, category: TeacherLoadCategory) {
	if (category === 'course') summary.coursePeriods += 1;
	else if (category === 'independentActivity') summary.independentActivityPeriods += 1;
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

function buildSummarySheetRows(rows: TeacherLoadSummaryRow[]): Array<Array<string | number>> {
	return [
		[
			'ครูผู้สอน',
			'วิชา (คาบ)',
			'กิจกรรม independent (คาบ)',
			'กิจกรรม synchronized (คาบ)',
			'รวม (คาบ)'
		],
		...rows.map((row) => [
			row.teacherName,
			row.coursePeriods,
			row.independentActivityPeriods,
			row.synchronizedActivityPeriods,
			row.totalPeriods
		])
	];
}

function buildDetailSheetRows(rows: TeacherLoadDetailRow[]): Array<Array<string | number>> {
	return [
		['ครูผู้สอน', 'ประเภท', 'วัน', 'คาบ', 'เวลา', 'ห้อง', 'รายการ'],
		...rows.map((row) => [
			row.teacherName,
			row.categoryLabel,
			row.dayLabel,
			row.periodName,
			row.timeLabel,
			row.classroomName,
			row.title
		])
	];
}

function compareSummaryRows(a: TeacherLoadSummaryRow, b: TeacherLoadSummaryRow): number {
	return (
		b.totalPeriods - a.totalPeriods ||
		a.teacherName.localeCompare(b.teacherName, 'th') ||
		a.teacherId.localeCompare(b.teacherId)
	);
}

function compareDetailRows(a: TeacherLoadDetailRow, b: TeacherLoadDetailRow): number {
	return (
		a.teacherName.localeCompare(b.teacherName, 'th') ||
		(DAY_ORDER[a.dayOfWeek] ?? 99) - (DAY_ORDER[b.dayOfWeek] ?? 99) ||
		(a.periodOrderIndex ?? 999) - (b.periodOrderIndex ?? 999) ||
		a.categoryLabel.localeCompare(b.categoryLabel, 'th') ||
		a.title.localeCompare(b.title, 'th')
	);
}
