import type { ActivitySlot, SlotClassroomAssignment, SlotInstructor } from '$lib/api/academic';

export interface InstructorActivityItem {
	slot: ActivitySlot;
	classroom_id: string;
	classroom_name: string;
}

export function synchronizedSlotsForInstructor(
	slots: ActivitySlot[],
	instructorsBySlot: Record<string, SlotInstructor[]>,
	instructorId: string
): ActivitySlot[] {
	return slots.filter(
		(slot) =>
			slot.scheduling_mode === 'synchronized' &&
			(instructorsBySlot[slot.id] ?? []).some((instructor) => instructor.user_id === instructorId)
	);
}

export function independentItemsForInstructor(
	slots: ActivitySlot[],
	assignmentsBySlot: Record<string, SlotClassroomAssignment[]>,
	instructorId: string
): InstructorActivityItem[] {
	const items: InstructorActivityItem[] = [];
	for (const slot of slots) {
		if (slot.scheduling_mode !== 'independent') continue;
		for (const assignment of assignmentsBySlot[slot.id] ?? []) {
			if (assignment.instructor_id !== instructorId) continue;
			items.push({
				slot,
				classroom_id: assignment.classroom_id,
				classroom_name: assignment.classroom_name ?? ''
			});
		}
	}
	return items;
}

export function activityInstructorEntries(
	assignmentsBySlot: Record<string, SlotClassroomAssignment[]>
): Array<[string, { id: string; name: string }]> {
	return Object.entries(assignmentsBySlot).flatMap(([slotId, assignments]) =>
		assignments.map(
			(assignment) =>
				[
					`${slotId}|${assignment.classroom_id}`,
					{
						id: assignment.instructor_id,
						name: assignment.instructor_name ?? ''
					}
				] as [string, { id: string; name: string }]
		)
	);
}
