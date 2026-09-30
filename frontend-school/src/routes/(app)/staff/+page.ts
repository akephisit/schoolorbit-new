import { getStaffDashboard } from '$lib/api/staff';
import { requireApiData } from '$lib/api/client';
import { waitForAuthenticatedUser } from '$lib/auth/settled-user';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';

/**
 * Staff Dashboard
 * Main dashboard for staff members (no specific permission required)
 */

export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'แดชบอร์ด',
		icon: 'LayoutDashboard',
		group: 'main',
		workspace: 'home',
		order: 1,
		user_type: 'staff'
		// No permission required - all authenticated staff can access
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId');
	const overview = academicYearId
		? captureRouteLoad(
				waitForAuthenticatedUser().then(async (user) =>
					user?.user_type === 'staff'
						? requireApiData(
								await getStaffDashboard(academicYearId, { requestFetch: fetch }),
								'โหลดภาพรวมโรงเรียนไม่สำเร็จ'
							)
						: null
				),
				'โหลดภาพรวมโรงเรียนไม่สำเร็จ'
			)
		: null;
	return { title: 'Staff Dashboard', academicYearId, overview };
};
