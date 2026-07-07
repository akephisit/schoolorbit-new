import type {
	ExamInvigilatorAssignmentSummary,
	ExamInvigilatorStaffOption,
	ExamInvigilatorStaffWorkload
} from '$lib/api/examSchedule';

export const INVIGILATOR_STAFF_DRAG_TYPE =
	'application/x-schoolorbit-exam-invigilator-staff-id';

export type InvigilatorStaffCardView = {
	staffId: string;
	displayName: string;
	selectedDayMinutes: number;
	totalMinutes: number;
	assignedAssignment: ExamInvigilatorAssignmentSummary | null;
};

export type InvigilatorWorkloadLevel = 'idle' | 'assigned' | 'heavy';

export function formatInvigilatorMinutes(minutes: number): string {
	const hours = Math.floor(minutes / 60);
	const remainder = minutes % 60;
	if (hours === 0) return `${remainder} นาที`;
	if (remainder === 0) return `${hours} ชม.`;
	return `${hours} ชม. ${remainder} นาที`;
}

export function workloadLevel(staff: InvigilatorStaffCardView): InvigilatorWorkloadLevel {
	if (staff.totalMinutes >= 240 || staff.selectedDayMinutes >= 180) return 'heavy';
	if (staff.assignedAssignment || staff.selectedDayMinutes > 0) return 'assigned';
	return 'idle';
}

export function workloadStaffName(workload: ExamInvigilatorStaffWorkload): string {
	return workload.staffName || workload.staffId;
}

export function staffOptionName(staff: ExamInvigilatorStaffOption): string {
	return staff.displayName || staff.staffId;
}
