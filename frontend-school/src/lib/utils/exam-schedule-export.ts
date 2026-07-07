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

export type ExamScheduleExportWorkbook = {
	report: WorksheetRow[];
	schedule: WorksheetObjectRow[];
	rooms: WorksheetObjectRow[];
	invigilators: WorksheetObjectRow[];
	workloads: WorksheetObjectRow[];
	readiness: WorksheetObjectRow[];
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

function dayTitle(day: ExamDayDetail): string {
	return safeText(day.label) || dateLabel(day.examDate);
}

function buildingRoomLabel(buildingName: string | null | undefined, roomName: string | null | undefined) {
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

function dateRange(workspace: ExamScheduleWorkspace): string {
	const days = sortedDays(workspace);
	const firstDay = days[0];
	const lastDay = days.at(-1);
	if (!firstDay || !lastDay) return '';
	if (firstDay.examDate === lastDay.examDate) return dateLabel(firstDay.examDate);
	return `${dateLabel(firstDay.examDate)} - ${dateLabel(lastDay.examDate)}`;
}

function reportRows(
	workspace: ExamScheduleWorkspace,
	invigilatorWorkspace: ExamInvigilatorWorkspace | null
): WorksheetRow[] {
	const days = sortedDays(workspace);
	const sessions = sortedSessions(workspace);
	const roomCount = days.reduce((count, day) => count + day.roomAssignments.length, 0);
	const invigilatorCount = invigilatorWorkspace?.staffWorkloads.length ?? 0;
	const rows: WorksheetRow[] = [
		['รายงานตารางสอบ'],
		['รอบสอบ', workspace.round.name],
		['สถานะ', workspace.round.status === 'published' ? 'เผยแพร่แล้ว' : 'ฉบับร่าง'],
		['ช่วงวันที่สอบ', dateRange(workspace)],
		[],
		['สรุปภาพรวม'],
		['วันสอบ', days.length],
		['รายการสอบที่จัดแล้ว', sessions.length],
		['รายการสอบที่ยังไม่จัด', workspace.unscheduledItems.length],
		['ห้องสอบ', roomCount],
		['กรรมการที่ถูกมอบหมาย', invigilatorCount],
		['ความพร้อม', readinessStatus(workspace.readiness)],
		[],
		['ตารางสอบรวม'],
		['วันสอบ', 'เวลา', 'ชั้นเรียน', 'วิชา', 'ห้องสอบ', 'กรรมการ']
	];

	for (const session of sessions) {
		rows.push([
			dateLabel(session.examDate),
			`${timeLabel(session.startsAt)}-${timeLabel(session.endsAt)}`,
			safeText(session.classroomName, '-'),
			subjectLabel(session),
			buildingRoomLabel(session.buildingName, session.roomName),
			sessionInvigilatorNames(session) || '-'
		]);
	}

	if (sessions.length === 0) {
		rows.push(['-', '-', '-', 'ยังไม่มีรายการสอบที่จัดเวลาแล้ว', '-', '-']);
	}

	rows.push([], ['ภาระงานกรรมการ'], ['กรรมการ', 'เวลารวม', 'จำนวนวัน', 'จำนวนห้อง']);
	for (const workload of invigilatorWorkspace?.staffWorkloads ?? []) {
		rows.push([
			workload.staffName,
			minutesLabel(workload.totalMinutes),
			workload.assignedDayCount,
			workload.assignmentCount
		]);
	}
	if ((invigilatorWorkspace?.staffWorkloads.length ?? 0) === 0) {
		rows.push(['ยังไม่มีกรรมการที่ถูกมอบหมาย', '-', '-', '-']);
	}

	rows.push([], ['รายการความพร้อม'], ['สถานะ', readinessStatus(workspace.readiness)]);
	for (const blocker of workspace.readiness.blockers) {
		rows.push(['ต้องแก้ไข', blocker]);
	}
	if (workspace.readiness.blockers.length === 0) {
		rows.push(['พร้อม', 'ไม่พบรายการที่ต้องแก้ไข']);
	}

	return rows;
}

function scheduleRows(
	workspace: ExamScheduleWorkspace,
	invigilatorWorkspace: ExamInvigilatorWorkspace | null
): WorksheetObjectRow[] {
	const assignments = assignmentByDayClassroom(invigilatorWorkspace);
	return sortedSessions(workspace).map((session) => {
		const assignment = assignments.get(assignmentKey(session.examDayId, session.classroomId));
		return {
			วันสอบ: dayTitle(workspace.days.find((day) => day.id === session.examDayId) ?? ({} as ExamDayDetail)),
			วันที่: dateLabel(session.examDate),
			เวลาเริ่ม: timeLabel(session.startsAt),
			เวลาจบ: timeLabel(session.endsAt),
			ระยะเวลา: minutesLabel(session.durationMinutes),
			ชั้นเรียน: safeText(session.classroomName, '-'),
			กลุ่มระดับ: safeText(session.gradeLevelName),
			วิชา: subjectLabel(session),
			รหัสวิชา: safeText(session.subjectCode),
			กลุ่มสาระ: safeText(session.subjectGroupName),
			ประเภทวิชา: safeText(session.subjectType),
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
				const assignment = assignments.get(
					assignmentKey(day.id, roomAssignment.classroomId)
				);
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
	return {
		report: reportRows(workspace, invigilatorWorkspace),
		schedule: scheduleRows(workspace, invigilatorWorkspace),
		rooms: roomRows(workspace, invigilatorWorkspace),
		invigilators: invigilatorRows(workspace, invigilatorWorkspace),
		workloads: workloadRows(workspace, invigilatorWorkspace),
		readiness: readinessRows(workspace)
	};
}

export function examScheduleExportFileName(roundName: string, exportedAt = new Date()): string {
	const datePart = exportedAt.toISOString().slice(0, 10);
	const safeRoundName = safeText(roundName, 'รอบสอบ').replace(/[\\/:*?"<>|]/g, '-');
	return `ตารางสอบ-${safeRoundName}-${datePart}.xlsx`;
}
