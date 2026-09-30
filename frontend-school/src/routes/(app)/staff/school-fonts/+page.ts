import type { PageLoad } from './$types';
import { listSchoolFonts } from '$lib/api/school-fonts';
import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'คลังฟอนต์โรงเรียน',
		icon: 'Type',
		group: 'settings',
		workspace: 'settings',
		order: 920,
		user_type: 'staff',
		permission: PERMISSIONS.FONT_MANAGE_SCHOOL
	}
};

export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: _meta.menu.title,
		fonts: captureRouteLoad(
			waitForAuthenticatedUser().then((user) =>
				user?.user_type === 'staff' && get(can).has(PERMISSIONS.FONT_MANAGE_SCHOOL)
					? listSchoolFonts({ requestFetch: fetch })
					: null
			),
			'โหลดข้อมูลไม่สำเร็จ'
		)
	};
};
