export const TIMELINE_SLOT_MINUTES = 5;

export interface TimelineBlockedWindow {
	label?: string | null;
	startTime: string;
	endTime: string;
}

export interface TimelinePlacementInput {
	dayStartTime: string;
	dayEndTime: string;
	startTime: string;
	durationMinutes: number;
	blockedWindows?: TimelineBlockedWindow[];
}

export interface TimelinePlacementResult {
	ok: boolean;
	reason?: string;
}

export interface TimelineClientXInput {
	clientX: number;
	trackLeft: number;
	dragOffsetPx?: number;
	dayStartTime: string;
	slotWidthPx?: number;
	slotMinutes?: number;
}

export interface TimelineExamDay {
	id: string;
	startTime: string;
	endTime: string;
	gradeLevelIds?: string[];
	blockedWindows?: TimelineBlockedWindow[];
	roomAssignments?: TimelineRoomAssignment[];
}

export interface TimelineRoomAssignment {
	classroomId: string;
	roomId: string;
}

export interface TimelineSessionCandidate {
	examScheduleItemId: string;
	classroomId: string;
	gradeLevelId: string;
	startTime: string;
	durationMinutes: number;
	sourceSessionId?: string;
}

export interface TimelineScheduledSession {
	id: string;
	examDayId: string;
	classroomId: string;
	roomId?: string | null;
	startsAt: string;
	endsAt: string;
}

export interface ExamSessionPlacementInput {
	day: TimelineExamDay;
	candidate: TimelineSessionCandidate;
	scheduledSessions?: TimelineScheduledSession[];
}

export interface TimelineDragPreviewInput {
	day: TimelineExamDay;
	clientX: number;
	trackLeft: number;
	dragOffsetPx: number;
	slotWidthPx: number;
	durationMinutes: number;
	candidate: {
		examScheduleItemId: string;
		classroomId: string;
		gradeLevelId: string;
		sourceSessionId?: string;
	};
	scheduledSessions: TimelineScheduledSession[];
}

export interface TimelineDragPreview {
	startTime: string;
	endTime: string;
	leftPx: number;
	widthPx: number;
	valid: boolean;
	reason?: string;
}

export function timeToMinutes(value: string): number {
	const [hours = '0', minutes = '0'] = value.slice(0, 5).split(':');
	return Number(hours) * 60 + Number(minutes);
}

export function minutesToTime(value: number): string {
	const normalized = Math.max(0, value);
	const hours = Math.floor(normalized / 60);
	const minutes = normalized % 60;
	return `${String(hours).padStart(2, '0')}:${String(minutes).padStart(2, '0')}`;
}

export function addMinutes(start: string, durationMinutes: number): string {
	return minutesToTime(timeToMinutes(start) + durationMinutes);
}

export function minutesBetween(start: string, end: string): number {
	return timeToMinutes(end) - timeToMinutes(start);
}

export function rangesOverlap(
	leftStart: string,
	leftEnd: string,
	rightStart: string,
	rightEnd: string
): boolean {
	return (
		timeToMinutes(leftStart) < timeToMinutes(rightEnd) &&
		timeToMinutes(rightStart) < timeToMinutes(leftEnd)
	);
}

export function snapMinutesToSlot(value: number, slotMinutes = TIMELINE_SLOT_MINUTES): number {
	return Math.round(value / slotMinutes) * slotMinutes;
}

export function snapTimeToSlot(value: string, slotMinutes = TIMELINE_SLOT_MINUTES): string {
	return minutesToTime(snapMinutesToSlot(timeToMinutes(value), slotMinutes));
}

export function isTimeOnSlot(value: string, slotMinutes = TIMELINE_SLOT_MINUTES): boolean {
	return timeToMinutes(value) % slotMinutes === 0;
}

export function clientXToTimelineStartTime({
	clientX,
	trackLeft,
	dragOffsetPx = 0,
	dayStartTime,
	slotWidthPx = 24,
	slotMinutes = TIMELINE_SLOT_MINUTES
}: TimelineClientXInput): string {
	const relativePx = clientX - trackLeft - dragOffsetPx;
	const offsetMinutes = snapMinutesToSlot((relativePx / slotWidthPx) * slotMinutes, slotMinutes);
	return minutesToTime(timeToMinutes(dayStartTime) + offsetMinutes);
}

export function validateTimelinePlacement({
	dayStartTime,
	dayEndTime,
	startTime,
	durationMinutes,
	blockedWindows = []
}: TimelinePlacementInput): TimelinePlacementResult {
	const endTime = addMinutes(startTime, durationMinutes);
	const startMinutes = timeToMinutes(startTime);
	const endMinutes = timeToMinutes(endTime);

	if (durationMinutes <= 0) {
		return { ok: false, reason: 'Exam duration must be greater than zero.' };
	}

	if (!isTimeOnSlot(startTime)) {
		return { ok: false, reason: 'เวลาเริ่มต้องอยู่บนช่วงเวลา 5 นาที' };
	}

	if (startMinutes < timeToMinutes(dayStartTime) || endMinutes > timeToMinutes(dayEndTime)) {
		return { ok: false, reason: 'Placement is outside the exam day.' };
	}

	const blockedWindow = blockedWindows.find((window) =>
		rangesOverlap(startTime, endTime, window.startTime, window.endTime)
	);
	if (blockedWindow) {
		return {
			ok: false,
			reason: `Placement overlaps blocked unavailable time${blockedWindow.label ? `: ${blockedWindow.label}` : '.'}`
		};
	}

	return { ok: true };
}

export function validateExamSessionPlacement({
	day,
	candidate,
	scheduledSessions = []
}: ExamSessionPlacementInput): TimelinePlacementResult {
	const gradeLevelIds = day.gradeLevelIds ?? [];
	if (gradeLevelIds.length > 0 && !gradeLevelIds.includes(candidate.gradeLevelId)) {
		return { ok: false, reason: 'วันสอบนี้ไม่ได้เปิดสำหรับระดับชั้นของรายการสอบ' };
	}

	const candidateAssignment = (day.roomAssignments ?? []).find(
		(assignment) => assignment.classroomId === candidate.classroomId
	);
	if (!candidateAssignment) {
		return { ok: false, reason: 'ยังไม่ได้กำหนดห้องสอบสำหรับห้องเรียนนี้ในวันสอบที่เลือก' };
	}

	const placement = validateTimelinePlacement({
		dayStartTime: day.startTime,
		dayEndTime: day.endTime,
		startTime: candidate.startTime,
		durationMinutes: candidate.durationMinutes,
		blockedWindows: day.blockedWindows
	});
	if (!placement.ok) return placement;

	const candidateEndTime = addMinutes(candidate.startTime, candidate.durationMinutes);
	const classroomConflict = scheduledSessions.find(
		(session) =>
			session.examDayId === day.id &&
			session.classroomId === candidate.classroomId &&
			session.id !== candidate.sourceSessionId &&
			rangesOverlap(candidate.startTime, candidateEndTime, session.startsAt, session.endsAt)
	);
	if (classroomConflict) {
		return { ok: false, reason: 'ช่วงเวลานี้มีรายการสอบอื่นในห้องเรียนเดียวกันแล้ว' };
	}

	const roomConflict = scheduledSessions.find(
		(session) =>
			session.examDayId === day.id &&
			session.roomId === candidateAssignment.roomId &&
			session.id !== candidate.sourceSessionId &&
			rangesOverlap(candidate.startTime, candidateEndTime, session.startsAt, session.endsAt)
	);
	if (roomConflict) {
		return { ok: false, reason: 'ช่วงเวลานี้มีรายการสอบอื่นในห้องสอบเดียวกันแล้ว' };
	}

	return { ok: true };
}

export function buildTimelineDragPreview(input: TimelineDragPreviewInput): TimelineDragPreview {
	const startTime = clientXToTimelineStartTime({
		clientX: input.clientX,
		trackLeft: input.trackLeft,
		dragOffsetPx: input.dragOffsetPx,
		dayStartTime: input.day.startTime,
		slotWidthPx: input.slotWidthPx
	});
	const endTime = addMinutes(startTime, input.durationMinutes);
	const validation = validateExamSessionPlacement({
		day: input.day,
		candidate: {
			...input.candidate,
			startTime,
			durationMinutes: input.durationMinutes
		},
		scheduledSessions: input.scheduledSessions
	});

	return {
		startTime,
		endTime,
		leftPx:
			(minutesBetween(input.day.startTime, startTime) / TIMELINE_SLOT_MINUTES) * input.slotWidthPx,
		widthPx: Math.max(
			input.slotWidthPx,
			(input.durationMinutes / TIMELINE_SLOT_MINUTES) * input.slotWidthPx
		),
		valid: validation.ok,
		reason: validation.reason
	};
}
