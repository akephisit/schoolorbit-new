import type { PageLoad } from './$types';
import { waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { getMyWorkItems } from '#lib/api/work.js';
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
