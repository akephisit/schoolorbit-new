import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import {
	getSupervisionObservation,
	getSupervisionCycle,
	getSupervisionTemplateSummary
} from '#lib/api/supervision.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	resolveSupervisionRead,
	waitForSupervisionAccess,
	SUPERVISION_OBSERVATION_READ_PERMISSIONS
} from '#lib/supervision/supervision-access.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'term_optional' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.SUPERVISION
	}
};

export const load: PageLoad = ({ params, fetch, url, depends }) => {
	depends('school:app-identity');
	const routeIdentity = waitForAuthenticatedUser().then((user) => (user ? appIdentityKey() : ''));
	const identity = Promise.all([
		waitForSupervisionAccess(_meta.access.permission),
		waitForSupervisionAccess(SUPERVISION_OBSERVATION_READ_PERMISSIONS)
	]).then(([route, read]) =>
		route && read && /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(params.id)
			? resolveSupervisionRead(getSupervisionObservation(params.id, { requestFetch: fetch }))
			: null
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
		identity: routeIdentity,
		title: 'รายละเอียดรายการนิเทศ',
		observationId: params.id,
		academicYearId: url.searchParams.get('academicYearId'),
		academicTermId: url.searchParams.get('academicTermId'),
		observation: captureRouteLoad(identity, 'โหลดรายการนิเทศไม่สำเร็จ'),
		cycle,
		template
	};
};
