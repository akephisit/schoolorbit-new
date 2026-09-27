import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';
import { waitForLifecycleReadAccess } from '$lib/academic/lifecycle/lifecycle-access';
import { getYearLifecycleWorkspace } from '$lib/api/academic-lifecycle';
import { captureRouteLoad } from '$lib/navigation/route-load';

export const _meta = {
	academicContext: 'year_required' as const,
	menu: {
		title: 'ปิดปีการศึกษา',
		icon: 'CalendarCheck',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 70,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL
	}
};
export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const workspace = academicYearId
		? captureRouteLoad(
				waitForLifecycleReadAccess().then((allowed) =>
					allowed ? getYearLifecycleWorkspace(academicYearId, { requestFetch: fetch }) : null
				),
				'โหลดความพร้อมปีการศึกษาไม่สำเร็จ'
			)
		: null;
	return { title: _meta.menu.title, academicYearId, workspace };
};
