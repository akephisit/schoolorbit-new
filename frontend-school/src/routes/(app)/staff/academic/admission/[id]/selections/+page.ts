/**
 * Admission Selections & Room Assignment Page
 */

import { PERMISSIONS } from '#lib/permissions/registry.js';
import { waitForAdmissionAccess } from '#lib/admission/admission-access.js';
import {
	getGlobalRanking,
	getRoomsForRound,
	getRound,
	getTrackRanking,
	listSubjects,
	listTracks
} from '#lib/api/admission.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
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
	const roundRead = access.then((allowed) =>
		allowed ? getRound(id, { requestFetch: fetch }) : null
	);
	const tracksRead = access.then((allowed) =>
		allowed ? listTracks(id, { requestFetch: fetch }) : null
	);
	const subjectsRead = access.then((allowed) =>
		allowed ? listSubjects(id, { requestFetch: fetch }) : null
	);
	const round = captureRouteLoad(roundRead, 'โหลดรอบรับสมัครไม่สำเร็จ');
	const tracks = captureRouteLoad(tracksRead, 'โหลดสายการเรียนไม่สำเร็จ');
	const subjects = captureRouteLoad(subjectsRead, 'โหลดวิชาสอบไม่สำเร็จ');
	const trackRanking = captureRouteLoad(
		Promise.all([roundRead, tracksRead, subjectsRead]).then(
			async ([roundData, trackRows, subjectRows]) => {
				if (!roundData || !trackRows?.length || !subjectRows) return null;
				if (roundData.selectionSettings?.assignmentMode === 'global') return null;
				const trackId = trackRows[0].id;
				const selectionSubjectIds =
					roundData.selectionSettings?.subjectsByTrack?.[trackId] ?? subjectRows.map((s) => s.id);
				const roomAssignmentMethod =
					roundData.selectionSettings?.methodByTrack?.[trackId] ?? 'sequential';
				return {
					trackId,
					result: await getTrackRanking(trackId, selectionSubjectIds, roomAssignmentMethod, {
						requestFetch: fetch
					})
				};
			}
		),
		'โหลดผลเรียงคะแนนไม่สำเร็จ'
	);
	const globalRanking = captureRouteLoad(
		roundRead.then((roundData) =>
			roundData?.selectionSettings?.assignmentMode === 'global'
				? getGlobalRanking(id, { requestFetch: fetch })
				: null
		),
		'โหลดผลรวมไม่สำเร็จ'
	);
	const roomOrder = captureRouteLoad(
		roundRead.then((roundData) =>
			roundData?.selectionSettings?.assignmentMode === 'global'
				? getRoomsForRound(id, { requestFetch: fetch })
				: null
		),
		'โหลดรายการห้องไม่สำเร็จ'
	);
	return {
		title: 'จัดห้องเรียน',
		id,
		round,
		tracks,
		subjects,
		trackRanking,
		globalRanking,
		roomOrder
	};
};
