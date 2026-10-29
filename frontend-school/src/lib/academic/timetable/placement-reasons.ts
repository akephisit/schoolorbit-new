import type { TimetableBlock } from '../../api/timetable';
import type {
	LocalTimetableConflict,
	TimetableBoardState,
	TimetableBoardView
} from './board-state.ts';
import { timetableTeacherLabel } from './teacher-label.ts';

export interface TimetablePlacementReasons {
	primary: string;
	lesson: string | null;
	context: string | null;
	details: string[];
}

interface PlacementReasonEntry {
	key: string;
	primary: string;
	lesson: string | null;
	context: string | null;
	causes: string[];
}

function unique(values: Array<string | null | undefined>): string[] {
	return [...new Set(values.filter((value): value is string => Boolean(value?.trim())))];
}

function blockIdentity(block: TimetableBlock): string {
	const title = block.offeringName || block.title || block.groups[0]?.name || 'คาบเรียน';
	return block.blockKind === 'structural'
		? `กิจกรรมรวม: ${title}`
		: block.blockKind === 'activity'
			? `กิจกรรม: ${title}`
			: `วิชา: ${unique([block.offeringCode, title]).join(' ')}`;
}

/** Resolve only references already returned in this workspace's authorized scope. */
export function timetablePlacementReasons(
	state: TimetableBoardState,
	view: TimetableBoardView,
	conflicts: readonly LocalTimetableConflict[]
): TimetablePlacementReasons {
	const priority =
		view === 'teacher'
			? ['homeroom', 'learning_group', 'teacher', 'room']
			: ['teacher', 'homeroom', 'learning_group', 'room'];
	const seen = new Set<string>();
	const entries = [...conflicts]
		.sort((a, b) => {
			const rank = (kind: string) =>
				priority.includes(kind) ? priority.indexOf(kind) : priority.length;
			return rank(a.conflictType) - rank(b.conflictType);
		})
		.flatMap((conflict): PlacementReasonEntry[] => {
			const key = [
				conflict.conflictType,
				conflict.existingBlockId,
				conflict.targetId,
				conflict.code.includes('SWAP_RETURN') ? 'return' : 'forward'
			].join(':');
			if (seen.has(key)) return [];
			seen.add(key);
			const block = conflict.existingBlockId
				? state.blocksById.get(conflict.existingBlockId)
				: undefined;
			// Server messages remain authoritative for hidden references and non-resource errors.
			if (!block || !priority.includes(conflict.conflictType))
				return [
					{
						key,
						primary: conflict.message,
						lesson: null,
						context: null,
						causes: [conflict.message]
					}
				];
			const teachers = unique([
				...block.groups.flatMap((group) => group.instructors.map((teacher) => teacher.displayName)),
				...block.teachers.map((teacher) => teacher.displayName)
			]).map(timetableTeacherLabel);
			const classes = unique([
				...block.groups.flatMap((group) => group.homeroomNames),
				...block.homerooms.map((homeroom) => homeroom.name)
			]);
			let resource: string | undefined;
			if (conflict.conflictType === 'teacher') {
				const teacher = state.workspace.staff.find((teacher) => teacher.id === conflict.targetId);
				const blockTeacher = [
					...block.groups.flatMap((group) => group.instructors),
					...block.teachers
				].find((teacher) => teacher.teacherId === conflict.targetId);
				const name = teacher?.displayName || blockTeacher?.displayName;
				resource = name ? timetableTeacherLabel(name) : undefined;
			} else if (conflict.conflictType === 'homeroom') {
				resource =
					state.homeroomsById.get(conflict.targetId ?? '')?.name ||
					block.homerooms.find((homeroom) => homeroom.homeroomId === conflict.targetId)?.name;
			} else if (conflict.conflictType === 'learning_group') {
				resource =
					state.groupsById.get(conflict.targetId ?? '')?.name ||
					block.groups.find((group) => group.learningGroupId === conflict.targetId)?.name;
			} else if (conflict.conflictType === 'room') {
				const room = state.workspace.rooms.find((room) => room.id === conflict.targetId);
				const target = [...block.groups, ...block.homerooms].find(
					(target) => target.roomId === conflict.targetId
				);
				const name = room?.name || target?.roomName || target?.roomCode;
				resource = name ? `ห้องสอน ${name}` : undefined;
			}
			if (!resource)
				return [
					{
						key,
						primary: conflict.message,
						lesson: null,
						context: null,
						causes: [conflict.message]
					}
				];
			const returning = conflict.code.includes('SWAP_RETURN') ? 'คาบที่สลับกลับ: ' : '';
			const context = conflict.conflictType === 'teacher' ? classes : [...classes, ...teachers];
			const cause = `${resource} ไม่ว่าง`;
			return [
				{
					key: `${block.id}:${returning}`,
					primary: `${returning}${cause}`,
					lesson: blockIdentity(block),
					context: context.join(', ') || null,
					causes: [cause]
				}
			];
		});
	// One lesson entry can block several resources; retain all causes without counting
	// the same group/class lesson twice in the compact +N indicator.
	const grouped = new Map<string, (typeof entries)[number]>();
	for (const entry of entries) {
		const existing = grouped.get(entry.key);
		if (existing) existing.causes = unique([...existing.causes, ...entry.causes]);
		else grouped.set(entry.key, { ...entry });
	}
	const reasons = [...grouped.values()];
	const first = reasons[0];
	const fallback = 'ช่องนี้มีคาบอยู่แล้ว วางคาบไม่ได้';
	return {
		primary: first?.primary ?? fallback,
		lesson: first?.lesson ?? null,
		context: first?.context ?? null,
		details: reasons.length
			? reasons.map((entry) =>
					unique([
						entry.primary.startsWith('คาบที่สลับกลับ: ')
							? `คาบที่สลับกลับ: ${entry.causes.join(' · ')}`
							: entry.causes.join(' · '),
						entry.lesson,
						entry.context
					]).join(' · ')
				)
			: [fallback]
	};
}
