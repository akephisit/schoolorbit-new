import { appIdentityKey, waitForAuthenticatedUser } from '#lib/auth/settled-user.js';
import {
	listSupervisionCycles,
	listSupervisionTemplateSummaries,
	listSupervisionObservations
} from '#lib/api/supervision.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import {
	resolveSupervisionRead,
	waitForSupervisionAccess,
	SUPERVISION_OBSERVATION_READ_PERMISSIONS
} from '#lib/supervision/supervision-access.js';
import { PERMISSIONS } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'term_optional' as const,
	access: {
		user_type: 'staff',
		permission: [
			PERMISSIONS.SUPERVISION_MANAGE_SCHOOL,
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
	const access = waitForSupervisionAccess(_meta.access.permission);
	const observationAccess = Promise.all([
		access,
		waitForSupervisionAccess(SUPERVISION_OBSERVATION_READ_PERMISSIONS)
	]).then(([route, read]) => route && read);
	const observations = academicYearId
		? captureRouteLoad(
				observationAccess.then(async (allowed) => {
					const items = allowed
						? await resolveSupervisionRead(
								listSupervisionObservations(
									{ academicYearId, ...(academicTermId ? { academicTermId } : {}) },
									{ requestFetch: fetch }
								)
							)
						: null;
					return { readable: items !== null, items: items ?? [] };
				}),
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
		access.then((allowed) =>
			allowed ? listSupervisionTemplateSummaries({ requestFetch: fetch }) : []
		),
		'โหลดข้อมูลแบบประเมินไม่สำเร็จ'
	);
	return {
		identity: routeIdentity,
		title: 'คำขอจองนิเทศ',
		section: 'requests' as const,
		academicYearId,
		academicTermId,
		cycleId: '',
		cycles,
		templates,
		observations,
		teacherStatus: null
	};
};
