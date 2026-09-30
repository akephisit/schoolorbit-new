/**
 * Menu Administration Page
 */

import { PERMISSIONS, PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	menu: {
		title: 'จัดการเมนู',
		icon: 'Menu',
		group: 'settings',
		workspace: 'settings',
		order: 1001,
		user_type: 'staff',
		permission: PERMISSION_MODULES.MENU
	}
};

import type { PageLoad } from './$types';
import { listMenuWorkspaces, listMenuGroups, listMenuItems } from '$lib/api/menu-admin';
import { appIdentityKey, waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { can } from '$lib/stores/permissions';
import { get } from 'svelte/store';
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const settled = waitForAuthenticatedUser();
	return {
		title: 'จัดการเมนู',
		workspaces: captureRouteLoad(
			settled.then(async (user) => {
				const identityKey = appIdentityKey();
				return {
					identityKey,
					records:
						user?.user_type === 'staff' && get(can).has(PERMISSIONS.MENU_READ_ALL)
							? await listMenuWorkspaces({ requestFetch: fetch })
							: null
				};
			}),
			'โหลดกลุ่มบริหารไม่สำเร็จ'
		),
		groups: captureRouteLoad(
			settled.then(async (user) => {
				const identityKey = appIdentityKey();
				return {
					identityKey,
					records:
						user?.user_type === 'staff' && get(can).has(PERMISSIONS.MENU_READ_ALL)
							? await listMenuGroups({ requestFetch: fetch })
							: null
				};
			}),
			'โหลดฝ่าย/งานไม่สำเร็จ'
		),
		items: captureRouteLoad(
			settled.then(async (user) => {
				const identityKey = appIdentityKey();
				return {
					identityKey,
					records:
						user?.user_type === 'staff' && get(can).has(PERMISSIONS.MENU_READ_ALL)
							? await listMenuItems(undefined, { requestFetch: fetch })
							: null
				};
			}),
			'โหลดเมนูบริการไม่สำเร็จ'
		)
	};
};
