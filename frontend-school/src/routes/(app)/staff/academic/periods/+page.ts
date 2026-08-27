import { PERMISSION_MODULES } from '$lib/permissions/registry';
import { redirect } from '@sveltejs/kit';

export const _meta = {
	academicContext: 'year_required' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.ACADEMIC_TERM
	}
};

export const load = () => redirect(308, '/staff/academic/core#bell-schedules');
