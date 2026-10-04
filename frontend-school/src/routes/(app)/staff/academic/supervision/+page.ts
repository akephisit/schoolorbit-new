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
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'term_optional' as const,
	menu: {
		title: 'นิเทศการสอน',
		icon: 'ClipboardCheck',
		group: 'academic_supervision',
		workspace: 'academic',
		order: 10,
		user_type: 'staff',
		permission: PERMISSION_MODULES.SUPERVISION
	}
};

export const load: PageLoad = ({ fetch, url }) => {
	const academicYearId = url.searchParams.get('academicYearId');
	const academicTermId = url.searchParams.get('academicTermId');
	const access = waitForSupervisionAccess(PERMISSION_MODULES.SUPERVISION);
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
	const cycleAccess = access;
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
		title: _meta.menu.title,
		section: 'mine' as const,
		academicYearId,
		academicTermId,
		cycleId: '',
		cycles,
		templates,
		observations,
		teacherStatus: null
	};
};
