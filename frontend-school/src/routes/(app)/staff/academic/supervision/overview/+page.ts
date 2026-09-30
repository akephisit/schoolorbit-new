import { listSupervisionCycles, getSupervisionTeacherStatusOverview } from '$lib/api/supervision';
import { waitForSupervisionAccess } from '$lib/supervision/supervision-access';
import { captureRouteLoad } from '$lib/navigation/route-load';
import { PERMISSIONS } from '$lib/permissions/registry';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.SUPERVISION_READ_SCHOOL,
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE,
			PERMISSIONS.SUPERVISION_APPROVE_SCHOOL
		]
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId');
	const academicTermId = url.searchParams.get('academicTermId');

	const cycleId = url.searchParams.get('cycleId') ?? '';
	const access = waitForSupervisionAccess(_meta.access.permission);
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
	const loadStatus = (id: string) =>
		access.then(async (allowed) => ({
			cycleId: id,
			items:
				allowed && id ? await getSupervisionTeacherStatusOverview(id, { requestFetch: fetch }) : []
		}));
	const teacherStatus = cycles
		? captureRouteLoad(
				cycleId
					? loadStatus(cycleId)
					: cycles.then((result) => {
							if (!result.ok) throw new Error(result.error);
							return loadStatus(result.data[0]?.id ?? '');
						}),
				'โหลดสถานะครูไม่สำเร็จ'
			)
		: null;
	return {
		title: 'ภาพรวมนิเทศการสอน',
		observations: null,
		section: 'overview' as const,
		academicYearId,
		academicTermId,
		cycleId,
		cycles,
		teacherStatus,
		templates: null
	};
};
