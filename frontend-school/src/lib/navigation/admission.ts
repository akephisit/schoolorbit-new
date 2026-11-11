/** Action-only sessions can return to their year context without entering applicant administration. */
export function admissionWorkflowBackHref(
	roundId: string,
	academicYearId: string | null | undefined,
	canReadAdmission: boolean
): string {
	if (canReadAdmission) return `/staff/academic/admission/${roundId}`;
	return academicYearId ? `/staff?academicYearId=${encodeURIComponent(academicYearId)}` : '/staff';
}
