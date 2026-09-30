import { authAPI } from '$lib/api/auth';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';
export const _meta = { academicContext: 'none' as const, access: { user_type: 'staff' } };
export const load: PageLoad = ({ fetch, depends }) => {
	depends('school:app-identity');
	return {
		profile: captureRouteLoad(
			waitForAuthenticatedUser().then((user) =>
				user?.user_type === 'staff' ? authAPI.getFullProfile({ requestFetch: fetch }) : null
			),
			'โหลดโปรไฟล์ไม่สำเร็จ'
		)
	};
};
