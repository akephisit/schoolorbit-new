import type { PageLoad } from './$types';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { getMyWorkItems } from '$lib/api/work';
export const _meta = { academicContext: 'none' as const, access: { user_type: 'staff' } };
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	const items = captureRouteLoad(
		waitForAuthenticatedUser().then((user) =>
			user?.user_type === 'staff' ? getMyWorkItems({}, { requestFetch: fetch }) : []
		),
		'โหลดรายการงานไม่สำเร็จ'
	);
	return { title: 'งานของฉัน', items };
};
