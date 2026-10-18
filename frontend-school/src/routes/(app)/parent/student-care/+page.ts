import type { PageLoad } from './$types';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { readCare, careReadPermissions } from '#lib/features/student-care/access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import { ownCareAssignments } from '#lib/features/student-care/own-route.js';
import { careListQuery } from '#lib/features/student-care/route.js';
export const _meta = {
	academicContext: 'none' as const,
	menu: {
		title: 'แบบประเมินเพื่อการดูแล',
		icon: 'HeartHandshake',
		group: 'main',
		workspace: 'home',
		permission: PERMISSIONS.STUDENT_CARE_READ_OWN,
		order: 26,
		user_type: 'parent'
	}
};
export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const query = careListQuery(url);
	return {
		url: url.href,
		query,
		initial: captureRouteLoad(
			readCare(careReadPermissions, () => ownCareAssignments(query, { requestFetch: fetch })),
			'โหลดแบบประเมินไม่ได้'
		)
	};
};
