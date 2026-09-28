import { PERMISSIONS } from '$lib/permissions/registry';
import { waitForAdmissionAccess } from '$lib/admission/admission-access';
import { getRound, listStudentIds } from '$lib/api/admission';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_MANAGE_ALL
	}
};

export const load: PageLoad = ({ fetch, params }) => {
	const id = params.id;
	const access = waitForAdmissionAccess(PERMISSIONS.ADMISSION_MANAGE_ALL);
	return {
		title: 'กำหนดเลขประจำตัว',
		id,
		round: captureRouteLoad(
			access.then((allowed) => (allowed ? getRound(id, { requestFetch: fetch }) : null)),
			'โหลดรอบรับสมัครไม่สำเร็จ'
		),
		entries: captureRouteLoad(
			access.then((allowed) => (allowed ? listStudentIds(id, { requestFetch: fetch }) : null)),
			'โหลดรายชื่อเพื่อกำหนดเลขประจำตัวไม่สำเร็จ'
		)
	};
};
