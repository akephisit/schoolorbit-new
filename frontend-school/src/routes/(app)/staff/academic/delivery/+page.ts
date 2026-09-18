import type { PageLoad } from './$types';
import {
	LEARNING_DELIVERY_CHANGE_SET_DETAIL_DEPENDENCY,
	LEARNING_DELIVERY_CHANGE_SETS_DEPENDENCY,
	LEARNING_DELIVERY_HOMEROOMS_DEPENDENCY,
	readLearningDeliveryRouteContext,
	selectAcademicTermChangeSetSummary
} from '$lib/academic/learning-delivery-page';
import {
	getAcademicTermChangeSet,
	getHomeroomDeliveryWorkspace,
	listAcademicTermChangeSets,
	type AcademicTermChangeSet
} from '$lib/api/learning-delivery';
import { captureRouteLoad, type RouteLoadResult } from '$lib/navigation/route-load';
import { PERMISSION_MODULES } from '$lib/permissions/registry';

export const _meta = {
	academicContext: 'term_required',
	menu: {
		title: 'รายวิชาและกิจกรรมที่เปิดสอน',
		icon: 'Workflow',
		group: 'academic_delivery',
		workspace: 'academic',
		order: 20,
		user_type: 'staff',
		permission: PERMISSION_MODULES.LEARNING_OFFERING
	}
};

export const load: PageLoad = ({ depends, fetch, url }) => {
	depends(LEARNING_DELIVERY_HOMEROOMS_DEPENDENCY);
	depends(LEARNING_DELIVERY_CHANGE_SETS_DEPENDENCY);
	depends(LEARNING_DELIVERY_CHANGE_SET_DETAIL_DEPENDENCY);
	const context = readLearningDeliveryRouteContext(url);
	if (!context) {
		return {
			title: _meta.menu.title,
			context,
			homerooms: null,
			changeSetSummaries: null,
			selectedChangeSet: null
		};
	}

	const loadSelectedChangeSet = (
		id: string
	): Promise<RouteLoadResult<AcademicTermChangeSet | null>> =>
		captureRouteLoad(
			getAcademicTermChangeSet(id, { requestFetch: fetch }).then((detail) => {
				if (detail.academicTermId !== context.academicTermId) {
					throw new Error('ชุดการเปลี่ยนแปลงไม่อยู่ในภาคเรียนที่เลือก');
				}
				return detail;
			}),
			'โหลดรายละเอียดชุดการเปลี่ยนแปลงกลางภาคไม่สำเร็จ'
		);

	const homerooms = captureRouteLoad(
		getHomeroomDeliveryWorkspace(context.academicYearId, context.academicTermId, {
			timetableVersionId: context.timetableVersionId,
			requestFetch: fetch
		}),
		'โหลดภาพรวมรายห้องประจำชั้นไม่สำเร็จ'
	);
	const changeSetSummaries = captureRouteLoad(
		listAcademicTermChangeSets(context.academicTermId, { requestFetch: fetch }),
		'โหลดรายการเปลี่ยนแปลงกลางภาคไม่สำเร็จ'
	);
	const selectedChangeSet = context.changeSetId
		? loadSelectedChangeSet(context.changeSetId)
		: changeSetSummaries.then((result) => {
				if (!result.ok) {
					return { ok: true, data: null, error: null } satisfies RouteLoadResult<null>;
				}
				const selected = selectAcademicTermChangeSetSummary(result.data);
				return selected
					? loadSelectedChangeSet(selected.id)
					: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>);
			});

	return {
		title: _meta.menu.title,
		context,
		homerooms,
		changeSetSummaries,
		selectedChangeSet
	};
};
