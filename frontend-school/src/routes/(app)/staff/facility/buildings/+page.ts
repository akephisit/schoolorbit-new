import type { PageLoad } from './$types';
import { listBuildings } from '$lib/api/facility';
import { get } from 'svelte/store';
import { can } from '$lib/stores/permissions';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'อาคารสถานที่',
		icon: 'School', // Changed to School icon which is more meaningful than Building (generic)
		group: 'general_admin',
		workspace: 'operations',
		permission: PERMISSION_MODULES.FACILITY,
		order: 10,
		user_type: 'staff'
	}
};

export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: _meta.menu.title,
		buildings: captureRouteLoad(
			waitForAuthenticatedUser().then((user) =>
				user?.user_type === 'staff' && get(can).has(PERMISSIONS.FACILITY_READ_ALL)
					? listBuildings({ requestFetch: fetch })
					: null
			),
			'โหลดรายการอาคารไม่สำเร็จ'
		)
	};
};
