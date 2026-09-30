import { listSupervisionCycles } from '$lib/api/supervision';
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
	const cycles = academicYearId
		? captureRouteLoad(
				access.then((allowed) =>
					allowed
						? listSupervisionCycles(academicYearId, academicTermId, { requestFetch: fetch })
						: []
				),
				'โหลดรอบนิเทศไม่สำเร็จ'
			)
		: null;
	return {
		title: 'รอบนิเทศ',
		observations: null,
		section: 'cycles' as const,
		academicYearId,
		academicTermId,
		cycleId: '',
		cycles,
		templates: null,
		teacherStatus: null
	};
};
