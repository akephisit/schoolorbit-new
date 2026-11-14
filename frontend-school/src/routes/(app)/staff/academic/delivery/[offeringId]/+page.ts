import type { PageLoad } from './$types';
import {
	getLearningOffering,
	listLearningGroups,
	getLearningGroup,
	getLearningGroupRosterTracking,
	listDatedRosterMemberships,
	type LearningGroup,
	type DatedRosterMembership,
	type LearningGroupRosterTracking
} from '#lib/api/learning-delivery.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
export const _meta = {
	academicContext: 'term_required' as const,
	access: { user_type: 'staff', permission: PERMISSION_MODULES.LEARNING_OFFERING }
};
export const load: PageLoad = async ({ fetch, params, url, depends }) => {
	depends('school:app-identity');
	const offeringId = params.offeringId,
		requestedGroupId = url.searchParams.get('groupId')?.trim() || null;
	const offering = captureRouteLoad(
		getLearningOffering(offeringId, { requestFetch: fetch }),
		'โหลดรายละเอียดรายการเปิดสอนไม่สำเร็จ'
	);
	const groups = captureRouteLoad(
		listLearningGroups(offeringId, { requestFetch: fetch }),
		'โหลดรายการกลุ่มเรียนไม่สำเร็จ'
	);
	const selectedGroup = requestedGroupId
		? captureRouteLoad(
				getLearningGroup(requestedGroupId, { requestFetch: fetch }).then((group) => {
					if (group.learningOfferingId !== offeringId)
						throw new Error('กลุ่มเรียนไม่อยู่ในรายการเปิดสอนนี้');
					return group;
				}),
				'โหลดกลุ่มเรียนไม่สำเร็จ'
			)
		: groups.then((result): RouteLoadResult<LearningGroup | null> => ({
				ok: true,
				data: result.ok ? (result.data[0] ?? null) : null,
				error: null
			}));
	const memberships = selectedGroup.then(
		async (result): Promise<RouteLoadResult<DatedRosterMembership[] | null>> =>
			result.ok && result.data
				? captureRouteLoad(
						listDatedRosterMemberships(result.data.id, { requestFetch: fetch }),
						'โหลดรายชื่อนักเรียนไม่สำเร็จ'
					)
				: ({ ok: true, data: null, error: null } as const)
	);
	const rosterTracking = selectedGroup.then(
		async (result): Promise<RouteLoadResult<LearningGroupRosterTracking | null>> =>
			result.ok && result.data
				? captureRouteLoad(
						getLearningGroupRosterTracking(result.data.id, { requestFetch: fetch }),
						'โหลดวิธีจัดรายชื่อไม่สำเร็จ'
					)
				: ({ ok: true, data: null, error: null } as const)
	);

	return {
		title: 'รายละเอียดรายการเปิดสอน',
		offeringId,
		requestedGroupId,
		offering,
		groups,
		selectedGroup,
		memberships,
		rosterTracking
	};
};
