import type { PageLoad } from './$types';
import { PERMISSIONS } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.BEHAVIOR_CLEANUP_EXECUTE_SCHOOL }
};
export const load: PageLoad = ({ url, depends }) => {
	depends('school:app-identity');
	return {
		term: url.searchParams.get('academicTermId'),
		year: url.searchParams.get('academicYearId')
	};
};
