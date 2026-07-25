import type {
	StaffPublishedExamInvigilator,
	StaffPublishedExamRoomAssignment,
	StaffPublishedExamScheduleRound,
	StaffPublishedExamSession
} from '$lib/api/examSchedule';

export type StaffExamScheduleLevelFilter = 'all' | 'lower_secondary' | 'upper_secondary';

export interface StaffExamScheduleFilters {
	dayId: string;
	level: StaffExamScheduleLevelFilter;
	classroomId: string;
	query: string;
}

export interface StaffExamScheduleSessionRecord extends StaffPublishedExamSession {
	examDayId: string;
	examDate: string;
	dayLabel: string | null;
	invigilators: StaffPublishedExamInvigilator[];
}

export interface StaffExamRoomAssignmentRecord extends StaffPublishedExamRoomAssignment {
	examDayId: string;
	examDate: string;
	dayLabel: string | null;
	sessions: StaffPublishedExamSession[];
}

export interface StaffExamScheduleRenderRow {
	session: StaffExamScheduleSessionRecord;
	showDayCell: boolean;
	dayRowSpan: number;
	showTimeCell: boolean;
	timeRowSpan: number;
	dayGroupIndex: number;
}

export interface StaffExamInvigilatorRenderRow {
	assignment: StaffExamRoomAssignmentRecord;
	showDayCell: boolean;
	dayRowSpan: number;
	dayGroupIndex: number;
	isCurrentUser: boolean;
}

export interface MyExamInvigilationItem extends StaffExamRoomAssignmentRecord {
	status: 'upcoming' | 'completed';
}

export interface MyExamInvigilationSummary {
	items: MyExamInvigilationItem[];
	assignedDayCount: number;
	assignmentCount: number;
	totalMinutes: number;
}

export interface StaffExamRoundSummary {
	examDayCount: number;
	examRoomCount: number;
	invigilatorCount: number;
	nextPersonalAssignment: StaffExamRoomAssignmentRecord | null;
}

export interface StaffExamScheduleDayGroup {
	examDate: string;
	dayLabel: string | null;
	rows: StaffExamScheduleRenderRow[];
}

export interface StaffExamInvigilatorDayGroup {
	examDate: string;
	dayLabel: string | null;
	rows: StaffExamInvigilatorRenderRow[];
}

const thaiCollator = new Intl.Collator('th', {
	numeric: true,
	sensitivity: 'base'
});

const thaiDateFormatter = new Intl.DateTimeFormat('th-TH', {
	weekday: 'long',
	day: 'numeric',
	month: 'short',
	year: 'numeric'
});

function compareText(left: string, right: string): number {
	return thaiCollator.compare(left, right);
}

function compareSessions(
	left: StaffExamScheduleSessionRecord,
	right: StaffExamScheduleSessionRecord
): number {
	return (
		left.examDate.localeCompare(right.examDate) ||
		left.startsAt.localeCompare(right.startsAt) ||
		left.endsAt.localeCompare(right.endsAt) ||
		left.gradeLevelYear - right.gradeLevelYear ||
		compareText(left.classroomName, right.classroomName) ||
		compareText(left.subjectName, right.subjectName) ||
		compareText(left.assessmentCategoryName, right.assessmentCategoryName) ||
		left.sessionId.localeCompare(right.sessionId)
	);
}

function compareAssignments(
	left: StaffExamRoomAssignmentRecord,
	right: StaffExamRoomAssignmentRecord
): number {
	return (
		left.examDate.localeCompare(right.examDate) ||
		compareText(left.classroomName, right.classroomName) ||
		compareText(left.roomName, right.roomName) ||
		left.assignmentId.localeCompare(right.assignmentId)
	);
}

function normalizeSearch(value: string): string {
	return value.trim().toLocaleLowerCase('th-TH');
}

function searchableText(values: Array<string | null | undefined>): string {
	return normalizeSearch(values.filter(Boolean).join(' '));
}

function sessionMatchesLevel(
	session: StaffPublishedExamSession,
	level: StaffExamScheduleLevelFilter
): boolean {
	if (level === 'all') return true;
	if (session.gradeLevelType !== 'secondary') return false;
	if (level === 'lower_secondary') {
		return session.gradeLevelYear >= 1 && session.gradeLevelYear <= 3;
	}
	return session.gradeLevelYear >= 4 && session.gradeLevelYear <= 6;
}

function sessionSearchText(session: StaffExamScheduleSessionRecord): string {
	return searchableText([
		session.subjectName,
		session.subjectCode,
		session.assessmentCategoryName,
		session.gradeLevelName,
		session.classroomName,
		session.buildingName,
		session.roomName,
		...session.invigilators.map((invigilator) => invigilator.displayName)
	]);
}

function assignmentSearchText(assignment: StaffExamRoomAssignmentRecord): string {
	return searchableText([
		assignment.classroomName,
		assignment.buildingName,
		assignment.roomName,
		...assignment.invigilators.map((invigilator) => invigilator.displayName),
		...assignment.sessions.flatMap((session) => [
			session.subjectName,
			session.subjectCode,
			session.assessmentCategoryName,
			session.gradeLevelName
		])
	]);
}

function assignmentEndTimestamp(assignment: StaffExamRoomAssignmentRecord): number {
	const endTime = assignment.latestEndsAt ?? '23:59:59';
	const timestamp = new Date(`${assignment.examDate}T${endTime}`).getTime();
	return Number.isNaN(timestamp) ? Number.POSITIVE_INFINITY : timestamp;
}

export function flattenStaffExamScheduleRound(round: StaffPublishedExamScheduleRound): {
	sessions: StaffExamScheduleSessionRecord[];
	assignments: StaffExamRoomAssignmentRecord[];
} {
	const sessions: StaffExamScheduleSessionRecord[] = [];
	const assignments: StaffExamRoomAssignmentRecord[] = [];

	for (const day of round.days) {
		const assignmentsById = new Map(
			day.roomAssignments.map((assignment) => [assignment.assignmentId, assignment])
		);

		for (const session of day.sessions) {
			sessions.push({
				...session,
				examDayId: day.examDayId,
				examDate: day.examDate,
				dayLabel: day.label,
				invigilators:
					assignmentsById.get(session.dayRoomAssignmentId)?.invigilators.map((item) => ({
						...item
					})) ?? []
			});
		}

		for (const assignment of day.roomAssignments) {
			assignments.push({
				...assignment,
				invigilators: assignment.invigilators.map((item) => ({ ...item })),
				examDayId: day.examDayId,
				examDate: day.examDate,
				dayLabel: day.label,
				sessions: day.sessions
					.filter((session) => session.dayRoomAssignmentId === assignment.assignmentId)
					.map((session) => ({ ...session }))
			});
		}
	}

	sessions.sort(compareSessions);
	assignments.sort(compareAssignments);
	return { sessions, assignments };
}

export function filterStaffExamScheduleRound(
	round: StaffPublishedExamScheduleRound,
	filters: StaffExamScheduleFilters
): {
	sessions: StaffExamScheduleSessionRecord[];
	assignments: StaffExamRoomAssignmentRecord[];
} {
	const flattened = flattenStaffExamScheduleRound(round);
	const query = normalizeSearch(filters.query);
	const dayMatches = (examDayId: string) => filters.dayId === 'all' || examDayId === filters.dayId;
	const classroomMatches = (classroomId: string) =>
		filters.classroomId === 'all' || classroomId === filters.classroomId;

	const sessions = flattened.sessions.filter(
		(session) =>
			dayMatches(session.examDayId) &&
			classroomMatches(session.classroomId) &&
			sessionMatchesLevel(session, filters.level) &&
			(!query || sessionSearchText(session).includes(query))
	);

	const assignments = flattened.assignments.filter((assignment) => {
		const hasMatchingLevel =
			filters.level === 'all' ||
			assignment.sessions.some((session) => sessionMatchesLevel(session, filters.level));
		return (
			dayMatches(assignment.examDayId) &&
			classroomMatches(assignment.classroomId) &&
			hasMatchingLevel &&
			(!query || assignmentSearchText(assignment).includes(query))
		);
	});

	return { sessions, assignments };
}

export function buildStaffExamScheduleRenderRows(
	sessions: StaffExamScheduleSessionRecord[]
): StaffExamScheduleRenderRow[] {
	const sortedSessions = [...sessions].sort(compareSessions);
	const rows = sortedSessions.map<StaffExamScheduleRenderRow>((session) => ({
		session,
		showDayCell: false,
		dayRowSpan: 0,
		showTimeCell: false,
		timeRowSpan: 0,
		dayGroupIndex: 0
	}));

	let dayStart = 0;
	let dayGroupIndex = 0;
	while (dayStart < rows.length) {
		let dayEnd = dayStart + 1;
		while (
			dayEnd < rows.length &&
			rows[dayEnd].session.examDate === rows[dayStart].session.examDate
		) {
			dayEnd += 1;
		}

		rows[dayStart].showDayCell = true;
		rows[dayStart].dayRowSpan = dayEnd - dayStart;
		for (let index = dayStart; index < dayEnd; index += 1) {
			rows[index].dayGroupIndex = dayGroupIndex;
		}

		let timeStart = dayStart;
		while (timeStart < dayEnd) {
			let timeEnd = timeStart + 1;
			const timeKey = `${rows[timeStart].session.startsAt}|${rows[timeStart].session.endsAt}`;
			while (
				timeEnd < dayEnd &&
				`${rows[timeEnd].session.startsAt}|${rows[timeEnd].session.endsAt}` === timeKey
			) {
				timeEnd += 1;
			}
			rows[timeStart].showTimeCell = true;
			rows[timeStart].timeRowSpan = timeEnd - timeStart;
			timeStart = timeEnd;
		}

		dayStart = dayEnd;
		dayGroupIndex += 1;
	}

	return rows;
}

export function buildStaffExamInvigilatorRenderRows(
	assignments: StaffExamRoomAssignmentRecord[],
	currentStaffId: string
): StaffExamInvigilatorRenderRow[] {
	const sortedAssignments = [...assignments].sort(compareAssignments);
	const rows = sortedAssignments.map<StaffExamInvigilatorRenderRow>((assignment) => ({
		assignment,
		showDayCell: false,
		dayRowSpan: 0,
		dayGroupIndex: 0,
		isCurrentUser:
			currentStaffId.length > 0 &&
			assignment.invigilators.some((invigilator) => invigilator.staffId === currentStaffId)
	}));

	let dayStart = 0;
	let dayGroupIndex = 0;
	while (dayStart < rows.length) {
		let dayEnd = dayStart + 1;
		while (
			dayEnd < rows.length &&
			rows[dayEnd].assignment.examDate === rows[dayStart].assignment.examDate
		) {
			dayEnd += 1;
		}
		rows[dayStart].showDayCell = true;
		rows[dayStart].dayRowSpan = dayEnd - dayStart;
		for (let index = dayStart; index < dayEnd; index += 1) {
			rows[index].dayGroupIndex = dayGroupIndex;
		}
		dayStart = dayEnd;
		dayGroupIndex += 1;
	}

	return rows;
}

export function groupStaffScheduleRowsByDay(
	rows: StaffExamScheduleRenderRow[]
): StaffExamScheduleDayGroup[] {
	const groups: StaffExamScheduleDayGroup[] = [];
	for (const row of rows) {
		const current = groups.at(-1);
		if (!current || current.examDate !== row.session.examDate) {
			groups.push({
				examDate: row.session.examDate,
				dayLabel: row.session.dayLabel,
				rows: [row]
			});
		} else {
			current.rows.push(row);
		}
	}
	return groups;
}

export function groupStaffInvigilatorRowsByDay(
	rows: StaffExamInvigilatorRenderRow[]
): StaffExamInvigilatorDayGroup[] {
	const groups: StaffExamInvigilatorDayGroup[] = [];
	for (const row of rows) {
		const current = groups.at(-1);
		if (!current || current.examDate !== row.assignment.examDate) {
			groups.push({
				examDate: row.assignment.examDate,
				dayLabel: row.assignment.dayLabel,
				rows: [row]
			});
		} else {
			current.rows.push(row);
		}
	}
	return groups;
}

export function buildMyExamInvigilationSummary(
	assignments: StaffExamRoomAssignmentRecord[],
	currentStaffId: string,
	now: Date
): MyExamInvigilationSummary {
	const nowTimestamp = now.getTime();
	const items = assignments
		.filter(
			(assignment) =>
				currentStaffId.length > 0 &&
				assignment.invigilators.some((invigilator) => invigilator.staffId === currentStaffId)
		)
		.map<MyExamInvigilationItem>((assignment) => ({
			...assignment,
			status: assignmentEndTimestamp(assignment) >= nowTimestamp ? 'upcoming' : 'completed'
		}))
		.sort((left, right) => {
			if (left.status !== right.status) return left.status === 'upcoming' ? -1 : 1;
			const direction = left.status === 'upcoming' ? 1 : -1;
			return (
				(assignmentEndTimestamp(left) - assignmentEndTimestamp(right)) * direction ||
				compareAssignments(left, right) * direction
			);
		});

	return {
		items,
		assignedDayCount: new Set(items.map((item) => item.examDayId)).size,
		assignmentCount: items.length,
		totalMinutes: items.reduce((total, item) => total + item.sessionMinutes, 0)
	};
}

export function buildStaffExamRoundSummary(
	round: StaffPublishedExamScheduleRound,
	currentStaffId: string,
	now: Date
): StaffExamRoundSummary {
	const { assignments } = flattenStaffExamScheduleRound(round);
	const personalSummary = buildMyExamInvigilationSummary(assignments, currentStaffId, now);

	return {
		examDayCount: round.days.length,
		examRoomCount: new Set(assignments.map((assignment) => assignment.roomId)).size,
		invigilatorCount: new Set(
			assignments.flatMap((assignment) =>
				assignment.invigilators.map((invigilator) => invigilator.staffId)
			)
		).size,
		nextPersonalAssignment: personalSummary.items.find((item) => item.status === 'upcoming') ?? null
	};
}

export function formatStaffExamDate(value: string): string {
	if (!value) return '-';
	const date = new Date(`${value}T00:00:00`);
	return Number.isNaN(date.getTime()) ? value : thaiDateFormatter.format(date);
}

export function formatStaffExamTime(value: string): string {
	if (!value) return '-';
	const match = /^([01]\d|2[0-3]):([0-5]\d)(?::[0-5]\d(?:\.\d+)?)?$/.exec(value);
	return match ? `${match[1]}:${match[2]}` : value;
}

export function formatStaffExamMinutes(value: number): string {
	const safeMinutes = Math.max(0, Math.trunc(value));
	const hours = Math.floor(safeMinutes / 60);
	const minutes = safeMinutes % 60;
	if (hours === 0) return `${minutes} นาที`;
	if (minutes === 0) return `${hours} ชม.`;
	return `${hours} ชม. ${minutes} นาที`;
}
