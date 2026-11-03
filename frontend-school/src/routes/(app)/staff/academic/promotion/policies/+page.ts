import { redirect } from '@sveltejs/kit';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';
export const _meta = {
	academicContext: 'year_required' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.ACADEMIC_PROMOTION_READ_SCHOOL }
};
export const load: PageLoad = ({ url }) => {
	const destination = new URL('/staff/academic/year-lifecycle', url);
	const yearId = url.searchParams.get('academicYearId');
	if (yearId) destination.searchParams.set('academicYearId', yearId);
	return redirect(307, destination.pathname + destination.search);
};
