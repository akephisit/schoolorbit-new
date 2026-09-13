import { PERMISSIONS as P } from '$lib/permissions/registry';

export const aggregateReadPermissions = [
	P.ACADEMIC_RESULT_READ_SCHOOL,
	P.ACADEMIC_RESULT_MANAGE_SCHOOL,
	P.ACADEMIC_RESULT_LOCK_SCHOOL,
	P.ACADEMIC_RESULT_CORRECT_SCHOOL,
	P.ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL,
	P.ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL,
	P.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL,
	P.ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL
];

/** Combined summaries must never be assembled from partial subject scopes. */
export function aggregateCapabilities(checker: { has: (permission: string) => boolean }) {
	const result = [
		P.ACADEMIC_RESULT_READ_SCHOOL,
		P.ACADEMIC_RESULT_MANAGE_SCHOOL,
		P.ACADEMIC_RESULT_LOCK_SCHOOL,
		P.ACADEMIC_RESULT_CORRECT_SCHOOL
	].some((permission) => checker.has(permission));
	const learner = [
		P.ACADEMIC_LEARNER_EVALUATION_READ_SCHOOL,
		P.ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL,
		P.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL,
		P.ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL
	].some((permission) => checker.has(permission));
	return {
		read: result && learner,
		lock:
			checker.has(P.ACADEMIC_RESULT_LOCK_SCHOOL) &&
			checker.has(P.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL),
		policy:
			checker.has(P.ACADEMIC_RESULT_MANAGE_SCHOOL) &&
			checker.has(P.ACADEMIC_LEARNER_EVALUATION_MANAGE_SCHOOL)
	};
}
