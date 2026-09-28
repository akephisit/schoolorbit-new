/**
 * Admission Enrollment (มอบตัว) Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';
import { waitForAdmissionAccess } from '$lib/admission/admission-access';
import { getRound, listEnrollmentPending } from '$lib/api/admission';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_ENROLL_ALL
	}
};

export const load: PageLoad = ({ fetch, params }) => {
	const id = params.id;
	const access = waitForAdmissionAccess(PERMISSIONS.ADMISSION_ENROLL_ALL);
	return {
		title: 'รับมอบตัว',
		id,
		round: captureRouteLoad(
			access.then((allowed) => (allowed ? getRound(id, { requestFetch: fetch }) : null)),
			'โหลดรอบรับสมัครไม่สำเร็จ'
		),
		pending: captureRouteLoad(
			access.then((allowed) =>
				allowed ? listEnrollmentPending(id, { requestFetch: fetch }) : null
			),
			'โหลดรายชื่อมอบตัวไม่สำเร็จ'
		)
	};
};
