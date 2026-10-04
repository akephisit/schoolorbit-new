import type { PageLoad } from './$types';
import {
	LEARNING_DELIVERY_CHANGE_SET_DETAIL_DEPENDENCY,
	LEARNING_DELIVERY_CHANGE_SETS_DEPENDENCY,
	LEARNING_DELIVERY_HOMEROOMS_DEPENDENCY,
	readLearningDeliveryRouteContext,
	selectDeliveryVersion
} from '#lib/academic/learning-delivery-page.js';
import {
	getAcademicTermChangeSet,
	getHomeroomDeliveryWorkspace,
	listAcademicTermChangeSets,
	listDeliveryVersions,
	type AcademicTermChangeSet
} from '#lib/api/learning-delivery.js';
import { captureRouteLoad, type RouteLoadResult } from '#lib/navigation/route-load.js';
import { PERMISSION_MODULES } from '#lib/permissions/registry.js';

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
			selectedChangeSet: null,
			deliveryVersions: null
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

	const deliveryVersions = captureRouteLoad(
		listDeliveryVersions(context.academicTermId, { requestFetch: fetch }),
		'โหลดรุ่นเปิดสอนไม่สำเร็จ'
	);
	const homerooms = deliveryVersions.then((result) => {
		const selected = result.ok
			? selectDeliveryVersion(result.data, context.deliveryVersionId)
			: null;
		return captureRouteLoad(
			getHomeroomDeliveryWorkspace(context.academicYearId, context.academicTermId, {
				deliveryVersionId: context.deliveryVersionId ?? selected?.id,
				requestFetch: fetch
			}),
			'โหลดภาพรวมรายห้องประจำชั้นไม่สำเร็จ'
		);
	});
	const changeSetSummaries = captureRouteLoad(
		listAcademicTermChangeSets(context.academicTermId, { requestFetch: fetch }),
		'โหลดรายการเปลี่ยนแปลงกลางภาคไม่สำเร็จ'
	);
	const selectedChangeSet = deliveryVersions.then((result) => {
		const selected = result.ok
			? selectDeliveryVersion(result.data, context.deliveryVersionId)
			: null;
		return selected?.changeSetId
			? loadSelectedChangeSet(selected.changeSetId)
			: ({ ok: true, data: null, error: null } satisfies RouteLoadResult<null>);
	});

	return {
		title: _meta.menu.title,
		context,
		deliveryVersions,
		homerooms,
		changeSetSummaries,
		selectedChangeSet
	};
};
