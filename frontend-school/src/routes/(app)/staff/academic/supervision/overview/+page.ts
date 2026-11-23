import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import {
	listSupervisionCycles,
	getSupervisionCycleProgress,
	getSupervisionTeacherStatusOverview
} from '#lib/api/supervision.js';
import {
	SUPERVISION_SCHOOL_REPORT_PERMISSIONS,
	waitForSupervisionAccess
} from '#lib/supervision/supervision-access.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'term_optional' as const,
	access: {
		user_type: 'staff',
		permission: [
			...SUPERVISION_SCHOOL_REPORT_PERMISSIONS,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_UNIT,
			PERMISSIONS.SUPERVISION_MANAGE_ORGANIZATION_TREE
		]
	}
};

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const routeIdentity = waitForAuthenticatedUser().then((user) => (user ? appIdentityKey() : ''));
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
	const selectedCycle = cycles
		? cycleId
			? Promise.resolve(cycleId)
			: cycles.then((result) => {
					if (!result.ok) throw new Error(result.error);
					return result.data[0]?.id ?? '';
				})
		: null;
	const teacherStatus = selectedCycle
		? captureRouteLoad(selectedCycle.then(loadStatus), 'โหลดสถานะครูไม่สำเร็จ')
		: null;
	const progress = selectedCycle
		? captureRouteLoad(
				Promise.all([
					selectedCycle,
					waitForSupervisionAccess(SUPERVISION_SCHOOL_REPORT_PERMISSIONS)
				]).then(async ([id, allowed]) => ({
					cycleId: id,
					report:
						allowed && id ? await getSupervisionCycleProgress(id, { requestFetch: fetch }) : null
				})),
				'โหลดความคืบหน้ารอบนิเทศไม่สำเร็จ'
			)
		: null;
	return {
		identity: routeIdentity,
		title: 'ภาพรวมนิเทศการสอน',
		observations: null,
		section: 'overview' as const,
		academicYearId,
		academicTermId,
		cycleId,
		cycles,
		progress,
		teacherStatus,
		templates: null
	};
};
