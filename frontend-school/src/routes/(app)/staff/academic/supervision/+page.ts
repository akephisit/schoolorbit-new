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
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import type { PageLoad } from './$types';

export const _meta = {
	academicContext: 'term_required' as const,
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

export const load: PageLoad = ({ fetch, url, depends }) => {
	depends('school:app-identity');
	const routeIdentity = waitForAuthenticatedUser().then((user) => (user ? appIdentityKey() : ''));
	const academicYearId = url.searchParams.get('academicYearId');
	const academicTermId = url.searchParams.get('academicTermId');
	const access = waitForSupervisionAccess(PERMISSION_MODULES.SUPERVISION);
	const observationAccess = Promise.all([
		access,
		waitForSupervisionAccess(SUPERVISION_OBSERVATION_READ_PERMISSIONS)
	]).then(([route, read]) => route && read);
	const observations =
		academicYearId && academicTermId
			? captureRouteLoad(
					observationAccess.then(async (allowed) => {
						const items = allowed
							? await resolveSupervisionRead(
									listSupervisionObservations(
										{ academicYearId, academicTermId },
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
	const cycles =
		academicYearId && academicTermId
			? captureRouteLoad(
					cycleAccess.then((allowed) =>
						allowed
							? listSupervisionCycles(academicYearId, academicTermId, { requestFetch: fetch })
							: []
					),
					'โหลดรอบนิเทศไม่สำเร็จ'
				)
			: null;
	const templates =
		academicYearId && academicTermId
			? captureRouteLoad(
					observationAccess.then((allowed) =>
						allowed ? listSupervisionTemplateSummaries({ requestFetch: fetch }) : []
					),
					'โหลดข้อมูลแบบประเมินไม่สำเร็จ'
				)
			: null;
	return {
		identity: routeIdentity,
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
