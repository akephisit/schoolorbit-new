import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import {
	getSupervisionObservation,
	getSupervisionCycle,
	getSupervisionTemplateSummary
} from '#lib/api/supervision.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	waitForSupervisionAccess,
	SUPERVISION_OBSERVATION_READ_PERMISSIONS
} from '#lib/supervision/supervision-access.js';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.SUPERVISION
	}
};

export const load: PageLoad = ({ params, fetch, url }) => {
	const identity = Promise.all([
		waitForSupervisionAccess(_meta.access.permission),
		waitForSupervisionAccess(SUPERVISION_OBSERVATION_READ_PERMISSIONS)
	]).then(([route, read]) =>
		route && read ? getSupervisionObservation(params.id, { requestFetch: fetch }) : null
	);
	// References depend only on identity, and neither waits for its sibling.
	const cycle = captureRouteLoad(
		identity.then((item) =>
			item ? getSupervisionCycle(item.cycleId, { requestFetch: fetch }) : null
		),
		'โหลดรอบนิเทศไม่สำเร็จ'
	);
	const template = captureRouteLoad(
		identity.then((item) =>
			item ? getSupervisionTemplateSummary(item.templateId, { requestFetch: fetch }) : null
		),
		'โหลดข้อมูลแบบประเมินไม่สำเร็จ'
	);
	return {
		title: 'รายละเอียดรายการนิเทศ',
		observationId: params.id,
		academicYearId: url.searchParams.get('academicYearId'),
		academicTermId: url.searchParams.get('academicTermId'),
		observation: captureRouteLoad(identity, 'โหลดรายการนิเทศไม่สำเร็จ'),
		cycle,
		template
	};
};
