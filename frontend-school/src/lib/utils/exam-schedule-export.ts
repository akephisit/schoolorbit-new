import type {
	ExamDayDetail,
	ExamInvigilatorAssignmentSummary,
	ExamInvigilatorWorkspace,
	ExamScheduleReadiness,
	ExamScheduleWorkspace,
	ExamSession
} from '$lib/api/examSchedule';

type WorksheetCell = string | number;
type WorksheetRow = WorksheetCell[];
type WorksheetObjectRow = Record<string, WorksheetCell>;

export type ExamScheduleExportCellAddress = {
	r: number;
	c: number;
};

export type ExamScheduleExportMerge = {
	s: ExamScheduleExportCellAddress;
	e: ExamScheduleExportCellAddress;
};

export type ExamScheduleExportColumn = {
	wch: number;
};

export type ExamScheduleExportSheet<
	Row extends WorksheetRow | WorksheetObjectRow = WorksheetRow | WorksheetObjectRow
> = {
	name?: string;
	rows: Row[];
	'!cols'?: ExamScheduleExportColumn[];
	'!merges'?: ExamScheduleExportMerge[];
};

export type ExamScheduleReportSheet = ExamScheduleExportSheet<WorksheetRow> & {
	name: string;
};

export type ExamScheduleExportWorkbook = {
	report: ExamScheduleReportSheet;
	reportSheets: ExamScheduleReportSheet[];
	lowerSecondaryReport?: ExamScheduleReportSheet;
	upperSecondaryReport?: ExamScheduleReportSheet;
	schedule: ExamScheduleExportSheet<WorksheetObjectRow>;
	rooms: ExamScheduleExportSheet<WorksheetObjectRow>;
	invigilators: ExamScheduleExportSheet<WorksheetObjectRow>;
	workloads: ExamScheduleExportSheet<WorksheetObjectRow>;
	readiness: ExamScheduleExportSheet<WorksheetObjectRow>;
};

const thaiNaturalCollator = new Intl.Collator('th', {
	numeric: true,
	sensitivity: 'base'
});

function compareThaiNatural(left: string, right: string): number {
	return thaiNaturalCollator.compare(left, right);
}

function safeText(value: string | null | undefined, fallback = ''): string {
	return value?.trim() || fallback;
}

function subjectLabel(session: ExamSession): string {
	return (
		safeText(session.subjectNameTh) ||
		safeText(session.subjectNameEn) ||
		safeText(session.subjectCode) ||
		'ไม่ระบุวิชา'
	);
}

function dateLabel(value: string | null | undefined): string {
	if (!value) return '';
	return new Date(value).toLocaleDateString('th-TH', {
		weekday: 'short',
		year: 'numeric',
		month: 'short',
		day: 'numeric'
	});
}

function timeLabel(value: string | null | undefined): string {
	return value?.slice(0, 5) ?? '';
}

function minutesLabel(minutes: number): string {
	const hours = Math.floor(minutes / 60);
	const remainder = minutes % 60;
	if (hours === 0) return `${remainder} นาที`;
	if (remainder === 0) return `${hours} ชม.`;
	return `${hours} ชม. ${remainder} นาที`;
}

function subjectTypeLabel(type: string | null | undefined): string {
	const normalized = safeText(type).toUpperCase();
	if (normalized === 'BASIC') return 'พื้นฐาน';
	if (normalized === 'ADDITIONAL') return 'เพิ่มเติม';
	return safeText(type);
}

function dayTitle(day: ExamDayDetail): string {
	return safeText(day.label) || dateLabel(day.examDate);
}

function buildingRoomLabel(
	buildingName: string | null | undefined,
	roomName: string | null | undefined
) {
	const room = safeText(roomName, '-');
	const building = safeText(buildingName);
	return building ? `${building} / ${room}` : room;
}

function sessionInvigilatorNames(session: ExamSession): string {
	return session.invigilators
		.map((invigilator) => safeText(invigilator.staffName))
		.filter(Boolean)
		.join(', ');
}

function assignmentInvigilatorNames(
	assignment: ExamInvigilatorAssignmentSummary | undefined
): string {
	return (
		assignment?.invigilators
			.map((invigilator) => safeText(invigilator.displayName))
			.filter(Boolean)
			.join(', ') ?? ''
	);
}

function sortedDays(workspace: ExamScheduleWorkspace): ExamDayDetail[] {
	return [...workspace.days].sort((a, b) => {
		const dateCompare = a.examDate.localeCompare(b.examDate);
		if (dateCompare !== 0) return dateCompare;
		return timeLabel(a.startTime).localeCompare(timeLabel(b.startTime));
	});
}

function sortedSessions(workspace: ExamScheduleWorkspace): ExamSession[] {
	const dayOrder = new Map(sortedDays(workspace).map((day, index) => [day.id, index]));
	return [...workspace.scheduledSessions].sort((a, b) => {
		const dayCompare = (dayOrder.get(a.examDayId) ?? 0) - (dayOrder.get(b.examDayId) ?? 0);
		if (dayCompare !== 0) return dayCompare;
		const timeCompare = timeLabel(a.startsAt).localeCompare(timeLabel(b.startsAt));
		if (timeCompare !== 0) return timeCompare;
		return compareThaiNatural(safeText(a.classroomName), safeText(b.classroomName));
	});
}

function groupByText<T>(items: T[], keyForItem: (item: T) => string): [string, T[]][] {
	const groups = new Map<string, T[]>();
	for (const item of items) {
		const key = keyForItem(item);
		const group = groups.get(key);
		if (group) {
			group.push(item);
		} else {
			groups.set(key, [item]);
		}
	}
	return Array.from(groups.entries());
}

type ParsedClassroomLabel = {
	gradeLabel: string;
	roomNumber: number | null;
	rawLabel: string;
};

function numericTextValue(value: string): number | null {
	const normalized = value.replace(/[๐-๙]/g, (digit) => String('๐๑๒๓๔๕๖๗๘๙'.indexOf(digit)));
	const parsed = Number.parseInt(normalized, 10);
	return Number.isNaN(parsed) ? null : parsed;
}

function parseClassroomLabel(value: string | null | undefined): ParsedClassroomLabel | null {
	const rawLabel = safeText(value);
	if (!rawLabel) return null;
	const match = rawLabel.match(/^(.+?[\d๐-๙]+)\/([\d๐-๙]+)$/u);
	if (!match) {
		return {
			gradeLabel: rawLabel,
			roomNumber: null,
			rawLabel
		};
	}

	return {
		gradeLabel: match[1].trim(),
		roomNumber: numericTextValue(match[2]),
		rawLabel
	};
}

function sessionGradeLabel(session: ExamSession): string {
	return (
		safeText(session.gradeLevelName) || safeText(session.classroomName).split('/')[0]?.trim() || '-'
	);
}

function sessionGradeYear(session: ExamSession): number | null {
	if (typeof session.gradeLevelYear === 'number') return session.gradeLevelYear;
	const parsedClassroom = parseClassroomLabel(session.classroomName);
	if (!parsedClassroom) return null;
	const gradeMatch = parsedClassroom.gradeLabel.match(/([\d๐-๙]+)$/u);
	return gradeMatch ? numericTextValue(gradeMatch[1]) : null;
}

function isLowerSecondarySession(session: ExamSession): boolean {
	const year = sessionGradeYear(session);
	return year !== null && year >= 1 && year <= 3 && sessionGradeLabel(session).includes('ม.');
}

function isUpperSecondarySession(session: ExamSession): boolean {
	const year = sessionGradeYear(session);
	return year !== null && year >= 4 && year <= 6 && sessionGradeLabel(session).includes('ม.');
}

function compactNumberRanges(values: number[]): string {
	const uniqueValues = Array.from(new Set(values)).sort((a, b) => a - b);
	const ranges: string[] = [];
	let rangeStart: number | null = null;
	let previous: number | null = null;

	for (const value of uniqueValues) {
		if (rangeStart === null || previous === null) {
			rangeStart = value;
			previous = value;
			continue;
		}

		if (value === previous + 1) {
			previous = value;
			continue;
		}

		ranges.push(rangeStart === previous ? String(rangeStart) : `${rangeStart}-${previous}`);
		rangeStart = value;
		previous = value;
	}

	if (rangeStart !== null && previous !== null) {
		ranges.push(rangeStart === previous ? String(rangeStart) : `${rangeStart}-${previous}`);
	}

	return ranges.join(',');
}

function compactClassroomLabels(classroomLabels: string[]): string {
	const labels = Array.from(
		new Set(classroomLabels.map((label) => safeText(label)).filter(Boolean))
	);
	const parsedLabels = labels
		.map(parseClassroomLabel)
		.filter((label): label is ParsedClassroomLabel => !!label);

	if (
		parsedLabels.length !== labels.length ||
		parsedLabels.some((label) => label.roomNumber === null)
	) {
		return labels.sort(compareThaiNatural).join(', ');
	}

	const gradeLabels = Array.from(new Set(parsedLabels.map((label) => label.gradeLabel)));
	if (gradeLabels.length !== 1) {
		return labels.sort(compareThaiNatural).join(', ');
	}

	const roomNumbers = parsedLabels
		.map((label) => label.roomNumber)
		.filter((roomNumber): roomNumber is number => roomNumber !== null);
	return `${gradeLabels[0]}/${compactNumberRanges(roomNumbers)}`;
}

function reportClassroomLabel(workspace: ExamScheduleWorkspace, sessions: ExamSession[]): string {
	const firstSession = sessions[0];
	if (!firstSession) return '-';

	const gradeLabel = sessionGradeLabel(firstSession);
	const day = workspace.days.find((item) => item.id === firstSession.examDayId);
	const assignedClassrooms =
		day?.roomAssignments.filter((assignment) => {
			const parsed = parseClassroomLabel(assignment.classroomName);
			return parsed?.gradeLabel === gradeLabel;
		}) ?? [];
	const scheduledClassroomIds = new Set(sessions.map((session) => session.classroomId));

	if (
		assignedClassrooms.length > 0 &&
		assignedClassrooms.every((assignment) => scheduledClassroomIds.has(assignment.classroomId))
	) {
		return gradeLabel;
	}

	const classroomLabels = sessions
		.map((session) => safeText(session.classroomName) || gradeLabel)
		.filter(Boolean);
	return compactClassroomLabels(classroomLabels);
}

function printableReportTitle(workspace: ExamScheduleWorkspace): string {
	const roundName = safeText(workspace.round.name, 'ตารางสอบ');
	return roundName.includes('ตารางสอบ') ? roundName : `ตารางสอบ${roundName}`;
}

function printableReportSubtitle(sessions: ExamSession[], fallback = 'ระดับชั้นที่จัดสอบ'): string {
	const gradeLabels = Array.from(
		new Set(sessions.map(sessionGradeLabel).filter((label) => label !== '-'))
	).sort(compareThaiNatural);

	if (gradeLabels.length === 0) return fallback;
	if (gradeLabels.length === 1) return `ระดับชั้น${gradeLabels[0]}`;

	const firstLabel = gradeLabels[0];
	const lastLabel = gradeLabels.at(-1) ?? firstLabel;
	const prefix = firstLabel.match(/^[^\d๐-๙]+/)?.[0]?.trim();
	const samePrefix = prefix ? gradeLabels.every((label) => label.startsWith(prefix)) : false;
	const secondaryYears = sessions
		.map((session) => session.gradeLevelYear)
		.filter((year): year is number => typeof year === 'number');
	const minSecondaryYear = secondaryYears.length ? Math.min(...secondaryYears) : null;
	const maxSecondaryYear = secondaryYears.length ? Math.max(...secondaryYears) : null;
	const levelGroup =
		samePrefix && prefix?.includes('ม.') && maxSecondaryYear !== null && maxSecondaryYear <= 3
			? 'ระดับชั้นมัธยมศึกษาตอนต้น'
			: samePrefix && prefix?.includes('ม.') && minSecondaryYear !== null && minSecondaryYear >= 4
				? 'ระดับชั้นมัธยมศึกษาตอนปลาย'
				: samePrefix && prefix?.includes('ม.')
					? 'ระดับชั้นมัธยมศึกษา'
					: samePrefix && prefix?.includes('ป.')
						? 'ระดับชั้นประถมศึกษา'
						: 'ระดับชั้น';

	return `${levelGroup} (${firstLabel} - ${lastLabel})`;
}

function assignmentKey(examDayId: string, classroomId: string): string {
	return `${examDayId}:${classroomId}`;
}

function assignmentByDayClassroom(invigilatorWorkspace: ExamInvigilatorWorkspace | null) {
	return new Map(
		(invigilatorWorkspace?.assignments ?? []).map((assignment) => [
			assignmentKey(assignment.examDayId, assignment.classroomId),
			assignment
		])
	);
}

function readinessStatus(readiness: ExamScheduleReadiness): string {
	return readiness.canPublish ? 'พร้อมเผยแพร่' : 'ยังไม่พร้อมเผยแพร่';
}

function printableTimeLabel(value: string | null | undefined): string {
	return timeLabel(value).replace(':', '.');
}

function printableTimeRangeLabel(session: ExamSession): string {
	return `${printableTimeLabel(session.startsAt)}-${printableTimeLabel(session.endsAt)} น.`;
}

function sameSlotKey(session: ExamSession): string {
	return [
		safeText(session.examDate),
		timeLabel(session.startsAt),
		timeLabel(session.endsAt),
		session.durationMinutes
	].join(':');
}

function reportSessionGroupKey(session: ExamSession): string {
	return [
		sameSlotKey(session),
		safeText(session.subjectId),
		safeText(session.subjectCode),
		subjectLabel(session),
		safeText(session.gradeLevelId),
		sessionGradeLabel(session)
	].join(':');
}

function reportSheetColumns(): ExamScheduleExportColumn[] {
	return [{ wch: 15 }, { wch: 18 }, { wch: 11 }, { wch: 34 }, { wch: 14 }, { wch: 12 }];
}

function reportSheetMerges(
	dayRanges: { start: number; end: number }[],
	slotRanges: { start: number; end: number }[]
): ExamScheduleExportMerge[] {
	const merges: ExamScheduleExportMerge[] = [
		{ s: { r: 0, c: 0 }, e: { r: 0, c: 5 } },
		{ s: { r: 1, c: 0 }, e: { r: 1, c: 5 } }
	];

	for (const range of dayRanges) {
		if (range.end > range.start) {
			merges.push({ s: { r: range.start, c: 0 }, e: { r: range.end, c: 0 } });
		}
	}

	for (const range of slotRanges) {
		if (range.end > range.start) {
			merges.push({ s: { r: range.start, c: 1 }, e: { r: range.end, c: 1 } });
			merges.push({ s: { r: range.start, c: 2 }, e: { r: range.end, c: 2 } });
		}
	}

	return merges;
}

function printableReportRows(
	workspace: ExamScheduleWorkspace,
	sessions: ExamSession[],
	fallbackSubtitle = 'ระดับชั้นที่จัดสอบ'
): {
	rows: WorksheetRow[];
	dayRanges: { start: number; end: number }[];
	slotRanges: { start: number; end: number }[];
} {
	const rows: WorksheetRow[] = [
		[printableReportTitle(workspace)],
		[printableReportSubtitle(sessions, fallbackSubtitle)],
		[],
		['วันเดือนปี', 'เวลา', 'เวลาสอบ', 'วิชา', 'รหัสวิชา', 'ชั้น']
	];
	const dayRanges: { start: number; end: number }[] = [];
	const slotRanges: { start: number; end: number }[] = [];

	if (sessions.length === 0) {
		rows.push(['-', '-', '-', 'ยังไม่มีรายการสอบที่จัดเวลาแล้ว', '-', '-']);
		return { rows, dayRanges, slotRanges };
	}

	const sessionsByDate = groupByText(sessions, (session) => safeText(session.examDate));
	for (const [, dateSessions] of sessionsByDate) {
		const dayStart = rows.length;
		const slotGroups = groupByText(dateSessions, sameSlotKey);

		for (const [, slotSessions] of slotGroups) {
			const slotStart = rows.length;
			const reportGroups = groupByText(slotSessions, reportSessionGroupKey)
				.map(([, groupSessions]) => groupSessions)
				.sort(
					(a, b) =>
						compareThaiNatural(subjectLabel(a[0]), subjectLabel(b[0])) ||
						compareThaiNatural(sessionGradeLabel(a[0]), sessionGradeLabel(b[0])) ||
						compareThaiNatural(
							reportClassroomLabel(workspace, a),
							reportClassroomLabel(workspace, b)
						)
				);

			for (const groupSessions of reportGroups) {
				const session = groupSessions[0];
				rows.push([
					dateLabel(session.examDate),
					printableTimeRangeLabel(session),
					minutesLabel(session.durationMinutes),
					subjectLabel(session),
					safeText(session.subjectCode, '-'),
					reportClassroomLabel(workspace, groupSessions)
				]);
			}

			slotRanges.push({ start: slotStart, end: rows.length - 1 });
		}

		dayRanges.push({ start: dayStart, end: rows.length - 1 });
	}

	return { rows, dayRanges, slotRanges };
}

function printableReportSheet(
	workspace: ExamScheduleWorkspace,
	name: string,
	sessions: ExamSession[],
	fallbackSubtitle = 'ระดับชั้นที่จัดสอบ'
): ExamScheduleReportSheet {
	const report = printableReportRows(workspace, sessions, fallbackSubtitle);
	return {
		name,
		rows: report.rows,
		'!cols': reportSheetColumns(),
		'!merges': reportSheetMerges(report.dayRanges, report.slotRanges)
	};
}

function objectSheet<Row extends WorksheetObjectRow>(
	rows: Row[],
	columns: ExamScheduleExportColumn[]
): ExamScheduleExportSheet<Row> {
	return {
		rows,
		'!cols': columns
	};
}

function scheduleRows(
	workspace: ExamScheduleWorkspace,
	invigilatorWorkspace: ExamInvigilatorWorkspace | null
): WorksheetObjectRow[] {
	const assignments = assignmentByDayClassroom(invigilatorWorkspace);
	return sortedSessions(workspace).map((session) => {
		const assignment = assignments.get(assignmentKey(session.examDayId, session.classroomId));
		return {
			วันสอบ: dayTitle(
				workspace.days.find((day) => day.id === session.examDayId) ?? ({} as ExamDayDetail)
			),
			วันที่: dateLabel(session.examDate),
			เวลาเริ่ม: timeLabel(session.startsAt),
			เวลาจบ: timeLabel(session.endsAt),
			ระยะเวลา: minutesLabel(session.durationMinutes),
			ชั้นเรียน: safeText(session.classroomName, '-'),
			กลุ่มระดับ: safeText(session.gradeLevelName),
			วิชา: subjectLabel(session),
			รหัสวิชา: safeText(session.subjectCode),
			กลุ่มสาระ: safeText(session.subjectGroupName),
			ประเภทวิชา: subjectTypeLabel(session.subjectType),
			ห้องสอบ: safeText(session.roomName, '-'),
			'อาคาร/ห้อง': buildingRoomLabel(session.buildingName, session.roomName),
			กรรมการ: assignmentInvigilatorNames(assignment) || sessionInvigilatorNames(session)
		};
	});
}

function roomRows(
	workspace: ExamScheduleWorkspace,
	invigilatorWorkspace: ExamInvigilatorWorkspace | null
): WorksheetObjectRow[] {
	const assignments = assignmentByDayClassroom(invigilatorWorkspace);
	return sortedDays(workspace).flatMap((day) =>
		[...day.roomAssignments]
			.sort((a, b) => compareThaiNatural(safeText(a.classroomName), safeText(b.classroomName)))
			.map((roomAssignment) => {
				const assignment = assignments.get(assignmentKey(day.id, roomAssignment.classroomId));
				const effectiveCapacity =
					roomAssignment.capacityOverride ?? roomAssignment.roomCapacity ?? '';
				return {
					วันสอบ: dayTitle(day),
					วันที่: dateLabel(day.examDate),
					ห้องเรียน: safeText(roomAssignment.classroomName, '-'),
					ห้องสอบ: safeText(roomAssignment.roomName, '-'),
					อาคาร: '',
					ความจุห้อง: roomAssignment.roomCapacity ?? '',
					ความจุที่ใช้: effectiveCapacity,
					จำนวนนักเรียน: '',
					สร้างเลขที่นั่งแล้ว: '',
					จำนวนกรรมการ: assignment?.invigilators.length ?? roomAssignment.invigilators.length
				};
			})
	);
}

function invigilatorRows(
	workspace: ExamScheduleWorkspace,
	invigilatorWorkspace: ExamInvigilatorWorkspace | null
): WorksheetObjectRow[] {
	const dayById = new Map(workspace.days.map((day) => [day.id, day]));
	return (invigilatorWorkspace?.assignments ?? []).flatMap((assignment) => {
		const day = dayById.get(assignment.examDayId);
		if (assignment.invigilators.length === 0) {
			return [
				{
					วันสอบ: day ? dayTitle(day) : '',
					วันที่: day ? dateLabel(day.examDate) : '',
					ห้องเรียน: assignment.classroomName,
					ห้องสอบ: assignment.roomName,
					ชื่อกรรมการ: '',
					บทบาท: '',
					เวลาสอบรวมของห้อง: minutesLabel(assignment.sessionMinutes)
				}
			];
		}

		return assignment.invigilators.map((invigilator) => ({
			วันสอบ: day ? dayTitle(day) : '',
			วันที่: day ? dateLabel(day.examDate) : '',
			ห้องเรียน: assignment.classroomName,
			ห้องสอบ: assignment.roomName,
			ชื่อกรรมการ: invigilator.displayName,
			บทบาท: '',
			เวลาสอบรวมของห้อง: minutesLabel(assignment.sessionMinutes)
		}));
	});
}

function workloadRows(
	workspace: ExamScheduleWorkspace,
	invigilatorWorkspace: ExamInvigilatorWorkspace | null
): WorksheetObjectRow[] {
	const dayById = new Map(workspace.days.map((day) => [day.id, day]));
	return (invigilatorWorkspace?.staffWorkloads ?? []).map((workload) => {
		const dayDetails = workload.days
			.map((dayWorkload) => {
				const day = dayById.get(dayWorkload.examDayId);
				const label = day ? dayTitle(day) : dayWorkload.examDayId;
				return `${label}: ${minutesLabel(dayWorkload.minutes)} / ${dayWorkload.assignmentCount} ห้อง`;
			})
			.join('\n');
		return {
			ชื่อกรรมการ: workload.staffName,
			ชั่วโมงรวม: minutesLabel(workload.totalMinutes),
			นาทีรวม: workload.totalMinutes,
			จำนวนวัน: workload.assignedDayCount,
			จำนวนห้อง: workload.assignmentCount,
			รายละเอียดรายวัน: dayDetails
		};
	});
}

function readinessRows(workspace: ExamScheduleWorkspace): WorksheetObjectRow[] {
	const rows: WorksheetObjectRow[] = [
		{
			ประเภท: 'สถานะ',
			รายการ: 'ความพร้อมเผยแพร่',
			'สถานะ/รายละเอียด': readinessStatus(workspace.readiness)
		},
		{
			ประเภท: 'สถานะ',
			รายการ: 'สถานะรอบสอบ',
			'สถานะ/รายละเอียด': workspace.round.status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง'
		},
		{
			ประเภท: 'สรุป',
			รายการ: 'รายการสอบที่ยังไม่จัด',
			'สถานะ/รายละเอียด': workspace.unscheduledItems.length
		}
	];

	for (const blocker of workspace.readiness.blockers) {
		rows.push({
			ประเภท: 'ต้องแก้ไข',
			รายการ: blocker,
			'สถานะ/รายละเอียด': 'ยังไม่ผ่าน'
		});
	}

	if (workspace.readiness.blockers.length === 0) {
		rows.push({
			ประเภท: 'พร้อม',
			รายการ: 'ไม่พบรายการที่ต้องแก้ไข',
			'สถานะ/รายละเอียด': 'ผ่าน'
		});
	}

	return rows;
}

export function buildExamScheduleExportWorkbook(
	workspace: ExamScheduleWorkspace,
	invigilatorWorkspace: ExamInvigilatorWorkspace | null
): ExamScheduleExportWorkbook {
	const allSessions = sortedSessions(workspace);
	const report = printableReportSheet(workspace, 'รายงาน', allSessions);
	const lowerSecondaryReport = printableReportSheet(
		workspace,
		'ม.ต้น',
		allSessions.filter(isLowerSecondarySession),
		'ระดับชั้นมัธยมศึกษาตอนต้น'
	);
	const upperSecondaryReport = printableReportSheet(
		workspace,
		'ม.ปลาย',
		allSessions.filter(isUpperSecondarySession),
		'ระดับชั้นมัธยมศึกษาตอนปลาย'
	);

	return {
		report,
		reportSheets: [report, lowerSecondaryReport, upperSecondaryReport],
		lowerSecondaryReport,
		upperSecondaryReport,
		schedule: objectSheet(scheduleRows(workspace, invigilatorWorkspace), [
			{ wch: 18 },
			{ wch: 14 },
			{ wch: 10 },
			{ wch: 10 },
			{ wch: 12 },
			{ wch: 14 },
			{ wch: 14 },
			{ wch: 34 },
			{ wch: 14 },
			{ wch: 22 },
			{ wch: 12 },
			{ wch: 14 },
			{ wch: 24 },
			{ wch: 34 }
		]),
		rooms: objectSheet(roomRows(workspace, invigilatorWorkspace), [
			{ wch: 18 },
			{ wch: 14 },
			{ wch: 14 },
			{ wch: 14 },
			{ wch: 16 },
			{ wch: 12 },
			{ wch: 12 },
			{ wch: 14 },
			{ wch: 16 },
			{ wch: 12 }
		]),
		invigilators: objectSheet(invigilatorRows(workspace, invigilatorWorkspace), [
			{ wch: 18 },
			{ wch: 14 },
			{ wch: 14 },
			{ wch: 14 },
			{ wch: 28 },
			{ wch: 12 },
			{ wch: 18 }
		]),
		workloads: objectSheet(workloadRows(workspace, invigilatorWorkspace), [
			{ wch: 28 },
			{ wch: 14 },
			{ wch: 10 },
			{ wch: 10 },
			{ wch: 10 },
			{ wch: 48 }
		]),
		readiness: objectSheet(readinessRows(workspace), [{ wch: 14 }, { wch: 40 }, { wch: 28 }])
	};
}

export function examScheduleExportFileName(roundName: string, exportedAt = new Date()): string {
	const datePart = exportedAt.toISOString().slice(0, 10);
	const safeRoundName = safeText(roundName, 'รอบสอบ').replace(/[\\/:*?"<>|]/g, '-');
	return `ตารางสอบ-${safeRoundName}-${datePart}.xlsx`;
}
