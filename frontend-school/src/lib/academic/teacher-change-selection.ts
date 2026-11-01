import type { AcademicTermChangeSet } from '#lib/api/learning-delivery.js';

export function teacherChangeSelection(
	changeSet: Pick<AcademicTermChangeSet, 'baseTeacherAssignments' | 'items'>,
	learningGroupId: string
) {
	const members = changeSet.baseTeacherAssignments.filter(
		(assignment) => assignment.learningGroupId === learningGroupId
	);
	const pending = changeSet.items.filter(
		(item) =>
			(item.actionKind === 'stop_group_teacher' ||
				item.actionKind === 'adjust_group_teacher_role' ||
				item.actionKind === 'add_group_teacher') &&
			item.learningGroupId === learningGroupId
	);
	const pendingEpisodeIds = new Set(
		pending.flatMap((item) =>
			item.actionKind === 'stop_group_teacher' || item.actionKind === 'adjust_group_teacher_role'
				? [item.learningGroupTeacherId]
				: []
		)
	);
	const stoppedIds = new Set(
		pending.flatMap((item) =>
			item.actionKind === 'stop_group_teacher' ? [item.learningGroupTeacherId] : []
		)
	);
	const blockedTeacherIds = new Set([
		...members
			.filter((member) => !stoppedIds.has(member.assignmentId))
			.map((member) => member.teacherId),
		...pending.flatMap((item) => (item.actionKind === 'add_group_teacher' ? [item.teacherId] : []))
	]);
	return {
		members,
		availableAssignments: members.filter((member) => !pendingEpisodeIds.has(member.assignmentId)),
		blockedTeacherIds
	};
}
