import { PERMISSIONS } from '$lib/permissions/registry';
import { waitForAdmissionAccess } from '$lib/admission/admission-access';
import { getRound, listExamRooms, getExamConfig } from '$lib/api/admission';
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
	const round = captureRouteLoad(
		access.then((allowed) => (allowed ? getRound(id, { requestFetch: fetch }) : null)),
		'โหลดรอบรับสมัครไม่สำเร็จ'
	);
	const rooms = captureRouteLoad(
		access.then((allowed) => (allowed ? listExamRooms(id, { requestFetch: fetch }) : null)),
		'โหลดห้องสอบไม่สำเร็จ'
	);
	const config = captureRouteLoad(
		access.then((allowed) => (allowed ? getExamConfig(id, { requestFetch: fetch }) : null)),
		'โหลดการตั้งค่าที่นั่งไม่สำเร็จ'
	);
	return { title: 'จัดห้องสอบ', id, round, rooms, config };
};
