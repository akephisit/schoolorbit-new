import { PERMISSIONS } from '$lib/permissions/registry';
import { waitForAdmissionAccess } from '$lib/admission/admission-access';
import { getRound, listApplications } from '$lib/api/admission';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_READ_ALL
	}
};

export const load: PageLoad = ({ fetch, params }) => {
	const id = params.id;
	const access = waitForAdmissionAccess(PERMISSIONS.ADMISSION_READ_ALL);
	return {
		title: 'รายงานการรับสมัคร',
		id,
		round: captureRouteLoad(
			access.then((allowed) => (allowed ? getRound(id, { requestFetch: fetch }) : null)),
			'โหลดรอบรับสมัครไม่สำเร็จ'
		),
		applications: captureRouteLoad(
			access.then((allowed) =>
				allowed ? listApplications(id, {}, { requestFetch: fetch }) : null
			),
			'โหลดข้อมูลรายงานไม่สำเร็จ'
		)
	};
};
