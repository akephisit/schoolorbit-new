import { PERMISSIONS } from '#lib/permissions/registry.js';
import { waitForAdmissionAccess } from '#lib/admission/admission-access.js';
import { getApplication } from '#lib/api/admission.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_READ_ALL
	}
};

export const load: PageLoad = ({ fetch, params }) => {
	const { id: roundId, appId } = params;
	const applicationResult = captureRouteLoad(
		waitForAdmissionAccess(PERMISSIONS.ADMISSION_READ_ALL).then((allowed) =>
			allowed ? getApplication(appId, { requestFetch: fetch }) : null
		),
		'โหลดรายละเอียดใบสมัครไม่สำเร็จ'
	);
	return { title: 'รายละเอียดใบสมัคร', roundId, appId, applicationResult };
};
