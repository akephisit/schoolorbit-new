/**
 * Admission Round Detail & Management Page
 */

import { PERMISSIONS } from '#lib/permissions/registry.js';
import { waitForAdmissionAccess } from '#lib/admission/admission-access.js';
import { getRound, listTracks, listSubjects } from '#lib/api/admission.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_READ_ALL
	}
};

export const load: PageLoad = ({ fetch, params }) => {
	const id = params.id;
	const readAccess = waitForAdmissionAccess(PERMISSIONS.ADMISSION_READ_ALL);
	const manageAccess = waitForAdmissionAccess(PERMISSIONS.ADMISSION_MANAGE_ALL);
	const round = captureRouteLoad(
		readAccess.then((allowed) => (allowed ? getRound(id, { requestFetch: fetch }) : null)),
		'โหลดรายละเอียดรอบรับสมัครไม่สำเร็จ'
	);
	const tracks = captureRouteLoad(
		manageAccess.then((allowed) => (allowed ? listTracks(id, { requestFetch: fetch }) : [])),
		'โหลดสายการเรียนไม่สำเร็จ'
	);
	const subjects = captureRouteLoad(
		manageAccess.then((allowed) => (allowed ? listSubjects(id, { requestFetch: fetch }) : [])),
		'โหลดวิชาสอบไม่สำเร็จ'
	);
	return { title: 'จัดการรอบรับสมัคร', id, round, tracks, subjects };
};
