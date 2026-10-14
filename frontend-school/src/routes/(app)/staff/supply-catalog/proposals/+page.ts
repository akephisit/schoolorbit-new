import type { PageLoad } from './$types';
import { get } from 'svelte/store';
import { can } from '#lib/stores/permissions.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { listProposals, listCategories } from '#lib/api/supply-catalog.js';
export const _meta = {
	academicContext: 'none' as const,
	access: { user_type: 'staff', permission: PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL }
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const number = Number(url.searchParams.get('page') ?? 1);
	const query = {
		review: url.searchParams.get('review') === 'true',
		page: Number.isSafeInteger(number) && number > 0 && number <= 100000 ? number : 1,
		pageSize: 20
	};
	const requestKey = JSON.stringify(query);
	const actor = waitForAuthenticatedUser().then((user) => ({
		identity: appIdentityKey(),
		allowed:
			user?.user_type === 'staff' &&
			get(can).has(PERMISSIONS.SUPPLY_CATALOG_READ_SCHOOL) &&
			(query.review
				? get(can).has(PERMISSIONS.SUPPLY_CATALOG_APPROVE_SCHOOL)
				: get(can).has(PERMISSIONS.SUPPLY_CATALOG_REQUEST_OWN))
	}));
	return {
		title: query.review ? 'ตรวจข้อเสนอพัสดุ' : 'ข้อเสนอพัสดุของฉัน',
		query,
		requestKey,
		proposals: captureRouteLoad(
			actor.then(async ({ identity, allowed }) => ({
				ownerKey: `${identity}|${requestKey}`,
				page: allowed ? await listProposals(query, { requestFetch: fetch }) : null
			})),
			'โหลดข้อเสนอไม่สำเร็จ'
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
