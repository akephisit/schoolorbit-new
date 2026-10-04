import type { PageLoad } from './$types';
import {
	getAcademicResultReadiness,
	type AcademicResultReadiness
} from '#lib/api/academicResults.js';
import {
	getLearnerEvaluationLockReadiness,
	type LearnerEvaluationSubjectLockReadiness
} from '#lib/api/academicLearnerEvaluations.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';

type LockTab = 'course' | 'activity' | 'learner';
type LockQueue =
	| { tab: 'learner'; rows: LearnerEvaluationSubjectLockReadiness[] }
	| { tab: 'course' | 'activity'; readiness: AcademicResultReadiness };

function requestedTab(value: string | null): LockTab {
	return value === 'activity' || value === 'learner' ? value : 'course';
}

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ล็อกผลการเรียน',
		icon: 'LockKeyhole',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 40,
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_RESULT_LOCK_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_LOCK_SCHOOL
		]
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const tab = requestedTab(url.searchParams.get('tab'));
	const context = academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	if (!context) return { title: _meta.menu.title, context, tab, queue: null };
	const queue = captureRouteLoad<LockQueue>(
		tab === 'learner'
			? getLearnerEvaluationLockReadiness(context, { requestFetch: fetch }).then((rows) => ({
					tab,
					rows
				}))
			: getAcademicResultReadiness(context, { requestFetch: fetch }).then((readiness) => ({
					tab,
					readiness
				})),
		'โหลดคิวล็อกผลไม่สำเร็จ'
	);
	return { title: _meta.menu.title, context, tab, queue };
};
