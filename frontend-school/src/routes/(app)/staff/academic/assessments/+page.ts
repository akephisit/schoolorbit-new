import type { PageLoad } from './$types';
import { listAssessmentPhaseControls, listAssessmentPlans } from '$lib/api/academicAssessments';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'โครงสร้างคะแนน',
		icon: 'ClipboardList',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_ASSESSMENT
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	return {
		title: _meta.menu.title,
		academicYearId,
		academicTermId,
		plans:
			academicYearId && academicTermId
				? captureRouteLoad(
						listAssessmentPlans({ academicTermId }, { requestFetch: fetch }),
						'โหลดโครงสร้างคะแนนไม่สำเร็จ'
					)
				: null,
		phaseControls:
			academicYearId && academicTermId
				? captureRouteLoad(
						listAssessmentPhaseControls(academicTermId, { requestFetch: fetch }),
						'โหลดช่วงการทำงานของครูไม่สำเร็จ'
					)
				: null
	};
};
