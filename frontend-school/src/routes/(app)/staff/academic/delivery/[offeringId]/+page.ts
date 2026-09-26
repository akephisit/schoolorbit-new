import type { PageLoad } from './$types';
import {
	getLearningGroup,
	getLearningOffering,
	listDatedRosterMemberships,
	listLearningGroups,
	type DatedRosterMembership,
	type LearningGroup
} from '$lib/api/learning-delivery';
import { listTimetableVersions } from '$lib/api/timetable';
import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load: PageLoad = ({ fetch, params, url }) => {
	const offeringId = params.offeringId;
	const requestedGroupId = url.searchParams.get('groupId')?.trim() || null;
	const offering = captureRouteLoad(
		getLearningOffering(offeringId, { requestFetch: fetch }),
		'โหลดรายละเอียดรายการเปิดสอนไม่สำเร็จ'
	);
	const groups = captureRouteLoad(
		listLearningGroups(offeringId, { requestFetch: fetch }),
		'โหลดรายการกลุ่มเรียนไม่สำเร็จ'
	);
	const versions = offering.then((result) =>
		result.ok
			? captureRouteLoad(
					listTimetableVersions(result.data.academicTermId, { requestFetch: fetch }),
					'โหลดรุ่นตารางสอนไม่สำเร็จ'
				)
			: ({ ok: true, data: [], error: null } satisfies RouteLoadResult<never[]>)
	);
	const loadSelectedGroup = (groupId: string): Promise<RouteLoadResult<LearningGroup | null>> =>
		captureRouteLoad(
			getLearningGroup(groupId, { requestFetch: fetch }).then((group) => {
				if (group.learningOfferingId !== offeringId)
					throw new Error('กลุ่มเรียนไม่อยู่ในรายการเปิดสอนที่เลือก');
				return group;
			}),
			'โหลดกลุ่มเรียนไม่สำเร็จ'
		);
	const selectedGroup = requestedGroupId
		? loadSelectedGroup(requestedGroupId)
		: groups.then((result): RouteLoadResult<LearningGroup | null> => ({
				ok: true,
				data: result.ok ? (result.data[0] ?? null) : null,
				error: null
			}));
	const memberships: Promise<RouteLoadResult<DatedRosterMembership[] | null>> = selectedGroup.then(
		async (result): Promise<RouteLoadResult<DatedRosterMembership[] | null>> =>
			result.ok && result.data?.rosterStatus === 'published'
				? captureRouteLoad(
						listDatedRosterMemberships(result.data.id, { requestFetch: fetch }),
						'โหลดประวัติสมาชิกกลุ่มเรียนไม่สำเร็จ'
					)
				: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>)
	);
	return {
		title: 'รายละเอียดรายการเปิดสอน',
		offeringId,
		requestedGroupId,
		offering,
		groups,
		versions,
		selectedGroup,
		memberships
	};
};
