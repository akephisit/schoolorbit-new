import { listSupervisionTemplateSummaries } from '$lib/api/supervision';
import { waitForSupervisionAccess } from '$lib/supervision/supervision-access';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.SUPERVISION_MANAGE_SCHOOL
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId');
	const academicTermId = url.searchParams.get('academicTermId');

	const access = waitForSupervisionAccess([PERMISSIONS.SUPERVISION_MANAGE_SCHOOL]);
	const templates = captureRouteLoad(
		access.then((allowed) =>
			allowed ? listSupervisionTemplateSummaries({ requestFetch: fetch }) : []
		),
		'โหลดแบบประเมินไม่สำเร็จ'
	);
	return {
		title: 'แบบประเมินนิเทศ',
		observations: null,
		section: 'templates' as const,
		academicYearId,
		academicTermId,
		cycleId: '',
		cycles: null,
		templates,
		teacherStatus: null
	};
};
