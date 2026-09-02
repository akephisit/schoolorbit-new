import type { TimetableBlock } from '../../api/timetable';

export type TimetableBlockDisplaySurface = 'scheduler' | 'personal';

export interface TimetableBlockDisplay {
	shared: boolean;
	contextLabel: string | null;
	teacherLabel: string | null;
	scopeLabel: string | null;
	groupLabel: string | null;
	roomLabel: string | null;
}

function unique(values: Array<string | null | undefined>): string[] {
	return values.filter(
		(value, index, all): value is string => Boolean(value) && all.indexOf(value) === index
	);
}

function compactNames(names: string[]): string | null {
	if (names.length === 0) return null;
	if (names.length <= 2) return names.join(', ');
	return `${names.slice(0, 2).join(', ')} +อีก ${names.length - 2} คน`;
}

function teacherNames(block: TimetableBlock): string[] {
	const teachers = [...block.groups.flatMap((group) => group.instructors), ...block.teachers];
	return teachers
		.filter(
			(teacher, index, all) =>
				all.findIndex((candidate) => candidate.teacherId === teacher.teacherId) === index
		)
		.map((teacher) => teacher.displayName);
}

function targetHomeroomCount(block: TimetableBlock): number {
	return new Set([
		...block.groups.flatMap((group) => group.homeroomIds),
		...block.homerooms.map((homeroom) => homeroom.homeroomId)
	]).size;
}

export function buildTimetableBlockDisplay(
	block: TimetableBlock,
	surface: TimetableBlockDisplaySurface
): TimetableBlockDisplay {
	const shared = block.blockKind === 'structural' || block.schedulingMode === 'synchronized';
	const contextLabel =
		block.blockKind === 'structural'
			? 'กิจกรรมรวม'
			: block.schedulingMode === 'synchronized'
				? 'กิจกรรมพร้อมกัน'
				: null;
	const groupNames = unique([
		...block.groups.map((group) => group.name),
		...block.homerooms.map((homeroom) => homeroom.name)
	]);
	const roomCodes = unique([
		...block.groups.map((group) => group.roomCode),
		...block.homerooms.map((homeroom) => homeroom.roomCode)
	]);
	const homeroomCount = targetHomeroomCount(block);

	return {
		shared,
		contextLabel,
		teacherLabel: compactNames(teacherNames(block)),
		scopeLabel:
			surface === 'scheduler' && shared && homeroomCount > 0 ? `${homeroomCount} ห้อง` : null,
		groupLabel: shared ? null : groupNames.join(', ') || null,
		roomLabel: shared ? null : roomCodes.join(', ') || null
	};
}
