import type { PageLoad } from './$types';
import {
	LEARNING_DELIVERY_HOMEROOMS_DEPENDENCY,
	LEARNING_DELIVERY_OVERVIEW_DEPENDENCY,
	readLearningDeliveryRouteContext
} from '#lib/academic/learning-delivery-page.js';
import { getDeliveryWorkspace, getHomeroomDeliveryWorkspace } from '#lib/api/learning-delivery.js';
import { captureRouteLoad } from '#lib/navigation/route-load.js';
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
	depends(LEARNING_DELIVERY_OVERVIEW_DEPENDENCY);
	const context = readLearningDeliveryRouteContext(url);
	return {
		title: _meta.menu.title,
		context,
		homerooms: context
			? captureRouteLoad(
					getHomeroomDeliveryWorkspace(context.academicYearId, context.academicTermId, {
						requestFetch: fetch
					}),
					'โหลดภาพรวมรายห้องไม่สำเร็จ'
				)
			: null,
		overview:
			context && url.searchParams.get('view') === 'offerings'
				? captureRouteLoad(
						getDeliveryWorkspace(context.academicYearId, context.academicTermId, {
							requestFetch: fetch
						}),
						'โหลดรายการเปิดสอนไม่สำเร็จ'
					)
				: null
	};
};
