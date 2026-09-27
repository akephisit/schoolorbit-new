import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';
import { waitForLifecycleReadAccess } from '$lib/academic/lifecycle/lifecycle-access';
import { getTermLifecycleWorkspace } from '$lib/api/academic-lifecycle';
import { captureRouteLoad } from '$lib/navigation/route-load';

export const _meta = {
	academicContext: 'term_required' as const,
	menu: {
		title: 'ปิดและเปลี่ยนภาคเรียน',
		icon: 'CalendarCheck',
		group: 'academic_assessment',
		workspace: 'academic',
		order: 60,
		user_type: 'staff',
		permission: PERMISSIONS.ACADEMIC_LIFECYCLE_READ_SCHOOL
	}
};
export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId')?.trim() || null;
	const academicTermId = url.searchParams.get('academicTermId')?.trim() || null;
	const context = academicYearId && academicTermId ? { academicYearId, academicTermId } : null;
	const workspace = context
		? captureRouteLoad(
				waitForLifecycleReadAccess().then((allowed) =>
					allowed
						? getTermLifecycleWorkspace(
								context.academicTermId,
								{ academicYearId: context.academicYearId },
								{ requestFetch: fetch }
							)
						: null
				),
				'โหลดความพร้อมภาคเรียนไม่สำเร็จ'
			)
		: null;
	return { title: _meta.menu.title, context, workspace };
};
