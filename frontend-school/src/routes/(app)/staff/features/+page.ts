import type { PageLoad } from './$types';
import { listFeatures } from '#lib/api/feature-toggles.js';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
/**
 * Feature Toggles Management Page
 */

import { PERMISSIONS, PERMISSION_MODULES } from '#lib/permissions/registry.js';

export const _meta = {
	menu: {
		title: 'จัดการระบบงาน',
		icon: 'Zap',
		group: 'settings',
		workspace: 'settings',
		order: 1000,
		user_type: 'staff',
		permission: PERMISSION_MODULES.FEATURES
	}
};

export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		title: _meta.menu.title,
		features: captureRouteLoad(
			waitForAuthenticatedUser().then((user) =>
				user?.user_type === 'staff' && get(can).has(PERMISSIONS.FEATURES_READ_ALL)
					? listFeatures({ requestFetch: fetch })
					: null
			),
			'โหลดข้อมูลไม่สำเร็จ'
		)
	};
};
