import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'year_required',
	menu: {
		title: 'ห้องประจำชั้น',
		icon: 'School',
		group: 'academic_registry',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.HOMEROOM
	}
};

export const load = () => ({ title: _meta.menu.title });
