import { timetableTeacherLabel } from './teacher-label.ts';
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

function compactNames(names: string[], surface: TimetableBlockDisplaySurface): string | null {
	if (names.length === 0) return null;
	if (surface === 'scheduler') {
		return names.length === 1 ? names[0] : `${names[0]} +${names.length - 1}`;
	}
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
		.map((teacher) => timetableTeacherLabel(teacher.displayName));
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
	const roomNames = unique([
		...block.groups.map(
			(group) => group.roomName?.trim() || (group.roomId ? 'ไม่พบชื่อห้อง' : null)
		),
		...block.homerooms.map(
			(homeroom) => homeroom.roomName?.trim() || (homeroom.roomId ? 'ไม่พบชื่อห้อง' : null)
		)
	]);
	const homeroomCount = targetHomeroomCount(block);

	return {
		shared,
		contextLabel,
		teacherLabel: compactNames(teacherNames(block), surface),
		scopeLabel:
			surface === 'scheduler' && shared && homeroomCount > 0 ? `${homeroomCount} ห้อง` : null,
		groupLabel: shared ? null : groupNames.join(', ') || null,
		roomLabel: shared ? null : roomNames.join(', ') || 'ไม่ระบุห้อง'
	};
}

export function buildSchedulerTargetLabel(
	block: TimetableBlock,
	view: 'homeroom' | 'learning_group' | 'teacher',
	homeroomNamesById: ReadonlyMap<string, string>
): string | null {
	if (block.blockKind === 'structural' || block.schedulingMode === 'synchronized') return null;
	if (view === 'homeroom') return null;
	if (view === 'teacher') {
		return (
			unique([
				...block.groups.flatMap((group) =>
					group.homeroomIds.map((homeroomId) => homeroomNamesById.get(homeroomId))
				),
				...block.homerooms.map((homeroom) => homeroom.name)
			]).join(', ') || null
		);
	}
	return unique(block.groups.map((group) => group.name)).join(', ') || null;
}

/** Staff self-service shows real classes only within the teacher's returned assignment scope. */
export function buildStaffOwnTimetableBlockDisplay(
	block: TimetableBlock,
	currentStaffId: string
): { classLabel: string | null; roomLabel: string | null } {
	if (block.blockKind === 'structural') return { classLabel: null, roomLabel: null };
	const synchronized = block.schedulingMode === 'synchronized';
	const groups = synchronized
		? block.groups.filter((group) =>
				group.instructors.some((teacher) => teacher.teacherId === currentStaffId)
			)
		: block.groups;
	const homerooms = synchronized ? [] : block.homerooms;
	if (synchronized && groups.length === 0) return { classLabel: null, roomLabel: null };
	const classNames = unique(
		[
			...groups.flatMap((group) => group.homeroomNames),
			...homerooms.map((homeroom) => homeroom.name)
		].map((name) => name.trim())
	);
	const roomNames = unique([
		...groups.map((group) => group.roomName?.trim() || (group.roomId ? 'ไม่พบชื่อห้อง' : null)),
		...homerooms.map(
			(homeroom) => homeroom.roomName?.trim() || (homeroom.roomId ? 'ไม่พบชื่อห้อง' : null)
		)
	]);
	return {
		classLabel: classNames.join(', ') || null,
		roomLabel: roomNames.join(', ') || 'ไม่ระบุห้อง'
	};
}
