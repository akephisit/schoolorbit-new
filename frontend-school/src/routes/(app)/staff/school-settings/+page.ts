import type { PageLoad } from './$types';
import { getSchoolSettings } from '$lib/api/school';
import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'ตั้งค่าโรงเรียน',
		icon: 'School',
		group: 'settings',
		workspace: 'settings',
		order: 900,
		user_type: 'staff',
		permission: PERMISSION_MODULES.SETTINGS
	}
};

export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: _meta.menu.title,
		settings: captureRouteLoad(
			waitForAuthenticatedUser().then((user) =>
				user?.user_type === 'staff' && get(can).has(PERMISSIONS.SETTINGS_READ_ALL)
					? getSchoolSettings({ requestFetch: fetch })
					: null
			),
			'โหลดข้อมูลไม่สำเร็จ'
		)
	};
};
