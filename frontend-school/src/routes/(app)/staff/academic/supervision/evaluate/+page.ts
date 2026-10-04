import {
	listSupervisionCycles,
	listSupervisionTemplateSummaries,
	listSupervisionObservations
} from '#lib/api/supervision.js';
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
		permission: PERMISSIONS.SUPERVISION_EVALUATE_ASSIGNED
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId');
	const academicTermId = url.searchParams.get('academicTermId');
	const access = waitForSupervisionAccess([_meta.access.permission]);
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
	const cycleAccess = observationAccess;
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
	const templates = captureRouteLoad(
		observationAccess.then((allowed) =>
			allowed ? listSupervisionTemplateSummaries({ requestFetch: fetch }) : []
		),
		'โหลดข้อมูลแบบประเมินไม่สำเร็จ'
	);
	return {
		title: 'รายการที่ต้องประเมิน',
		section: 'evaluate' as const,
		academicYearId,
		academicTermId,
		cycleId: '',
		cycles,
		templates,
		observations,
		teacherStatus: null
	};
};
