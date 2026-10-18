import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import {
	careStaff,
	careStudents,
	careAssignment,
	careVisit,
	careRound
} from '#lib/api/student-care.js';
import type { ApiRequestOptions } from '#lib/api/client.js';
/** UI form context, composed from the existing resource contracts. */
export async function careCaseSetup(
	context: {
		term: string;
		date: string;
		sourceAssignmentId: string | null;
		sourceVisitId: string | null;
	},
	options: ApiRequestOptions = {}
) {
	if (
		!context.sourceAssignmentId &&
		!context.sourceVisitId &&
		!get(can).has(PERMISSIONS.STUDENT_CARE_SENSITIVE_MANAGE_SCHOOL)
	)
		return {
			staff: [],
			students: [],
			term: context.term,
			studentId: null,
			studentName: '',
			sourceRequired: true
		};
	const source = async () => {
		if (context.sourceAssignmentId) {
			const detail = await careAssignment(context.sourceAssignmentId, options);
			const round = await careRound(detail.assignment.roundId, options);
			return {
				term: round.academicTermId,
				studentId: detail.assignment.studentId,
				studentName: detail.assignment.studentName
			};
		}
		if (context.sourceVisitId) {
			const record = await careVisit(context.sourceVisitId, options);
			return {
				term: record.visit.academicTermId,
				studentId: record.visit.studentId,
				studentName: record.visit.studentName
			};
		}
		return { term: context.term, studentId: null, studentName: '' };
	};
	const [staff, selected] = await Promise.all([careStaff(options), source()]);
	const students =
		!selected.studentId && selected.term
			? await careStudents(
					{ academicTermId: selected.term, date: context.date, limit: 50 },
					options
				)
			: [];
	return { ...selected, staff, students, sourceRequired: false };
}
