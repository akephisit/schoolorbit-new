import { listSupervisionCycles, listSupervisionObservations } from '#lib/api/supervision.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	waitForSupervisionAccess,
	SUPERVISION_OBSERVATION_READ_PERMISSIONS
} from '#lib/supervision/supervision-access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: [
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
	const access = waitForSupervisionAccess(_meta.access.permission);
	const observationAccess = Promise.all([
		access,
		waitForSupervisionAccess(SUPERVISION_OBSERVATION_READ_PERMISSIONS)
	]).then(([route, read]) => route && read);
	const observations = academicYearId
		? captureRouteLoad(
				observationAccess.then(async (readable) => ({
					readable,
					items: readable
						? await listSupervisionObservations(
								{ academicYearId, ...(academicTermId ? { academicTermId } : {}) },
								{ requestFetch: fetch }
							)
						: []
				})),
				'โหลดรายการนิเทศไม่สำเร็จ'
			)
		: null;
	const cycleAccess = Promise.all([
		access,
		waitForSupervisionAccess([
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
			PERMISSIONS.SUPERVISION_APPROVE_SCHOOL,
			PERMISSIONS.SUPERVISION_READ_SCHOOL
		])
	]).then(([route, read]) => route && read);
	const cycles = academicYearId
		? captureRouteLoad(
				cycleAccess.then((allowed) =>
					allowed
						? listSupervisionCycles(academicYearId, academicTermId, { requestFetch: fetch })
						: []
				),
				'โหลดรอบนิเทศไม่สำเร็จ'
			)
		: null;
	const templates = null;
	return {
		title: 'รับรองและอนุมัติผลนิเทศ',
		section: 'approvals' as const,
		academicYearId,
		academicTermId,
		cycleId: '',
		cycles,
		templates,
		observations,
		teacherStatus: null
	};
};
