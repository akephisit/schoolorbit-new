import type { AttendanceSession, AttendanceWorkspace } from '#lib/api/attendance.js';

export type LessonChoice = AttendanceWorkspace['lessonChoices'][number];
export const OTHER_LESSON_ROOM = 'other';

export function lessonRoomOptions(choices: LessonChoice[], sessions: AttendanceSession[]) {
	const rooms = new Map<string, string>();
	const visible = new Set(sessions.map((session) => session.id));
	for (const choice of choices) {
		if (!visible.has(choice.sessionId)) continue;
		for (const room of choice.homerooms) rooms.set(room.id, room.name);
	}
	const options = [...rooms]
		.map(([value, label]) => ({ value, label }))
		.sort((a, b) => a.label.localeCompare(b.label, 'th', { numeric: true }));
	if (
		sessions.some(
			(session) => !choices.find((choice) => choice.sessionId === session.id)?.homerooms.length
		)
	) {
		options.push({ value: OTHER_LESSON_ROOM, label: 'ห้องอื่น / กลุ่มอิสระ' });
	}
	return options;
}

export function matchesLessonRoom(
	session: AttendanceSession,
	choices: LessonChoice[],
	room: string
) {
	if (!room) return true;
	const homerooms = choices.find((choice) => choice.sessionId === session.id)?.homerooms ?? [];
	return room === OTHER_LESSON_ROOM
		? !homerooms.length
		: homerooms.some((entry) => entry.id === room);
}

export function roundOption(session: AttendanceSession, choices: LessonChoice[]) {
	const lesson =
		session.kind === 'lesson'
			? choices.find((choice) => choice.sessionId === session.id)
			: undefined;
	const cancelled = session.cancelled ? ' · งด' : '';
	if (!lesson)
		return {
			value: session.id,
			label: `${session.startTime.slice(0, 5)} · ${session.title}${cancelled}`
		};
	const suffix = ` · ${lesson.offeringName}`;
	const group = lesson.groupName.endsWith(suffix)
		? lesson.groupName.slice(0, -suffix.length)
		: lesson.groupName;
	return {
		value: session.id,
		label: `${lesson.periodName} · ${session.startTime.slice(0, 5)} · ${lesson.offeringName}${cancelled}`,
		description: group === lesson.offeringName ? undefined : group
	};
}
