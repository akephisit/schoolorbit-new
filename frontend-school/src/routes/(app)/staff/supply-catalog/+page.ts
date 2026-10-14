import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listItems, listCategories } from '#lib/api/supply-catalog.js';
import { itemFilters } from '#lib/supply-catalog/drafts.js';
export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'คลังพัสดุ',
		icon: 'Package',
		group: 'budget_procurement',
		workspace: 'budget',
		order: 510,
		user_type: 'staff',
		permission: PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const query = itemFilters(url),
		requestKey = JSON.stringify(query);
	const actor = waitForAuthenticatedUser().then((user) => ({
		identity: appIdentityKey(),
		allowed: user?.user_type === 'staff' && get(can).has(PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL)
	}));
	return {
		title: _meta.menu.title,
		query,
		requestKey,
		items: captureRouteLoad(
			actor.then(async ({ identity, allowed }) => ({
				ownerKey: `${identity}|${requestKey}`,
				page: allowed
					? await listItems(
							{ ...query, categoryId: query.categoryId || undefined },
							{ requestFetch: fetch }
						)
					: null
			})),
			'โหลดคลังพัสดุไม่สำเร็จ'
		),
		categories: captureRouteLoad(
			actor.then(async ({ identity, allowed }) => ({
				ownerKey: identity,
				categories: allowed ? await listCategories({ requestFetch: fetch }) : null
			})),
			'โหลดหมวดหมู่ไม่สำเร็จ'
		)
	};
};
