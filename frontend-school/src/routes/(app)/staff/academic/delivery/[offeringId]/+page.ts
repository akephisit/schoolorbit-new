import type { PageLoad } from './$types';
import {
	getLearningGroup,
	getLearningGroupRosterTracking,
	getDeliveryVersion,
	getHomeroomDeliveryWorkspace,
	listDeliveryVersions,
	getLearningOffering,
	listDatedRosterMemberships,
	listLearningGroups,
	type DatedRosterMembership,
	type LearningGroup,
	type LearningGroupRosterTracking
} from '#lib/api/learning-delivery.js';
import { selectDeliveryVersion } from '#lib/academic/learning-delivery-page.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';
import { redirect } from '@sveltejs/kit';
import { repairedDeliveryOfferingHref } from '#lib/navigation/learning-delivery.js';

export const _meta = {
	academicContext: 'term_required' as const,
	access: {
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load: PageLoad = async ({ fetch, params, url }) => {
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
					listDeliveryVersions(result.data.academicTermId, { requestFetch: fetch }),
					'โหลดรุ่นเปิดสอนไม่สำเร็จ'
				)
			: ({ ok: true, data: [], error: null } satisfies RouteLoadResult<never[]>)
	);
	const deliveryVersion = versions.then(async (result) => {
		if (!result.ok) return result;
		const selected = selectDeliveryVersion(
			result.data,
			url.searchParams.get('deliveryVersionId') ?? undefined
		);
		return selected
			? captureRouteLoad(
					getDeliveryVersion(selected.id, { requestFetch: fetch }),
					'โหลดข้อมูลรุ่นเปิดสอนไม่สำเร็จ'
				)
			: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>);
	});
	const audienceWorkspace = deliveryVersion.then(async (result) => {
		if (
			!result.ok ||
			!result.data ||
			!result.data.snapshot.offerings.some(
				(item) => item.id === offeringId && item.homeroomIds.length > 0
			)
		)
			return null;
		const version = result.data;
		return {
			academicYearId: version.academicYearId,
			academicTermId: version.academicTermId,
			deliveryVersionId: version.id,
			result: await captureRouteLoad(
				getHomeroomDeliveryWorkspace(version.academicYearId, version.academicTermId, {
					deliveryVersionId: version.id,
					requestFetch: fetch
				}),
				'โหลดชื่อห้องในรุ่นเปิดสอนไม่สำเร็จ'
			)
		};
	});
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
	const rosterTracking = selectedGroup.then(
		async (result): Promise<RouteLoadResult<LearningGroupRosterTracking | null>> =>
			result.ok && result.data
				? captureRouteLoad(
						getLearningGroupRosterTracking(result.data.id, { requestFetch: fetch }),
						'โหลดวิธีจัดรายชื่อไม่สำเร็จ'
					)
				: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>)
	);
	if (!url.searchParams.get('academicYearId') || !url.searchParams.get('academicTermId')) {
		const identity = await offering;
		if (identity.ok) {
			const versionId = url.searchParams.get('deliveryVersionId');
			const summaries = versionId ? await versions : null;
			const verified = summaries?.ok
				? summaries.data.find((summary) => summary.id === versionId)
				: null;
			const repaired = repairedDeliveryOfferingHref(url, {
				...identity.data,
				deliveryVersionId: verified?.id
			});
			if (repaired) redirect(307, repaired);
		}
	}
	return {
		audienceWorkspace,
		rosterTracking,
		title: 'รายละเอียดรายการเปิดสอน',
		offeringId,
		requestedGroupId,
		offering,
		groups,
		versions,
		deliveryVersion,
		selectedGroup,
		memberships
	};
};
