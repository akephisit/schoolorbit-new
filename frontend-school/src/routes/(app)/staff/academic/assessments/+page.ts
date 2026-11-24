import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { assessmentReadPermissions } from '#lib/academic/assessment-access.js';
import { listAssessmentPhaseControls, listAssessmentPlans } from '#lib/api/academicAssessments.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { can } from '#lib/stores/permissions.js';

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

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const actor = waitForAuthenticatedUser().then((user) => ({
		identity: appIdentityKey(),
		canRead: user?.user_type === 'staff' && get(can).hasAny(...assessmentReadPermissions)
	}));
	return {
		title: _meta.menu.title,
		academicYearId,
		academicTermId,
		plans:
			academicYearId && academicTermId
				? actor.then(async ({ identity, canRead }) => ({
						identity,
						result: canRead
							? await captureRouteLoad(
									listAssessmentPlans({ academicTermId }, { requestFetch: fetch }),
									'โหลดโครงสร้างคะแนนไม่สำเร็จ'
								)
							: null
					}))
				: null,
		phaseControls:
			academicYearId && academicTermId
				? actor.then(async ({ identity, canRead }) => ({
						identity,
						result: canRead
							? await captureRouteLoad(
									listAssessmentPhaseControls(academicTermId, { requestFetch: fetch }),
									'โหลดช่วงการทำงานของครูไม่สำเร็จ'
								)
							: null
					}))
				: null
	};
};
