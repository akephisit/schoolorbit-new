import type { PageLoad } from './$types';
import { searchEffectiveAcademicResults, type EffectiveResultKind } from '$lib/api/academicResults';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';

function requestedKind(value: string | null): 'all' | EffectiveResultKind {
	return value === 'course' || value === 'activity' || value === 'learner_evaluation'
		? value
		: 'all';
}

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'แก้ผลการเรียน',
		icon: 'History',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 50,
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_RESULT_CORRECT_SCHOOL,
			PERMISSIONS.ACADEMIC_LEARNER_EVALUATION_CORRECT_SCHOOL
		]
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const search = url.searchParams.get('search')?.trim() || '';
	const kind = requestedKind(url.searchParams.get('kind'));
	const context = academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	if (!context)
		return { title: _meta.menu.title, context, filters: { search, kind }, results: null };
	const results = captureRouteLoad(
		searchEffectiveAcademicResults(
			{
				...context,
				...(search ? { search } : {}),
				...(kind === 'all' ? {} : { kind }),
				limit: 100
			},
			{ requestFetch: fetch }
		),
		'ค้นหาผลการเรียนไม่สำเร็จ'
	);
	return { title: _meta.menu.title, context, filters: { search, kind }, results };
};
