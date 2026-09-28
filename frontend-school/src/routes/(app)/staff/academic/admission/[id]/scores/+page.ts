/**
 * Admission Scores Entry Page
 */

import { PERMISSIONS } from '$lib/permissions/registry';
import { waitForAdmissionAccess } from '$lib/admission/admission-access';
import {
	getAllScores,
	getRound,
	getScoreRoomRoster,
	listSubjects,
	listTracks
} from '$lib/api/admission';
import { captureRouteLoad } from '$lib/navigation/route-load';
import type { PageLoad } from './$types';

export const _meta = {
	access: {
		user_type: 'staff',
		permission: PERMISSIONS.ADMISSION_SCORES_ALL
	}
};

export const load: PageLoad = ({ fetch, params }) => {
	const id = params.id;
	const access = waitForAdmissionAccess(PERMISSIONS.ADMISSION_SCORES_ALL);
	const round = captureRouteLoad(
		access.then((allowed) => (allowed ? getRound(id, { requestFetch: fetch }) : null)),
		'โหลดรอบรับสมัครไม่สำเร็จ'
	);
	const tracks = captureRouteLoad(
		access.then((allowed) => (allowed ? listTracks(id, { requestFetch: fetch }) : null)),
		'โหลดสายการเรียนไม่สำเร็จ'
	);
	const subjects = captureRouteLoad(
		access.then((allowed) => (allowed ? listSubjects(id, { requestFetch: fetch }) : null)),
		'โหลดวิชาสอบไม่สำเร็จ'
	);
	const rawScores = captureRouteLoad(
		access.then((allowed) => (allowed ? getAllScores(id, { requestFetch: fetch }) : null)),
		'โหลดคะแนนไม่สำเร็จ'
	);
	const roomRoster = captureRouteLoad(
		access.then((allowed) => (allowed ? getScoreRoomRoster(id, { requestFetch: fetch }) : null)),
		'โหลดรายชื่อห้องสอบไม่สำเร็จ'
	);
	return { title: 'กรอกคะแนนสอบ', id, round, tracks, subjects, rawScores, roomRoster };
};
