import { redirect } from '@sveltejs/kit';
import { resolve } from '$app/paths';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'none',
	menu: {
		title: 'กลุ่มสาระการเรียนรู้',
		icon: 'Layers3',
		group: 'academic_curriculum',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: [
			PERMISSIONS.ACADEMIC_CATALOG_READ_SCHOOL,
			PERMISSIONS.ACADEMIC_CATALOG_MANAGE_SCHOOL
		]
	}
};

export const load: PageLoad = ({ url }) => {
	const destination = new URL(resolve('staff/organization'), url);
	destination.searchParams.set('tab', 'subject-groups');
	const selected = url.searchParams.get('group');
	if (selected) destination.searchParams.set('group', selected);
	redirect(307, destination.pathname + destination.search);
};
